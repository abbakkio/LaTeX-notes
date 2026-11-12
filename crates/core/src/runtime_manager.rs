use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use tar::Archive;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};

use crate::error::{CoreError, Result};

const APP_DIR_NAME: &str = "dev.latexnotes.app";
const LLAMA_RELEASES_API: &str = "https://api.github.com/repos/ggml-org/llama.cpp/releases?per_page=8";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub approx_size_mb: u64,
    pub recommended: bool,
    pub model_filename: String,
    pub model_url: String,
    pub mmproj_filename: String,
    pub mmproj_url: String,
    pub downloaded: bool,
    pub disk_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub stage: String,
    pub file_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedRuntimeStatus {
    pub data_dir: String,
    pub engine_installed: bool,
    pub engine_path: Option<String>,
    pub server_running: bool,
    pub server_port: Option<u16>,
    pub server_base_url: Option<String>,
    pub active_model_id: Option<String>,
    pub models: Vec<CatalogModel>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub fn app_data_root() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join(APP_DIR_NAME)
}

pub fn runtime_dir() -> PathBuf {
    app_data_root().join("runtime")
}

pub fn models_dir() -> PathBuf {
    app_data_root().join("models")
}

pub fn builtin_catalog() -> Vec<CatalogModel> {
    let dir = models_dir();
    let specs = vec![
        (
            "qwen2.5-vl-7b-q4",
            "Qwen2.5-VL-7B-Instruct (4-bit Q4_K_M)",
            "Recommended: best accuracy for messy Russian & English handwriting, integrals, matrices, and diagrams. (~5.3 GB, 8+ GB RAM)",
            5280u64,
            true,
            "Qwen2.5-VL-7B-Instruct-Q4_K_M.gguf",
            "https://huggingface.co/ggml-org/Qwen2.5-VL-7B-Instruct-GGUF/resolve/main/Qwen2.5-VL-7B-Instruct-Q4_K_M.gguf",
            "mmproj-Qwen2.5-VL-7B-Instruct-Q8_0.gguf",
            "https://huggingface.co/ggml-org/Qwen2.5-VL-7B-Instruct-GGUF/resolve/main/mmproj-Qwen2.5-VL-7B-Instruct-Q8_0.gguf",
        ),
        (
            "qwen2.5-vl-3b-q4",
            "Qwen2.5-VL-3B-Instruct (4-bit Q4_K_M)",
            "Lightweight & fast: great for 8 GB RAM laptops and reasonably legible handwriting. (~2.6 GB)",
            2650u64,
            false,
            "Qwen2.5-VL-3B-Instruct-Q4_K_M.gguf",
            "https://huggingface.co/ggml-org/Qwen2.5-VL-3B-Instruct-GGUF/resolve/main/Qwen2.5-VL-3B-Instruct-Q4_K_M.gguf",
            "mmproj-Qwen2.5-VL-3B-Instruct-Q8_0.gguf",
            "https://huggingface.co/ggml-org/Qwen2.5-VL-3B-Instruct-GGUF/resolve/main/mmproj-Qwen2.5-VL-3B-Instruct-Q8_0.gguf",
        ),
        (
            "qwen2.5-vl-7b-q8",
            "Qwen2.5-VL-7B-Instruct (8-bit Q8_0)",
            "Maximum precision: best for dense formulas & difficult handwriting on 16+ GB RAM machines. (~8.5 GB)",
            8540u64,
            false,
            "Qwen2.5-VL-7B-Instruct-Q8_0.gguf",
            "https://huggingface.co/ggml-org/Qwen2.5-VL-7B-Instruct-GGUF/resolve/main/Qwen2.5-VL-7B-Instruct-Q8_0.gguf",
            "mmproj-Qwen2.5-VL-7B-Instruct-Q8_0.gguf",
            "https://huggingface.co/ggml-org/Qwen2.5-VL-7B-Instruct-GGUF/resolve/main/mmproj-Qwen2.5-VL-7B-Instruct-Q8_0.gguf",
        ),
    ];

    specs
        .into_iter()
        .map(
            |(id, name, desc, size_mb, rec, m_file, m_url, p_file, p_url)| {
                let m_path = dir.join(m_file);
                let p_path = dir.join(p_file);
                let m_bytes = std::fs::metadata(&m_path).map(|m| m.len()).unwrap_or(0);
                let p_bytes = std::fs::metadata(&p_path).map(|m| m.len()).unwrap_or(0);
                let downloaded = m_path.exists() && p_path.exists() && m_bytes > 0 && p_bytes > 0;
                CatalogModel {
                    id: id.to_string(),
                    name: name.to_string(),
                    description: desc.to_string(),
                    approx_size_mb: size_mb,
                    recommended: rec,
                    model_filename: m_file.to_string(),
                    model_url: m_url.to_string(),
                    mmproj_filename: p_file.to_string(),
                    mmproj_url: p_url.to_string(),
                    downloaded,
                    disk_bytes: m_bytes + p_bytes,
                }
            },
        )
        .collect()
}

