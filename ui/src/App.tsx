import React, { useEffect, useState } from "react";
import {
  AlertTriangle,
  BookOpen,
  Check,
  ChevronLeft,
  ChevronRight,
  Copy,
  Cpu,
  Download,
  FileText,
  FolderOpen,
  Loader2,
  Settings,
  Sparkles,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import {
  checkAiHealth,
  convertPagesToLatex,
  exportNoteFile,
  getPdfCompilerStatus,
  loadLocalFiles,
  onenoteAuthStatus,
  onenoteFetchPageImages,
  onenoteListNotebooks,
  onenoteListPages,
  onFileLoadProgress,
  pickLocalNoteFiles,
  pickSavePath,
  runtimeStatus,
} from "./api";
import { CircularProgressSpinner } from "./components/CircularProgressSpinner";
import { LatexEditor } from "./components/LatexEditor";
import { LatexPreview } from "./components/LatexPreview";
import { LocalAiSetupModal } from "./components/LocalAiSetupModal";
import { SettingsModal } from "./components/SettingsModal";
import type {
  ConversionOptions,
  FileLoadProgress,
  ManagedRuntimeStatus,
  Notebook,
  OneNoteAuthStatus,
  PageImageDto,
  PageMeta,
  PdfCompilerInfo,
  ProviderConfig,
  ProviderHealth,
} from "./types";

type SourceTab = "local" | "onenote";
type RightViewMode = "split" | "preview" | "code";

const DEFAULT_PROVIDER: ProviderConfig = {
  kind: "open_ai_compatible",
  base_url: "http://localhost:11434/v1",
  model: "qwen2.5vl:7b",
  api_key: null,
};

const DEFAULT_OPTIONS: ConversionOptions = {
  language_hint: "English and Russian (Cyrillic + Latin)",
  output_mode: "document_body",
  extra_instructions: null,
  temperature: 0.1,
  max_tokens: 4096,
};

function loadSavedJson<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

export const App: React.FC = () => {
  const [sourceTab, setSourceTab] = useState<SourceTab>("local");
  const [rightMode, setRightMode] = useState<RightViewMode>("split");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [aiSetupOpen, setAiSetupOpen] = useState(false);

  const [providerConfig, setProviderConfig] = useState<ProviderConfig>(() =>
    loadSavedJson("ln_provider_cfg", DEFAULT_PROVIDER)
  );
  const [conversionOptions, setConversionOptions] = useState<ConversionOptions>(
    () => loadSavedJson("ln_conversion_opts", DEFAULT_OPTIONS)
  );
  const [oneNoteClientId, setOneNoteClientId] = useState<string>(
    () => localStorage.getItem("ln_onenote_client_id") ?? ""
  );

  const [health, setHealth] = useState<ProviderHealth | null>(null);
  const [managedStatus, setManagedStatus] =
    useState<ManagedRuntimeStatus | null>(null);
  const [pdfCompiler, setPdfCompiler] = useState<PdfCompilerInfo>({
    available: false,
    binary_name: null,
  });

  // Pages & Conversion State
  const [pages, setPages] = useState<PageImageDto[]>([]);
  const [activePageIdx, setActivePageIdx] = useState(0);
  const [noteTitle, setNoteTitle] = useState("Handwritten Notes");
  const [zoom, setZoom] = useState(1);

  const [latexText, setLatexText] = useState("");
  const [warnings, setWarnings] = useState<string[]>([]);
  const [converting, setConverting] = useState(false);
  const [loadingSource, setLoadingSource] = useState(false);
  const [fileLoadProgress, setFileLoadProgress] =
    useState<FileLoadProgress | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [toastMsg, setToastMsg] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // OneNote Browser State
  const [oneNoteAuth, setOneNoteAuth] = useState<OneNoteAuthStatus | null>(
    null
  );
  const [notebooks, setNotebooks] = useState<Notebook[]>([]);
  const [selectedSectionId, setSelectedSectionId] = useState<string>("");
  const [oneNotePages, setOneNotePages] = useState<PageMeta[]>([]);

  useEffect(() => {
    localStorage.setItem("ln_provider_cfg", JSON.stringify(providerConfig));
  }, [providerConfig]);

  useEffect(() => {
    localStorage.setItem(
      "ln_conversion_opts",
      JSON.stringify(conversionOptions)
    );
  }, [conversionOptions]);

  useEffect(() => {
    localStorage.setItem("ln_onenote_client_id", oneNoteClientId);
    onenoteAuthStatus(oneNoteClientId)
      .then(setOneNoteAuth)
      .catch(() => {});
  }, [oneNoteClientId]);

  useEffect(() => {
    getPdfCompilerStatus()
      .then(setPdfCompiler)
      .catch(() => {});
    checkAiHealth(providerConfig)
      .then(setHealth)
      .catch(() => {});
    runtimeStatus()
      .then(setManagedStatus)
      .catch(() => {});
  }, []);

  // Support Ctrl+V / Cmd+V image paste directly from OneNote / Screenshot tool
  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      const items = e.clipboardData?.items;
      if (!items) return;
      for (const item of items) {
        if (item.type.startsWith("image/")) {
          const blob = item.getAsFile();
          if (!blob) continue;
          const reader = new FileReader();
          reader.onload = () => {
            const dataUrl = String(reader.result ?? "");
            const img = new Image();
            img.onload = () => {
              const pasted: PageImageDto = {
                page_index: 0,
                label: `Clipboard Note (${new Date().toLocaleTimeString()})`,
                mime_type: item.type || "image/png",
                width: img.naturalWidth,
                height: img.naturalHeight,
                data_url: dataUrl,
              };
              setPages((prev) => {
                const next = [...prev, { ...pasted, page_index: prev.length }];
                setActivePageIdx(next.length - 1);
                return next;
              });
              setNoteTitle("Clipboard Note");
              setToastMsg("Pasted image from clipboard.");
            };
            img.src = dataUrl;
          };
          reader.readAsDataURL(blob);
        }
      }
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, []);

  const showToast = (msg: string) => {
    setToastMsg(msg);
    setTimeout(() => setToastMsg(null), 4000);
  };

  const handleOpenLocalFiles = async () => {
    if (loadingSource) return;
    setErrorMsg(null);
    let unlisten: (() => void) | null = null;
    try {
      const paths = await pickLocalNoteFiles();
      if (paths.length === 0) return;

      const firstFileName = paths[0].split(/[/\\]/).pop() || "Handwritten Notes";
      setFileLoadProgress({
        file_name: firstFileName,
        file_index: 0,
        total_files: paths.length,
        current_page: 0,
        total_pages: 0,
        percent_complete: 0,
        percent_left: 100,
      });
      setLoadingSource(true);

      unlisten = await onFileLoadProgress((prog) => {
        setFileLoadProgress(prog);
      });

      const loaded = await loadLocalFiles(paths);
      setPages(loaded);
      setActivePageIdx(0);
      const firstName =
        firstFileName.replace(/\.[^.]+$/, "") || "Handwritten Notes";
      setNoteTitle(firstName);
      setWarnings([]);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      if (unlisten) unlisten();
      setLoadingSource(false);
      setFileLoadProgress(null);
    }
  };

  const handleLoadOneNoteNotebooks = async () => {
    setErrorMsg(null);
    setLoadingSource(true);
    try {
      const nbs = await onenoteListNotebooks(oneNoteClientId);
      setNotebooks(nbs);
      const firstSec = nbs.flatMap((n) => n.sections)[0];
      if (firstSec) {
        setSelectedSectionId(firstSec.id);
        const pgs = await onenoteListPages(oneNoteClientId, firstSec.id);
        setOneNotePages(pgs);
      }
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setLoadingSource(false);
    }
  };

  const handleSelectOneNoteSection = async (sectionId: string) => {
    setSelectedSectionId(sectionId);
    setErrorMsg(null);
    setLoadingSource(true);
    try {
      const pgs = await onenoteListPages(oneNoteClientId, sectionId);
      setOneNotePages(pgs);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setLoadingSource(false);
    }
  };

  const handleSelectOneNotePage = async (page: PageMeta) => {
    setErrorMsg(null);
    setLoadingSource(true);
    try {
      const imgs = await onenoteFetchPageImages(oneNoteClientId, page.id);
      setPages(imgs);
      setActivePageIdx(0);
      setNoteTitle(page.title || "OneNote Page");
      setWarnings([]);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setLoadingSource(false);
    }
  };

  const handleConvert = async (convertAll: boolean) => {
    if (pages.length === 0) return;
    setErrorMsg(null);
    setConverting(true);
    try {
      const targetPages = convertAll ? pages : [pages[activePageIdx]];
      const res = await convertPagesToLatex(
        targetPages,
        providerConfig,
        conversionOptions
      );
      setLatexText(res.latex_body);
      setWarnings(res.warnings);
      showToast(`Transcribed with ${res.model_used}`);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setConverting(false);
    }
  };

  const handleCopy = async () => {
    if (!latexText.trim()) return;
    await navigator.clipboard.writeText(latexText);
    setCopied(true);
    setTimeout(() => setCopied(false), 1800);
  };

  const handleExport = async (format: "tex" | "md" | "pdf") => {
    if (!latexText.trim()) return;
    setErrorMsg(null);
    try {
      const safeName =
        noteTitle.replace(/[^a-zA-Z0-9_\-\u0400-\u04FF ]+/g, "").trim() ||
        "notes";
      const path = await pickSavePath(safeName, format);
      if (!path) return;
      const msg = await exportNoteFile(path, format, latexText, noteTitle);
      showToast(msg);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    }
  };

  const activePage = pages[activePageIdx] ?? null;

  const aiReady = Boolean(health?.reachable || managedStatus?.server_running);
  const activeModelLabel =
    providerConfig.kind === "gemini"
      ? providerConfig.model
      : managedStatus?.active_model_id ?? providerConfig.model;

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <div className="brand-mark">TeX</div>
          <div className="brand-meta">
            <span className="brand-title">LaTeX Notes</span>
            <button
              className="ai-pill"
              onClick={() => setAiSetupOpen(true)}
              title="Manage Local AI engine & models"
            >
              <span className={`status-dot ${aiReady ? "ok" : "warn"}`} />
              <span>{aiReady ? activeModelLabel : "Setup Local AI"}</span>
            </button>
          </div>
        </div>

        <div className="topbar-actions">
          <button
            className="btn-secondary"
            onClick={handleOpenLocalFiles}
            disabled={loadingSource}
          >
            {loadingSource ? (
              <Loader2 size={14} className="spin" />
            ) : (
              <FolderOpen size={14} />
            )}
            {loadingSource && fileLoadProgress
              ? `${Math.round(fileLoadProgress.percent_left)}% left`
              : loadingSource
              ? "Loading…"
              : "Open"}
          </button>

          <button
            className="btn-primary"
            onClick={() => handleConvert(false)}
            disabled={!activePage || converting}
          >
            {converting ? (
              <Loader2 size={14} className="spin" />
            ) : (
              <Sparkles size={14} />
            )}
            {converting ? "Transcribing…" : "Convert"}
          </button>

          {pages.length > 1 && (
            <button
              className="btn-secondary"
              onClick={() => handleConvert(true)}
              disabled={converting}
            >
              All ({pages.length})
            </button>
          )}

          <div className="export-capsule" role="group" aria-label="Export">
            <button
              onClick={handleCopy}
              disabled={!latexText.trim()}
              title="Copy LaTeX"
            >
              {copied ? <Check size={13} /> : <Copy size={13} />}
              <span>{copied ? "Copied" : "Copy"}</span>
            </button>
            <button
              onClick={() => handleExport("tex")}
              disabled={!latexText.trim()}
              title="Download .tex"
            >
              <Download size={13} />
              <span>.tex</span>
            </button>
            <button
              onClick={() => handleExport("md")}
              disabled={!latexText.trim()}
              title="Download .md"
            >
              <span>.md</span>
            </button>
            <button
              onClick={() => handleExport("pdf")}
              disabled={!latexText.trim() || !pdfCompiler.available}
              title={
                pdfCompiler.available
                  ? `Compile PDF using ${pdfCompiler.binary_name}`
                  : "Install tectonic, xelatex, or pdflatex to enable direct PDF export"
              }
            >
              <span>.pdf</span>
            </button>
          </div>

          <button
            className="icon-btn topbar-icon"
            onClick={() => setAiSetupOpen(true)}
            title="1-Click Local AI Installer"
            aria-label="Local AI"
          >
            <Cpu size={16} />
          </button>

          <button
            className="icon-btn topbar-icon"
            onClick={() => setSettingsOpen(true)}
            title="Settings & Providers"
            aria-label="Settings"
          >
            <Settings size={16} />
          </button>
        </div>
      </header>

      {health && !aiReady && (
        <div className="banner info">
          <Cpu size={14} />
          <span>
            Local AI is not running. Download <strong>Qwen2.5-VL</strong> in one
            click or configure Ollama / Gemini in Settings.
          </span>
          <button
            className="btn-secondary compact-btn"
            onClick={() => setAiSetupOpen(true)}
          >
            Install Local AI
          </button>
        </div>
      )}

      {errorMsg && (
        <div className="banner error">
          <AlertTriangle size={15} />
          <span>{errorMsg}</span>
          <button className="icon-btn" onClick={() => setErrorMsg(null)}>
            ✕
          </button>
        </div>
      )}

      {warnings.length > 0 && (
        <div className="banner warn">
          <AlertTriangle size={15} />
          <span>{warnings.join(" ")}</span>
        </div>
      )}

      <div className="workspace">
        <aside className="sidebar">
          <div className="source-tabs">
            <button
              className={sourceTab === "local" ? "active" : ""}
              onClick={() => setSourceTab("local")}
            >
              <FileText size={14} /> Files & PDF
            </button>
            <button
              className={sourceTab === "onenote" ? "active" : ""}
              onClick={() => setSourceTab("onenote")}
            >
              <BookOpen size={14} /> OneNote
            </button>
          </div>

          {sourceTab === "local" ? (
            <div className="sidebar-body">
              <div
                className={`drop-card ${loadingSource ? "is-loading" : ""}`}
                onClick={loadingSource ? undefined : handleOpenLocalFiles}
              >
                {loadingSource && fileLoadProgress ? (
                  <>
                    <Loader2 size={20} className="spin" />
                    <div className="drop-title">
                      Loading… {Math.round(fileLoadProgress.percent_left)}% left
                    </div>
                    <div className="drop-sub">
                      {fileLoadProgress.total_pages > 0
                        ? `Page ${Math.min(
                            fileLoadProgress.total_pages,
                            fileLoadProgress.current_page + 1
                          )} of ${fileLoadProgress.total_pages}`
                        : fileLoadProgress.file_name}
                    </div>
                  </>
                ) : (
                  <>
                    <FolderOpen size={22} />
                    <div className="drop-title">Import PNG, JPG, WebP, or PDF</div>
                    <div className="drop-sub">
                      Export from OneNote as PDF, or press Cmd/Ctrl+V to paste a
                      screenshot
                    </div>
                  </>
                )}
              </div>

              {pages.length > 0 && (
                <div className="page-list">
                  <div className="list-heading">Loaded Pages ({pages.length})</div>
                  {pages.map((p, idx) => (
                    <button
                      key={`${p.label}-${idx}`}
                      className={`page-item ${
                        idx === activePageIdx ? "active" : ""
                      }`}
                      onClick={() => setActivePageIdx(idx)}
                    >
                      <img src={p.data_url} alt={p.label} />
                      <div className="page-item-meta">
                        <span className="page-item-title">{p.label}</span>
                        <span className="page-item-dims">
                          {p.width}×{p.height}
                        </span>
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </div>
          ) : (
            <div className="sidebar-body">
              {!oneNoteAuth?.connected ? (
                <div className="onenote-empty">
                  <p>
                    Connect your Microsoft account in{" "}
                    <strong>Settings → Microsoft OneNote</strong> to browse
                    notebooks and render handwritten InkML pages.
                  </p>
                  <button
                    className="btn-primary"
                    onClick={() => setSettingsOpen(true)}
                  >
                    Configure OneNote
                  </button>
                </div>
              ) : (
                <div className="onenote-browser">
                  <button
                    className="btn-secondary full-width"
                    onClick={handleLoadOneNoteNotebooks}
                    disabled={loadingSource}
                  >
                    {loadingSource ? "Loading..." : "Refresh Notebooks"}
                  </button>

                  {notebooks.length > 0 && (
                    <label className="sidebar-select">
                      <span>Section</span>
                      <select
                        value={selectedSectionId}
                        onChange={(e) =>
                          handleSelectOneNoteSection(e.target.value)
                        }
                      >
                        {notebooks.map((nb) => (
                          <optgroup key={nb.id} label={nb.name}>
                            {nb.sections.map((sec) => (
                              <option key={sec.id} value={sec.id}>
                                {sec.name}
                              </option>
                            ))}
                          </optgroup>
                        ))}
                      </select>
                    </label>
                  )}

                  <div className="page-list">
                    {oneNotePages.map((pg) => (
                      <button
                        key={pg.id}
                        className="onenote-page-btn"
                        onClick={() => handleSelectOneNotePage(pg)}
                      >
                        <span className="page-item-title">{pg.title}</span>
                        {pg.last_modified && (
                          <span className="page-item-dims">
                            {new Date(pg.last_modified).toLocaleDateString()}
                          </span>
                        )}
                      </button>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </aside>

        <main className="split-main">
          <section className="pane image-pane">
            <div className="pane-header">
              <span className="pane-title">
                {activePage ? activePage.label : "Handwritten Note"}
              </span>
              <div className="pane-controls">
                {pages.length > 1 && (
                  <>
                    <button
                      className="icon-btn"
                      disabled={activePageIdx === 0}
                      onClick={() => setActivePageIdx((i) => Math.max(0, i - 1))}
                    >
                      <ChevronLeft size={15} />
                    </button>
                    <span className="pager-label">
                      {activePageIdx + 1} / {pages.length}
                    </span>
                    <button
                      className="icon-btn"
                      disabled={activePageIdx >= pages.length - 1}
                      onClick={() =>
                        setActivePageIdx((i) =>
                          Math.min(pages.length - 1, i + 1)
                        )
                      }
                    >
                      <ChevronRight size={15} />
                    </button>
                  </>
                )}
                <button
                  className="icon-btn"
                  onClick={() => setZoom((z) => Math.max(0.4, +(z - 0.15).toFixed(2)))}
                >
                  <ZoomOut size={15} />
                </button>
                <span className="pager-label">{Math.round(zoom * 100)}%</span>
                <button
                  className="icon-btn"
                  onClick={() => setZoom((z) => Math.min(3, +(z + 0.15).toFixed(2)))}
                >
                  <ZoomIn size={15} />
                </button>
              </div>
            </div>
            <div className="image-stage">
              {loadingSource && (
                <div className="image-stage-overlay">
                  <CircularProgressSpinner progress={fileLoadProgress} />
                </div>
              )}
              {activePage ? (
                <img
                  src={activePage.data_url}
                  alt={activePage.label}
                  style={{ width: `${zoom * 100}%` }}
                />
              ) : (
                !loadingSource && (
                  <div className="preview-empty">
                    Import a handwritten note (PNG/JPG/PDF), paste a screenshot, or
                    pick a OneNote page to begin.
                  </div>
                )
              )}
            </div>
          </section>

          <section className="pane output-pane">
            <div className="pane-header">
              <span className="pane-title">LaTeX Output</span>
              <div className="segmented compact">
                <button
                  className={rightMode === "split" ? "active" : ""}
                  onClick={() => setRightMode("split")}
                >
                  Split
                </button>
                <button
                  className={rightMode === "preview" ? "active" : ""}
                  onClick={() => setRightMode("preview")}
                >
                  Preview
                </button>
                <button
                  className={rightMode === "code" ? "active" : ""}
                  onClick={() => setRightMode("code")}
                >
                  Code
                </button>
              </div>
            </div>

            <div className={`output-body mode-${rightMode}`}>
              {rightMode !== "preview" && (
                <div className="editor-pane">
                  <LatexEditor value={latexText} onChange={setLatexText} />
                </div>
              )}
              {rightMode !== "code" && (
                <div className="preview-pane">
                  <LatexPreview source={latexText} />
                </div>
              )}
            </div>
          </section>
        </main>
      </div>

      {toastMsg && <div className="floating-toast">{toastMsg}</div>}

      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        providerConfig={providerConfig}
        onChangeProviderConfig={setProviderConfig}
        conversionOptions={conversionOptions}
        onChangeConversionOptions={setConversionOptions}
        oneNoteClientId={oneNoteClientId}
        onChangeOneNoteClientId={setOneNoteClientId}
        oneNoteAuth={oneNoteAuth}
        onUpdateOneNoteAuth={setOneNoteAuth}
        onHealthUpdated={setHealth}
      />

      <LocalAiSetupModal
        open={aiSetupOpen}
        onClose={() => setAiSetupOpen(false)}
        onStatusChanged={setManagedStatus}
        onServerReady={(baseUrl, modelId) => {
          const nextCfg: ProviderConfig = {
            kind: "open_ai_compatible",
            base_url: baseUrl,
            model: modelId,
            api_key: null,
          };
          setProviderConfig(nextCfg);
          checkAiHealth(nextCfg)
            .then(setHealth)
            .catch(() => {});
          showToast(`Local AI (${modelId}) is running and ready!`);
        }}
      />
    </div>
  );
};
