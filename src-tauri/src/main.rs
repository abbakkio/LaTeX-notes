#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine;
use latex_notes_core::latex::{
    compile_latex_to_pdf, detect_pdf_compiler, latex_body_to_markdown, wrap_standalone_document,
};
use latex_notes_core::models::{
    ConversionOptions, LatexResult, Notebook, PageImage, PageMeta, PdfCompilerInfo,
    ProviderConfig, ProviderHealth, ProviderKind,
};
use latex_notes_core::providers::{build_provider, check_provider_health};
use latex_notes_core::runtime_manager::{
    ManagedLlamaServer, ManagedRuntimeStatus, delete_downloaded_model, ensure_engine_installed,
    ensure_model_downloaded,
};
use latex_notes_core::secrets::{delete_secret, get_secret, set_secret};
use latex_notes_core::sources::{LocalFileSource, OneNoteAuthStatus, OneNoteGraphSource};
use latex_notes_core::traits::NoteSource;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

struct AppState {
    llama_server: Arc<Mutex<ManagedLlamaServer>>,
    downloading: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageImageDto {
    pub page_index: usize,
    pub label: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub data_url: String,
}

impl From<PageImage> for PageImageDto {
    fn from(p: PageImage) -> Self {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&p.png_bytes);
        Self {
            page_index: p.page_index,
            label: p.label,
            data_url: format!("data:{};base64,{}", p.mime_type, b64),
            mime_type: p.mime_type,
            width: p.width,
            height: p.height,
        }
    }
}

fn page_dtos_to_images(pages: Vec<PageImageDto>) -> Result<Vec<PageImage>, String> {
    let mut out = Vec::with_capacity(pages.len());
    for p in pages {
        let raw_b64 = p
            .data_url
            .split_once(",")
            .map(|(_, b)| b)
            .unwrap_or(&p.data_url);
        let png_bytes = base64::engine::general_purpose::STANDARD
            .decode(raw_b64)
            .map_err(|e| format!("Invalid base64 image payload: {e}"))?;
        out.push(PageImage {
            page_index: p.page_index,
            label: p.label,
            mime_type: p.mime_type,
            width: p.width,
            height: p.height,
            png_bytes,
        });
    }
    Ok(out)
}

