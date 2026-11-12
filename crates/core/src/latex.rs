use std::path::Path;

use regex::Regex;
use tokio::process::Command;

use crate::error::{CoreError, Result};
use crate::models::{ConversionOptions, OutputMode, PdfCompilerInfo};

/// Builds the system prompt tailored to handwritten math/text conversion.
pub fn build_system_prompt(options: &ConversionOptions) -> String {
    let mode_instruction = match options.output_mode {
        OutputMode::DocumentBody => {
            "Output ONLY the LaTeX body content (using \\section*, \\subsection*, paragraphs, itemize/enumerate, and standard math environments like $...$, \\[...\\], align*, cases, matrix). Do NOT include \\documentclass, \\usepackage, \\begin{document}, or \\end{document}."
        }
        OutputMode::StandaloneDocument => {
            "Output a complete, compilable LaTeX document including \\documentclass, necessary \\usepackage declarations, and \\begin{document}...\\end{document}."
        }
        OutputMode::MathOnly => {
            "Output ONLY the transcribed mathematical expressions in clean LaTeX math syntax (e.g. align*, cases, pmatrix, or $$...$$)."
        }
    };

    let extra = options
        .extra_instructions
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("\nAdditional user instructions: {s}"))
        .unwrap_or_default();

    format!(
        "You are an expert mathematical and scientific handwriting-to-LaTeX transcriber.\n\
         Transcribe ALL handwritten text, equations, matrices, integrals, tables, and structured notes from the provided page image(s) into accurate, clean LaTeX.\n\
         Expected languages: {lang}.\n\
         Rules:\n\
         1. {mode_instruction}\n\
         2. Never wrap your output in markdown code fences (no ```latex or ```).\n\
         3. Preserve logical page structure, bullet lists, numbered derivations, and subscripts/superscripts faithfully.\n\
         4. If a word or symbol is genuinely illegible, mark it briefly as `\\text{{[illegible]}}` rather than inventing unrelated equations.\n\
         5. Do not add conversational commentary before or after the LaTeX.{extra}",
        lang = options.language_hint
    )
}

/// Strips markdown code fences and extracts document body if the model emitted a full preamble.
pub fn sanitize_latex_output(raw: &str, mode: OutputMode) -> (String, Vec<String>) {
    let mut warnings = Vec::new();
    let mut text = raw.trim().to_string();

    // Strip leading/trailing markdown code fences such as ```latex ... ``` or ```tex ... ```
    let fence_re = Regex::new(r"(?s)^```(?:latex|tex|math)?\s*\n?(.*?)\n?```\s*$").unwrap();
    if let Some(caps) = fence_re.captures(&text) {
        text = caps[1].trim().to_string();
    } else if text.starts_with("```") {
        if let Some(first_nl) = text.find('\n') {
            text = text[first_nl + 1..].trim().to_string();
        }
        if text.ends_with("```") {
            text = text[..text.len() - 3].trim().to_string();
        }
    }

    if mode != OutputMode::StandaloneDocument {
        // If model ignored instructions and emitted \begin{document}...\end{document}, extract body.
        let doc_re = Regex::new(r"(?s)\\begin\{document\}(.*?)\\end\{document\}").unwrap();
        if let Some(caps) = doc_re.captures(&text) {
            text = caps[1].trim().to_string();
            warnings.push("Stripped redundant document preamble emitted by the model.".to_string());
        }
    }

    // Check basic brace balance as a non-destructive lint warning
    let opens = text.chars().filter(|&c| c == '{').count();
    let closes = text.chars().filter(|&c| c == '}').count();
    if opens != closes {
        warnings.push(format!(
            "Unbalanced curly braces detected ({{ = {opens}, }} = {closes}); review before compiling PDF."
        ));
    }

    (text, warnings)
}

/// Wraps a LaTeX body into a self-contained UTF-8 LaTeX document supporting both Latin and Cyrillic.
pub fn wrap_standalone_document(body: &str, title: Option<&str>) -> String {
    let trimmed = body.trim();
    if trimmed.contains("\\documentclass") && trimmed.contains("\\begin{document}") {
        return trimmed.to_string();
    }

    let doc_title = title.unwrap_or("Handwritten Notes");
    format!(
        "\\documentclass[11pt,a4paper]{{article}}\n\
         \\usepackage[utf8]{{inputenc}}\n\
         \\usepackage[T2A,T1]{{fontenc}}\n\
         \\usepackage[russian,english]{{babel}}\n\
         \\usepackage{{amsmath,amssymb,mathtools,amsthm}}\n\
         \\usepackage{{geometry}}\n\
         \\geometry{{margin=2cm}}\n\
         \\usepackage{{hyperref}}\n\n\
         \\title{{{doc_title}}}\n\
         \\date{{\\today}}\n\n\
         \\begin{{document}}\n\
         \\maketitle\n\n\
         {trimmed}\n\n\
         \\end{{document}}\n"
    )
}

