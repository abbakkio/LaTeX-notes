use async_trait::async_trait;
use base64::Engine;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::error::{CoreError, Result};
use crate::image_prep::normalize_image_bytes;
use crate::models::{Notebook, PageImage, PageMeta, Section};
use crate::secrets::{delete_secret, get_secret, set_secret};
use crate::sources::inkml::render_inkml_to_page_image;
use crate::traits::NoteSource;

const ONENOTE_TOKEN_KEY: &str = "onenote_oauth_tokens";
const GRAPH_BASE: &str = "https://graph.microsoft.com/v1.0";
const MS_AUTH_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
const MS_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneNoteAuthStatus {
    pub connected: bool,
    pub client_id_configured: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct GraphTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NotebooksResp {
    #[serde(default)]
    value: Vec<GraphNotebook>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphNotebook {
    id: String,
    display_name: Option<String>,
    #[serde(default)]
    sections: Vec<GraphSection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphSection {
    id: String,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PagesResp {
    #[serde(default)]
    value: Vec<GraphPage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphPage {
    id: String,
    title: Option<String>,
    last_modified_date_time: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OneNoteGraphSource {
    client: reqwest::Client,
    client_id: String,
}

impl OneNoteGraphSource {
    pub fn new(client_id: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            client_id,
        }
    }

    pub fn auth_status(&self) -> OneNoteAuthStatus {
        let configured = !self.client_id.trim().is_empty();
        let has_tokens = get_secret(ONENOTE_TOKEN_KEY).ok().flatten().is_some();
        let message = if !configured {
            "Set a Microsoft Entra (Azure) public Client ID in Settings to connect OneNote."
                .to_string()
        } else if has_tokens {
            "OneNote account connected via Microsoft Graph.".to_string()
        } else {
            "Not signed in to Microsoft OneNote.".to_string()
        };
        OneNoteAuthStatus {
            connected: has_tokens,
            client_id_configured: configured,
            message,
        }
    }

    pub fn disconnect(&self) -> Result<()> {
        delete_secret(ONENOTE_TOKEN_KEY)
    }

    /// Runs interactive OAuth2 Authorization Code + PKCE flow via a local callback server.
    pub async fn login_interactive(&self) -> Result<()> {
        let client_id = self.client_id.trim();
        if client_id.is_empty() {
            return Err(CoreError::Auth(
                "Microsoft Azure Client ID is required before signing in.".to_string(),
            ));
        }

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let redirect_uri = format!("http://127.0.0.1:{port}/callback");

        let verifier = generate_code_verifier();
        let challenge = code_challenge_s256(&verifier);
        let state = generate_state();

        let mut auth_url = url::Url::parse(MS_AUTH_URL)
            .map_err(|e| CoreError::Auth(format!("Invalid auth URL: {e}")))?;
        auth_url
            .query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("response_mode", "query")
            .append_pair("scope", "Notes.Read offline_access")
            .append_pair("state", &state)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");

        open::that(auth_url.as_str())
            .map_err(|e| CoreError::Auth(format!("Failed to open browser for sign-in: {e}")))?;

        let (mut socket, _) = tokio::time::timeout(
            std::time::Duration::from_secs(180),
            listener.accept(),
        )
        .await
        .map_err(|_| CoreError::Auth("Timed out waiting for Microsoft sign-in.".to_string()))??;

        let mut buf = vec![0u8; 4096];
        let n = socket.read(&mut buf).await?;
        let req_str = String::from_utf8_lossy(&buf[..n]);
        let path_part = req_str
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .ok_or_else(|| CoreError::Auth("Malformed OAuth callback HTTP request.".to_string()))?;

        let callback_url = url::Url::parse(&format!("http://127.0.0.1{path_part}"))
            .map_err(|e| CoreError::Auth(format!("Invalid callback query: {e}")))?;

        let mut code: Option<String> = None;
        let mut returned_state: Option<String> = None;
        let mut error_desc: Option<String> = None;
        for (k, v) in callback_url.query_pairs() {
            match k.as_ref() {
                "code" => code = Some(v.into_owned()),
                "state" => returned_state = Some(v.into_owned()),
                "error_description" | "error" => error_desc = Some(v.into_owned()),
                _ => {}
            }
        }

        let response_html = if error_desc.is_some() || code.is_none() {
            "<html><body><h3>Sign-in failed.</h3><p>You can close this window and check LaTeX Notes.</p></body></html>"
        } else {
            "<html><body><h3>OneNote connected!</h3><p>You can close this browser tab and return to LaTeX Notes.</p></body></html>"
        };
        let http_resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_html.len(),
            response_html
        );
        let _ = socket.write_all(http_resp.as_bytes()).await;

        if let Some(err) = error_desc {
            return Err(CoreError::Auth(format!("Microsoft OAuth error: {err}")));
        }
        if returned_state.as_deref() != Some(&state) {
            return Err(CoreError::Auth("OAuth state mismatch.".to_string()));
        }
        let auth_code = code.ok_or_else(|| CoreError::Auth("Missing OAuth code.".to_string()))?;

        let resp = self
            .client
            .post(MS_TOKEN_URL)
            .form(&[
                ("client_id", client_id),
                ("grant_type", "authorization_code"),
                ("code", auth_code.as_str()),
                ("redirect_uri", redirect_uri.as_str()),
                ("code_verifier", verifier.as_str()),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(CoreError::Auth(format!("Token exchange failed: {txt}")));
        }

        let parsed: GraphTokenResponse = resp.json().await?;
        let tokens = OAuthTokens {
            access_token: parsed.access_token,
            refresh_token: parsed.refresh_token,
        };
        set_secret(ONENOTE_TOKEN_KEY, &serde_json::to_string(&tokens)?)?;
        Ok(())
    }

    async fn get_access_token(&self) -> Result<String> {
        let raw = get_secret(ONENOTE_TOKEN_KEY)?.ok_or_else(|| {
            CoreError::Auth("Not signed in to OneNote. Click 'Connect OneNote' first.".to_string())
        })?;
        let tokens: OAuthTokens = serde_json::from_str(&raw)?;
        Ok(tokens.access_token)
    }

    async fn refresh_access_token(&self) -> Result<String> {
        let raw = get_secret(ONENOTE_TOKEN_KEY)?.ok_or_else(|| {
            CoreError::Auth("Not signed in to OneNote.".to_string())
        })?;
        let tokens: OAuthTokens = serde_json::from_str(&raw)?;
        let refresh = tokens.refresh_token.ok_or_else(|| {
            CoreError::Auth("No refresh token available; please sign in again.".to_string())
        })?;

        let resp = self
            .client
            .post(MS_TOKEN_URL)
            .form(&[
                ("client_id", self.client_id.trim()),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh.as_str()),
                ("scope", "Notes.Read offline_access"),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(CoreError::Auth(format!("Token refresh failed: {txt}")));
        }

        let parsed: GraphTokenResponse = resp.json().await?;
        let updated = OAuthTokens {
            access_token: parsed.access_token.clone(),
            refresh_token: parsed.refresh_token.or(Some(refresh)),
        };
        set_secret(ONENOTE_TOKEN_KEY, &serde_json::to_string(&updated)?)?;
        Ok(parsed.access_token)
    }

    async fn graph_get(&self, url: &str) -> Result<reqwest::Response> {
        let token = self.get_access_token().await?;
        let resp = self.client.get(url).bearer_auth(&token).send().await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            let refreshed = self.refresh_access_token().await?;
            return Ok(self.client.get(url).bearer_auth(refreshed).send().await?);
        }
        Ok(resp)
    }
}

#[async_trait]
impl NoteSource for OneNoteGraphSource {
    fn id(&self) -> &'static str {
        "onenote"
    }

    fn display_name(&self) -> &'static str {
        "Microsoft OneNote"
    }

    async fn list_notebooks(&self) -> Result<Vec<Notebook>> {
        let url = format!("{GRAPH_BASE}/me/onenote/notebooks?$expand=sections");
        let resp = self.graph_get(&url).await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err_txt = resp.text().await.unwrap_or_default();
            return Err(CoreError::Source {
                source_name: "onenote".to_string(),
                message: format!("Failed to list notebooks (HTTP {status}): {err_txt}"),
            });
        }
        let parsed: NotebooksResp = resp.json().await?;
        Ok(parsed
            .value
            .into_iter()
            .map(|nb| Notebook {
                id: nb.id,
                name: nb.display_name.unwrap_or_else(|| "Untitled Notebook".into()),
                sections: nb
                    .sections
                    .into_iter()
                    .map(|sec| Section {
                        id: sec.id,
                        name: sec.display_name.unwrap_or_else(|| "Untitled Section".into()),
                    })
                    .collect(),
            })
            .collect())
    }

    async fn list_pages(&self, section_id: &str) -> Result<Vec<PageMeta>> {
        let url = format!(
            "{GRAPH_BASE}/me/onenote/sections/{section_id}/pages?$select=id,title,lastModifiedDateTime&$orderby=lastModifiedDateTime desc&$top=50"
        );
        let resp = self.graph_get(&url).await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err_txt = resp.text().await.unwrap_or_default();
            return Err(CoreError::Source {
                source_name: "onenote".to_string(),
                message: format!("Failed to list pages (HTTP {status}): {err_txt}"),
            });
        }
        let parsed: PagesResp = resp.json().await?;
        Ok(parsed
            .value
            .into_iter()
            .map(|p| PageMeta {
                id: p.id,
                title: p.title.unwrap_or_else(|| "Untitled Page".into()),
                last_modified: p.last_modified_date_time,
                source_kind: "onenote".to_string(),
            })
            .collect())
    }

    async fn fetch_page_images(&self, page_id: &str) -> Result<Vec<PageImage>> {
        let url = format!("{GRAPH_BASE}/me/onenote/pages/{page_id}/content?includeinkML=true");
        let resp = self.graph_get(&url).await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let err_txt = resp.text().await.unwrap_or_default();
            return Err(CoreError::Source {
                source_name: "onenote".to_string(),
                message: format!("Failed to fetch OneNote page content (HTTP {status}): {err_txt}"),
            });
        }

        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body_bytes = resp.bytes().await?;

        let mut pages = Vec::new();
        if let Some(boundary) = extract_multipart_boundary(&content_type) {
            let parts = split_multipart_parts(&body_bytes, &boundary);
            for (idx, part) in parts.into_iter().enumerate() {
                let ct_lower = part.content_type.to_ascii_lowercase();
                if ct_lower.contains("inkml+xml") || part.body.windows(6).any(|w| w == b"<trace" || w == b":trace") {
                    let xml = String::from_utf8_lossy(&part.body);
                    if let Ok(img) = render_inkml_to_page_image(
                        &xml,
                        pages.len(),
                        format!("OneNote Ink Layer {}", idx + 1),
                    ) {
                        pages.push(img);
                    }
                } else if ct_lower.starts_with("image/") {
                    if let Ok(img) = normalize_image_bytes(
                        &part.body,
                        pages.len(),
                        format!("OneNote Image {}", idx + 1),
                        None,
                    ) {
                        pages.push(img);
                    }
                }
            }
        } else {
            // Single HTML or InkML body
            let text = String::from_utf8_lossy(&body_bytes);
            if text.contains("<trace") || text.contains(":trace") {
                if let Ok(img) = render_inkml_to_page_image(&text, 0, "OneNote Ink".into()) {
                    pages.push(img);
                }
            }
        }

        if pages.is_empty() {
            return Err(CoreError::Source {
                source_name: "onenote".to_string(),
                message: "This OneNote page contained no handwritten InkML strokes or embedded images.".to_string(),
            });
        }

        Ok(pages)
    }
}

