use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::error::{CoreError, Result};
use crate::image_prep::{normalize_image_bytes, rasterize_pdf_bytes_with_progress};
use crate::models::{Notebook, PageImage, PageMeta, Section};
use crate::traits::NoteSource;

/// Reads local handwritten images (.png, .jpg, .jpeg, .webp) or exported OneNote/tablet PDFs.
#[derive(Debug, Clone)]
pub struct LocalFileSource {
    paths: Vec<PathBuf>,
}

impl LocalFileSource {
    pub fn from_paths(paths: Vec<PathBuf>) -> Self {
        Self { paths }
    }

    pub fn load_file_sync(path: &Path) -> Result<Vec<PageImage>> {
        Self::load_file_sync_with_progress(path, |_, _| {})
    }

    pub fn load_file_sync_with_progress<F>(path: &Path, mut on_progress: F) -> Result<Vec<PageImage>>
    where
        F: FnMut(usize, usize),
    {
        if !path.exists() {
            return Err(CoreError::Source {
                source_name: "local_file".to_string(),
                message: format!("File does not exist: {}", path.display()),
            });
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("note")
            .to_string();

        let bytes = std::fs::read(path)?;
        match ext.as_str() {
            "pdf" => rasterize_pdf_bytes_with_progress(bytes, &file_name, on_progress),
            "png" | "jpg" | "jpeg" | "webp" => {
                on_progress(0, 1);
                let page = normalize_image_bytes(&bytes, 0, file_name, None)?;
                on_progress(1, 1);
                Ok(vec![page])
            }
            other => Err(CoreError::InvalidInput(format!(
                "Unsupported file extension '.{other}'. Supported formats: PNG, JPG, WebP, PDF."
            ))),
        }
    }
}

#[async_trait]
impl NoteSource for LocalFileSource {
    fn id(&self) -> &'static str {
        "local_file"
    }

    fn display_name(&self) -> &'static str {
        "Local Files & PDFs"
    }

    async fn list_notebooks(&self) -> Result<Vec<Notebook>> {
        Ok(vec![Notebook {
            id: "local".to_string(),
            name: "Selected Files".to_string(),
            sections: vec![Section {
                id: "local_files".to_string(),
                name: "Imported Notes".to_string(),
            }],
        }])
    }

    async fn list_pages(&self, _container_id: &str) -> Result<Vec<PageMeta>> {
        let mut metas = Vec::new();
        for p in &self.paths {
            let title = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Untitled")
                .to_string();
            metas.push(PageMeta {
                id: p.to_string_lossy().into_owned(),
                title,
                last_modified: None,
                source_kind: "local_file".to_string(),
            });
        }
        Ok(metas)
    }

    async fn fetch_page_images(&self, page_id: &str) -> Result<Vec<PageImage>> {
        let path = PathBuf::from(page_id);
        tokio::task::spawn_blocking(move || Self::load_file_sync(&path))
            .await
            .map_err(|e| CoreError::Source {
                source_name: "local_file".to_string(),
                message: format!("Background image loader panicked: {e}"),
            })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    #[test]
    fn reports_progress_when_loading_local_image() {
        let dir = std::env::temp_dir().join(format!(
            "latex_notes_test_{}_{}",
            std::process::id(),
            rand::random::<u32>()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let img_path = dir.join("sample.png");
        let img = ImageBuffer::from_pixel(32, 32, Rgb([255u8, 255, 255]));
        img.save(&img_path).unwrap();

        let mut events = Vec::new();
        let pages = LocalFileSource::load_file_sync_with_progress(&img_path, |done, total| {
            events.push((done, total));
        })
        .unwrap();

        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(pages.len(), 1);
        assert_eq!(events, vec![(0, 1), (1, 1)]);
    }
}