/// Converts LaTeX body into clean Markdown suitable for Obsidian / Notion / GitHub.
pub fn latex_body_to_markdown(body: &str) -> String {
    let mut md = body.to_string();
    let sec_re = Regex::new(r"\\section\*?\{([^}]*)\}").unwrap();
    md = sec_re.replace_all(&md, "## $1").into_owned();

    let subsec_re = Regex::new(r"\\subsection\*?\{([^}]*)\}").unwrap();
    md = subsec_re.replace_all(&md, "### $1").into_owned();

    let subsubsec_re = Regex::new(r"\\subsubsection\*?\{([^}]*)\}").unwrap();
    md = subsubsec_re.replace_all(&md, "#### $1").into_owned();

    let bold_re = Regex::new(r"\\textbf\{([^}]*)\}").unwrap();
    md = bold_re.replace_all(&md, "**$1**").into_owned();

    let italic_re = Regex::new(r"\\textit\{([^}]*)\}").unwrap();
    md = italic_re.replace_all(&md, "*$1*").into_owned();

    md = md.replace("\\[", "$$").replace("\\]", "$$");
    md
}

/// Detects an installed LaTeX-to-PDF compiler (`tectonic`, `xelatex`, or `pdflatex`).
pub async fn detect_pdf_compiler() -> PdfCompilerInfo {
    for bin in ["tectonic", "xelatex", "pdflatex"] {
        if let Ok(status) = Command::new(bin).arg("--version").output().await {
            if status.status.success() {
                return PdfCompilerInfo {
                    available: true,
                    binary_name: Some(bin.to_string()),
                };
            }
        }
    }
    PdfCompilerInfo {
        available: false,
        binary_name: None,
    }
}

/// Compiles standalone LaTeX source to a PDF file at `output_pdf` using an available compiler.
pub async fn compile_latex_to_pdf(standalone_tex: &str, output_pdf: &Path) -> Result<String> {
    let compiler = detect_pdf_compiler().await;
    let bin = compiler.binary_name.ok_or_else(|| {
        CoreError::Export(
            "No LaTeX PDF compiler found in PATH. Install `tectonic`, `xelatex`, or `pdflatex` to export PDFs directly."
                .to_string(),
        )
    })?;

    let temp_dir = std::env::temp_dir().join(format!(
        "latex-notes-compile-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    tokio::fs::create_dir_all(&temp_dir).await?;

    let tex_path = temp_dir.join("document.tex");
    tokio::fs::write(&tex_path, standalone_tex).await?;

    let output = match bin.as_str() {
        "tectonic" => {
            Command::new(&bin)
                .arg(&tex_path)
                .arg("--outdir")
                .arg(&temp_dir)
                .output()
                .await?
        }
        _ => {
            Command::new(&bin)
                .arg("-interaction=nonstopmode")
                .arg("-output-directory")
                .arg(&temp_dir)
                .arg(&tex_path)
                .output()
                .await?
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        return Err(CoreError::Export(format!(
            "{bin} failed to compile LaTeX:\n{stderr}\n{stdout}"
        )));
    }

    let generated_pdf = temp_dir.join("document.pdf");
    if !generated_pdf.exists() {
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        return Err(CoreError::Export(format!(
            "{bin} completed without producing document.pdf"
        )));
    }

    tokio::fs::copy(&generated_pdf, output_pdf).await?;
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    Ok(bin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markdown_fences_and_preamble() {
        let raw = "```latex\n\\documentclass{article}\n\\begin{document}\nE = mc^2\n\\end{document}\n```";
        let (clean, warnings) = sanitize_latex_output(raw, OutputMode::DocumentBody);
        assert_eq!(clean, "E = mc^2");
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn warns_on_unbalanced_braces() {
        let (clean, warnings) = sanitize_latex_output("\\frac{a}{b", OutputMode::MathOnly);
        assert_eq!(clean, "\\frac{a}{b");
        assert!(!warnings.is_empty());
    }

    #[test]
    fn converts_basic_latex_to_markdown() {
        let md = latex_body_to_markdown("\\section*{Lecture 1}\n\\[x^2 + y^2 = 1\\]");
        assert_eq!(md, "## Lecture 1\n$$x^2 + y^2 = 1$$");
    }
}
