use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use serde::Deserialize;
use serde_json::json;

use crate::error::{CoreError, Result};
use crate::latex::{
    build_system_prompt, latex_body_to_markdown, sanitize_latex_output, wrap_standalone_document,
};
use crate::models::{ConversionOptions, LatexResult, PageImage, ProviderHealth};
use crate::traits::AiProvider;

#[derive(Debug, Clone)]
pub struct GeminiProvider {
    client: reqwest::Client,
    model: String,
    api_key: String,
}

#[derive(Debug, Deserialize)]
struct GeminiModelsResponse {
    #[serde(default)]
    models: Vec<GeminiModelInfo>,
}

#[derive(Debug, Deserialize)]
struct GeminiModelInfo {
    name: String,
}

#[derive(Debug, Deserialize)]
struct GeminiGenerateResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
    #[serde(default)]
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Deserialize)]
struct GeminiPart {
    text: Option<String>,
}

impl GeminiProvider {
    pub fn new(model: String, api_key: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            model: model.trim_start_matches("models/").to_string(),
            api_key,
        }
    }

    pub async fn health_check(&self) -> Result<ProviderHealth> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models?key={}",
            self.api_key.trim()
        );
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let parsed: GeminiModelsResponse = resp.json().await.unwrap_or(GeminiModelsResponse {
                    models: vec![],
                });
                let models: Vec<String> = parsed
                    .models
                    .into_iter()
                    .map(|m| m.name.trim_start_matches("models/").to_string())
                    .filter(|n| n.contains("gemini"))
                    .collect();
                Ok(ProviderHealth {
                    reachable: true,
                    available_models: models,
                    message: "Connected to Google AI Studio (Gemini API).".to_string(),
                })
            }
            Ok(resp) => Ok(ProviderHealth {
                reachable: false,
                available_models: vec![],
                message: format!("Google AI Studio returned HTTP {}", resp.status()),
            }),
            Err(err) => Ok(ProviderHealth {
                reachable: false,
                available_models: vec![],
                message: format!("Failed to reach Google AI Studio: {err}"),
            }),
        }
    }
}

#[async_trait]
impl AiProvider for GeminiProvider {
    fn id(&self) -> &'static str {
        "gemini"
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
        if self.api_key.trim().is_empty() {
            return Err(CoreError::Auth(
                "Google AI Studio API key is missing.".to_string(),
            ));
        }

        let system_prompt = build_system_prompt(options);
        let mut parts = vec![json!({
            "text": "Transcribe the following handwritten note page(s) into LaTeX:"
        })];

        for page in images {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&page.png_bytes);
            parts.push(json!({
                "inline_data": {
                    "mime_type": page.mime_type,
                    "data": b64
                }
            }));
        }

        let payload = json!({
            "system_instruction": {
                "parts": [{ "text": system_prompt }]
            },
            "contents": [{
                "role": "user",
                "parts": parts
            }],
            "generationConfig": {
                "temperature": options.temperature,
                "maxOutputTokens": options.max_tokens
            }
        });

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model,
            self.api_key.trim()
        );
        let resp = self.client.post(&url).json(&payload).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            return Err(CoreError::Provider {
                provider: self.id().to_string(),
                message: format!("HTTP {status}: {err_body}"),
            });
        }

        let parsed: GeminiGenerateResponse = resp.json().await?;
        let raw_text = parsed
            .candidates
            .into_iter()
            .next()
            .and_then(|c| c.content)
            .map(|c| {
                c.parts
                    .into_iter()
                    .filter_map(|p| p.text)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| CoreError::Provider {
                provider: self.id().to_string(),
                message: "Empty response from Gemini API.".to_string(),
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
