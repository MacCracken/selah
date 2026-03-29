//! MCP server for Selah, powered by bote.
//!
//! Exposes 5 tools via the Model Context Protocol over stdio:
//! `selah_capture`, `selah_annotate`, `selah_ocr`, `selah_redact`, `selah_history`.

use std::collections::HashMap;
use std::sync::Arc;

use bote::{Dispatcher, ToolDef, ToolRegistry, ToolSchema};
use serde_json::Value;

/// Build a [`ToolRegistry`] containing all five selah tools.
fn build_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    registry.register(ToolDef::new(
        "selah_capture",
        "Take a screenshot via the daimon screen capture API",
        ToolSchema::new(
            "object",
            HashMap::from([
                (
                    "region".into(),
                    serde_json::json!({
                        "type": "object",
                        "description": "Capture region (omit for full screen)",
                        "properties": {
                            "x": { "type": "number" },
                            "y": { "type": "number" },
                            "width": { "type": "number" },
                            "height": { "type": "number" }
                        },
                        "required": ["x", "y", "width", "height"]
                    }),
                ),
                (
                    "format".into(),
                    serde_json::json!({
                        "type": "string",
                        "enum": ["png", "jpg", "bmp", "webp"],
                        "default": "png"
                    }),
                ),
                (
                    "output".into(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Output file path"
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
                    serde_json::json!({ "type": "string", "description": "Path to the source image" }),
                ),
                (
                    "annotations".into(),
                    serde_json::json!({
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "kind": { "type": "string" },
                                "position": {
                                    "type": "object",
                                    "properties": {
                                        "x": { "type": "number" },
                                        "y": { "type": "number" },
                                        "width": { "type": "number" },
                                        "height": { "type": "number" }
                                    }
                                },
                                "color": {
                                    "type": "object",
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
                    serde_json::json!({ "type": "string", "description": "Output file path" }),
                ),
            ]),
            vec!["image_path".into(), "annotations".into()],
        ),
    ));

    registry.register(ToolDef::new(
        "selah_ocr",
        "Extract text from an image",
        ToolSchema::new(
            "object",
            HashMap::from([
                (
                    "image_path".into(),
                    serde_json::json!({ "type": "string", "description": "Path to the image" }),
                ),
                (
                    "engine".into(),
                    serde_json::json!({
                        "type": "string",
                        "enum": ["local", "hoosh"],
                        "default": "local"
                    }),
                ),
            ]),
            vec!["image_path".into()],
        ),
    ));

    registry.register(ToolDef::new(
        "selah_redact",
        "Detect and redact sensitive content in an image",
        ToolSchema::new(
            "object",
            HashMap::from([
                (
                    "image_path".into(),
                    serde_json::json!({ "type": "string", "description": "Path to the image" }),
                ),
                (
                    "targets".into(),
                    serde_json::json!({
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["email", "phone", "credit_card", "ip_address"]
                        },
                        "description": "Types of sensitive data to redact (default: all)"
                    }),
                ),
                (
                    "output".into(),
                    serde_json::json!({ "type": "string", "description": "Output file path" }),
                ),
            ]),
            vec!["image_path".into()],
        ),
    ));

    registry.register(ToolDef::new(
        "selah_history",
        "List recent screenshots and their metadata",
        ToolSchema::new(
            "object",
            HashMap::from([
                (
                    "limit".into(),
                    serde_json::json!({
                        "type": "integer",
                        "default": 20,
                        "description": "Max results to return"
                    }),
                ),
                (
                    "since".into(),
                    serde_json::json!({
                        "type": "string",
                        "description": "ISO 8601 timestamp — only return captures after this time"
                    }),
                ),
            ]),
            vec![],
        ),
    ));

    registry
}

/// Build a [`Dispatcher`] with all tool handlers wired up.
fn build_dispatcher(api_url: &str) -> Dispatcher {
    let registry = build_registry();
    let mut dispatcher = Dispatcher::new(registry);

    // selah_capture — needs async (CaptureClient), so we block on the future.
    let api_url_owned = api_url.to_string();
    dispatcher.handle(
        "selah_capture",
        Arc::new(move |args: Value| -> Value {
            match tool_capture_sync(&args, &api_url_owned) {
                Ok(v) => v,
                Err(e) => error_content(&e),
            }
        }),
    );

    // selah_annotate
    dispatcher.handle(
        "selah_annotate",
        Arc::new(|args: Value| -> Value {
            match tool_annotate(&args) {
                Ok(v) => v,
                Err(e) => error_content(&e),
            }
        }),
    );

    // selah_ocr
    dispatcher.handle(
        "selah_ocr",
        Arc::new(|args: Value| -> Value {
            match tool_ocr(&args) {
                Ok(v) => v,
                Err(e) => error_content(&e),
            }
        }),
    );

    // selah_redact
    dispatcher.handle(
        "selah_redact",
        Arc::new(|args: Value| -> Value {
            match tool_redact(&args) {
                Ok(v) => v,
                Err(e) => error_content(&e),
            }
        }),
    );

    // selah_history
    dispatcher.handle(
        "selah_history",
        Arc::new(|args: Value| -> Value {
            match tool_history(&args) {
                Ok(v) => v,
                Err(e) => error_content(&e),
            }
        }),
    );

    dispatcher
}

/// Run the MCP server over stdio using bote's transport.
pub fn run_server(api_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dispatcher = build_dispatcher(api_url);
    bote::transport::stdio::run(&dispatcher)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helper: wrap an error string into MCP content format
// ---------------------------------------------------------------------------

fn error_content(msg: &str) -> Value {
    serde_json::json!({
        "content": [{
            "type": "text",
            "text": msg
        }],
        "isError": true
    })
}

// ---------------------------------------------------------------------------
// Path / PII helpers
// ---------------------------------------------------------------------------

/// Validate that a file path does not use path traversal components.
fn validate_path(path: &str) -> Result<&str, String> {
    let p = std::path::Path::new(path);
    for component in p.components() {
        if let std::path::Component::ParentDir = component {
            return Err(format!("path traversal not allowed: {path}"));
        }
    }
    Ok(path)
}

/// Mask a sensitive string for safe logging, keeping only the first and last 2 chars.
fn mask_pii(s: &str) -> String {
    if s.len() <= 4 {
        "*".repeat(s.len())
    } else {
        let first = &s[..2];
        let last = &s[s.len() - 2..];
        format!("{first}{}{last}", "*".repeat(s.len() - 4))
    }
}

// ---------------------------------------------------------------------------
// Tool handlers
// ---------------------------------------------------------------------------

/// Synchronous wrapper around the async capture logic.
fn tool_capture_sync(args: &Value, api_url: &str) -> Result<Value, String> {
    // We're inside a sync handler called by bote's stdio transport.
    // Use tokio's Handle::current() if a runtime is active, otherwise create one.
    let rt = match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            // A runtime is active — use block_on via a thread to avoid
            // blocking the current async context.
            return std::thread::scope(|s| {
                s.spawn(|| handle.block_on(tool_capture(args, api_url)))
                    .join()
                    .map_err(|_| "capture handler panicked".to_string())?
            });
        }
        Err(_) => tokio::runtime::Runtime::new().map_err(|e| e.to_string())?,
    };
    rt.block_on(tool_capture(args, api_url))
}

async fn tool_capture(args: &Value, api_url: &str) -> Result<Value, String> {
    let client = crate::capture::CaptureClient::new(api_url);
    let format_str = args.get("format").and_then(|v| v.as_str()).unwrap_or("png");
    let format: crate::core::ImageFormat = format_str.parse().unwrap_or_default();

    let region = if let Some(r) = args.get("region") {
        let x = r.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = r.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let w = r.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let h = r.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);
        crate::core::CaptureRegion::Rect(crate::geometry::Rect::new(
            x as f32, y as f32, w as f32, h as f32,
        ))
    } else {
        crate::core::CaptureRegion::FullScreen
    };

    let response = client
        .capture(&region, format)
        .await
        .map_err(|e| e.to_string())?;

    let data = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &response.image_data,
    )
    .map_err(|e| format!("failed to decode image data: {e}"))?;

    let output = args
        .get("output")
        .and_then(|v| v.as_str())
        .unwrap_or("screenshot.png");
    validate_path(output)?;

    crate::capture::CaptureClient::save_to_file(&data, std::path::Path::new(output))
        .map_err(|e| e.to_string())?;

    // Record in history
    if let Ok(store) = crate::history::HistoryStore::open_default() {
        let source = match &region {
            crate::core::CaptureRegion::FullScreen => "full screen".to_string(),
            crate::core::CaptureRegion::Rect(r) => {
                format!("region {}x{} at {},{}", r.width(), r.height(), r.x(), r.y())
            }
            crate::core::CaptureRegion::Window(w) => format!("window {w}"),
        };
        let _ = store.record(crate::history::HistoryEntry {
            id: uuid::Uuid::new_v4(),
            path: output.to_string(),
            timestamp: chrono::Utc::now(),
            source,
            width: response.width,
            height: response.height,
            format: format_str.to_string(),
        });
    }

    Ok(serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string(&serde_json::json!({
                "path": output,
                "width": response.width,
                "height": response.height,
                "format": format_str,
                "timestamp": chrono::Utc::now().to_rfc3339()
            })).unwrap()
        }]
    }))
}

