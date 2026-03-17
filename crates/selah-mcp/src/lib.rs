//! selah-mcp — MCP JSON-RPC server for Selah.
//!
//! Implements the Model Context Protocol over stdio, exposing 5 tools:
//! `selah_capture`, `selah_annotate`, `selah_ocr`, `selah_redact`, `selah_history`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Write};

/// MCP JSON-RPC request.
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// MCP JSON-RPC response.
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

/// MCP tool definition.
#[derive(Debug, Serialize)]
struct ToolDef {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

/// Run the MCP server, reading JSON-RPC from stdin and writing to stdout.
pub async fn run_server(api_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("parse error: {e}"),
                    }),
                };
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
                continue;
            }
        };

        if request.jsonrpc != "2.0" {
            let resp = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id.unwrap_or(Value::Null),
                result: None,
                error: Some(JsonRpcError {
                    code: -32600,
                    message: "invalid jsonrpc version".to_string(),
                }),
            };
            writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
            stdout.flush()?;
            continue;
        }

        let id = request.id.unwrap_or(Value::Null);
        let result = handle_method(&request.method, &request.params, api_url).await;

        let resp = match result {
            Ok(value) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(value),
                error: None,
            },
            Err(e) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32603,
                    message: e,
                }),
            },
        };

        writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
        stdout.flush()?;
    }

    Ok(())
}

async fn handle_method(method: &str, params: &Value, api_url: &str) -> Result<Value, String> {
    match method {
        "initialize" => handle_initialize(),
        "tools/list" => handle_tools_list(),
        "tools/call" => handle_tools_call(params, api_url).await,
        _ => Err(format!("unknown method: {method}")),
    }
}

fn handle_initialize() -> Result<Value, String> {
    Ok(serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "selah",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

fn handle_tools_list() -> Result<Value, String> {
    let tools = vec![
        ToolDef {
            name: "selah_capture".to_string(),
            description: "Take a screenshot via the daimon screen capture API".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "region": {
                        "type": "object",
                        "description": "Capture region (omit for full screen)",
                        "properties": {
                            "x": { "type": "number" },
                            "y": { "type": "number" },
                            "width": { "type": "number" },
                            "height": { "type": "number" }
                        },
                        "required": ["x", "y", "width", "height"]
                    },
                    "format": {
                        "type": "string",
                        "enum": ["png", "jpg", "bmp", "webp"],
                        "default": "png"
                    },
                    "output": {
                        "type": "string",
                        "description": "Output file path"
                    }
                }
            }),
        },
        ToolDef {
            name: "selah_annotate".to_string(),
            description: "Add annotations to an image".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "image_path": { "type": "string", "description": "Path to the source image" },
                    "annotations": {
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
                    },
                    "output": { "type": "string", "description": "Output file path" }
                },
                "required": ["image_path", "annotations"]
            }),
        },
        ToolDef {
            name: "selah_ocr".to_string(),
            description: "Extract text from an image".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "image_path": { "type": "string", "description": "Path to the image" },
                    "engine": {
                        "type": "string",
                        "enum": ["local", "hoosh"],
                        "default": "local"
                    }
                },
                "required": ["image_path"]
            }),
        },
        ToolDef {
            name: "selah_redact".to_string(),
            description: "Detect and redact sensitive content in an image".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "image_path": { "type": "string", "description": "Path to the image" },
                    "targets": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["email", "phone", "credit_card", "ip_address"]
                        },
                        "description": "Types of sensitive data to redact (default: all)"
                    },
                    "output": { "type": "string", "description": "Output file path" }
                },
                "required": ["image_path"]
            }),
        },
        ToolDef {
            name: "selah_history".to_string(),
            description: "List recent screenshots and their metadata".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "default": 20,
                        "description": "Max results to return"
                    },
                    "since": {
                        "type": "string",
                        "description": "ISO 8601 timestamp — only return captures after this time"
                    }
                }
            }),
        },
    ];

    Ok(serde_json::json!({ "tools": tools }))
}

async fn handle_tools_call(params: &Value, api_url: &str) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("missing tool name")?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(Value::Object(serde_json::Map::new()));

    match name {
        "selah_capture" => tool_capture(&arguments, api_url).await,
        "selah_annotate" => tool_annotate(&arguments),
        "selah_ocr" => tool_ocr(&arguments),
        "selah_redact" => tool_redact(&arguments),
        "selah_history" => tool_history(&arguments),
        _ => Err(format!("unknown tool: {name}")),
    }
}

