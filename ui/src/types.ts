export interface PageImageDto {
  page_index: number;
  label: string;
  mime_type: string;
  width: number;
  height: number;
  data_url: string;
}

export interface Section {
  id: string;
  name: string;
}

export interface Notebook {
  id: string;
  name: string;
  sections: Section[];
}

export interface PageMeta {
  id: string;
  title: string;
  last_modified?: string | null;
  source_kind: string;
}

export type OutputMode = "document_body" | "standalone_document" | "math_only";

export interface ConversionOptions {
  language_hint: string;
  output_mode: OutputMode;
  extra_instructions?: string | null;
  temperature: number;
  max_tokens: number;
}

export interface LatexResult {
  latex_body: string;
  standalone_tex: string;
  markdown: string;
  model_used: string;
  provider_id: string;
  warnings: string[];
}

export type ProviderKind = "open_ai_compatible" | "gemini";

export interface ProviderConfig {
  kind: ProviderKind;
  base_url: string;
  model: string;
  api_key?: string | null;
}

export interface ProviderHealth {
  reachable: boolean;
  available_models: string[];
  message: string;
}

export interface PdfCompilerInfo {
  available: boolean;
  binary_name?: string | null;
}

export interface OneNoteAuthStatus {
  connected: boolean;
  client_id_configured: boolean;
  message: string;
}

export interface CatalogModel {
  id: string;
  name: string;
  description: string;
  approx_size_mb: number;
  recommended: boolean;
  model_filename: string;
  model_url: string;
  mmproj_filename: string;
  mmproj_url: string;
  downloaded: boolean;
  disk_bytes: number;
}

export interface DownloadProgress {
  stage: string;
  file_name: string;
  downloaded_bytes: number;
  total_bytes?: number | null;
  percent?: number | null;
}

export interface ManagedRuntimeStatus {
  data_dir: string;
  engine_installed: boolean;
  engine_path?: string | null;
  server_running: boolean;
  server_port?: number | null;
  server_base_url?: string | null;
  active_model_id?: string | null;
  models: CatalogModel[];
}

export interface FileLoadProgress {
  file_name: string;
  file_index: number;
  total_files: number;
  current_page: number;
  total_pages: number;
  percent_complete: number;
  percent_left: number;
}

