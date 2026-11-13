//! MCP server for Selah, powered by bote.
//!
//! Exposes 5 tools via the Model Context Protocol over stdio:
//! `selah_capture`, `selah_annotate`, `selah_ocr`, `selah_redact`, `selah_history`.
//!
//! All file paths supplied by the client are checked: `..` components are
//! rejected for both inputs and outputs; inputs must be regular files no
//! larger than [`MAX_INPUT_FILE_BYTES`] whose content is a supported image;
//! outputs must carry an image extension and may not be symlinks or other
//! non-regular files; default outputs sit next to the input file.
//!
//! Absolute paths are allowed: the server acts with the permissions of the
//! user running it and does not confine clients to a directory.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use bote::{Dispatcher, ToolAnnotations, ToolDef, ToolRegistry, ToolSchema};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::annotate::AnnotationCanvas;
use crate::capture::CaptureClient;
use crate::core::{Annotation, AnnotationKind, CaptureRegion, CaptureSource, Color, ImageFormat};
use crate::geometry::Rect;
use crate::history::{HistoryEntry, HistoryStore};

/// Maximum size of an input image read by the MCP tools.
pub const MAX_INPUT_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// Maximum number of annotations accepted by `selah_annotate`.
pub const MAX_ANNOTATIONS: usize = 10_000;

/// Maximum number of entries returned by `selah_history`.
const MAX_HISTORY_LIMIT: u64 = 1_000;
const DEFAULT_HISTORY_LIMIT: u64 = 20;

/// Redaction target keys accepted by `selah_redact`.
const REDACTION_KEYS: [&str; 4] = ["email", "phone", "credit_card", "ip_address"];

/// Shared state for tool handlers.
struct Context {
    api_url: String,
    history: Option<HistoryStore>,
}

/// Build a [`ToolRegistry`] containing all five selah tools.
fn build_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    let rect_schema = json!({
        "type": "object",
        "properties": {
            "x": { "type": "number" },
            "y": { "type": "number" },
            "width": { "type": "number" },
            "height": { "type": "number" }
        },
        "required": ["x", "y", "width", "height"]
    });

    registry.register(ToolDef::new(
        "selah_capture",
        "Take a screenshot via the daimon screen capture API",
        ToolSchema::new(
            "object",
            HashMap::from([
                ("region".into(), {
                    let mut r = rect_schema.clone();
                    r["description"] = json!("Capture region (omit for full screen)");
                    r
                }),
                (
                    "format".into(),
                    json!({
                        "type": "string",
                        "enum": ["png", "jpg", "jpeg", "bmp", "webp"],
                        "default": "png"
                    }),
                ),
                (
                    "output".into(),
                    json!({
                        "type": "string",
                        "description": "Output file path (default: screenshot.<format>)"
                    }),
                ),
            ]),
            vec![],
        ),
    ));

    registry.register(ToolDef::new(
        "selah_annotate",
        "Add annotations to an image",
        ToolSchema::new(
            "object",
            HashMap::from([
                (
                    "image_path".into(),
                    json!({ "type": "string", "description": "Path to the source image" }),
                ),
                (
                    "annotations".into(),
                    json!({
                        "type": "array",
                        "maxItems": MAX_ANNOTATIONS,
                        "items": {
                            "type": "object",
                            "properties": {
                                "kind": {
                                    "type": "string",
                                    "enum": ["arrow", "rectangle", "circle", "text", "highlight", "redaction", "freeform"]
                                },
                                "position": rect_schema,
                                "color": {
                                    "type": "object",
                                    "description": "RGBA color, 0-255 per channel (default: opaque red)",
                                    "properties": {
                                        "r": { "type": "integer" },
                                        "g": { "type": "integer" },
                                        "b": { "type": "integer" },
                                        "a": { "type": "integer" }
                                    }
                                },
                                "text": { "type": "string" }
                            },
                            "required": ["kind", "position"]
                        }
                    }),
                ),
                (
                    "output".into(),
                    json!({
                        "type": "string",
                        "description": "Output file path; format follows the extension (default: <image>_annotated.<ext> next to the input)"
                    }),
                ),
            ]),
            vec!["image_path".into(), "annotations".into()],
        ),
    ));

    registry.register(
        ToolDef::new(
            "selah_ocr",
            "Extract text from an image. The 'local' engine is a stub that only finds text \
             embedded in file metadata; use 'hoosh' for real OCR of rendered text.",
            ToolSchema::new(
                "object",
                HashMap::from([
                    (
                        "image_path".into(),
                        json!({ "type": "string", "description": "Path to the image" }),
                    ),
                    (
                        "engine".into(),
                        json!({
                            "type": "string",
                            "enum": ["local", "hoosh"],
                            "default": "local"
                        }),
                    ),
                ]),
                vec!["image_path".into()],
            ),
        )
        .with_annotations({
            let mut a = ToolAnnotations::read_only();
            a.open_world_hint = Some(true); // the hoosh engine calls out to a service
            a
        }),
    );

    registry.register(ToolDef::new(
        "selah_redact",
        "Detect and redact sensitive content in an image. Fails (without writing output) \
         if sensitive text is detected but cannot be located in the image.",
        ToolSchema::new(
            "object",
            HashMap::from([
                (
                    "image_path".into(),
                    json!({ "type": "string", "description": "Path to the image" }),
                ),
                (
                    "targets".into(),
                    json!({
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": REDACTION_KEYS
                        },
                        "description": "Types of sensitive data to redact (default: all)"
                    }),
                ),
                (
                    "output".into(),
                    json!({
                        "type": "string",
                        "description": "Output file path; format follows the extension (default: <image>_redacted.<ext> next to the input)"
                    }),
                ),
            ]),
            vec!["image_path".into()],
        ),
    ));

    registry.register(
        ToolDef::new(
            "selah_history",
            "List recent screenshots and their metadata",
            ToolSchema::new(
                "object",
                HashMap::from([
                    (
                        "limit".into(),
                        json!({
                            "type": "integer",
                            "minimum": 0,
                            "maximum": MAX_HISTORY_LIMIT,
                            "default": DEFAULT_HISTORY_LIMIT,
                            "description": "Max results to return"
                        }),
                    ),
                    (
                        "since".into(),
                        json!({
                            "type": "string",
                            "description": "RFC 3339 timestamp — only return captures at or after this time"
                        }),
                    ),
                ]),
                vec![],
            ),
        )
        .with_annotations(ToolAnnotations::read_only()),
    );

    registry
}