async fn tool_capture(args: &Value, api_url: &str) -> Result<Value, String> {
    let client = selah_capture::CaptureClient::new(api_url);
    let format_str = args.get("format").and_then(|v| v.as_str()).unwrap_or("png");
    let format = match format_str {
        "jpg" | "jpeg" => selah_core::ImageFormat::Jpeg,
        "bmp" => selah_core::ImageFormat::Bmp,
        "webp" => selah_core::ImageFormat::WebP,
        _ => selah_core::ImageFormat::Png,
    };

    let region = if let Some(r) = args.get("region") {
        let x = r.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = r.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let w = r.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let h = r.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);
        selah_core::CaptureRegion::Rect(selah_core::Rect::new(x, y, w, h))
    } else {
        selah_core::CaptureRegion::FullScreen
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

    selah_capture::CaptureClient::save_to_file(&data, std::path::Path::new(output), format)
        .map_err(|e| e.to_string())?;

    // Record in history
    if let Ok(store) = selah_capture::history::HistoryStore::open_default() {
        let source = match &region {
            selah_core::CaptureRegion::FullScreen => "full screen".to_string(),
            selah_core::CaptureRegion::Rect(r) => {
                format!("region {}x{} at {},{}", r.width, r.height, r.x, r.y)
            }
            selah_core::CaptureRegion::Window(w) => format!("window {w}"),
        };
        let _ = store.record(selah_capture::history::HistoryEntry {
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

    let source =
        std::fs::read(image_path).map_err(|e| format!("failed to read {image_path}: {e}"))?;

    let annotations_json = args.get("annotations").ok_or("missing annotations")?;

    let annotations: Vec<selah_core::Annotation> = serde_json::from_value(annotations_json.clone())
        .map_err(|e| format!("invalid annotations: {e}"))?;

    let output = args
        .get("output")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let p = std::path::Path::new(image_path);
            let stem = p.file_stem().unwrap_or_default().to_string_lossy();
            let ext = p.extension().unwrap_or_default().to_string_lossy();
            format!("{stem}_annotated.{ext}")
        });

    let format = selah_core::ImageFormat::Png; // default
    let result = selah_annotate::AnnotationCanvas::render_to_image(&source, &annotations, format)
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

    let data =
        std::fs::read(image_path).map_err(|e| format!("failed to read {image_path}: {e}"))?;
    let result = selah_ai::extract_text_regions(&data);

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

    let data =
        std::fs::read(image_path).map_err(|e| format!("failed to read {image_path}: {e}"))?;
    let ocr = selah_ai::extract_text_regions(&data);
    let suggestions = selah_ai::suggest_redactions(&ocr.text);

    // Filter by requested targets if specified
    let targets = args.get("targets").and_then(|v| v.as_array());
    let filtered: Vec<_> = if let Some(targets) = targets {
        let target_strs: Vec<&str> = targets.iter().filter_map(|v| v.as_str()).collect();
        suggestions
            .into_iter()
            .filter(|s| {
                let t = match &s.target_type {
                    selah_core::RedactionTarget::Email => "email",
                    selah_core::RedactionTarget::Phone => "phone",
                    selah_core::RedactionTarget::CreditCard => "credit_card",
                    selah_core::RedactionTarget::IpAddress => "ip_address",
                    selah_core::RedactionTarget::Custom(_) => "custom",
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
        .unwrap_or_else(|| {
            let p = std::path::Path::new(image_path);
            let stem = p.file_stem().unwrap_or_default().to_string_lossy();
            let ext = p.extension().unwrap_or_default().to_string_lossy();
            format!("{stem}_redacted.{ext}")
        });

    // Build redaction annotations from suggestions
    let annotations: Vec<selah_core::Annotation> = filtered
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
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(&output, &result).map_err(|e| format!("failed to write {output}: {e}"))?;

    let redaction_info: Vec<Value> = filtered
        .iter()
        .map(|s| {
            serde_json::json!({
                "target_type": s.target_type.to_string(),
                "confidence": s.confidence,
                "matched_text": s.matched_text
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

    let store = selah_capture::history::HistoryStore::open_default()
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
    fn test_tools_list_returns_five_tools() {
        let result = handle_tools_list().unwrap();
        let tools = result.get("tools").unwrap().as_array().unwrap();
        assert_eq!(tools.len(), 5);

        let names: Vec<&str> = tools
            .iter()
            .map(|t| t.get("name").unwrap().as_str().unwrap())
            .collect();
        assert!(names.contains(&"selah_capture"));
        assert!(names.contains(&"selah_annotate"));
        assert!(names.contains(&"selah_ocr"));
        assert!(names.contains(&"selah_redact"));
        assert!(names.contains(&"selah_history"));
    }

    #[test]
    fn test_initialize() {
        let result = handle_initialize().unwrap();
        assert!(result.get("protocolVersion").is_some());
        assert!(result.get("serverInfo").is_some());
        let name = result["serverInfo"]["name"].as_str().unwrap();
        assert_eq!(name, "selah");
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

    #[tokio::test]
    async fn test_unknown_method() {
        let result = handle_method("nonexistent", &Value::Null, "http://localhost:8090").await;
        assert!(result.is_err());
    }
}
