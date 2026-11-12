use std::io::Cursor;

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings, render};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat};

use crate::error::{CoreError, Result};
use crate::models::PageImage;

const DEFAULT_MAX_EDGE: u32 = 2048;

/// Decodes raw image bytes (PNG/JPG/WebP), normalizes size for VLM vision tokens, and re-encodes as PNG.
pub fn normalize_image_bytes(
    bytes: &[u8],
    page_index: usize,
    label: String,
    max_edge: Option<u32>,
) -> Result<PageImage> {
    let img = image::load_from_memory(bytes)?;
    normalize_dynamic_image(img, page_index, label, max_edge.unwrap_or(DEFAULT_MAX_EDGE))
}

pub fn normalize_dynamic_image(
    img: DynamicImage,
    page_index: usize,
    label: String,
    max_edge: u32,
) -> Result<PageImage> {
    let (w, h) = img.dimensions();
    let resized = if w > max_edge || h > max_edge {
        img.resize(max_edge, max_edge, FilterType::Lanczos3)
    } else {
        img
    };
    let (rw, rh) = resized.dimensions();
    let mut out = Cursor::new(Vec::new());
    resized.write_to(&mut out, ImageFormat::Png)?;

    Ok(PageImage {
        page_index,
        label,
        mime_type: "image/png".to_string(),
        width: rw,
        height: rh,
        png_bytes: out.into_inner(),
    })
}

/// Rasterizes all pages of a PDF byte buffer into normalized PNG `PageImage` items using pure-Rust `hayro`.
pub fn rasterize_pdf_bytes(pdf_bytes: Vec<u8>, label_prefix: &str) -> Result<Vec<PageImage>> {
    rasterize_pdf_bytes_with_progress(pdf_bytes, label_prefix, |_, _| {})
}

/// Rasterizes all pages of a PDF byte buffer and invokes `on_progress(completed_pages, total_pages)`
/// before rendering starts (`0, total_pages`) and after each page completes.
pub fn rasterize_pdf_bytes_with_progress<F>(
    pdf_bytes: Vec<u8>,
    label_prefix: &str,
    mut on_progress: F,
) -> Result<Vec<PageImage>>
where
    F: FnMut(usize, usize),
{
    let pdf = Pdf::new(pdf_bytes)
        .map_err(|_| CoreError::Pdf("Failed to parse PDF file (it may be encrypted or malformed).".to_string()))?;

    let pdf_pages = pdf.pages();
    let total_pages = pdf_pages.len();
    if total_pages == 0 {
        return Err(CoreError::Pdf("PDF contains 0 pages.".to_string()));
    }

    on_progress(0, total_pages);

    let interpreter_settings = InterpreterSettings::default();
    let render_settings = RenderSettings::default();
    let cache = RenderCache::new();
    let pixmap_settings = PixmapSettings {
        x_scale: 2.0,
        y_scale: 2.0,
        bg_color: WHITE,
    };

    let mut pages = Vec::with_capacity(total_pages);
    for (idx, page) in pdf_pages.iter().enumerate() {
        let rendered = render(
            page,
            &cache,
            &interpreter_settings,
            &render_settings,
            &pixmap_settings,
        );
        let width = rendered.width() as u32;
        let height = rendered.height() as u32;
        let png_bytes = rendered
            .into_png()
            .map_err(|e| CoreError::Pdf(format!("Failed to encode PDF page {} as PNG: {e}", idx + 1)))?;

        // Re-run normalize if oversized
        if width > DEFAULT_MAX_EDGE || height > DEFAULT_MAX_EDGE {
            pages.push(normalize_image_bytes(
                &png_bytes,
                idx,
                format!("{label_prefix} (Page {})", idx + 1),
                Some(DEFAULT_MAX_EDGE),
            )?);
        } else {
            pages.push(PageImage {
                page_index: idx,
                label: format!("{label_prefix} (Page {})", idx + 1),
                mime_type: "image/png".to_string(),
                width,
                height,
                png_bytes,
            });
        }

        on_progress(idx + 1, total_pages);
    }

    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    #[test]
    fn downscales_oversized_image_preserving_aspect() {
        let img = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(3000, 1500, Rgb([255, 255, 255])));
        let page = normalize_dynamic_image(img, 0, "test".into(), 1000).unwrap();
        assert_eq!(page.width, 1000);
        assert_eq!(page.height, 500);
        assert!(!page.png_bytes.is_empty());
    }
}
