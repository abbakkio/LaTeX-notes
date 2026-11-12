use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),

    #[error("PDF rendering error: {0}")]
    Pdf(String),

    #[error("InkML parsing/rendering error: {0}")]
    InkMl(String),

    #[error("AI provider error ({provider}): {message}")]
    Provider { provider: String, message: String },

    #[error("Source error ({source_name}): {message}")]
    Source { source_name: String, message: String },

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Keyring error: {0}")]
    Keyring(String),

    #[error("Export error: {0}")]
    Export(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