/// Wrap a fallible tool function as a bote handler that reports errors as
/// MCP error content.
fn handler(
    ctx: &Arc<Context>,
    f: fn(&Context, &Value) -> Result<Value, String>,
) -> bote::dispatch::ToolHandler {
    let ctx = Arc::clone(ctx);
    Arc::new(move |args: Value| -> Value {
        f(&ctx, &args).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "MCP tool call failed");
            error_content(&e)
        })
    })
}

/// Build a [`Dispatcher`] with all tool handlers wired up.
fn build_dispatcher(ctx: Context) -> Dispatcher {
    let ctx = Arc::new(ctx);
    let mut dispatcher = Dispatcher::new(build_registry());
    dispatcher.handle("selah_capture", handler(&ctx, tool_capture_sync));
    dispatcher.handle("selah_annotate", handler(&ctx, tool_annotate));
    dispatcher.handle("selah_ocr", handler(&ctx, tool_ocr));
    dispatcher.handle("selah_redact", handler(&ctx, tool_redact));
    dispatcher.handle("selah_history", handler(&ctx, tool_history));
    dispatcher
}

/// Run the MCP server over stdio using bote's transport.
///
/// Captures are recorded in the default [`HistoryStore`]; if it cannot be
/// opened, history is disabled (and `selah_history` reports an error).
pub fn run_server(api_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let history = HistoryStore::open_default()
        .inspect_err(|e| tracing::warn!(error = %e, "screenshot history unavailable"))
        .ok();
    let dispatcher = build_dispatcher(Context {
        api_url: api_url.to_string(),
        history,
    });
    tracing::info!(api_url, "starting selah MCP server on stdio");
    bote::transport::stdio::run(&dispatcher)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Response / argument helpers
// ---------------------------------------------------------------------------

fn error_content(msg: &str) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": msg
        }],
        "isError": true
    })
}

/// Wrap a JSON payload as MCP text content.
fn text_content(payload: &Value) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": payload.to_string()
        }]
    })
}

/// A required string argument.
fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    optional_str(args, key)?.ok_or_else(|| format!("missing {key}"))
}

/// An optional string argument (`null` counts as absent).
fn optional_str<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(format!("{key} must be a string")),
    }
}

/// Validate that a file path is non-empty and has no `..` components.
fn validate_path(path: &str) -> Result<&str, String> {
    if path.is_empty() {
        return Err("path must not be empty".into());
    }
    if path.contains('\0') {
        return Err("path must not contain NUL bytes".into());
    }
    if Path::new(path)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(format!("path traversal not allowed: {path}"));
    }
    Ok(path)
}

/// Read a validated input file with a size cap (rejects devices, FIFOs,
/// dirs) and require its content to be a supported image, so the tools
/// cannot be used to read arbitrary files (e.g. OCR of a private key).
fn read_input(path: &str) -> Result<Vec<u8>, String> {
    let data = crate::fsutil::read_limited(Path::new(path), MAX_INPUT_FILE_BYTES)
        .map_err(|e| format!("failed to read {path}: {e}"))?;
    match image::guess_format(&data) {
        Ok(
            image::ImageFormat::Png
            | image::ImageFormat::Jpeg
            | image::ImageFormat::Bmp
            | image::ImageFormat::WebP,
        ) => Ok(data),
        _ => Err(format!(
            "{path} is not a supported image (png, jpeg, bmp, webp)"
        )),
    }
}

/// Validate an output path: no traversal, an image file extension, and not
/// an existing symlink / directory / device (which `write` would follow or
/// clobber). Returns the format implied by the extension.
fn validate_output(path: &str) -> Result<ImageFormat, String> {
    validate_path(path)?;
    let format = ImageFormat::from_path(Path::new(path)).ok_or_else(|| {
        format!("output {path} must have an image extension (png, jpg, jpeg, bmp, webp)")
    })?;
    match std::fs::symlink_metadata(path) {
        Ok(meta) if !meta.file_type().is_file() => Err(format!(
            "refusing to write {path}: it exists and is not a regular file"
        )),
        _ => Ok(format),
    }
}

