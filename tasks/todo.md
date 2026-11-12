# Prototype Implementation Checklist

## Phase 1: Workspace Foundation & Local VLM Adapter
- [x] Initialize Cargo workspace (`crates/core`, `crates/cli`, `src-tauri`) and React + TS + Vite `ui/`
- [x] Define core traits & models (`NoteSource`, `AiProvider`, `PageImage`, `LatexResult`, `ConversionOptions`)
- [x] Implement `LocalOpenAiProvider` (Ollama / LM Studio / `llama-server` vision API + health check + LaTeX fence sanitizer)

## Phase 2: Local Files/PDF Import, Live Preview & Export
- [x] Implement `LocalFileSource` (PNG/JPG/WebP + multi-page PDF rasterization via `hayro` + smart image resizing)
- [x] Build split-pane UI: Note Image viewer | Editable `.tex` (CodeMirror) | Live KaTeX math & document preview
- [x] Implement `.tex`, `.md`, and optional PDF export (`tectonic` / `pdflatex` auto-detection)

## Phase 3: Native OneNote Integration (Microsoft Graph API + InkML)
- [x] Implement Microsoft OAuth2 PKCE flow + OS Keyring token persistence (`keyring`)
- [x] Implement `OneNoteGraphSource` (notebooks, sections, pages tree)
- [x] Implement InkML `<trace>` parser (`quick-xml`) & stroke rasterizer (`tiny-skia`) onto canvas PNG

## Phase 4: Extensibility, CLI & GitHub Releases
- [x] Implement `GeminiProvider` (Google AI Studio `generateContent` vision adapter)
- [x] Implement `crates/cli` standalone binary for manual/headless deployment
- [x] Add `.github/workflows/release.yml` for macOS, Windows, and manual/CLI release builds
