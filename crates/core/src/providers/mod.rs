pub mod gemini;
pub mod openai_compat;

use std::sync::Arc;

use crate::error::Result;
use crate::models::{ProviderConfig, ProviderHealth, ProviderKind};
use crate::traits::AiProvider;

pub use gemini::GeminiProvider;
pub use openai_compat::OpenAiCompatibleProvider;

pub fn build_provider(config: &ProviderConfig) -> Arc<dyn AiProvider> {
    match config.kind {
        ProviderKind::OpenAiCompatible => Arc::new(OpenAiCompatibleProvider::new(
            config.base_url.clone(),
            config.model.clone(),
            config.api_key.clone(),
        )),
        ProviderKind::Gemini => Arc::new(GeminiProvider::new(
            config.model.clone(),
            config.api_key.clone().unwrap_or_default(),
        )),
    }
}

pub async fn check_provider_health(config: &ProviderConfig) -> Result<ProviderHealth> {
    match config.kind {
        ProviderKind::OpenAiCompatible => {
            let p = OpenAiCompatibleProvider::new(
                config.base_url.clone(),
                config.model.clone(),
                config.api_key.clone(),
            );
            p.health_check().await
        }
        ProviderKind::Gemini => {
            let key = config.api_key.as_deref().unwrap_or("").trim();
            if key.is_empty() {
                return Ok(ProviderHealth {
                    reachable: false,
                    available_models: vec![],
                    message: "Google AI Studio API key is not configured.".to_string(),
                });
            }
            let p = GeminiProvider::new(config.model.clone(), key.to_string());
            p.health_check().await
        }
    }
}