fn tool_annotate(args: &Value) -> Result<Value, String> {
    let image_path = args
        .get("image_path")
        .and_then(|v| v.as_str())
        .ok_or("missing image_path")?;
    validate_path(image_path)?;

    let source =
        std::fs::read(image_path).map_err(|e| format!("failed to read {image_path}: {e}"))?;

    let annotations_json = args.get("annotations").ok_or("missing annotations")?;

    let annotations: Vec<crate::core::Annotation> =
        serde_json::from_value(annotations_json.clone())
            .map_err(|e| format!("invalid annotations: {e}"))?;

    let output = args
        .get("output")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::core::derive_output_path(image_path, "annotated"));

    let format = crate::core::ImageFormat::Png; // default
    let result = crate::annotate::AnnotationCanvas::render_to_image(&source, &annotations, format)
        .map_err(|e| e.to_string())?;

    std::fs::write(&output, &result).map_err(|e| format!("failed to write {output}: {e}"))?;

    Ok(serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string(&serde_json::json!({
                "path": output,
                "annotation_count": annotations_json.as_array().map(|a| a.len()).unwrap_or(0)
            })).unwrap()
        }]
    }))
}

fn tool_ocr(args: &Value) -> Result<Value, String> {
    let image_path = args
        .get("image_path")
        .and_then(|v| v.as_str())
        .ok_or("missing image_path")?;
    validate_path(image_path)?;

    let data =
        std::fs::read(image_path).map_err(|e| format!("failed to read {image_path}: {e}"))?;
    let result = crate::ai::extract_text_regions(&data);

    Ok(serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string(&serde_json::json!({
                "text": result.text,
                "confidence": result.confidence,
                "regions": result.bounding_boxes
            })).unwrap()
        }]
    }))
}

