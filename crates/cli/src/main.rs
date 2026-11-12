use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use latex_notes_core::latex::{compile_latex_to_pdf, detect_pdf_compiler};
use latex_notes_core::models::{
    ConversionOptions, OutputMode, ProviderConfig, ProviderKind,
};
use latex_notes_core::providers::{build_provider, check_provider_health};
use latex_notes_core::runtime_manager::{
    builtin_catalog, ensure_engine_installed, ensure_model_downloaded, find_llama_server_binary,
};
use latex_notes_core::sources::LocalFileSource;

#[derive(Parser)]
#[command(
    name = "latex-notes",
    version,
    about = "Convert handwritten notes (images or PDFs) to LaTeX via local or cloud Vision AI."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, ValueEnum)]
enum ProviderArg {
    Local,
    Gemini,
}

#[derive(Subcommand)]
enum Commands {
    /// Download local llama-server engine and Vision AI model (1-step local setup)
    Setup {
        #[arg(long, default_value = "qwen2.5-vl-7b-q4")]
        model: String,
    },
    /// List curated local Vision AI models and download status
    Models,
    /// Check AI runtime availability and list discovered models
    Health {
        #[arg(long, value_enum, default_value_t = ProviderArg::Local)]
        provider: ProviderArg,
        #[arg(long, default_value = "http://localhost:11434/v1")]
        base_url: String,
        #[arg(long, default_value = "qwen2.5vl:7b")]
        model: String,
        #[arg(long, env = "GEMINI_API_KEY")]
        api_key: Option<String>,
    },
    /// Convert a local image (.png/.jpg/.webp) or PDF file into LaTeX
    Convert {
        /// Input image or PDF path
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = ProviderArg::Local)]
        provider: ProviderArg,
        #[arg(long, default_value = "http://localhost:11434/v1")]
        base_url: String,
        #[arg(long, default_value = "qwen2.5vl:7b")]
        model: String,
        #[arg(long, env = "GEMINI_API_KEY")]
        api_key: Option<String>,
        #[arg(long, default_value = "English and Russian (Cyrillic + Latin)")]
        lang: String,
        /// Write standalone .tex file
        #[arg(long)]
        out_tex: Option<PathBuf>,
        /// Write Markdown file
        #[arg(long)]
        out_md: Option<PathBuf>,
        /// Compile and write PDF (requires tectonic/xelatex/pdflatex in PATH)
        #[arg(long)]
        out_pdf: Option<PathBuf>,
    },
    /// Detect installed LaTeX-to-PDF compiler
    PdfCheck,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup { model } => {
            println!("Ensuring llama.cpp engine is installed...");
            let bin = ensure_engine_installed(|p| {
                if let Some(pct) = p.percent {
                    eprint!("\r{}: {} ({:.1}%)", p.stage, p.file_name, pct);
                } else {
                    eprintln!("\n{}: {}", p.stage, p.file_name);
                }
            })
            .await?;
            eprintln!("\nEngine ready at {}", bin.display());

            println!("Ensuring model '{model}' is downloaded...");
            let (m_path, mm_path) = ensure_model_downloaded(&model, |p| {
                if let Some(pct) = p.percent {
                    let mb = p.downloaded_bytes / (1024 * 1024);
                    eprint!("\r{}: {} ({:.1}% · {} MB)", p.stage, p.file_name, pct, mb);
                }
            })
            .await?;
            eprintln!(
                "\nModel ready:\n  weights: {}\n  mmproj:  {}",
                m_path.display(),
                mm_path.display()
            );
        }
        Commands::Models => {
            println!(
                "Engine installed: {}",
                find_llama_server_binary()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "no (run `latex-notes setup`)".to_string())
            );
            for m in builtin_catalog() {
                println!(
                    "- {} [{}] ({} MB): {}",
                    m.id,
                    if m.downloaded { "downloaded" } else { "not downloaded" },
                    m.approx_size_mb,
                    m.name
                );
            }
        }
        Commands::Health {
            provider,
            base_url,
            model,
            api_key,
        } => {
            let cfg = build_cfg(provider, base_url, model, api_key);
            let health = check_provider_health(&cfg).await?;
            println!("Reachable: {}", health.reachable);
            println!("Message:   {}", health.message);
            if !health.available_models.is_empty() {
                println!("Models:");
                for m in health.available_models {
                    println!("  - {m}");
                }
            }
        }
        Commands::Convert {
            input,
            provider,
            base_url,
            model,
            api_key,
            lang,
            out_tex,
            out_md,
            out_pdf,
        } => {
            let pages = LocalFileSource::load_file_sync(&input)?;
            eprintln!("Loaded {} page(s) from {}", pages.len(), input.display());

            let cfg = build_cfg(provider, base_url, model, api_key);
            let ai = build_provider(&cfg);
            let opts = ConversionOptions {
                language_hint: lang,
                output_mode: OutputMode::DocumentBody,
                ..Default::default()
            };

            let res = ai.convert_to_latex(&pages, &opts).await?;
            for w in &res.warnings {
                eprintln!("Warning: {w}");
            }

            if let Some(tex_path) = out_tex {
                std::fs::write(&tex_path, &res.standalone_tex)?;
                eprintln!("Saved LaTeX to {}", tex_path.display());
            }
            if let Some(md_path) = out_md {
                std::fs::write(&md_path, &res.markdown)?;
                eprintln!("Saved Markdown to {}", md_path.display());
            }
            if let Some(pdf_path) = out_pdf {
                let compiler = compile_latex_to_pdf(&res.standalone_tex, &pdf_path).await?;
                eprintln!("Compiled PDF using {compiler} -> {}", pdf_path.display());
            }

            println!("{}", res.latex_body);
        }
        Commands::PdfCheck => {
            let info = detect_pdf_compiler().await;
            println!(
                "PDF compiler available: {} ({})",
                info.available,
                info.binary_name.unwrap_or_else(|| "none".to_string())
            );
        }
    }

    Ok(())
}

fn build_cfg(
    provider: ProviderArg,
    base_url: String,
    model: String,
    api_key: Option<String>,
) -> ProviderConfig {
    match provider {
        ProviderArg::Local => ProviderConfig {
            kind: ProviderKind::OpenAiCompatible,
            base_url,
            model,
            api_key,
        },
        ProviderArg::Gemini => ProviderConfig {
            kind: ProviderKind::Gemini,
            base_url: String::new(),
            model: if model == "qwen2.5vl:7b" {
                "gemini-2.5-flash".to_string()
            } else {
                model
            },
            api_key,
        },
    }
}
