use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageImage {
    pub page_index: usize,
    pub label: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub png_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notebook {
    pub id: String,
    pub name: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMeta {
    pub id: String,
    pub title: String,
    pub last_modified: Option<String>,
    pub source_kind: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    DocumentBody,
    StandaloneDocument,
    MathOnly,
}

impl Default for OutputMode {
    fn default() -> Self {
        Self::DocumentBody
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionOptions {
    pub language_hint: String,
    pub output_mode: OutputMode,
    pub extra_instructions: Option<String>,
    pub temperature: f32,
    pub max_tokens: u32,
}

impl Default for ConversionOptions {
    fn default() -> Self {
        Self {
            language_hint: "English and Russian (Cyrillic + Latin)".to_string(),
            output_mode: OutputMode::DocumentBody,
            extra_instructions: None,
            temperature: 0.1,
            max_tokens: 4096,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatexResult {
    pub latex_body: String,
    pub standalone_tex: String,
    pub markdown: String,
    pub model_used: String,
    pub provider_id: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    OpenAiCompatible,
    Gemini,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            kind: ProviderKind::OpenAiCompatible,
            base_url: "http://localhost:11434/v1".to_string(),
            model: "qwen2.5vl:7b".to_string(),
            api_key: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub reachable: bool,
    pub available_models: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfCompilerInfo {
    pub available: bool,
    pub binary_name: Option<String>,
}