fn tool_redact(args: &Value) -> Result<Value, String> {
    let image_path = args
        .get("image_path")
        .and_then(|v| v.as_str())
        .ok_or("missing image_path")?;
    validate_path(image_path)?;

    let data =
        std::fs::read(image_path).map_err(|e| format!("failed to read {image_path}: {e}"))?;
    let ocr = crate::ai::extract_text_regions(&data);
    let suggestions = crate::ai::suggest_redactions(&ocr.text);

    // Skip zero-size regions (stub OCR doesn't produce real bounding boxes)
    let suggestions: Vec<_> = suggestions
        .into_iter()
        .filter(|s| s.region.area() > 0.0)
        .collect();

    // Filter by requested targets if specified
    let targets = args.get("targets").and_then(|v| v.as_array());
    let filtered: Vec<_> = if let Some(targets) = targets {
        let target_strs: Vec<&str> = targets.iter().filter_map(|v| v.as_str()).collect();
        suggestions
            .into_iter()
            .filter(|s| {
                let t = match &s.target_type {
                    crate::core::RedactionTarget::Email => "email",
                    crate::core::RedactionTarget::Phone => "phone",
                    crate::core::RedactionTarget::CreditCard => "credit_card",
                    crate::core::RedactionTarget::IpAddress => "ip_address",
                    crate::core::RedactionTarget::Custom(_) => "custom",
                };
                target_strs.contains(&t)
            })
            .collect()
    } else {
        suggestions
    };

    let output = args
        .get("output")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| crate::core::derive_output_path(image_path, "redacted"));

    // Build redaction annotations from suggestions
    let annotations: Vec<crate::core::Annotation> = filtered
        .iter()
        .map(|s| {
            crate::core::Annotation::new(
                crate::core::AnnotationKind::Redaction,
                s.region,
                crate::core::Color::BLACK,
            )
        })
        .collect();

    let result = crate::annotate::AnnotationCanvas::render_to_image(
        &data,
        &annotations,
        crate::core::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(&output, &result).map_err(|e| format!("failed to write {output}: {e}"))?;

    let redaction_info: Vec<Value> = filtered
        .iter()
        .map(|s| {
            serde_json::json!({
                "target_type": s.target_type.to_string(),
                "confidence": s.confidence,
                "matched_text": mask_pii(&s.matched_text)
            })
        })
        .collect();

    Ok(serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string(&serde_json::json!({
                "path": output,
                "redactions": redaction_info
            })).unwrap()
        }]
    }))
}

