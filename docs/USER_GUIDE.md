# LaTeX Notes — User & Deployment Guide

## 1. Choosing Your AI Mode

LaTeX Notes works completely offline by default, and also supports cloud APIs when you need speed on lightweight hardware.

| Mode | Best For | Hardware / Requirements |
| --- | --- | --- |
| **Built-in Local AI (`Qwen2.5-VL-3B`)** | Fast offline notes on laptops | ~3.2 GB disk, ~6 GB RAM / VRAM |
| **Built-in Local AI (`Qwen2.5-VL-7B`)** | Dense math, mixed English + Cyrillic handwriting | ~5.8 GB disk, ~8–10 GB Unified RAM / VRAM |
| **External Local (`Ollama` / `LM Studio`)** | Custom GGUF or already-installed models | Running local OpenAI-compatible endpoint |
| **Google AI Studio (`Gemini`)** | Maximum accuracy on low-spec machines | Free/paid API key stored in OS Keyring |

### Hardware Acceleration
- **macOS (Apple Silicon)**: Automatically uses Apple **Metal** GPU acceleration.
- **Windows / Linux**: Uses **Vulkan** / CPU acceleration automatically via the managed `llama.cpp` runtime.

---

## 2. Importing Handwritten Notes

1. **Multi-Page PDFs**: Click **Open** (or the sidebar drop card) and select a `.pdf`. A circular progress indicator displays the exact remaining percentage (`% left`) as pages are rasterized locally in Rust.
2. **Images**: Import `.png`, `.jpg`, `.jpeg`, or `.webp` directly.
3. **Clipboard Screenshots**: Copy any region in OneNote, Preview, or Snipping Tool and press `Cmd+V` (`Ctrl+V` on Windows/Linux) inside LaTeX Notes.
4. **Microsoft OneNote**: Connect your personal Microsoft account in **Settings → Microsoft OneNote** to list notebooks, sections, and pages, including native InkML stroke rendering.

---

## 3. Exporting Notes

- **Copy**: Copies raw LaTeX body to the clipboard.
- **`.tex`**: Downloads the LaTeX document (configurable in **Settings → Output Mode**: *Document Body*, *Standalone Document*, or *Math Only*).
- **`.md`**: Downloads Markdown with `$ ... $` / `$$ ... $$` math delimiters.
- **`.pdf`**: Compiles the generated LaTeX directly into a PDF when `tectonic`, `xelatex`, or `pdflatex` is installed on your `PATH`.

---

## 4. Troubleshooting

### macOS: "LaTeX Notes is damaged or cannot be opened"
Because community GitHub builds are not notarized with an Apple Developer ID certificate, macOS applies a quarantine flag on download. Remove it with:
```bash
xattr -cr "/Applications/LaTeX Notes.app"
```

### Local AI server takes long to respond on first convert
- First request warms up vision weights in memory.
- If using CPU-only mode on an older machine, try switching from `Qwen2.5-VL-7B` to `Qwen2.5-VL-3B` inside **Setup Local AI**, or switch temporarily to **Google AI Studio (Gemini)** in **Settings**.
