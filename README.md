<div align="center">

# LaTeX Notes

**Transform handwritten notes from Microsoft OneNote, PDFs, and screenshots into live-previewed, export-ready LaTeX — powered by local Vision AI or cloud APIs.**

[![Release](https://img.shields.io/github/v/release/abbakkio/LaTeX-notes?style=flat-square&color=111111&label=Release)](https://github.com/abbakkio/LaTeX-notes/releases/latest)
[![Build Status](https://img.shields.io/github/actions/workflow/status/abbakkio/LaTeX-notes/release.yml?style=flat-square&label=Builds)](https://github.com/abbakkio/LaTeX-notes/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-111111?style=flat-square)](LICENSE)
[![Built with Rust & Tauri](https://img.shields.io/badge/Rust%20%2B%20Tauri%20v2-000000?style=flat-square)](https://tauri.app)

[Download Latest Release](https://github.com/abbakkio/LaTeX-notes/releases/latest) · [User & Setup Guide](docs/USER_GUIDE.md) · [OneNote Integration](#microsoft-onenote-setup) · [CLI Usage](#standalone-cli-manual-deployment)

</div>

---

## Downloads

Pre-built installers for **macOS** and **Windows** (plus Linux and standalone CLI binaries) are published on the [**Releases**](https://github.com/abbakkio/LaTeX-notes/releases/latest) page:

| Platform | Architecture | Installer / Package | Notes |
| --- | --- | --- | --- |
| **macOS** | Apple Silicon (`M1/M2/M3/M4`) | `LaTeX.Notes_*_aarch64.dmg` | Metal GPU acceleration supported out of the box |
| **macOS** | Intel (`x86_64`) | `LaTeX.Notes_*_x64.dmg` | Native macOS bundle |
| **Windows** | `x86_64` (64-bit) | `LaTeX.Notes_*_x64-setup.exe` / `.msi` | Vulkan & CPU inference supported |
| **Linux** | `x86_64` | `.AppImage` / `.deb` | Requires WebKitGTK 4.1 |
| **Manual / CLI** | macOS, Windows, Linux | `latex-notes-cli-*.tar.gz` / `.zip` | Headless `latex-notes` binary |

> [!NOTE]
> **First launch on macOS / Windows (unsigned community builds):**
> - **macOS**: If Gatekeeper warns that the app is from an unidentified developer, right-click **LaTeX Notes.app** → **Open**, or run:
>   `xattr -cr "/Applications/LaTeX Notes.app"`
> - **Windows**: In SmartScreen, click **More info** → **Run anyway**.

---

## Highlights

- **1-Click Built-In Local AI**: No terminal or manual Ollama setup needed. Open **Setup Local AI** inside the app to download `llama-server` (`Metal` on macOS, `Vulkan`/`CPU` on Windows & Linux) and `Qwen2.5-VL` vision models (`3B` fast or `7B` accurate) with resumable progress.
- **Minimalist Pure-Black Workspace**: Designed around high-contrast typography, quiet hairline surfaces, side-by-side handwritten page viewer, **CodeMirror** LaTeX editor, and real-time **KaTeX** math preview.
- **Multi-Source Input (`NoteSource` trait)**:
  - **Local Files & PDFs**: Import `.png`, `.jpg`, `.webp`, or multi-page `.pdf` files rendered locally in pure Rust ([`hayro`](https://crates.io/crates/hayro)) with an animated **percentage-left spinner**.
  - **Clipboard Paste**: Press `Cmd+V` / `Ctrl+V` anywhere to paste screenshots directly from OneNote or your OS snipping tool.
  - **Microsoft OneNote**: OAuth2 PKCE login + Graph API notebook/section/page browser + built-in **InkML stroke rasterizer** (`quick-xml` + `tiny-skia`) that turns vector handwriting strokes into crisp PNGs.
- **Flexible AI Backends (`AiProvider` trait)**: Switch effortlessly between the built-in local `llama.cpp` runtime, **Ollama**, **LM Studio**, **Google AI Studio (Gemini)**, or any OpenAI-compatible vision endpoint. API keys are stored securely in your OS Keyring.
- **Watch & Export**: Copy LaTeX to clipboard or download as `.tex`, `.md`, or compiled `.pdf` (when `tectonic`, `xelatex`, or `pdflatex` is available).

---

## Quick Start (End Users)

1. Download and install **LaTeX Notes** for your OS from [**Releases**](https://github.com/abbakkio/LaTeX-notes/releases/latest).
2. Click **`Setup Local AI`** in the top-left header to install the local vision engine + `Qwen2.5-VL` in one click — **or** open **Settings (`⚙`)** to connect **Ollama**, **LM Studio**, or **Google AI Studio (Gemini)**.
3. Import a handwritten `.pdf`, image, or paste a screenshot (`Cmd/Ctrl+V`), then click **`Convert`**.
4. Inspect and tweak the LaTeX in the split **KaTeX Preview + Editor** pane, then download `.tex`, `.md`, or `.pdf`.

See [docs/USER_GUIDE.md](docs/USER_GUIDE.md) for detailed instructions, hardware recommendations, and troubleshooting.

---

## Microsoft OneNote Setup

To browse your Microsoft OneNote notebooks directly inside the app:

1. Open [Microsoft Entra Admin Center](https://entra.microsoft.com) → **App registrations** → **New registration**.
2. Select **Personal Microsoft accounts only** (or Multitenant + Personal).
3. Under **Authentication → Platform configurations**, add **Mobile and desktop applications** with redirect URI: `http://localhost`.
4. Under **API permissions**, add Microsoft Graph **Delegated permission**: `Notes.Read`.
5. Copy the **Application (client) ID**, paste it into **Settings → Microsoft OneNote** inside LaTeX Notes, and click **Sign In to OneNote**.

---

## Standalone CLI (Manual Deployment)

You can run conversions from the terminal, servers, or scripts using the standalone `latex-notes` binary:

```bash
# List curated local vision models
latex-notes models

# 1-Click download of llama-server + Qwen2.5-VL-7B (or 3B)
latex-notes setup --model qwen2.5-vl-7b-q4

# Verify provider connectivity
latex-notes health

# Convert a PDF or image to LaTeX and Markdown
latex-notes convert ./lecture1.pdf --out-tex lecture1.tex --out-md lecture1.md
```

Use **Google AI Studio (Gemini)** from the CLI:

```bash
GEMINI_API_KEY="your-api-key" latex-notes convert ./note.png \
  --provider gemini \
  --model gemini-2.5-flash \
  --out-tex note.tex
```

---

## Architecture

```text
LaTeX-notes/
├── crates/
│   ├── core/          # Shared Rust library: NoteSource, AiProvider, InkML & PDF (hayro) prep,
│   │                  # 1-Click llama.cpp runtime manager, KaTeX-safe cleanup, keyring secrets
│   └── cli/           # Headless cross-platform `latex-notes` CLI binary
├── src-tauri/         # Tauri v2 desktop shell, IPC commands, progress events, OS file dialogs
├── ui/                # React + TypeScript + Vite + CodeMirror + KaTeX frontend
└── .github/workflows/ # Automated cross-platform builds (macOS DMG, Windows MSI/EXE, Linux, CLI)
```

---

## Build From Source

### Prerequisites
- [Rust toolchain](https://rustup.rs/) (`stable`)
- [Node.js](https://nodejs.org/) `20+`
- Linux only: `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`

### Run in Development Mode
```bash
cd ui && npm install && cd ..
npm --prefix ui exec tauri -- dev
```

### Run Tests & Build Desktop Installers
```bash
npm --prefix ui run build
cargo test --workspace
npm --prefix ui exec tauri -- build
```

---

## License

Released under the [MIT License](LICENSE).