fn tool_history(args: &Value) -> Result<Value, String> {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let since = args
        .get("since")
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let store = crate::history::HistoryStore::open_default()
        .map_err(|e| format!("failed to open history: {e}"))?;

    let entries = store.list(limit, since).map_err(|e| e.to_string())?;

    let screenshots: Vec<Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id.to_string(),
                "path": e.path,
                "timestamp": e.timestamp.to_rfc3339(),
                "source": e.source,
                "dimensions": format!("{}x{}", e.width, e.height)
            })
        })
        .collect();

    Ok(serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string(&serde_json::json!({
                "screenshots": screenshots
            })).unwrap()
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let dispatcher = build_dispatcher("http://localhost:8090");
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
    fn test_dispatcher_initialize() {
        let dispatcher = build_dispatcher("http://localhost:8090");
        let req = bote::JsonRpcRequest::new(1, "initialize");
        let resp = dispatcher.dispatch(&req).unwrap();
        let result = resp.result.unwrap();
        assert!(result.get("protocolVersion").is_some());
        assert!(result.get("serverInfo").is_some());
    }

    #[test]
    fn test_tool_ocr_missing_path() {
        let args = serde_json::json!({});
        let result = tool_ocr(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing image_path"));
    }

    #[test]
    fn test_tool_history_empty() {
        let args = serde_json::json!({ "limit": 5 });
        let result = tool_history(&args);
        // Should succeed (empty history is fine)
        assert!(result.is_ok());
    }

    #[test]
    fn test_tool_annotate_missing_path() {
        let args = serde_json::json!({ "annotations": [] });
        let result = tool_annotate(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_redact_missing_path() {
        let args = serde_json::json!({});
        let result = tool_redact(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_dispatcher_unknown_method() {
        let dispatcher = build_dispatcher("http://localhost:8090");
        let req = bote::JsonRpcRequest::new(1, "nonexistent");
        let resp = dispatcher.dispatch(&req).unwrap();
        assert!(resp.error.is_some());
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
    fn test_mask_pii() {
        assert_eq!(mask_pii("user@example.com"), "us************om");
        assert_eq!(mask_pii("ab"), "**");
        assert_eq!(mask_pii("abcde"), "ab*de");
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