/// The `output` argument, or `<stem>_<suffix>.<ext>` next to `input`;
/// validated. Returns the path and the output format from its extension.
fn output_path(args: &Value, input: &str, suffix: &str) -> Result<(String, ImageFormat), String> {
    let output = match optional_str(args, "output")? {
        Some(o) => o.to_owned(),
        None => {
            let mut name = crate::core::derive_output_path(input, suffix);
            if ImageFormat::from_path(Path::new(&name)).is_none() {
                name.push_str(".png");
            }
            Path::new(input)
                .with_file_name(name)
                .to_string_lossy()
                .into_owned()
        }
    };
    let format = validate_output(&output)?;
    Ok((output, format))
}

/// Mask a sensitive string for safe display, keeping only the first and
/// last 2 characters (char-based, so multibyte text never panics).
fn mask_pii(s: &str) -> String {
    let n = s.chars().count();
    if n <= 4 {
        return "*".repeat(n);
    }
    let mut out = String::with_capacity(s.len());
    out.extend(s.chars().take(2));
    out.extend(std::iter::repeat_n('*', n - 4));
    out.extend(s.chars().skip(n - 2));
    out
}

/// Run `fut` to completion on a dedicated thread with a private
/// current-thread runtime.
///
/// bote handlers are synchronous and may run inside a caller's tokio
/// runtime. Blocking on that runtime's `Handle` deadlocks a current-thread
/// runtime (its IO driver cannot run while the handler blocks it), and
/// `Runtime::block_on` panics inside any runtime — a private runtime on a
/// scoped thread is safe in every context.
fn run_async<T: Send>(
    fut: impl std::future::Future<Output = Result<T, String>> + Send,
) -> Result<T, String> {
    std::thread::scope(|s| {
        s.spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("failed to start async runtime: {e}"))?
                .block_on(fut)
        })
        .join()
        .map_err(|_| "async task panicked".to_string())?
    })
}

// ---------------------------------------------------------------------------
// Tool handlers
// ---------------------------------------------------------------------------

fn tool_capture_sync(ctx: &Context, args: &Value) -> Result<Value, String> {
    run_async(tool_capture(ctx, args))
}

async fn tool_capture(ctx: &Context, args: &Value) -> Result<Value, String> {
    let requested: Option<ImageFormat> = optional_str(args, "format")?
        .map(str::parse)
        .transpose()
        .map_err(|e: crate::SelahError| e.to_string())?;

    let region = match args.get("region") {
        None | Some(Value::Null) => CaptureRegion::FullScreen,
        Some(r) => {
            CaptureRegion::Rect(Rect::deserialize(r).map_err(|e| format!("invalid region: {e}"))?)
        }
    };

    // Validate the destination before taking the screenshot. The format
    // comes from `format`, else the output extension, else PNG.
    let (output, format) = match optional_str(args, "output")? {
        Some(o) => {
            let from_ext = validate_output(o)?;
            let format = requested.unwrap_or(from_ext);
            if format != from_ext {
                return Err(format!(
                    "output {o} has a .{from_ext} extension but format is {format}"
                ));
            }
            (o.to_owned(), format)
        }
        None => {
            let format = requested.unwrap_or_default();
            let output = format!("screenshot.{}", format.extension());
            validate_output(&output)?;
            (output, format)
        }
    };

    let response = CaptureClient::new(&ctx.api_url)
        .capture(&region, format)
        .await
        .map_err(|e| e.to_string())?;
    let data = crate::decode_image_data(&response.image_data).map_err(|e| e.to_string())?;
    CaptureClient::save_to_file(&data, Path::new(&output))
        .map_err(|e| format!("failed to write {output}: {e}"))?;

    // Record the absolute path so history stays meaningful from any cwd.
    let saved_path = std::fs::canonicalize(&output)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| output.clone());
    let timestamp = chrono::Utc::now();

    if let Some(store) = &ctx.history {
        let source = match &region {
            CaptureRegion::Rect(r) => CaptureSource::Region(*r),
            _ => CaptureSource::FullScreen,
        };
        let entry = HistoryEntry {
            id: uuid::Uuid::new_v4(),
            path: saved_path.clone(),
            timestamp,
            source: source.to_string(),
            width: response.width,
            height: response.height,
            format: format.extension().to_string(),
        };
        if let Err(e) = store.record(entry) {
            tracing::warn!(error = %e, "failed to record capture in history");
        }
    }

    Ok(text_content(&json!({
        "path": saved_path,
        "width": response.width,
        "height": response.height,
        "format": format.extension(),
        "timestamp": timestamp.to_rfc3339()
    })))
}

/// Annotation as accepted over MCP: `kind` is case-insensitive, `color` is
/// optional, and unknown fields (such as `id`) are ignored.
#[derive(Deserialize)]
struct AnnotationInput {
    kind: String,
    position: Rect,
    #[serde(default)]
    color: Option<Color>,
    #[serde(default)]
    text: Option<String>,
}

impl AnnotationInput {
    fn into_annotation(self) -> Result<Annotation, String> {
        let kind: AnnotationKind = self
            .kind
            .parse()
            .map_err(|e: crate::SelahError| e.to_string())?;
        let mut ann = Annotation::new(kind, self.position, self.color.unwrap_or_default());
        ann.text = self.text;
        Ok(ann)
    }
}