pub fn find_catalog_model(model_id: &str) -> Result<CatalogModel> {
    builtin_catalog()
        .into_iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| CoreError::InvalidInput(format!("Unknown catalog model '{model_id}'")))
}

pub fn find_llama_server_binary() -> Option<PathBuf> {
    let exe_name = if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    };
    find_file_recursive(&runtime_dir(), exe_name)
}

fn find_file_recursive(dir: &Path, target_name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_recursive(&path, target_name) {
                return Some(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(target_name) {
            return Some(path);
        }
    }
    None
}

fn preferred_asset_suffixes(os: &str, arch: &str) -> Vec<&'static str> {
    match (os, arch) {
        ("macos", "aarch64") => vec!["-bin-macos-arm64.tar.gz"],
        ("macos", "x86_64") => vec!["-bin-macos-x64.tar.gz"],
        ("windows", "x86_64") => vec!["-bin-win-vulkan-x64.zip", "-bin-win-cpu-x64.zip"],
        ("windows", "aarch64") => vec!["-bin-win-vulkan-arm64.zip", "-bin-win-cpu-arm64.zip"],
        ("linux", "x86_64") => vec!["-bin-ubuntu-vulkan-x64.tar.gz", "-bin-ubuntu-x64.tar.gz"],
        ("linux", "aarch64") => vec!["-bin-ubuntu-arm64.tar.gz"],
        _ => vec![],
    }
}

fn select_matching_asset(releases: &[GithubRelease], os: &str, arch: &str) -> Option<(String, String, String)> {
    let suffixes = preferred_asset_suffixes(os, arch);
    for release in releases {
        for suffix in &suffixes {
            if let Some(asset) = release
                .assets
                .iter()
                .find(|a| a.name.starts_with("llama-") && a.name.ends_with(suffix))
            {
                return Some((
                    release.tag_name.clone(),
                    asset.name.clone(),
                    asset.browser_download_url.clone(),
                ));
            }
        }
    }
    None
}

