//! Selah — AI-native screenshot and annotation tool for AGNOS.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "selah",
    version,
    about = "AI-native screenshot & annotation tool for AGNOS"
)]
struct Cli {
    /// Daimon API URL
    #[arg(long, default_value = "http://localhost:8090")]
    api_url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Take a screenshot (full screen or region)
    Capture {
        /// Capture a specific region: x,y,width,height
        #[arg(long)]
        region: Option<String>,

        /// Output file path
        #[arg(short, long, default_value = "screenshot.png")]
        output: String,

        /// Image format (png, jpg, bmp, webp)
        #[arg(short, long, default_value = "png")]
        format: String,
    },
    /// Open annotation editor on an image
    Annotate {
        /// Path to the image file
        path: String,
    },
    /// Extract text from an image
    Ocr {
        /// Path to the image file
        path: String,
    },
    /// Auto-detect and redact sensitive content
    Redact {
        /// Path to the image file
        path: String,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Capture {
            region,
            output,
            format,
        } => {
            let client = selah_capture::CaptureClient::new(&cli.api_url);
            let _format = match format.as_str() {
                "jpg" | "jpeg" => selah_core::ImageFormat::Jpeg,
                "bmp" => selah_core::ImageFormat::Bmp,
                "webp" => selah_core::ImageFormat::WebP,
                _ => selah_core::ImageFormat::Png,
            };

            let response = if let Some(region_str) = region {
                let parts: Vec<f64> = region_str
                    .split(',')
                    .map(|s| s.trim().parse::<f64>())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| anyhow::anyhow!("invalid region format: {e}"))?;
                if parts.len() != 4 {
                    anyhow::bail!("region must be x,y,width,height");
                }
                let rect = selah_core::Rect::new(parts[0], parts[1], parts[2], parts[3]);
                client.capture_region(rect).await?
            } else {
                client.capture_full().await?
            };

            let data = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                &response.image_data,
            )
            .map_err(|e| anyhow::anyhow!("failed to decode image data: {e}"))?;

            selah_capture::CaptureClient::save_to_file(
                &data,
                std::path::Path::new(&output),
                _format,
            )?;

            println!(
                "Screenshot saved to {output} ({}x{})",
                response.width, response.height
            );
        }
        Commands::Annotate { path } => {
            if !std::path::Path::new(&path).exists() {
                anyhow::bail!("file not found: {path}");
            }
            println!("Annotation editor for {path} — not yet implemented (Phase 2: GUI)");
        }
        Commands::Ocr { path } => {
            let data =
                std::fs::read(&path).map_err(|e| anyhow::anyhow!("failed to read {path}: {e}"))?;
            let result = selah_ai::extract_text_regions(&data);

            if result.text.is_empty() {
                println!("No text detected in {path}");
            } else {
                println!(
                    "Extracted text (confidence: {:.0}%):",
                    result.confidence * 100.0
                );
                println!("{}", result.text);
            }
        }
        Commands::Redact { path, output } => {
            let data =
                std::fs::read(&path).map_err(|e| anyhow::anyhow!("failed to read {path}: {e}"))?;
            let ocr = selah_ai::extract_text_regions(&data);
            let suggestions = selah_ai::suggest_redactions(&ocr.text);

            if suggestions.is_empty() {
                println!("No sensitive content detected in {path}");
            } else {
                println!("Found {} sensitive item(s):", suggestions.len());
                for s in &suggestions {
                    println!(
                        "  - {} (confidence: {:.0}%): {}",
                        s.target_type,
                        s.confidence * 100.0,
                        s.matched_text
                    );
                }
                let out = output.unwrap_or_else(|| {
                    let p = std::path::Path::new(&path);
                    let stem = p.file_stem().unwrap_or_default().to_string_lossy();
                    let ext = p.extension().unwrap_or_default().to_string_lossy();
                    format!("{stem}_redacted.{ext}")
                });
                println!(
                    "Redacted output would be saved to {out} (Phase 3: pixel-level redaction)"
                );
            }
        }
    }

    Ok(())
}
