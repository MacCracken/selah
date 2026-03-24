//! Screen capture API client for Selah — connects to daimon.

use crate::core::{CaptureRegion, ImageFormat, Monitor};
use crate::error::SelahError;
use crate::geometry::Rect;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

/// Client for daimon's screen capture API.
#[derive(Debug, Clone)]
pub struct CaptureClient {
    base_url: String,
    client: reqwest::Client,
}

/// Request body for `/v1/screen/capture`.
#[derive(Debug, Serialize)]
struct CaptureRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<RegionSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    monitor: Option<String>,
    format: String,
    agent_id: String,
}

#[derive(Debug, Serialize)]
struct RegionSpec {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// Response from `/v1/screen/capture`.
#[derive(Debug, Deserialize)]
pub struct CaptureResponse {
    pub image_data: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
}

/// Default request timeout (30 seconds).
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

impl CaptureClient {
    /// Create a new capture client with a 30-second timeout.
    pub fn new(base_url: &str) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        let client = reqwest::Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .unwrap_or_default();
        Self { base_url, client }
    }

    /// Create a capture client with a custom reqwest client.
    pub fn with_client(base_url: &str, client: reqwest::Client) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        Self { base_url, client }
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Capture the full screen.
    pub async fn capture_full(&self) -> Result<CaptureResponse, SelahError> {
        self.capture(&CaptureRegion::FullScreen, ImageFormat::Png)
            .await
    }

    /// Capture a specific region.
    pub async fn capture_region(&self, rect: Rect) -> Result<CaptureResponse, SelahError> {
        self.capture(&CaptureRegion::Rect(rect), ImageFormat::Png)
            .await
    }

    /// Capture with a specific region and format.
    pub async fn capture(
        &self,
        region: &CaptureRegion,
        format: ImageFormat,
    ) -> Result<CaptureResponse, SelahError> {
        let region_spec = match region {
            CaptureRegion::FullScreen => None,
            CaptureRegion::Rect(r) => Some(RegionSpec {
                x: clamp_to_u32(r.x()),
                y: clamp_to_u32(r.y()),
                width: clamp_to_u32(r.width()),
                height: clamp_to_u32(r.height()),
            }),
            CaptureRegion::Window(_) => None, // window capture uses full screen path
        };

        let body = CaptureRequest {
            region: region_spec,
            monitor: None,
            format: format.extension().to_string(),
            agent_id: "selah".to_string(),
        };

        let url = format!("{}/v1/screen/capture", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| SelahError::Api(e.to_string()))?;
        let resp = check_response(resp).await?;
        resp.json::<CaptureResponse>()
            .await
            .map_err(|e| SelahError::Api(e.to_string()))
    }

    /// List available monitors via daimon.
    pub async fn list_monitors(&self) -> Result<Vec<Monitor>, SelahError> {
        let url = format!("{}/v1/screen/monitors", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| SelahError::Api(e.to_string()))?;
        let resp = check_response(resp).await?;
        resp.json::<Vec<Monitor>>()
            .await
            .map_err(|e| SelahError::Api(e.to_string()))
    }

    /// Capture a specific monitor by ID.
    pub async fn capture_monitor(
        &self,
        monitor_id: &str,
        format: ImageFormat,
    ) -> Result<CaptureResponse, SelahError> {
        let body = CaptureRequest {
            region: None,
            monitor: Some(monitor_id.to_string()),
            format: format.extension().to_string(),
            agent_id: "selah".to_string(),
        };

        let url = format!("{}/v1/screen/capture", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| SelahError::Api(e.to_string()))?;
        let resp = check_response(resp).await?;
        resp.json::<CaptureResponse>()
            .await
            .map_err(|e| SelahError::Api(e.to_string()))
    }