/// Downloads and extracts the prebuilt `llama.cpp` server binary for the current OS/architecture.
pub async fn ensure_engine_installed<F>(mut on_progress: F) -> Result<PathBuf>
where
    F: FnMut(DownloadProgress) + Send,
{
    if let Some(existing) = find_llama_server_binary() {
        return Ok(existing);
    }

    let client = reqwest::Client::builder()
        .user_agent("latex-notes-app/0.1")
        .timeout(Duration::from_secs(60))
        .build()?;

    let releases: Vec<GithubRelease> = client.get(LLAMA_RELEASES_API).send().await?.json().await?;
    let (_tag, asset_name, download_url) = select_matching_asset(
        &releases,
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
    .ok_or_else(|| {
        CoreError::Provider {
            provider: "runtime_manager".to_string(),
            message: format!(
                "No prebuilt llama.cpp binary found for {}/{}.",
                std::env::consts::OS,
                std::env::consts::ARCH
            ),
        }
    })?;

    let rt_dir = runtime_dir();
    tokio::fs::create_dir_all(&rt_dir).await?;
    let archive_path = rt_dir.join(&asset_name);

    download_file_with_progress(&download_url, &archive_path, "Downloading AI engine", &asset_name, &mut on_progress).await?;

    on_progress(DownloadProgress {
        stage: "Extracting AI engine".to_string(),
        file_name: asset_name.clone(),
        downloaded_bytes: 0,
        total_bytes: None,
        percent: None,
    });

    let rt_clone = rt_dir.clone();
    let arch_clone = archive_path.clone();
    tokio::task::spawn_blocking(move || extract_archive(&arch_clone, &rt_clone))
        .await
        .map_err(|e| CoreError::Io(std::io::Error::other(format!("Extract task panicked: {e}"))))??;

    let _ = tokio::fs::remove_file(&archive_path).await;

    let bin_path = find_llama_server_binary().ok_or_else(|| CoreError::Provider {
        provider: "runtime_manager".to_string(),
        message: "Extracted llama.cpp archive did not contain llama-server binary.".to_string(),
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&bin_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin_path, perms)?;
    }

    Ok(bin_path)
}

/// Downloads both the GGUF model weights and its multimodal vision projector (`mmproj`) if missing.
pub async fn ensure_model_downloaded<F>(model_id: &str, mut on_progress: F) -> Result<(PathBuf, PathBuf)>
where
    F: FnMut(DownloadProgress) + Send,
{
    let model = find_catalog_model(model_id)?;
    let dir = models_dir();
    tokio::fs::create_dir_all(&dir).await?;

    let mmproj_path = dir.join(&model.mmproj_filename);
    if !mmproj_path.exists() {
        download_file_with_progress(
            &model.mmproj_url,
            &mmproj_path,
            "Downloading vision projector",
            &model.mmproj_filename,
            &mut on_progress,
        )
        .await?;
    }

    let model_path = dir.join(&model.model_filename);
    if !model_path.exists() {
        download_file_with_progress(
            &model.model_url,
            &model_path,
            "Downloading vision AI model",
            &model.model_filename,
            &mut on_progress,
        )
        .await?;
    }

    Ok((model_path, mmproj_path))
}

pub fn delete_downloaded_model(model_id: &str) -> Result<()> {
    let model = find_catalog_model(model_id)?;
    let dir = models_dir();
    let m_path = dir.join(&model.model_filename);
    let m_part = dir.join(format!("{}.part", model.model_filename));
    let p_path = dir.join(&model.mmproj_filename);
    let p_part = dir.join(format!("{}.part", model.mmproj_filename));

    for path in [&m_path, &m_part, &p_path, &p_part] {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

async fn download_file_with_progress<F>(
    url: &str,
    dest: &Path,
    stage: &str,
    file_name: &str,
    on_progress: &mut F,
) -> Result<()>
where
    F: FnMut(DownloadProgress),
{
    let part_path = dest.with_extension(
        dest.extension()
            .and_then(|e| e.to_str())
            .map(|e| format!("{e}.part"))
            .unwrap_or_else(|| "part".to_string()),
    );

    let existing_len = tokio::fs::metadata(&part_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    let client = reqwest::Client::builder()
        .user_agent("latex-notes-app/0.1")
        .connect_timeout(Duration::from_secs(30))
        .build()?;

    let mut req = client.get(url);
    if existing_len > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={existing_len}-"));
    }

    let mut resp = req.send().await?;
    let status = resp.status();
    if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE && existing_len > 0 {
        // Already completely downloaded in .part
        tokio::fs::rename(&part_path, dest).await?;
        return Ok(());
    }
    if !status.is_success() {
        return Err(CoreError::Provider {
            provider: "downloader".to_string(),
            message: format!("Failed to download {file_name}: HTTP {status}"),
        });
    }

    let is_partial = status == reqwest::StatusCode::PARTIAL_CONTENT;
    let mut downloaded = if is_partial { existing_len } else { 0 };
    let total_bytes = resp
        .content_length()
        .map(|len| if is_partial { len + existing_len } else { len });

    let mut file = if is_partial {
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&part_path)
            .await?
    } else {
        tokio::fs::File::create(&part_path).await?
    };

    let mut last_emit = std::time::Instant::now();
    while let Some(chunk) = resp.chunk().await? {
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        if last_emit.elapsed() >= Duration::from_millis(120) {
            let percent = total_bytes.map(|t| {
                if t == 0 {
                    100.0
                } else {
                    ((downloaded as f64 / t as f64) * 100.0).min(100.0) as f32
                }
            });
            on_progress(DownloadProgress {
                stage: stage.to_string(),
                file_name: file_name.to_string(),
                downloaded_bytes: downloaded,
                total_bytes,
                percent,
            });
            last_emit = std::time::Instant::now();
        }
    }

    file.flush().await?;
    drop(file);

    on_progress(DownloadProgress {
        stage: stage.to_string(),
        file_name: file_name.to_string(),
        downloaded_bytes: downloaded,
        total_bytes: total_bytes.or(Some(downloaded)),
        percent: Some(100.0),
    });

    tokio::fs::rename(&part_path, dest).await?;
    Ok(())
}

fn sanitize_archive_rel_path(rel: &Path) -> Option<PathBuf> {
    let mut clean = PathBuf::new();
    for comp in rel.components() {
        match comp {
            Component::Normal(seg) => clean.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if clean.as_os_str().is_empty() {
        None
    } else {
        Some(clean)
    }
}

fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        let file = File::open(archive_path)?;
        let gz = GzDecoder::new(file);
        let mut archive = Archive::new(gz);
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            let Some(clean_rel) = sanitize_archive_rel_path(&path) else {
                continue;
            };
            let out_path = dest_dir.join(clean_rel);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            entry.unpack(&out_path)?;
        }
        Ok(())
    } else if name.ends_with(".zip") {
        let file = File::open(archive_path)?;
        let mut zip = zip::ZipArchive::new(file)
            .map_err(|e| CoreError::Io(std::io::Error::other(format!("Invalid zip archive: {e}"))))?;
        for i in 0..zip.len() {
            let mut entry = zip
                .by_index(i)
                .map_err(|e| CoreError::Io(std::io::Error::other(format!("Zip entry error: {e}"))))?;
            let Some(raw_path) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
                continue;
            };
            let Some(clean_rel) = sanitize_archive_rel_path(&raw_path) else {
                continue;
            };
            let out_path = dest_dir.join(clean_rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&out_path)?;
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out_file = File::create(&out_path)?;
                let mut buf = [0u8; 65536];
                loop {
                    let n = entry.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    std::io::Write::write_all(&mut out_file, &buf[..n])?;
                }
            }
        }
        Ok(())
    } else {
        Err(CoreError::InvalidInput(format!(
            "Unsupported archive format: {name}"
        )))
    }
}