struct MultipartPart {
    content_type: String,
    body: Vec<u8>,
}

fn extract_multipart_boundary(content_type: &str) -> Option<String> {
    for segment in content_type.split(';') {
        let trimmed = segment.trim();
        if let Some(rest) = trimmed.strip_prefix("boundary=") {
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

fn split_multipart_parts(data: &[u8], boundary: &str) -> Vec<MultipartPart> {
    let marker = format!("--{boundary}").into_bytes();
    let mut parts = Vec::new();

    let mut positions = Vec::new();
    let mut i = 0;
    while i + marker.len() <= data.len() {
        if &data[i..i + marker.len()] == marker.as_slice() {
            positions.push(i);
            i += marker.len();
        } else {
            i += 1;
        }
    }

    for pair in positions.windows(2) {
        let start = pair[0] + marker.len();
        let end = pair[1];
        if start >= end {
            continue;
        }
        let slice = &data[start..end];
        let slice = slice.strip_prefix(b"\r\n").or_else(|| slice.strip_prefix(b"\n")).unwrap_or(slice);
        if let Some((hdr_end, sep_len)) = find_header_end(slice) {
            let headers = String::from_utf8_lossy(&slice[..hdr_end]);
            let mut ct = String::new();
            for line in headers.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    if k.trim().eq_ignore_ascii_case("content-type") {
                        ct = v.trim().to_string();
                    }
                }
            }
            let body = slice[hdr_end + sep_len..]
                .strip_suffix(b"\r\n")
                .or_else(|| slice[hdr_end + sep_len..].strip_suffix(b"\n"))
                .unwrap_or(&slice[hdr_end + sep_len..])
                .to_vec();
            parts.push(MultipartPart {
                content_type: ct,
                body,
            });
        }
    }

    parts
}

fn find_header_end(buf: &[u8]) -> Option<(usize, usize)> {
    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
        return Some((pos, 4));
    }
    if let Some(pos) = buf.windows(2).position(|w| w == b"\n\n") {
        return Some((pos, 2));
    }
    None
}

fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn code_challenge_s256(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash)
}

fn generate_state() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_onenote_multipart_html_and_inkml() {
        let ct = "multipart/form-data; boundary=OneNotePartBoundary";
        let boundary = extract_multipart_boundary(ct).unwrap();
        assert_eq!(boundary, "OneNotePartBoundary");

        let payload = b"--OneNotePartBoundary\r\nContent-Type: text/html\r\n\r\n<html><body>hi</body></html>\r\n--OneNotePartBoundary\r\nContent-Type: application/inkml+xml\r\n\r\n<inkml:ink><inkml:trace>0 0, 10 10</inkml:trace></inkml:ink>\r\n--OneNotePartBoundary--\r\n";
        let parts = split_multipart_parts(payload, &boundary);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].content_type, "text/html");
        assert_eq!(parts[1].content_type, "application/inkml+xml");
        assert!(String::from_utf8_lossy(&parts[1].body).contains("<inkml:trace>"));
    }
}