fn parse_annotations(value: &Value) -> Result<Vec<Annotation>, String> {
    let items = value.as_array().ok_or("annotations must be an array")?;
    if items.len() > MAX_ANNOTATIONS {
        return Err(format!(
            "too many annotations ({}, limit {MAX_ANNOTATIONS})",
            items.len()
        ));
    }
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            AnnotationInput::deserialize(item)
                .map_err(|e| e.to_string())
                .and_then(AnnotationInput::into_annotation)
                .map_err(|e| format!("invalid annotation {i}: {e}"))
        })
        .collect()
}

fn tool_annotate(_ctx: &Context, args: &Value) -> Result<Value, String> {
    let image_path = validate_path(require_str(args, "image_path")?)?;
    let annotations = parse_annotations(args.get("annotations").ok_or("missing annotations")?)?;
    let (output, format) = output_path(args, image_path, "annotated")?;

    let source = read_input(image_path)?;
    let result = AnnotationCanvas::render_to_image(&source, &annotations, format)
        .map_err(|e| e.to_string())?;
    std::fs::write(&output, &result).map_err(|e| format!("failed to write {output}: {e}"))?;

    Ok(text_content(&json!({
        "path": output,
        "format": format.extension(),
        "annotation_count": annotations.len()
    })))
}

fn tool_ocr(_ctx: &Context, args: &Value) -> Result<Value, String> {
    let image_path = validate_path(require_str(args, "image_path")?)?;
    let engine = optional_str(args, "engine")?.unwrap_or("local");
    if !matches!(engine, "local" | "hoosh") {
        return Err(format!(
            "unknown OCR engine {engine:?} (use local or hoosh)"
        ));
    }

    let data = read_input(image_path)?;
    let result = if engine == "hoosh" {
        run_async(async {
            crate::daimon::HooshClient::new()
                .ocr(&data)
                .await
                .map_err(|e| e.to_string())
        })?
    } else {
        crate::ai::extract_text_regions(&data)
    };

    Ok(text_content(&json!({
        "text": result.text,
        "confidence": result.confidence,
        "regions": result.bounding_boxes,
        "engine": engine,
        "is_stub": result.is_stub
    })))
}