/// Manages a child `llama-server` process for zero-setup local Vision AI execution.
pub struct ManagedLlamaServer {
    child: Option<Child>,
    port: Option<u16>,
    active_model_id: Option<String>,
}

impl Default for ManagedLlamaServer {
    fn default() -> Self {
        Self::new()
    }
}

impl ManagedLlamaServer {
    pub fn new() -> Self {
        Self {
            child: None,
            port: None,
            active_model_id: None,
        }
    }

    pub fn is_running(&mut self) -> bool {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(None) => true,
                _ => {
                    self.child = None;
                    self.port = None;
                    self.active_model_id = None;
                    false
                }
            }
        } else {
            false
        }
    }

    pub fn status(&mut self) -> ManagedRuntimeStatus {
        let running = self.is_running();
        let engine_path = find_llama_server_binary();
        ManagedRuntimeStatus {
            data_dir: app_data_root().to_string_lossy().into_owned(),
            engine_installed: engine_path.is_some(),
            engine_path: engine_path.map(|p| p.to_string_lossy().into_owned()),
            server_running: running,
            server_port: if running { self.port } else { None },
            server_base_url: if running {
                self.port.map(|p| format!("http://127.0.0.1:{p}/v1"))
            } else {
                None
            },
            active_model_id: if running {
                self.active_model_id.clone()
            } else {
                None
            },
            models: builtin_catalog(),
        }
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        self.port = None;
        self.active_model_id = None;
        Ok(())
    }

    pub async fn start(&mut self, model_id: &str) -> Result<ManagedRuntimeStatus> {
        self.stop().await?;

        let bin_path = find_llama_server_binary().ok_or_else(|| CoreError::Provider {
            provider: "runtime_manager".to_string(),
            message: "AI engine is not installed yet. Click 'Install & Start Local AI' first."
                .to_string(),
        })?;
        let model = find_catalog_model(model_id)?;
        let model_path = models_dir().join(&model.model_filename);
        let mmproj_path = models_dir().join(&model.mmproj_filename);
        if !model_path.exists() || !mmproj_path.exists() {
            return Err(CoreError::Provider {
                provider: "runtime_manager".to_string(),
                message: format!("Model '{model_id}' is not downloaded yet."),
            });
        }

        // Bind ephemerally to pick a free localhost port
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);

        let mut cmd = Command::new(&bin_path);
        cmd.arg("-m")
            .arg(&model_path)
            .arg("--mmproj")
            .arg(&mmproj_path)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("-ngl")
            .arg("99")
            .arg("-c")
            .arg("8192")
            .arg("--alias")
            .arg(model_id)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if let Some(bin_dir) = bin_path.parent() {
            cmd.current_dir(bin_dir);
        }

        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn()?;
        let health_url = format!("http://127.0.0.1:{port}/health");
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()?;

        // Wait up to 90s for model weights + mmproj to finish loading into memory/VRAM
        let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
        loop {
            if let Ok(Some(exit_status)) = child.try_wait() {
                let mut err_out = String::new();
                if let Some(mut stderr) = child.stderr.take() {
                    let mut buf = Vec::new();
                    let _ = tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut buf).await;
                    err_out = String::from_utf8_lossy(&buf).chars().take(1200).collect();
                }
                return Err(CoreError::Provider {
                    provider: "llama-server".to_string(),
                    message: format!("llama-server exited early ({exit_status}): {err_out}"),
                });
            }

            if let Ok(resp) = client.get(&health_url).send().await {
                if resp.status().is_success() {
                    break;
                }
            }

            if tokio::time::Instant::now() >= deadline {
                let _ = child.kill().await;
                return Err(CoreError::Provider {
                    provider: "llama-server".to_string(),
                    message: "Timed out waiting for llama-server to become ready.".to_string(),
                });
            }

            tokio::time::sleep(Duration::from_millis(350)).await;
        }

        self.child = Some(child);
        self.port = Some(port);
        self.active_model_id = Some(model_id.to_string());
        Ok(self.status())
    }
}

