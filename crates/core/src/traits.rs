use async_trait::async_trait;

use crate::error::Result;
use crate::models::{ConversionOptions, LatexResult, Notebook, PageImage, PageMeta};

/// Abstraction over any note input source (Local Files/PDF, Microsoft OneNote, Google Drive, etc.).
#[async_trait]
pub trait NoteSource: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    async fn list_notebooks(&self) -> Result<Vec<Notebook>>;
    async fn list_pages(&self, container_id: &str) -> Result<Vec<PageMeta>>;
    async fn fetch_page_images(&self, page_id: &str) -> Result<Vec<PageImage>>;
}

/// Abstraction over any Vision-to-LaTeX AI backend (Ollama, LM Studio, llama-server, Gemini, etc.).
#[async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn convert_to_latex(
        &self,
        images: &[PageImage],
        options: &ConversionOptions,
    ) -> Result<LatexResult>;
}
