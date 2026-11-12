import React, { useEffect, useState } from "react";
import { CheckCircle2, KeyRound, RefreshCw, X, AlertCircle } from "lucide-react";
import {
  checkAiHealth,
  deleteSecretKey,
  hasSecretKey,
  onenoteDisconnect,
  onenoteLogin,
  saveSecretKey,
} from "../api";
import type {
  ConversionOptions,
  OneNoteAuthStatus,
  OutputMode,
  ProviderConfig,
  ProviderHealth,
  ProviderKind,
} from "../types";

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
  providerConfig: ProviderConfig;
  onChangeProviderConfig: (cfg: ProviderConfig) => void;
  conversionOptions: ConversionOptions;
  onChangeConversionOptions: (opts: ConversionOptions) => void;
  oneNoteClientId: string;
  onChangeOneNoteClientId: (id: string) => void;
  oneNoteAuth: OneNoteAuthStatus | null;
  onUpdateOneNoteAuth: (status: OneNoteAuthStatus) => void;
  onHealthUpdated: (health: ProviderHealth) => void;
}

const LOCAL_PRESETS = [
  {
    label: "Ollama",
    baseUrl: "http://localhost:11434/v1",
    model: "qwen2.5vl:7b",
  },
  {
    label: "LM Studio",
    baseUrl: "http://localhost:1234/v1",
    model: "qwen2.5-vl-7b-instruct",
  },
  {
    label: "llama-server",
    baseUrl: "http://localhost:8080/v1",
    model: "qwen2.5-vl-7b-instruct",
  },
];

