import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  ConversionOptions,
  DownloadProgress,
  LatexResult,
  ManagedRuntimeStatus,
  Notebook,
  OneNoteAuthStatus,
  PageImageDto,
  PageMeta,
  PdfCompilerInfo,
  ProviderConfig,
  ProviderHealth,
} from "./types";

export async function pickLocalNoteFiles(): Promise<string[]> {
  const selected = await open({
    multiple: true,
    filters: [
      {
        name: "Handwritten Notes & PDFs",
        extensions: ["png", "jpg", "jpeg", "webp", "pdf"],
      },
    ],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function pickSavePath(
  defaultName: string,
  ext: "tex" | "md" | "pdf"
): Promise<string | null> {
  const filterNames = {
    tex: "LaTeX Source",
    md: "Markdown Document",
    pdf: "PDF Document",
  };
  return await save({
    defaultPath: `${defaultName}.${ext}`,
    filters: [{ name: filterNames[ext], extensions: [ext] }],
  });
}

export async function loadLocalFiles(paths: string[]): Promise<PageImageDto[]> {
  return await invoke<PageImageDto[]>("load_local_files", { paths });
}

export async function checkAiHealth(
  config: ProviderConfig
): Promise<ProviderHealth> {
  return await invoke<ProviderHealth>("check_ai_health", { config });
}

export async function convertPagesToLatex(
  pages: PageImageDto[],
  config: ProviderConfig,
  options: ConversionOptions
): Promise<LatexResult> {
  return await invoke<LatexResult>("convert_pages_to_latex", {
    pages,
    config,
    options,
  });
}

export async function saveSecretKey(name: string, value: string): Promise<void> {
  await invoke("save_secret_key", { name, value });
}

export async function hasSecretKey(name: string): Promise<boolean> {
  return await invoke<boolean>("has_secret_key", { name });
}

export async function deleteSecretKey(name: string): Promise<void> {
  await invoke("delete_secret_key", { name });
}

export async function getPdfCompilerStatus(): Promise<PdfCompilerInfo> {
  return await invoke<PdfCompilerInfo>("get_pdf_compiler_status");
}

export async function exportNoteFile(
  path: string,
  format: "tex" | "md" | "pdf",
  latexBody: string,
  title?: string
): Promise<string> {
  return await invoke<string>("export_note_file", {
    path,
    format,
    latexBody,
    title: title ?? null,
  });
}

export async function onenoteAuthStatus(
  clientId: string
): Promise<OneNoteAuthStatus> {
  return await invoke<OneNoteAuthStatus>("onenote_auth_status", { clientId });
}

export async function onenoteLogin(
  clientId: string
): Promise<OneNoteAuthStatus> {
  return await invoke<OneNoteAuthStatus>("onenote_login", { clientId });
}

export async function onenoteDisconnect(
  clientId: string
): Promise<OneNoteAuthStatus> {
  return await invoke<OneNoteAuthStatus>("onenote_disconnect", { clientId });
}

export async function onenoteListNotebooks(
  clientId: string
): Promise<Notebook[]> {
  return await invoke<Notebook[]>("onenote_list_notebooks", { clientId });
}

export async function onenoteListPages(
  clientId: string,
  sectionId: string
): Promise<PageMeta[]> {
  return await invoke<PageMeta[]>("onenote_list_pages", {
    clientId,
    sectionId,
  });
}

export async function onenoteFetchPageImages(
  clientId: string,
  pageId: string
): Promise<PageImageDto[]> {
  return await invoke<PageImageDto[]>("onenote_fetch_page_images", {
    clientId,
    pageId,
  });
}

export async function runtimeStatus(): Promise<ManagedRuntimeStatus> {
  return await invoke<ManagedRuntimeStatus>("runtime_status");
}

export async function runtimeInstallAndStart(
  modelId: string
): Promise<ManagedRuntimeStatus> {
  return await invoke<ManagedRuntimeStatus>("runtime_install_and_start", {
    modelId,
  });
}

export async function runtimeStart(
  modelId: string
): Promise<ManagedRuntimeStatus> {
  return await invoke<ManagedRuntimeStatus>("runtime_start", { modelId });
}

export async function runtimeStop(): Promise<ManagedRuntimeStatus> {
  return await invoke<ManagedRuntimeStatus>("runtime_stop");
}

export async function runtimeDeleteModel(
  modelId: string
): Promise<ManagedRuntimeStatus> {
  return await invoke<ManagedRuntimeStatus>("runtime_delete_model", {
    modelId,
  });
}

export async function onAiDownloadProgress(
  handler: (progress: DownloadProgress) => void
): Promise<UnlistenFn> {
  return await listen<DownloadProgress>("ai-download-progress", (event) => {
    handler(event.payload);
  });
}

export async function onFileLoadProgress(
  handler: (progress: import("./types").FileLoadProgress) => void
): Promise<UnlistenFn> {
  return await listen<import("./types").FileLoadProgress>(
    "file-load-progress",
    (event) => {
      handler(event.payload);
    }
  );
}

