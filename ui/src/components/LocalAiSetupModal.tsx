import React, { useEffect, useState } from "react";
import {
  CheckCircle2,
  Cpu,
  Download,
  Loader2,
  Play,
  Square,
  Trash2,
  X,
} from "lucide-react";
import {
  onAiDownloadProgress,
  runtimeDeleteModel,
  runtimeInstallAndStart,
  runtimeStart,
  runtimeStatus,
  runtimeStop,
} from "../api";
import type { DownloadProgress, ManagedRuntimeStatus } from "../types";

interface LocalAiSetupModalProps {
  open: boolean;
  onClose: () => void;
  onServerReady: (baseUrl: string, modelId: string) => void;
  onStatusChanged: (status: ManagedRuntimeStatus) => void;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export const LocalAiSetupModal: React.FC<LocalAiSetupModalProps> = ({
  open,
  onClose,
  onServerReady,
  onStatusChanged,
}) => {
  const [status, setStatus] = useState<ManagedRuntimeStatus | null>(null);
  const [busyModelId, setBusyModelId] = useState<string | null>(null);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const s = await runtimeStatus();
      setStatus(s);
      onStatusChanged(s);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    }
  };

  useEffect(() => {
    if (!open) return;
    refresh();
    let unlisten: (() => void) | undefined;
    onAiDownloadProgress((prog) => setProgress(prog)).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [open]);

  if (!open) return null;

  const handleInstallAndStart = async (modelId: string) => {
    setBusyModelId(modelId);
    setErrorMsg(null);
    setProgress({
      stage: "Preparing download",
      file_name: modelId,
      downloaded_bytes: 0,
      total_bytes: null,
      percent: null,
    });
    try {
      const next = await runtimeInstallAndStart(modelId);
      setStatus(next);
      onStatusChanged(next);
      if (next.server_base_url && next.active_model_id) {
        onServerReady(next.server_base_url, next.active_model_id);
      }
      setProgress(null);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyModelId(null);
    }
  };

  const handleStartOnly = async (modelId: string) => {
    setBusyModelId(modelId);
    setErrorMsg(null);
    try {
      const next = await runtimeStart(modelId);
      setStatus(next);
      onStatusChanged(next);
      if (next.server_base_url && next.active_model_id) {
        onServerReady(next.server_base_url, next.active_model_id);
      }
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyModelId(null);
    }
  };

  const handleStop = async () => {
    setBusyModelId("__stop__");
    setErrorMsg(null);
    try {
      const next = await runtimeStop();
      setStatus(next);
      onStatusChanged(next);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyModelId(null);
    }
  };

  const handleDelete = async (modelId: string) => {
    setBusyModelId(modelId);
    setErrorMsg(null);
    try {
      const next = await runtimeDeleteModel(modelId);
      setStatus(next);
      onStatusChanged(next);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyModelId(null);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-card wide" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <div className="modal-title-row">
            <Cpu size={18} />
            <h2>1-Click Local AI Installer</h2>
          </div>
          <button className="icon-btn" onClick={onClose} aria-label="Close">
            <X size={18} />
          </button>
        </div>

        <div className="modal-body">
          <p className="hint-text">
            No terminal or external Ollama/Python setup required. Click{" "}
            <strong>Install &amp; Start</strong> on a model below to
            automatically download the cross-platform <code>llama.cpp</code>{" "}
            engine + Vision AI weights directly into your app folder. Downloads
            resume automatically if interrupted.
          </p>

          {status && (
            <div className="runtime-summary-bar">
              <div>
                <strong>Engine:</strong>{" "}
                {status.engine_installed ? (
                  <span className="text-ok">Installed</span>
                ) : (
                  <span className="text-muted">Will auto-download (~30–60 MB)</span>
                )}
              </div>
              <div>
                <strong>Server:</strong>{" "}
                {status.server_running ? (
                  <span className="text-ok">
                    Running on port {status.server_port} (
                    {status.active_model_id})
                  </span>
                ) : (
                  <span className="text-muted">Stopped</span>
                )}
              </div>
              {status.server_running && (
                <button
                  className="btn-secondary"
                  onClick={handleStop}
                  disabled={busyModelId !== null}
                >
                  <Square size={13} /> Stop Server
                </button>
              )}
            </div>
          )}

          {progress && (
            <div className="download-progress-card">
              <div className="progress-top">
                <span>
                  <strong>{progress.stage}:</strong> {progress.file_name}
                </span>
                <span>
                  {progress.percent !== null && progress.percent !== undefined
                    ? `${progress.percent.toFixed(1)}%`
                    : "Working..."}
                  {progress.total_bytes
                    ? ` · ${formatBytes(progress.downloaded_bytes)} / ${formatBytes(
                        progress.total_bytes
                      )}`
                    : progress.downloaded_bytes > 0
                    ? ` · ${formatBytes(progress.downloaded_bytes)}`
                    : ""}
                </span>
              </div>
              <div className="progress-track">
                <div
                  className={`progress-fill ${
                    progress.percent == null ? "indeterminate" : ""
                  }`}
                  style={{
                    width:
                      progress.percent != null ? `${progress.percent}%` : "45%",
                  }}
                />
              </div>
            </div>
          )}

          {errorMsg && <div className="banner error">{errorMsg}</div>}

          <div className="model-catalog-list">
            {status?.models.map((m) => {
              const isActive =
                status.server_running && status.active_model_id === m.id;
              const isBusy = busyModelId === m.id;

              return (
                <div
                  key={m.id}
                  className={`catalog-card ${isActive ? "active" : ""}`}
                >
                  <div className="catalog-info">
                    <div className="catalog-title-row">
                      <span className="catalog-title">{m.name}</span>
                      {m.recommended && (
                        <span className="badge badge-rec">Recommended</span>
                      )}
                      {isActive && (
                        <span className="badge badge-live">
                          <CheckCircle2 size={11} /> Active
                        </span>
                      )}
                    </div>
                    <p className="catalog-desc">{m.description}</p>
                    <div className="catalog-meta">
                      <span>Size: ~{(m.approx_size_mb / 1024).toFixed(1)} GB</span>
                      {m.downloaded && (
                        <span>On disk: {formatBytes(m.disk_bytes)}</span>
                      )}
                    </div>
                  </div>

                  <div className="catalog-actions">
                    {!m.downloaded || !status.engine_installed ? (
                      <button
                        className="btn-primary"
                        onClick={() => handleInstallAndStart(m.id)}
                        disabled={busyModelId !== null}
                      >
                        {isBusy ? (
                          <Loader2 size={14} className="spin" />
                        ) : (
                          <Download size={14} />
                        )}
                        {isBusy ? "Installing..." : "Install & Start"}
                      </button>
                    ) : (
                      <>
                        {!isActive && (
                          <button
                            className="btn-primary"
                            onClick={() => handleStartOnly(m.id)}
                            disabled={busyModelId !== null}
                          >
                            {isBusy ? (
                              <Loader2 size={14} className="spin" />
                            ) : (
                              <Play size={14} />
                            )}
                            {isBusy ? "Starting..." : "Start"}
                          </button>
                        )}
                        <button
                          className="btn-secondary"
                          onClick={() => handleDelete(m.id)}
                          disabled={busyModelId !== null}
                          title="Delete model files to free disk space"
                        >
                          <Trash2 size={14} />
                        </button>
                      </>
                    )}
                  </div>
                </div>
              );
            })}
          </div>

          {status && (
            <p className="hint-text">
              Storage folder: <code>{status.data_dir}</code>
            </p>
          )}
        </div>
      </div>
    </div>
  );
};
