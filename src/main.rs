//! Selah — AI-native screenshot and annotation tool for AGNOS.

mod gui;

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

        /// Copy to system clipboard
        #[arg(long)]
        copy: bool,

        /// List available monitors
        #[arg(long)]
        list_monitors: bool,

        /// Capture a specific monitor by ID
        #[arg(long)]
        monitor: Option<String>,
    },
    /// Annotate an image (batch/headless mode)
    Annotate {
        /// Path to the image file
        path: String,

        /// JSON array of annotations
        #[arg(long)]
        json: Option<String>,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,

        /// Save annotation layer to a JSON file
        #[arg(long)]
        save: Option<String>,

        /// Load annotation layer from a previously saved JSON file
        #[arg(long)]
        load: Option<String>,
    },
    /// Convert an image to a different format
    Convert {
        /// Path to the input image file
        input: String,

        /// Target format (png, jpg, bmp, webp)
        #[arg(long)]
        format: String,

        /// Output file path (default: input stem with new extension)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Open the interactive annotation GUI
    Gui {
        /// Path to the image file
        path: String,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
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
    /// List recent screenshot captures
    History {
        /// Maximum number of entries to show
        #[arg(long, default_value = "20")]
        limit: usize,

        /// Only show captures since this date (ISO 8601, e.g. 2026-03-01T00:00:00Z)
        #[arg(long)]
        since: Option<String>,

        /// Show detailed info for a specific entry
        #[arg(long)]
        info: Option<String>,

        /// Delete a specific entry by ID
        #[arg(long)]
        delete: Option<String>,

        /// Output as JSON for scripting
        #[arg(long)]
        json: bool,
    },
    /// Start the MCP (Model Context Protocol) server over stdio
    Mcp,
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
            copy,
            list_monitors,
            monitor,
        } => {
            let client = selah_capture::CaptureClient::new(&cli.api_url);

            // Handle --list-monitors
            if list_monitors {
                let monitors = client.list_monitors().await?;
                if monitors.is_empty() {
                    println!("No monitors detected");
                } else {
                    println!("Available monitors:");
                    for m in &monitors {
                        println!(
                            "  {} | {} | {}x{} at ({},{}){}",
                            m.id,
                            m.name,
                            m.width,
                            m.height,
                            m.x,
                            m.y,
                            if m.primary { " [primary]" } else { "" }
                        );
                    }
                }
                return Ok(());
            }

            let img_format = match format.as_str() {
                "jpg" | "jpeg" => selah_core::ImageFormat::Jpeg,
                "bmp" => selah_core::ImageFormat::Bmp,
                "webp" => selah_core::ImageFormat::WebP,
                _ => selah_core::ImageFormat::Png,
            };

            let capture_source;
            let response = if let Some(monitor_id) = &monitor {
                capture_source = format!("monitor {monitor_id}");
                client.capture_monitor(monitor_id, img_format).await?
            } else if let Some(region_str) = region {
                let parts: Vec<f64> = region_str
                    .split(',')
                    .map(|s| s.trim().parse::<f64>())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| anyhow::anyhow!("invalid region format: {e}"))?;
                if parts.len() != 4 {
                    anyhow::bail!("region must be x,y,width,height");
                }
                let rect = selah_core::Rect::new(parts[0], parts[1], parts[2], parts[3]);
                capture_source = format!(
                    "region {}x{} at {},{}",
                    rect.width, rect.height, rect.x, rect.y
                );
                client.capture_region(rect).await?
            } else {
                capture_source = "full screen".to_string();
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
                img_format,
            )?;

            println!(
                "Screenshot saved to {output} ({}x{})",
                response.width, response.height
            );

            if copy {
                selah_capture::CaptureClient::copy_to_clipboard(&data)?;
                println!("Copied to clipboard");
            }

            // Record in history
            if let Ok(store) = selah_capture::history::HistoryStore::open_default() {
                let _ = store.record(selah_capture::history::HistoryEntry {
                    id: uuid::Uuid::new_v4(),
                    path: std::fs::canonicalize(&output)
                        .unwrap_or_else(|_| std::path::PathBuf::from(&output))
                        .to_string_lossy()
                        .to_string(),
                    timestamp: chrono::Utc::now(),
                    source: capture_source,
                    width: response.width,
                    height: response.height,
                    format: format.clone(),
                });
            }
        }
        Commands::Annotate {
            path,
            json,
            output,
            save,
            load,
        } => {
            if !std::path::Path::new(&path).exists() {
                anyhow::bail!("file not found: {path}");
            }

            let annotations: Vec<selah_core::Annotation> = if let Some(load_path) = &load {
                let canvas = selah_annotate::AnnotationCanvas::load_from_file(
                    std::path::Path::new(load_path),
                )
                .map_err(|e| anyhow::anyhow!("failed to load annotations: {e}"))?;
                canvas.get_annotations().to_vec()
            } else {
                let json_str = match &json {
                    Some(j) => j.clone(),
                    None => {
                        anyhow::bail!(
                            "batch mode requires --json <annotations> or --load <file>. Example:\n  \
                             selah annotate image.png --json '[{{\"kind\":\"rectangle\",\"position\":{{\"x\":10,\"y\":10,\"width\":100,\"height\":50}},\"color\":{{\"r\":255,\"g\":0,\"b\":0,\"a\":255}}}}]' -o output.png"
                        );
                    }
                };
                serde_json::from_str(&json_str)
                    .map_err(|e| anyhow::anyhow!("invalid annotation JSON: {e}"))?
            };

            let source =
                std::fs::read(&path).map_err(|e| anyhow::anyhow!("failed to read {path}: {e}"))?;

            let result = selah_annotate::AnnotationCanvas::render_to_image(
                &source,
                &annotations,
                selah_core::ImageFormat::Png,
            )?;

            let out = output.unwrap_or_else(|| {
                let p = std::path::Path::new(&path);
                let stem = p.file_stem().unwrap_or_default().to_string_lossy();
                let ext = p.extension().unwrap_or_default().to_string_lossy();
                format!("{stem}_annotated.{ext}")
            });

            std::fs::write(&out, &result)
                .map_err(|e| anyhow::anyhow!("failed to write {out}: {e}"))?;

            println!(
                "Applied {} annotation(s) to {path} → {out}",
                annotations.len()
            );

            if let Some(save_path) = &save {
                let img = image::load_from_memory(&source)
                    .map_err(|e| anyhow::anyhow!("failed to read image dimensions: {e}"))?;
                let mut canvas = selah_annotate::AnnotationCanvas::new(img.width(), img.height());
                for ann in &annotations {
                    canvas.add_annotation(ann.clone());
                }
                canvas
                    .save_to_file(std::path::Path::new(save_path))
                    .map_err(|e| anyhow::anyhow!("failed to save annotations: {e}"))?;
                println!("Saved annotation layer to {save_path}");
            }
        }
        Commands::Convert {
            input,
            format,
            output,
        } => {
            if !std::path::Path::new(&input).exists() {
                anyhow::bail!("file not found: {input}");
            }

            let img_format = match format.as_str() {
                "png" => selah_core::ImageFormat::Png,
                "jpg" | "jpeg" => selah_core::ImageFormat::Jpeg,
                "bmp" => selah_core::ImageFormat::Bmp,
                "webp" => selah_core::ImageFormat::WebP,
                other => anyhow::bail!("unsupported format: {other} (use png, jpg, bmp, or webp)"),
            };

            let source = std::fs::read(&input)
                .map_err(|e| anyhow::anyhow!("failed to read {input}: {e}"))?;

            let result = selah_annotate::convert_format(&source, img_format)?;

            let out = output.unwrap_or_else(|| {
                let p = std::path::Path::new(&input);
                let stem = p.file_stem().unwrap_or_default().to_string_lossy();
                format!("{stem}.{}", img_format.extension())
            });

            std::fs::write(&out, &result)
                .map_err(|e| anyhow::anyhow!("failed to write {out}: {e}"))?;

            println!("Converted {input} → {out} ({})", img_format);
        }
        Commands::Gui { path, output } => {
            if !std::path::Path::new(&path).exists() {
                anyhow::bail!("file not found: {path}");
            }
            gui::run_gui(std::path::PathBuf::from(path), output)
                .map_err(|e| anyhow::anyhow!("GUI error: {e}"))?;
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

                // Build redaction annotations from suggestions
                let annotations: Vec<selah_core::Annotation> = suggestions
                    .iter()
                    .map(|s| {
                        selah_core::Annotation::new(
                            selah_core::AnnotationKind::Redaction,
                            s.region,
                            selah_core::Color::BLACK,
                        )
                    })
                    .collect();

                let result = selah_annotate::AnnotationCanvas::render_to_image(
                    &data,
                    &annotations,
                    selah_core::ImageFormat::Png,
                )?;

                std::fs::write(&out, &result)
                    .map_err(|e| anyhow::anyhow!("failed to write {out}: {e}"))?;

                println!("Redacted output saved to {out}");
            }
        }
        Commands::History {
            limit,
            since,
            info,
            delete,
            json,
        } => {
            let store = selah_capture::history::HistoryStore::open_default()?;

            if let Some(id_str) = delete {
                let id: uuid::Uuid = id_str
                    .parse()
                    .map_err(|e| anyhow::anyhow!("invalid UUID: {e}"))?;
                if store.delete(id)? {
                    println!("Deleted history entry {id}");
                } else {
                    println!("No entry found with ID {id}");
                }
                return Ok(());
            }

            if let Some(id_str) = info {
                let id: uuid::Uuid = id_str
                    .parse()
                    .map_err(|e| anyhow::anyhow!("invalid UUID: {e}"))?;
                match store.get(id)? {
                    Some(entry) => {
                        if json {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&entry)
                                    .map_err(|e| anyhow::anyhow!("JSON error: {e}"))?
                            );
                        } else {
                            println!("ID:        {}", entry.id);
                            println!("Path:      {}", entry.path);
                            println!(
                                "Timestamp: {}",
                                entry.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
                            );
                            println!("Source:    {}", entry.source);
                            println!("Size:      {}x{}", entry.width, entry.height);
                            println!("Format:    {}", entry.format);
                        }
                    }
                    None => println!("No entry found with ID {id}"),
                }
                return Ok(());
            }

            let since_dt = if let Some(since_str) = since {
                Some(
                    chrono::DateTime::parse_from_rfc3339(&since_str)
                        .map_err(|e| {
                            anyhow::anyhow!(
                                "invalid --since date (use ISO 8601 e.g. 2026-03-01T00:00:00Z): {e}"
                            )
                        })?
                        .with_timezone(&chrono::Utc),
                )
            } else {
                None
            };

            let entries = store.list(limit, since_dt)?;

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&entries)
                        .map_err(|e| anyhow::anyhow!("JSON error: {e}"))?
                );
            } else if entries.is_empty() {
                println!("No captures in history");
            } else {
                println!("Recent captures ({} shown):", entries.len());
                for entry in &entries {
                    println!(
                        "  {} | {} | {}x{} {} | {} | {}",
                        entry.id,
                        entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                        entry.width,
                        entry.height,
                        entry.format,
                        entry.source,
                        entry.path
                    );
                }
            }
        }
        Commands::Mcp => {
            selah_mcp::run_server(&cli.api_url)
                .await
                .map_err(|e| anyhow::anyhow!("MCP server error: {e}"))?;
        }
    }

    Ok(())
}
