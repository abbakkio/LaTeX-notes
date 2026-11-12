use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{CoreError, Result};
use crate::latex::{
    build_system_prompt, latex_body_to_markdown, sanitize_latex_output, wrap_standalone_document,
};
use crate::models::{ConversionOptions, LatexResult, PageImage, ProviderHealth};
use crate::traits::AiProvider;

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: Option<String>,
}

impl OpenAiCompatibleProvider {
    pub fn new(base_url: String, model: String, api_key: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            api_key: api_key.filter(|k| !k.trim().is_empty()),
        }
    }

    pub async fn health_check(&self) -> Result<ProviderHealth> {
        let url = format!("{}/models", self.base_url);
        let mut req = self.client.get(&url);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                let parsed: ModelsResponse = resp.json().await.unwrap_or(ModelsResponse {
                    data: vec![],
                });
                let models: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
                let msg = if models.is_empty() {
                    format!("Connected to {}, no models reported.", self.base_url)
                } else {
                    format!("Connected ({} model(s) available).", models.len())
                };
                Ok(ProviderHealth {
                    reachable: true,
                    available_models: models,
                    message: msg,
                })
            }
            Ok(resp) => Ok(ProviderHealth {
                reachable: false,
                available_models: vec![],
                message: format!("Endpoint returned HTTP {}", resp.status()),
            }),
            Err(err) => Ok(ProviderHealth {
                reachable: false,
                available_models: vec![],
                message: format!(
                    "Cannot reach local runtime at {}: {}",
                    self.base_url, err
                ),
            }),
        }
    }
}

#[async_trait]
impl AiProvider for OpenAiCompatibleProvider {
    fn id(&self) -> &'static str {
        "openai_compatible"
    }

    async fn convert_to_latex(
        &self,
        images: &[PageImage],
        options: &ConversionOptions,
    ) -> Result<LatexResult> {
        if images.is_empty() {
            return Err(CoreError::InvalidInput(
                "No note images provided for conversion.".to_string(),
            ));
        }

        let system_prompt = build_system_prompt(options);
        let mut user_content = Vec::new();
        let page_summary = if images.len() == 1 {
            "Transcribe this handwritten note page into LaTeX:".to_string()
        } else {
            format!(
                "Transcribe these {} handwritten note pages in order into cohesive LaTeX:",
                images.len()
            )
        };
        user_content.push(json!({ "type": "text", "text": page_summary }));

        for page in images {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&page.png_bytes);
            user_content.push(json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{};base64,{}", page.mime_type, b64)
                }
            }));
        }

        let body = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: serde_json::Value::String(system_prompt),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: serde_json::Value::Array(user_content),
                },
            ],
            temperature: options.temperature,
            max_tokens: options.max_tokens,
        };

        let url = format!("{}/chat/completions", self.base_url);
        let mut req = self.client.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }

        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            return Err(CoreError::Provider {
                provider: self.id().to_string(),
                message: format!("HTTP {status}: {err_body}"),
            });
        }

        let parsed: ChatResponse = resp.json().await?;
        let raw_text = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| CoreError::Provider {
                provider: self.id().to_string(),
                message: "Empty response choices from model.".to_string(),
            })?;

        let (latex_body, warnings) = sanitize_latex_output(&raw_text, options.output_mode);
        let standalone_tex = wrap_standalone_document(&latex_body, None);
        let markdown = latex_body_to_markdown(&latex_body);

        Ok(LatexResult {
            latex_body,
            standalone_tex,
            markdown,
            model_used: self.model.clone(),
            provider_id: self.id().to_string(),
            warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn converts_image_against_mock_openai_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 8192];
            let n = socket.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            assert!(req.contains("/v1/chat/completions"));
            assert!(req.contains("data:image/png;base64,"));

            let body = r#"{"choices":[{"message":{"content":"```latex\n\\int_0^1 x^2 \\, dx = \\frac{1}{3}\n```"}}]}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(resp.as_bytes()).await.unwrap();
        });

        let provider = OpenAiCompatibleProvider::new(
            format!("http://127.0.0.1:{port}/v1"),
            "qwen2.5vl:7b".into(),
            None,
        );
        let page = PageImage {
            page_index: 0,
            label: "test.png".into(),
            mime_type: "image/png".into(),
            width: 10,
            height: 10,
            png_bytes: vec![137, 80, 78, 71],
        };
        let res = provider
            .convert_to_latex(&[page], &ConversionOptions::default())
            .await
            .unwrap();
        assert_eq!(res.latex_body, "\\int_0^1 x^2 \\, dx = \\frac{1}{3}");
        assert!(res.standalone_tex.contains("\\documentclass"));
        server.await.unwrap();
    }
}