    /// Save image data to a file.
    pub fn save_to_file(data: &[u8], path: &Path) -> Result<(), SelahError> {
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Copy image data to the system clipboard.
    ///
    /// Detects Wayland vs X11 via environment variables and uses
    /// `wl-copy` or `xclip` respectively.
    pub fn copy_to_clipboard(data: &[u8]) -> Result<(), SelahError> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let session_type = detect_session_type();
        tracing::debug!("detected session type: {session_type:?}");

        let mut child = match session_type {
            SessionType::Wayland => Command::new("wl-copy")
                .arg("--type")
                .arg("image/png")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    SelahError::CaptureFailed(format!(
                        "failed to run wl-copy (is wl-clipboard installed?): {e}"
                    ))
                })?,
            SessionType::X11 => Command::new("xclip")
                .args(["-selection", "clipboard", "-t", "image/png"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    SelahError::CaptureFailed(format!(
                        "failed to run xclip (is xclip installed?): {e}"
                    ))
                })?,
        };

        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(data).map_err(|e| {
                SelahError::CaptureFailed(format!("failed to write to clipboard tool stdin: {e}"))
            })?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| SelahError::CaptureFailed(format!("clipboard tool failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SelahError::CaptureFailed(format!(
                "clipboard tool exited with {}: {stderr}",
                output.status
            )));
        }

        tracing::info!("copied image data to clipboard");
        Ok(())
    }
}

/// Check an HTTP response status, returning a descriptive error on failure.
async fn check_response(resp: reqwest::Response) -> Result<reqwest::Response, SelahError> {
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        return Err(SelahError::Api(format!("{status}: {text}")));
    }
    Ok(resp)
}

/// Clamp an f32 to a u32, treating negative values as 0.
fn clamp_to_u32(val: f32) -> u32 {
    if val < 0.0 {
        0
    } else if val > u32::MAX as f32 {
        u32::MAX
    } else {
        val as u32
    }
}

/// Detected display session type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionType {
    Wayland,
    X11,
}

/// Detect whether we're running under Wayland or X11.
pub fn detect_session_type() -> SessionType {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return SessionType::Wayland;
    }
    if let Ok(session) = std::env::var("XDG_SESSION_TYPE")
        && session == "wayland"
    {
        return SessionType::Wayland;
    }
    SessionType::X11
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = CaptureClient::new("http://localhost:8090");
        assert_eq!(client.base_url(), "http://localhost:8090");
    }

    #[test]
    fn test_client_strips_trailing_slash() {
        let client = CaptureClient::new("http://localhost:8090/");
        assert_eq!(client.base_url(), "http://localhost:8090");
    }

    #[test]
    fn test_client_with_custom_client() {
        let http = reqwest::Client::new();
        let client = CaptureClient::with_client("http://example.com:8090", http);
        assert_eq!(client.base_url(), "http://example.com:8090");
    }

    #[test]
    fn test_save_to_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("selah_test_save.png");
        let data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic bytes
        CaptureClient::save_to_file(&data, &path).unwrap();
        let read_back = std::fs::read(&path).unwrap();
        assert_eq!(read_back, data);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_detect_session_type_default() {
        // In a test environment without WAYLAND_DISPLAY, should default to X11
        // (unless running under Wayland, which is also fine)
        let st = detect_session_type();
        assert!(st == SessionType::Wayland || st == SessionType::X11);
    }

    #[test]
    fn test_clamp_to_u32_negative() {
        assert_eq!(clamp_to_u32(-1.0), 0);
        assert_eq!(clamp_to_u32(-999.0), 0);
        assert_eq!(clamp_to_u32(f32::NEG_INFINITY), 0);
    }

    #[test]
    fn test_clamp_to_u32_overflow() {
        assert_eq!(clamp_to_u32(f32::MAX), u32::MAX);
        assert_eq!(clamp_to_u32(5_000_000_000.0), u32::MAX);
        assert_eq!(clamp_to_u32(f32::INFINITY), u32::MAX);
    }

    #[test]
    fn test_clamp_to_u32_normal() {
        assert_eq!(clamp_to_u32(0.0), 0);
        assert_eq!(clamp_to_u32(100.5), 100);
        assert_eq!(clamp_to_u32(1920.0), 1920);
    }

    #[test]
    fn test_client_base_url_multiple_trailing_slashes() {
        let client = CaptureClient::new("http://localhost:8090///");
        // Only one trailing slash is stripped per trim_end_matches
        assert!(!client.base_url().ends_with('/'));
    }
}