fn parse_targets(args: &Value) -> Result<Option<Vec<&str>>, String> {
    let Some(value) = args.get("targets").filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    let items = value.as_array().ok_or("targets must be an array")?;
    items
        .iter()
        .map(|t| match t.as_str() {
            Some(k) if REDACTION_KEYS.contains(&k) => Ok(k),
            _ => Err(format!(
                "invalid redaction target {t} (use {})",
                REDACTION_KEYS.join(", ")
            )),
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn tool_redact(_ctx: &Context, args: &Value) -> Result<Value, String> {
    let image_path = validate_path(require_str(args, "image_path")?)?;
    let targets = parse_targets(args)?;
    let (output, format) = output_path(args, image_path, "redacted")?;

    let data = read_input(image_path)?;
    let ocr = crate::ai::extract_text_regions(&data);
    let mut suggestions = crate::ai::suggest_redactions(&ocr.text);
    if let Some(targets) = &targets {
        suggestions.retain(|s| targets.contains(&s.target_type.key()));
    }

    let (located, unlocated): (Vec<_>, Vec<_>) =
        suggestions.into_iter().partition(|s| s.region.area() > 0.0);
    // The stub OCR reads the encoded file bytes, so whatever it finds lives
    // in embedded metadata, which re-encoding below does not carry over.
    // A real OCR engine reads pixels: if it finds sensitive text it cannot
    // locate, fail closed rather than hand back an unredacted "redacted" file.
    if !unlocated.is_empty() && !ocr.is_stub {
        let mut kinds: Vec<&str> = unlocated.iter().map(|s| s.target_type.key()).collect();
        kinds.sort_unstable();
        kinds.dedup();
        return Err(format!(
            "detected {} sensitive match(es) ({}) that the OCR engine could not locate in the \
             image; refusing to write an unredacted copy to {output}",
            unlocated.len(),
            kinds.join(", ")
        ));
    }

    let annotations: Vec<Annotation> = located
        .iter()
        .map(|s| Annotation::new(AnnotationKind::Redaction, s.region, Color::BLACK))
        .collect();
    let result = AnnotationCanvas::render_to_image(&data, &annotations, format)
        .map_err(|e| e.to_string())?;
    std::fs::write(&output, &result).map_err(|e| format!("failed to write {output}: {e}"))?;

    let redactions: Vec<Value> = located
        .iter()
        .map(|s| {
            json!({
                "target_type": s.target_type.to_string(),
                "confidence": s.confidence,
                "matched_text": mask_pii(&s.matched_text)
            })
        })
        .collect();

    let mut payload = json!({
        "path": output,
        "format": format.extension(),
        "redactions": redactions
    });
    if ocr.is_stub {
        let metadata_matches: Vec<Value> = unlocated
            .iter()
            .map(|s| {
                json!({
                    "target_type": s.target_type.to_string(),
                    "matched_text": mask_pii(&s.matched_text)
                })
            })
            .collect();
        payload["metadata_matches"] = json!(metadata_matches);
        payload["warning"] = json!(
            "local OCR is a stub: it scans the file's embedded bytes, not the rendered pixels, so \
             text visible in the image was NOT detected or redacted. Matches found in embedded \
             metadata are listed in metadata_matches; the re-encoded output does not include \
             that metadata."
        );
    }
    Ok(text_content(&payload))
}

fn tool_history(ctx: &Context, args: &Value) -> Result<Value, String> {
    let limit = match args.get("limit") {
        None | Some(Value::Null) => DEFAULT_HISTORY_LIMIT,
        Some(v) => v.as_u64().ok_or("limit must be a non-negative integer")?,
    }
    .min(MAX_HISTORY_LIMIT);

    let since = optional_str(args, "since")?
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| format!("invalid since timestamp {s:?}: {e}"))
        })
        .transpose()?;

    let store = ctx
        .history
        .as_ref()
        .ok_or("screenshot history is unavailable")?;
    // `limit` ≤ MAX_HISTORY_LIMIT, so this conversion is lossless.
    let entries = store
        .list(limit as usize, since)
        .map_err(|e| e.to_string())?;

    let screenshots: Vec<Value> = entries
        .iter()
        .map(|e| {
            json!({
                "id": e.id.to_string(),
                "path": e.path,
                "timestamp": e.timestamp.to_rfc3339(),
                "source": e.source,
                "format": e.format,
                "dimensions": format!("{}x{}", e.width, e.height)
            })
        })
        .collect();

    Ok(text_content(&json!({ "screenshots": screenshots })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::test_server::serve_once;

    /// Temp dir removed on drop.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("selah_mcp_test_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self, name: &str) -> String {
            self.0.join(name).to_string_lossy().into_owned()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn ctx_with_history(dir: &TempDir) -> Context {
        Context {
            api_url: "http://127.0.0.1:9".into(),
            history: Some(HistoryStore::open(dir.0.join("history.jsonl"))),
        }
    }

    fn ctx() -> Context {
        Context {
            api_url: "http://127.0.0.1:9".into(),
            history: None,
        }
    }

    fn write_png(path: &str, w: u32, h: u32) {
        image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]))
            .save_with_format(path, image::ImageFormat::Png)
            .unwrap();
    }

    /// Parse the JSON payload out of an MCP text content response.
    fn payload(v: &Value) -> Value {
        serde_json::from_str(v["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    #[test]
    fn test_registry_has_five_tools() {
        let registry = build_registry();
        assert_eq!(registry.len(), 5);
        assert!(registry.contains("selah_capture"));
        assert!(registry.contains("selah_annotate"));
        assert!(registry.contains("selah_ocr"));
        assert!(registry.contains("selah_redact"));
        assert!(registry.contains("selah_history"));
    }

    #[test]
    fn test_dispatcher_tools_list() {
        let dispatcher = build_dispatcher(ctx());
        let req = bote::JsonRpcRequest::new(1, "tools/list");
        let resp = dispatcher.dispatch(&req).unwrap();
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 5);

        let names: Vec<&str> = tools
            .iter()
            .map(|t: &serde_json::Value| t.get("name").unwrap().as_str().unwrap())
            .collect();
        assert!(names.contains(&"selah_capture"));
        assert!(names.contains(&"selah_annotate"));
        assert!(names.contains(&"selah_ocr"));
        assert!(names.contains(&"selah_redact"));
        assert!(names.contains(&"selah_history"));
    }

    #[test]
    fn test_read_only_tools_annotated() {
        let registry = build_registry();
        let hint = |name: &str| {
            registry
                .get(name)
                .and_then(|t| t.annotations.as_ref())
                .and_then(|a| a.read_only_hint)
        };
        assert_eq!(hint("selah_ocr"), Some(true));
        assert_eq!(hint("selah_history"), Some(true));
        assert_eq!(hint("selah_annotate"), None);
    }

    #[test]
    fn test_dispatcher_initialize() {
        let dispatcher = build_dispatcher(ctx());
        let req = bote::JsonRpcRequest::new(1, "initialize");
        let resp = dispatcher.dispatch(&req).unwrap();
        let result = resp.result.unwrap();
        assert!(result.get("protocolVersion").is_some());
        assert!(result.get("serverInfo").is_some());
    }

    #[test]
    fn test_dispatcher_unknown_method() {
        let dispatcher = build_dispatcher(ctx());
        let req = bote::JsonRpcRequest::new(1, "nonexistent");
        let resp = dispatcher.dispatch(&req).unwrap();
        assert!(resp.error.is_some());
    }

    #[test]
    fn test_dispatcher_tool_error_is_mcp_error_content() {
        let dispatcher = build_dispatcher(ctx());
        let req = bote::JsonRpcRequest::new(1, "tools/call").with_params(json!({
            "name": "selah_ocr",
            "arguments": { "image_path": "../etc/passwd" }
        }));
        let resp = dispatcher.dispatch(&req).unwrap();
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("path traversal")
        );
    }

    #[test]
    fn test_tool_ocr_missing_path() {
        let result = tool_ocr(&ctx(), &json!({}));
        assert!(result.unwrap_err().contains("missing image_path"));
    }

    #[test]
    fn test_tool_ocr_local_and_unknown_engine() {
        let dir = TempDir::new();
        let img = dir.path("in.png");
        write_png(&img, 2, 2);
        let out = payload(&tool_ocr(&ctx(), &json!({ "image_path": img })).unwrap());
        assert_eq!(out["engine"], "local");
        assert_eq!(out["is_stub"], true);

        let err = tool_ocr(&ctx(), &json!({ "image_path": img, "engine": "magic" })).unwrap_err();
        assert!(err.contains("unknown OCR engine"));
    }

    #[test]
    fn test_tool_ocr_rejects_non_regular_file() {
        let dir = TempDir::new();
        let err = tool_ocr(&ctx(), &json!({ "image_path": dir.0.to_string_lossy() })).unwrap_err();
        assert!(err.contains("not a regular file"), "{err}");
    }

    #[test]
    fn test_tool_history_without_store() {
        let err = tool_history(&ctx(), &json!({ "limit": 5 })).unwrap_err();
        assert!(err.contains("unavailable"));
    }

    #[test]
    fn test_tool_history_lists_and_validates() {
        let dir = TempDir::new();
        let ctx = ctx_with_history(&dir);
        let store = ctx.history.as_ref().unwrap();
        for i in 0..3 {
            store
                .record(HistoryEntry {
                    id: uuid::Uuid::new_v4(),
                    path: format!("/tmp/{i}.png"),
                    timestamp: chrono::Utc::now(),
                    source: "full screen".into(),
                    width: 10,
                    height: 20,
                    format: "png".into(),
                })
                .unwrap();
        }
        let out = payload(&tool_history(&ctx, &json!({ "limit": 2 })).unwrap());
        let shots = out["screenshots"].as_array().unwrap();
        assert_eq!(shots.len(), 2);
        assert_eq!(shots[0]["dimensions"], "10x20");

        let err = tool_history(&ctx, &json!({ "since": "yesterday" })).unwrap_err();
        assert!(err.contains("invalid since"), "{err}");
        let err = tool_history(&ctx, &json!({ "limit": -1 })).unwrap_err();
        assert!(err.contains("non-negative"), "{err}");
        assert!(tool_history(&ctx, &json!({ "since": "2000-01-01T00:00:00Z" })).is_ok());
    }

    #[test]
    fn test_tool_annotate_missing_path() {
        let result = tool_annotate(&ctx(), &json!({ "annotations": [] }));
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_annotate_schema_shaped_input() {
        // Exactly what the advertised schema describes: no id, lowercase kind, no color
        let dir = TempDir::new();
        let img = dir.path("shot.png");
        write_png(&img, 8, 8);
        let out = payload(
            &tool_annotate(
                &ctx(),
                &json!({
                    "image_path": img,
                    "annotations": [
                        { "kind": "redaction", "position": { "x": 0, "y": 0, "width": 4, "height": 4 } },
                        { "kind": "Rectangle", "position": { "x": 1, "y": 1, "width": 5, "height": 5 },
                          "color": { "r": 0, "g": 0, "b": 255, "a": 255 } },
                        { "kind": "text", "position": { "x": 0, "y": 0, "width": 8, "height": 8 }, "text": "hi" }
                    ]
                }),
            )
            .unwrap(),
        );
        assert_eq!(out["annotation_count"], 3);
        // Default output sits next to the input
        assert_eq!(out["path"], dir.path("shot_annotated.png"));
        let rendered = image::open(dir.path("shot_annotated.png"))
            .unwrap()
            .to_rgba8();
        assert_eq!(rendered.get_pixel(0, 0).0, [0, 0, 0, 255]);
    }

    #[test]
    fn test_tool_annotate_output_format_from_extension() {
        let dir = TempDir::new();
        let img = dir.path("in.png");
        write_png(&img, 4, 4);
        let out_path = dir.path("out.jpg");
        let out = payload(
            &tool_annotate(
                &ctx(),
                &json!({ "image_path": img, "annotations": [], "output": out_path }),
            )
            .unwrap(),
        );
        assert_eq!(out["format"], "jpg");
        let bytes = std::fs::read(&out_path).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn test_tool_annotate_rejects_bad_annotations() {
        let dir = TempDir::new();
        let img = dir.path("in.png");
        write_png(&img, 4, 4);
        let bad_kind = json!({ "image_path": img, "annotations": [
            { "kind": "blob", "position": { "x": 0, "y": 0, "width": 1, "height": 1 } }
        ]});
        assert!(
            tool_annotate(&ctx(), &bad_kind)
                .unwrap_err()
                .contains("invalid annotation 0")
        );
        let bad_rect = json!({ "image_path": img, "annotations": [
            { "kind": "arrow", "position": { "x": 1e39, "y": 0, "width": 1, "height": 1 } }
        ]});
        assert!(tool_annotate(&ctx(), &bad_rect).is_err());
        let not_array = json!({ "image_path": img, "annotations": {} });
        assert!(
            tool_annotate(&ctx(), &not_array)
                .unwrap_err()
                .contains("array")
        );
    }

    #[test]
    fn test_tool_annotate_rejects_output_traversal() {
        let dir = TempDir::new();
        let img = dir.path("in.png");
        write_png(&img, 4, 4);
        let err = tool_annotate(
            &ctx(),
            &json!({ "image_path": img, "annotations": [], "output": "../../evil.png" }),
        )
        .unwrap_err();
        assert!(err.contains("path traversal"), "{err}");
    }

    #[test]
    fn test_tool_redact_missing_path() {
        let result = tool_redact(&ctx(), &json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_redact_rejects_output_traversal() {
        let dir = TempDir::new();
        let img = dir.path("in.png");
        write_png(&img, 4, 4);
        let err = tool_redact(
            &ctx(),
            &json!({ "image_path": img, "output": "a/../../b.png" }),
        )
        .unwrap_err();
        assert!(err.contains("path traversal"), "{err}");
    }

    #[test]
    fn test_tool_redact_metadata_matches_reported_and_stripped() {
        // PNG with an email address in trailing bytes: the stub OCR finds it
        // in the file bytes; the re-encoded output must not contain it.
        let dir = TempDir::new();
        let img = dir.path("leaky.png");
        write_png(&img, 4, 4);
        let mut bytes = std::fs::read(&img).unwrap();
        bytes.extend_from_slice(b"\x00contact admin@example.com\x00");
        std::fs::write(&img, bytes).unwrap();

        let out = payload(&tool_redact(&ctx(), &json!({ "image_path": img })).unwrap());
        assert!(out["warning"].as_str().unwrap().contains("stub"));
        assert_eq!(out["redactions"].as_array().unwrap().len(), 0);
        let meta = out["metadata_matches"].as_array().unwrap();
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0]["target_type"], "email");
        assert_eq!(meta[0]["matched_text"], "ad*************om");

        let written = std::fs::read(dir.path("leaky_redacted.png")).unwrap();
        assert!(
            !written.windows(17).any(|w| w == b"admin@example.com"),
            "embedded PII carried into the output"
        );
        assert!(image::load_from_memory(&written).is_ok());
    }

    #[test]
    fn test_inputs_must_be_images() {
        // Prevents reading arbitrary files (e.g. a private key) via OCR
        let dir = TempDir::new();
        let secret = dir.path("id_ed25519");
        std::fs::write(
            &secret,
            "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXk=\n",
        )
        .unwrap();
        for result in [
            tool_ocr(&ctx(), &json!({ "image_path": secret })),
            tool_redact(
                &ctx(),
                &json!({ "image_path": secret, "output": dir.path("o.png") }),
            ),
            tool_annotate(
                &ctx(),
                &json!({ "image_path": secret, "annotations": [], "output": dir.path("o.png") }),
            ),
        ] {
            let err = result.unwrap_err();
            assert!(err.contains("not a supported image"), "{err}");
            assert!(!err.contains("PRIVATE KEY"));
        }
    }

    #[test]
    fn test_outputs_must_be_image_files() {
        let dir = TempDir::new();
        let img = dir.path("in.png");
        write_png(&img, 4, 4);
        let target = dir.path(".bashrc");
        std::fs::write(&target, "export SAFE=1\n").unwrap();
        let err = tool_annotate(
            &ctx(),
            &json!({ "image_path": img, "annotations": [], "output": target }),
        )
        .unwrap_err();
        assert!(err.contains("image extension"), "{err}");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "export SAFE=1\n");

        // An existing directory (or device) named like an image is refused
        let dir_out = dir.path("d.png");
        std::fs::create_dir(&dir_out).unwrap();
        let err =
            tool_redact(&ctx(), &json!({ "image_path": img, "output": dir_out })).unwrap_err();
        assert!(err.contains("not a regular file"), "{err}");

        // Overwriting an existing regular image file is fine
        let existing = dir.path("prev.png");
        write_png(&existing, 1, 1);
        assert!(tool_redact(&ctx(), &json!({ "image_path": img, "output": existing })).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn test_output_symlink_refused() {
        let dir = TempDir::new();
        let img = dir.path("in.png");
        write_png(&img, 4, 4);
        let victim = dir.path("victim.txt");
        std::fs::write(&victim, "keep me").unwrap();
        let link = dir.path("out.png");
        std::os::unix::fs::symlink(&victim, &link).unwrap();
        let err = tool_annotate(
            &ctx(),
            &json!({ "image_path": img, "annotations": [], "output": link }),
        )
        .unwrap_err();
        assert!(err.contains("not a regular file"), "{err}");
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep me");
    }

    #[test]
    fn test_default_output_without_extension_gets_png() {
        let (out, fmt) = output_path(&json!({}), "/data/shot", "annotated").unwrap();
        assert_eq!(out, "/data/shot_annotated.png");
        assert_eq!(fmt, ImageFormat::Png);
        let (_, fmt) = output_path(&json!({}), "/data/a.webp", "redacted").unwrap();
        assert_eq!(fmt, ImageFormat::WebP);
    }

    #[test]
    fn test_tool_redact_invalid_target() {
        let dir = TempDir::new();
        let img = dir.path("in.png");
        write_png(&img, 4, 4);
        let err =
            tool_redact(&ctx(), &json!({ "image_path": img, "targets": ["ssn"] })).unwrap_err();
        assert!(err.contains("invalid redaction target"), "{err}");
    }

    #[test]
    fn test_tool_capture_validates_before_network() {
        // api_url points at a closed port: these must fail before any request
        let c = ctx();
        let err = tool_capture_sync(&c, &json!({ "format": "tiff" })).unwrap_err();
        assert!(err.contains("unsupported format"), "{err}");
        let err = tool_capture_sync(&c, &json!({ "output": "../x.png" })).unwrap_err();
        assert!(err.contains("path traversal"), "{err}");
        let err = tool_capture_sync(
            &c,
            &json!({ "region": { "x": 0, "y": 0, "width": 0, "height": 10 } }),
        )
        .unwrap_err();
        assert!(err.contains("invalid region"), "{err}");
        let err = tool_capture_sync(&c, &json!({ "region": { "x": 0 } })).unwrap_err();
        assert!(err.contains("invalid region"), "{err}");
        let err = tool_capture_sync(&c, &json!({ "output": "notes.txt" })).unwrap_err();
        assert!(err.contains("image extension"), "{err}");
        let err =
            tool_capture_sync(&c, &json!({ "output": "a.png", "format": "jpg" })).unwrap_err();
        assert!(err.contains("extension"), "{err}");
    }

    #[test]
    fn test_tool_capture_end_to_end_records_history() {
        let dir = TempDir::new();
        let img = image::RgbaImage::new(3, 2);
        let mut png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png.get_ref());
        let (url, rx) = serve_once(
            200,
            json!({ "image_data": b64, "width": 3, "height": 2, "format": "png" }).to_string(),
        );
        let mut ctx = ctx_with_history(&dir);
        ctx.api_url = url;

        let out_path = dir.path("cap.png");
        let out = payload(
            &tool_capture_sync(
                &ctx,
                &json!({ "output": out_path, "region": { "x": 1, "y": 2, "width": 3, "height": 4 } }),
            )
            .unwrap(),
        );
        assert_eq!(out["width"], 3);
        assert!(Path::new(&out_path).exists());
        assert!(Path::new(out["path"].as_str().unwrap()).is_absolute());
        assert_eq!(
            rx.recv().unwrap().json()["region"],
            json!({ "x": 1, "y": 2, "width": 3, "height": 4 })
        );

        let history = ctx.history.as_ref().unwrap().list(10, None).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].source, "region (3x4 at 1,2)");
    }

    #[test]
    fn test_run_async_inside_current_thread_runtime() {
        // Calling a sync handler from within a current-thread runtime used to
        // risk deadlock via Handle::block_on; it must just work.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let v = rt.block_on(async { run_async(async { Ok::<_, String>(42) }) });
        assert_eq!(v.unwrap(), 42);
    }

    #[test]
    fn test_validate_path_rejects_traversal() {
        assert!(validate_path("../etc/passwd").is_err());
        assert!(validate_path("foo/../../bar").is_err());
        assert!(validate_path("screenshot.png").is_ok());
        assert!(validate_path("/tmp/screenshot.png").is_ok());
        assert!(validate_path("subdir/file.png").is_ok());
    }

    #[test]
    fn test_validate_path_rejects_empty_and_nul() {
        assert!(validate_path("").is_err());
        assert!(validate_path("a\0b").is_err());
    }

    #[test]
    fn test_output_path_default_is_sibling_of_input() {
        let args = json!({});
        assert_eq!(
            output_path(&args, "/data/shots/a.png", "redacted")
                .unwrap()
                .0,
            "/data/shots/a_redacted.png"
        );
        assert_eq!(
            output_path(&args, "a.png", "annotated").unwrap().0,
            "a_annotated.png"
        );
        assert_eq!(
            output_path(&json!({ "output": "x.jpg" }), "a.png", "s").unwrap(),
            ("x.jpg".to_string(), ImageFormat::Jpeg)
        );
        assert!(output_path(&json!({ "output": 5 }), "a.png", "s").is_err());
    }

    #[test]
    fn test_mask_pii() {
        assert_eq!(mask_pii("user@example.com"), "us************om");
        assert_eq!(mask_pii("ab"), "**");
        assert_eq!(mask_pii("abcde"), "ab*de");
    }

    #[test]
    fn test_mask_pii_multibyte_does_not_panic() {
        // Byte slicing used to panic on these
        assert_eq!(mask_pii("aü@x.com"), "aü****om");
        assert_eq!(mask_pii("日本語メール"), "日本**ール");
        assert_eq!(mask_pii("ééé"), "***");
    }

    #[test]
    fn test_mask_pii_single_char() {
        assert_eq!(mask_pii("x"), "*");
    }

    #[test]
    fn test_mask_pii_empty() {
        assert_eq!(mask_pii(""), "");
    }

    #[test]
    fn test_mask_pii_four_chars() {
        assert_eq!(mask_pii("abcd"), "****");
    }

    #[test]
    fn test_mask_pii_five_chars() {
        assert_eq!(mask_pii("abcde"), "ab*de");
    }

    #[test]
    fn test_validate_path_absolute() {
        assert!(validate_path("/home/user/screenshot.png").is_ok());
    }

    #[test]
    fn test_validate_path_relative_subdir() {
        assert!(validate_path("output/file.png").is_ok());
    }

    #[test]
    fn test_validate_path_double_traversal() {
        assert!(validate_path("a/../../etc/shadow").is_err());
    }

    #[test]
    fn test_validate_path_current_dir() {
        assert!(validate_path("./screenshot.png").is_ok());
    }

    #[test]
    fn test_error_content_format() {
        let result = error_content("something went wrong");
        assert_eq!(result["isError"], true);
        let content = result["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "something went wrong");
    }
}