fn hydrate_provider_secrets(mut cfg: ProviderConfig) -> ProviderConfig {
    let missing_key = cfg
        .api_key
        .as_deref()
        .map(|k| k.trim().is_empty())
        .unwrap_or(true);
    if missing_key && cfg.kind == ProviderKind::Gemini {
        if let Ok(Some(saved)) = get_secret("gemini_api_key") {
            cfg.api_key = Some(saved);
        }
    }
    cfg
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileLoadProgress {
    pub file_name: String,
    pub file_index: usize,
    pub total_files: usize,
    pub current_page: usize,
    pub total_pages: usize,
    pub percent_complete: f32,
    pub percent_left: f32,
}

fn emit_file_progress(
    app: &tauri::AppHandle,
    file_name: &str,
    file_index: usize,
    total_files: usize,
    current_page: usize,
    total_pages: usize,
) {
    let total_files_denom = total_files.max(1) as f32;
    let page_fraction = if total_pages == 0 {
        0.0
    } else {
        (current_page as f32 / total_pages as f32).clamp(0.0, 1.0)
    };
    let percent_complete = (((file_index as f32 + page_fraction) / total_files_denom) * 100.0)
        .clamp(0.0, 100.0);
    let percent_left = (100.0 - percent_complete).clamp(0.0, 100.0);

    let _ = app.emit(
        "file-load-progress",
        FileLoadProgress {
            file_name: file_name.to_string(),
            file_index,
            total_files,
            current_page,
            total_pages,
            percent_complete,
            percent_left,
        },
    );
}

#[tauri::command]
async fn load_local_files(
    app: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<Vec<PageImageDto>, String> {
    let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let total_files = path_bufs.len();

    tokio::task::spawn_blocking(move || -> Result<Vec<PageImageDto>, String> {
        let mut all_pages = Vec::new();
        for (file_idx, path) in path_bufs.into_iter().enumerate() {
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("note")
                .to_string();

            emit_file_progress(&app, &file_name, file_idx, total_files, 0, 0);

            let loaded = LocalFileSource::load_file_sync_with_progress(&path, |completed, total| {
                emit_file_progress(&app, &file_name, file_idx, total_files, completed, total);
            })
            .map_err(|e| e.to_string())?;

            for page in loaded {
                all_pages.push(PageImageDto::from(page));
            }
        }
        Ok(all_pages)
    })
    .await
    .map_err(|e| format!("Failed to join file loader: {e}"))?
}

#[tauri::command]
async fn check_ai_health(config: ProviderConfig) -> Result<ProviderHealth, String> {
    let hydrated = hydrate_provider_secrets(config);
    check_provider_health(&hydrated)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn convert_pages_to_latex(
    pages: Vec<PageImageDto>,
    config: ProviderConfig,
    options: ConversionOptions,
) -> Result<LatexResult, String> {
    let hydrated = hydrate_provider_secrets(config);
    let images = page_dtos_to_images(pages)?;
    let provider = build_provider(&hydrated);
    provider
        .convert_to_latex(&images, &options)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn save_secret_key(name: String, value: String) -> Result<(), String> {
    if value.trim().is_empty() {
        delete_secret(&name).map_err(|e| e.to_string())
    } else {
        set_secret(&name, value.trim()).map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn has_secret_key(name: String) -> Result<bool, String> {
    get_secret(&name)
        .map(|opt| opt.is_some())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_secret_key(name: String) -> Result<(), String> {
    delete_secret(&name).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_pdf_compiler_status() -> PdfCompilerInfo {
    detect_pdf_compiler().await
}

#[tauri::command]
async fn export_note_file(
    path: String,
    format: String,
    latex_body: String,
    title: Option<String>,
) -> Result<String, String> {
    let out_path = PathBuf::from(&path);
    match format.as_str() {
        "tex" => {
            let content = wrap_standalone_document(&latex_body, title.as_deref());
            tokio::fs::write(&out_path, content)
                .await
                .map_err(|e| e.to_string())?;
            Ok(format!("Exported .tex to {}", out_path.display()))
        }
        "md" => {
            let content = latex_body_to_markdown(&latex_body);
            tokio::fs::write(&out_path, content)
                .await
                .map_err(|e| e.to_string())?;
            Ok(format!("Exported .md to {}", out_path.display()))
        }
        "pdf" => {
            let standalone = wrap_standalone_document(&latex_body, title.as_deref());
            let compiler = compile_latex_to_pdf(&standalone, &out_path)
                .await
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "Compiled PDF via {compiler} to {}",
                out_path.display()
            ))
        }
        other => Err(format!("Unsupported export format: {other}")),
    }
}

#[tauri::command]
fn onenote_auth_status(client_id: String) -> OneNoteAuthStatus {
    OneNoteGraphSource::new(client_id).auth_status()
}

#[tauri::command]
async fn onenote_login(client_id: String) -> Result<OneNoteAuthStatus, String> {
    let src = OneNoteGraphSource::new(client_id);
    src.login_interactive().await.map_err(|e| e.to_string())?;
    Ok(src.auth_status())
}

#[tauri::command]
fn onenote_disconnect(client_id: String) -> Result<OneNoteAuthStatus, String> {
    let src = OneNoteGraphSource::new(client_id);
    src.disconnect().map_err(|e| e.to_string())?;
    Ok(src.auth_status())
}

#[tauri::command]
async fn onenote_list_notebooks(client_id: String) -> Result<Vec<Notebook>, String> {
    OneNoteGraphSource::new(client_id)
        .list_notebooks()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn onenote_list_pages(
    client_id: String,
    section_id: String,
) -> Result<Vec<PageMeta>, String> {
    OneNoteGraphSource::new(client_id)
        .list_pages(&section_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn onenote_fetch_page_images(
    client_id: String,
    page_id: String,
) -> Result<Vec<PageImageDto>, String> {
    let pages = OneNoteGraphSource::new(client_id)
        .fetch_page_images(&page_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(pages.into_iter().map(PageImageDto::from).collect())
}

#[tauri::command]
async fn runtime_status(state: tauri::State<'_, AppState>) -> Result<ManagedRuntimeStatus, String> {
    let mut srv = state.llama_server.lock().await;
    Ok(srv.status())
}

#[tauri::command]
async fn runtime_install_and_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    model_id: String,
) -> Result<ManagedRuntimeStatus, String> {
    if state
        .downloading
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("An AI download/setup is already in progress.".to_string());
    }

    struct ResetGuard(Arc<AtomicBool>);
    impl Drop for ResetGuard {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let _guard = ResetGuard(state.downloading.clone());

    let app_for_engine = app.clone();
    ensure_engine_installed(move |prog| {
        let _ = app_for_engine.emit("ai-download-progress", &prog);
    })
    .await
    .map_err(|e| e.to_string())?;

    let app_for_model = app.clone();
    ensure_model_downloaded(&model_id, move |prog| {
        let _ = app_for_model.emit("ai-download-progress", &prog);
    })
    .await
    .map_err(|e| e.to_string())?;

    let _ = app.emit(
        "ai-download-progress",
        latex_notes_core::runtime_manager::DownloadProgress {
            stage: "Starting local AI server".to_string(),
            file_name: model_id.clone(),
            downloaded_bytes: 0,
            total_bytes: None,
            percent: None,
        },
    );

    let mut srv = state.llama_server.lock().await;
    srv.start(&model_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn runtime_start(
    state: tauri::State<'_, AppState>,
    model_id: String,
) -> Result<ManagedRuntimeStatus, String> {
    let mut srv = state.llama_server.lock().await;
    srv.start(&model_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn runtime_stop(state: tauri::State<'_, AppState>) -> Result<ManagedRuntimeStatus, String> {
    let mut srv = state.llama_server.lock().await;
    srv.stop().await.map_err(|e| e.to_string())?;
    Ok(srv.status())
}

#[tauri::command]
async fn runtime_delete_model(
    state: tauri::State<'_, AppState>,
    model_id: String,
) -> Result<ManagedRuntimeStatus, String> {
    let mut srv = state.llama_server.lock().await;
    let current = srv.status();
    if current.active_model_id.as_deref() == Some(model_id.as_str()) {
        srv.stop().await.map_err(|e| e.to_string())?;
    }
    delete_downloaded_model(&model_id).map_err(|e| e.to_string())?;
    Ok(srv.status())
}

fn main() {
    let app_state = AppState {
        llama_server: Arc::new(Mutex::new(ManagedLlamaServer::new())),
        downloading: Arc::new(AtomicBool::new(false)),
    };

    let app = tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            load_local_files,
            check_ai_health,
            convert_pages_to_latex,
            save_secret_key,
            has_secret_key,
            delete_secret_key,
            get_pdf_compiler_status,
            export_note_file,
            onenote_auth_status,
            onenote_login,
            onenote_disconnect,
            onenote_list_notebooks,
            onenote_list_pages,
            onenote_fetch_page_images,
            runtime_status,
            runtime_install_and_start,
            runtime_start,
            runtime_stop,
            runtime_delete_model,
        ])
        .build(tauri::generate_context!())
        .expect("error while building LaTeX Notes Tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            let state = app_handle.state::<AppState>();
            let srv_arc = state.llama_server.clone();
            tauri::async_runtime::block_on(async move {
                let mut srv = srv_arc.lock().await;
                let _ = srv.stop().await;
            });
        }
    });
}