impl Drop for ManagedLlamaServer {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.start_kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_recommended_qwen_vl_models() {
        let catalog = builtin_catalog();
        assert!(catalog.iter().any(|m| m.id == "qwen2.5-vl-7b-q4" && m.recommended));
        assert!(catalog.iter().any(|m| m.id == "qwen2.5-vl-3b-q4"));
    }

    #[test]
    fn matches_expected_platform_assets() {
        let fake = vec![GithubRelease {
            tag_name: "b12319".into(),
            assets: vec![
                GithubAsset {
                    name: "llama-b12319-bin-macos-arm64.tar.gz".into(),
                    browser_download_url: "https://example.com/mac.tar.gz".into(),
                },
                GithubAsset {
                    name: "llama-b12319-bin-win-vulkan-x64.zip".into(),
                    browser_download_url: "https://example.com/win.zip".into(),
                },
            ],
        }];

        let mac = select_matching_asset(&fake, "macos", "aarch64").unwrap();
        assert_eq!(mac.1, "llama-b12319-bin-macos-arm64.tar.gz");

        let win = select_matching_asset(&fake, "windows", "x86_64").unwrap();
        assert_eq!(win.1, "llama-b12319-bin-win-vulkan-x64.zip");
    }

    #[test]
    fn rejects_parent_traversal_in_archive_paths() {
        assert!(sanitize_archive_rel_path(Path::new("../../etc/passwd")).is_none());
        assert_eq!(
            sanitize_archive_rel_path(Path::new("./build/bin/llama-server")),
            Some(PathBuf::from("build/bin/llama-server"))
        );
    }
}