export const SettingsModal: React.FC<SettingsModalProps> = ({
  open,
  onClose,
  providerConfig,
  onChangeProviderConfig,
  conversionOptions,
  onChangeConversionOptions,
  oneNoteClientId,
  onChangeOneNoteClientId,
  oneNoteAuth,
  onUpdateOneNoteAuth,
  onHealthUpdated,
}) => {
  const [geminiKeyInput, setGeminiKeyInput] = useState("");
  const [hasSavedGeminiKey, setHasSavedGeminiKey] = useState(false);
  const [health, setHealth] = useState<ProviderHealth | null>(null);
  const [testing, setTesting] = useState(false);
  const [authBusy, setAuthBusy] = useState(false);
  const [statusMsg, setStatusMsg] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    hasSecretKey("gemini_api_key")
      .then(setHasSavedGeminiKey)
      .catch(() => setHasSavedGeminiKey(false));
  }, [open]);

  if (!open) return null;

  const handleKindChange = (kind: ProviderKind) => {
    if (kind === "open_ai_compatible") {
      onChangeProviderConfig({
        kind,
        base_url: providerConfig.base_url || "http://localhost:11434/v1",
        model:
          providerConfig.kind === "gemini"
            ? "qwen2.5vl:7b"
            : providerConfig.model,
        api_key: null,
      });
    } else {
      onChangeProviderConfig({
        kind,
        base_url: "",
        model:
          providerConfig.kind === "open_ai_compatible"
            ? "gemini-2.5-flash"
            : providerConfig.model,
        api_key: null,
      });
    }
    setHealth(null);
  };

  const handleTestHealth = async () => {
    setTesting(true);
    setStatusMsg(null);
    try {
      const res = await checkAiHealth(providerConfig);
      setHealth(res);
      onHealthUpdated(res);
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setTesting(false);
    }
  };

  const handleSaveGeminiKey = async () => {
    try {
      await saveSecretKey("gemini_api_key", geminiKeyInput);
      setHasSavedGeminiKey(geminiKeyInput.trim().length > 0);
      setGeminiKeyInput("");
      setStatusMsg("Gemini API key saved securely in OS Keyring.");
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : String(err));
    }
  };

  const handleClearGeminiKey = async () => {
    try {
      await deleteSecretKey("gemini_api_key");
      setHasSavedGeminiKey(false);
      setStatusMsg("Removed Gemini API key from OS Keyring.");
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : String(err));
    }
  };

  const handleOneNoteLogin = async () => {
    setAuthBusy(true);
    setStatusMsg(null);
    try {
      const status = await onenoteLogin(oneNoteClientId);
      onUpdateOneNoteAuth(status);
      setStatusMsg(status.message);
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setAuthBusy(false);
    }
  };

  const handleOneNoteDisconnect = async () => {
    setAuthBusy(true);
    try {
      const status = await onenoteDisconnect(oneNoteClientId);
      onUpdateOneNoteAuth(status);
      setStatusMsg("Disconnected OneNote account.");
    } catch (err) {
      setStatusMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setAuthBusy(false);
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-card" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>Settings & Providers</h2>
          <button className="icon-btn" onClick={onClose} aria-label="Close">
            <X size={18} />
          </button>
        </div>

        <div className="modal-body">
          <section className="settings-section">
            <h3>AI Backend</h3>
            <div className="segmented">
              <button
                className={
                  providerConfig.kind === "open_ai_compatible" ? "active" : ""
                }
                onClick={() => handleKindChange("open_ai_compatible")}
              >
                Local AI (Ollama / LM Studio / llama.cpp)
              </button>
              <button
                className={providerConfig.kind === "gemini" ? "active" : ""}
                onClick={() => handleKindChange("gemini")}
              >
                Google AI Studio (Gemini)
              </button>
            </div>

            {providerConfig.kind === "open_ai_compatible" ? (
              <div className="form-grid">
                <div className="preset-row">
                  <span>Presets:</span>
                  {LOCAL_PRESETS.map((p) => (
                    <button
                      key={p.label}
                      className="chip-btn"
                      onClick={() =>
                        onChangeProviderConfig({
                          ...providerConfig,
                          base_url: p.baseUrl,
                          model: p.model,
                        })
                      }
                    >
                      {p.label}
                    </button>
                  ))}
                </div>
                <label>
                  <span>Base URL (OpenAI-compatible `/v1`)</span>
                  <input
                    type="text"
                    value={providerConfig.base_url}
                    onChange={(e) =>
                      onChangeProviderConfig({
                        ...providerConfig,
                        base_url: e.target.value,
                      })
                    }
                    placeholder="http://localhost:11434/v1"
                  />
                </label>
                <label>
                  <span>Vision Model Name</span>
                  <input
                    type="text"
                    value={providerConfig.model}
                    onChange={(e) =>
                      onChangeProviderConfig({
                        ...providerConfig,
                        model: e.target.value,
                      })
                    }
                    placeholder="qwen2.5vl:7b"
                  />
                </label>
              </div>
            ) : (
              <div className="form-grid">
                <label>
                  <span>Gemini Model</span>
                  <input
                    type="text"
                    value={providerConfig.model}
                    onChange={(e) =>
                      onChangeProviderConfig({
                        ...providerConfig,
                        model: e.target.value,
                      })
                    }
                    placeholder="gemini-2.5-flash"
                  />
                </label>
                <label>
                  <span>
                    Google AI Studio API Key{" "}
                    {hasSavedGeminiKey ? "(Saved in OS Keyring)" : ""}
                  </span>
                  <div className="inline-input-row">
                    <input
                      type="password"
                      value={geminiKeyInput}
                      onChange={(e) => setGeminiKeyInput(e.target.value)}
                      placeholder={
                        hasSavedGeminiKey
                          ? "•••••••••••••••• (stored securely)"
                          : "Paste AIza... API key"
                      }
                    />
                    <button
                      className="btn-secondary"
                      onClick={handleSaveGeminiKey}
                      disabled={!geminiKeyInput.trim()}
                    >
                      <KeyRound size={14} /> Save Key
                    </button>
                    {hasSavedGeminiKey && (
                      <button
                        className="btn-secondary"
                        onClick={handleClearGeminiKey}
                      >
                        Clear
                      </button>
                    )}
                  </div>
                </label>
              </div>
            )}

            <div className="health-row">
              <button
                className="btn-secondary"
                onClick={handleTestHealth}
                disabled={testing}
              >
                <RefreshCw size={14} className={testing ? "spin" : ""} />
                {testing ? "Checking..." : "Test Connection & Fetch Models"}
              </button>

              {health && (
                <div
                  className={`health-badge ${
                    health.reachable ? "ok" : "err"
                  }`}
                >
                  {health.reachable ? (
                    <CheckCircle2 size={14} />
                  ) : (
                    <AlertCircle size={14} />
                  )}
                  <span>{health.message}</span>
                </div>
              )}
            </div>

            {health && health.available_models.length > 0 && (
              <div className="model-pills">
                {health.available_models.slice(0, 12).map((m) => (
                  <button
                    key={m}
                    className={`chip-btn ${
                      providerConfig.model === m ? "selected" : ""
                    }`}
                    onClick={() =>
                      onChangeProviderConfig({ ...providerConfig, model: m })
                    }
                  >
                    {m}
                  </button>
                ))}
              </div>
            )}
          </section>

          <section className="settings-section">
            <h3>Transcription & LaTeX Preferences</h3>
            <div className="form-grid">
              <label>
                <span>Expected Handwriting Languages</span>
                <input
                  type="text"
                  value={conversionOptions.language_hint}
                  onChange={(e) =>
                    onChangeConversionOptions({
                      ...conversionOptions,
                      language_hint: e.target.value,
                    })
                  }
                />
              </label>
              <label>
                <span>Output Mode</span>
                <select
                  value={conversionOptions.output_mode}
                  onChange={(e) =>
                    onChangeConversionOptions({
                      ...conversionOptions,
                      output_mode: e.target.value as OutputMode,
                    })
                  }
                >
                  <option value="document_body">
                    Document Body (recommended for live preview & export)
                  </option>
                  <option value="math_only">Math Expressions Only</option>
                  <option value="standalone_document">
                    Full Standalone .tex Preamble
                  </option>
                </select>
              </label>
              <label>
                <span>Extra Prompt Instructions (Optional)</span>
                <input
                  type="text"
                  value={conversionOptions.extra_instructions ?? ""}
                  onChange={(e) =>
                    onChangeConversionOptions({
                      ...conversionOptions,
                      extra_instructions: e.target.value || null,
                    })
                  }
                  placeholder="e.g. Use align* for multi-step derivations"
                />
              </label>
            </div>
          </section>

          <section className="settings-section">
            <h3>Microsoft OneNote (Graph API OAuth2)</h3>
            <p className="hint-text">
              Register a Public Client (Desktop/Mobile) app in Microsoft Entra
              with <code>http://localhost</code> redirect URI and{" "}
              <code>Notes.Read</code> permission.
            </p>
            <div className="form-grid">
              <label>
                <span>Azure Application (Client) ID</span>
                <input
                  type="text"
                  value={oneNoteClientId}
                  onChange={(e) => onChangeOneNoteClientId(e.target.value)}
                  placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
                />
              </label>
              <div className="health-row">
                {oneNoteAuth?.connected ? (
                  <button
                    className="btn-secondary"
                    onClick={handleOneNoteDisconnect}
                    disabled={authBusy}
                  >
                    Disconnect OneNote
                  </button>
                ) : (
                  <button
                    className="btn-primary"
                    onClick={handleOneNoteLogin}
                    disabled={authBusy || !oneNoteClientId.trim()}
                  >
                    {authBusy ? "Waiting for Browser..." : "Sign In to OneNote"}
                  </button>
                )}
                {oneNoteAuth && (
                  <span className="hint-text">{oneNoteAuth.message}</span>
                )}
              </div>
            </div>
          </section>

          {statusMsg && <div className="status-toast">{statusMsg}</div>}
        </div>
      </div>
    </div>
  );
};
