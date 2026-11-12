pub mod error;
pub mod image_prep;
pub mod latex;
pub mod models;
pub mod providers;
pub mod runtime_manager;
pub mod secrets;
pub mod sources;
pub mod traits;

pub use error::{CoreError, Result};
pub use models::*;
pub use traits::{AiProvider, NoteSource};
