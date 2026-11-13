//! Screen capture API client for Selah — connects to daimon.

use crate::core::{CaptureRegion, ImageFormat, Monitor};
use crate::error::SelahError;
use crate::geometry::Rect;
use crate::http;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

/// Client for daimon's screen capture API.
#[derive(Debug, Clone)]
pub struct CaptureClient {
    base_url: String,
    http: http::HttpClient,
}

/// Request body for `/v1/screen/capture`.
#[derive(Debug, Serialize)]
struct CaptureRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<RegionSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    monitor: Option<&'a str>,
    format: &'static str,
    agent_id: &'static str,
}

#[derive(Debug, Serialize, PartialEq)]
struct RegionSpec {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl RegionSpec {
    /// Convert a (possibly negative-size or partly off-screen) rect into a
    /// pixel region: normalized, clipped to the non-negative quadrant, with
    /// fractional edges rounded outward. Rejects non-finite and empty regions.
    fn from_rect(rect: &Rect) -> Result<Self, SelahError> {
        if !rect.is_finite() {
            return Err(SelahError::InvalidRegion(format!(
                "region coordinates must be finite (got {rect:?})"
            )));
        }
        let r = rect.normalized();
        let r = r.as_hisab();
        let x0 = clamp_to_u32(r.min.x.floor());
        let y0 = clamp_to_u32(r.min.y.floor());
        let x1 = clamp_to_u32(r.max.x.ceil());
        let y1 = clamp_to_u32(r.max.y.ceil());
        let spec = Self {
            x: x0,
            y: y0,
            width: x1.saturating_sub(x0),
            height: y1.saturating_sub(y0),
        };
        if spec.width == 0 || spec.height == 0 {
            return Err(SelahError::InvalidRegion(format!(
                "region must cover at least 1x1 on-screen pixel (got {}x{} at {},{})",
                rect.width(),
                rect.height(),
                rect.x(),
                rect.y()
            )));
        }
        Ok(spec)
    }
}

/// Response from `/v1/screen/capture`.
#[derive(Debug, Clone, Deserialize)]
pub struct CaptureResponse {
    pub image_data: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
}

/// Default request timeout (30 seconds).
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Agent identifier sent with capture requests.
const AGENT_ID: &str = "selah";

impl CaptureClient {
    /// Create a new capture client with a 30-second request timeout.
    #[must_use]
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: http::HttpClient::new(DEFAULT_TIMEOUT),
        }
    }

    /// Create a capture client with a custom reqwest client.
    #[must_use]
    pub fn with_client(base_url: &str, client: reqwest::Client) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: http::HttpClient::from_client(client),
        }
    }

    /// Get the base URL.
    #[must_use]
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
    ///
    /// Regions are normalized (negative sizes allowed) and clipped to
    /// non-negative coordinates; non-finite or empty regions are rejected
    /// with [`SelahError::InvalidRegion`].
    ///
    /// [`CaptureRegion::Window`] is rejected: the daimon capture API has no
    /// window selector, and silently falling back to a full-screen capture
    /// would expose every other window on screen.
    pub async fn capture(
        &self,
        region: &CaptureRegion,
        format: ImageFormat,
    ) -> Result<CaptureResponse, SelahError> {
        let region_spec = match region {
            CaptureRegion::FullScreen => None,
            CaptureRegion::Rect(r) => Some(RegionSpec::from_rect(r)?),
            CaptureRegion::Window(id) => {
                return Err(SelahError::InvalidRegion(format!(
                    "window capture ({id}) is not supported by the daimon capture API; \
                     capture the full screen or a region instead"
                )));
            }
        };

        self.post_capture(&CaptureRequest {
            region: region_spec,
            monitor: None,
            format: format.extension(),
            agent_id: AGENT_ID,
        })
        .await
    }

    /// List available monitors via daimon.
    pub async fn list_monitors(&self) -> Result<Vec<Monitor>, SelahError> {
        let url = format!("{}/v1/screen/monitors", self.base_url);
        let resp = self
            .http
            .client()?
            .get(&url)
            .send()
            .await
            .map_err(|e| SelahError::Api(format!("daimon monitors request failed: {e}")))?;
        let resp = http::check_status(resp, "daimon monitors").await?;
        http::read_json(resp, http::MAX_API_RESPONSE_BYTES, "daimon monitors").await
    }

    /// Capture a specific monitor by ID.
    pub async fn capture_monitor(
        &self,
        monitor_id: &str,
        format: ImageFormat,
    ) -> Result<CaptureResponse, SelahError> {
        if monitor_id.trim().is_empty() {
            return Err(SelahError::InvalidRegion(
                "monitor id must not be empty".into(),
            ));
        }
        self.post_capture(&CaptureRequest {
            region: None,
            monitor: Some(monitor_id),
            format: format.extension(),
            agent_id: AGENT_ID,
        })
        .await
    }

    async fn post_capture(&self, body: &CaptureRequest<'_>) -> Result<CaptureResponse, SelahError> {
        let url = format!("{}/v1/screen/capture", self.base_url);
        tracing::debug!(
            region = ?body.region,
            monitor = ?body.monitor,
            format = body.format,
            "requesting screen capture"
        );
        let resp = self
            .http
            .client()?
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| SelahError::Api(format!("daimon capture request failed: {e}")))?;
        let resp = http::check_status(resp, "daimon capture").await?;
        let capture: CaptureResponse =
            http::read_json(resp, http::MAX_CAPTURE_RESPONSE_BYTES, "daimon capture").await?;
        tracing::info!(
            width = capture.width,
            height = capture.height,
            format = %capture.format,
            "captured screenshot"
        );
        Ok(capture)
    }

    /// Save image data to a file.
    pub fn save_to_file(data: &[u8], path: &Path) -> Result<(), SelahError> {
        std::fs::write(path, data)?;
        tracing::debug!(path = %path.display(), bytes = data.len(), "saved capture");
        Ok(())
    }

    /// Copy image data to the system clipboard.
    ///
    /// Detects Wayland vs X11 via environment variables and uses
    /// `wl-copy` or `xclip` respectively. The MIME type is sniffed from the
    /// data (falling back to `image/png`).
    pub fn copy_to_clipboard(data: &[u8]) -> Result<(), SelahError> {
        use std::io::{Read, Write};
        use std::process::{Command, Stdio};

        let session_type = detect_session_type();
        let mime = image::guess_format(data)
            .map(|f| f.to_mime_type())
            .unwrap_or("image/png");
        tracing::debug!(?session_type, mime, "copying to clipboard");

        let (tool, mut cmd) = match session_type {
            SessionType::Wayland => {
                let mut c = Command::new("wl-copy");
                c.args(["--type", mime]);
                ("wl-copy (is wl-clipboard installed?)", c)
            }
            SessionType::X11 => {
                let mut c = Command::new("xclip");
                c.args(["-selection", "clipboard", "-t", mime]);
                ("xclip (is xclip installed?)", c)
            }
        };

        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| SelahError::CaptureFailed(format!("failed to run {tool}: {e}")))?;

        // Feed stdin from a separate thread so a tool that fills its stderr
        // pipe before draining stdin cannot deadlock us. Only the exit status
        // is awaited: both tools fork a background process that keeps serving
        // the selection and may hold the inherited pipes open.
        let stdin = child.stdin.take();
        let (write_result, status) = std::thread::scope(|s| {
            let writer = s.spawn(move || match stdin {
                Some(mut stdin) => stdin.write_all(data), // dropped here → EOF
                None => Ok(()),
            });
            let status = child.wait();
            (writer.join(), status)
        });
        let status =
            status.map_err(|e| SelahError::CaptureFailed(format!("clipboard tool failed: {e}")))?;

        if !status.success() {
            let mut stderr = String::new();
            if let Some(err) = child.stderr.take() {
                let _ = err.take(64 * 1024).read_to_string(&mut stderr);
            }
            return Err(SelahError::CaptureFailed(format!(
                "clipboard tool exited with {status}: {}",
                http::truncate(stderr.trim(), 1024)
            )));
        }
        match write_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                return Err(SelahError::CaptureFailed(format!(
                    "failed to write to clipboard tool stdin: {e}"
                )));
            }
            Err(_) => {
                return Err(SelahError::CaptureFailed(
                    "clipboard writer thread panicked".into(),
                ));
            }
        }

        tracing::info!(bytes = data.len(), mime, "copied image data to clipboard");
        Ok(())
    }
}

/// Clamp an f32 to a u32, treating negative values and NaN as 0.
#[inline]
fn clamp_to_u32(val: f32) -> u32 {
    // Float-to-int `as` casts saturate (and map NaN to 0).
    val as u32
}

/// Detected display session type.
///
/// Represents the windowing system protocol in use on the current session.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionType {
    /// The Wayland display protocol.
    Wayland,
    /// The X Window System (X11) protocol.
    X11,
}

/// Detect whether we're running under Wayland or X11.
#[must_use]
pub fn detect_session_type() -> SessionType {
    session_type_from(
        std::env::var_os("WAYLAND_DISPLAY").as_deref(),
        std::env::var_os("XDG_SESSION_TYPE").as_deref(),
    )
}

fn session_type_from(
    wayland_display: Option<&std::ffi::OsStr>,
    xdg_session_type: Option<&std::ffi::OsStr>,
) -> SessionType {
    if wayland_display.is_some_and(|d| !d.is_empty()) {
        return SessionType::Wayland;
    }
    if xdg_session_type.is_some_and(|t| t.eq_ignore_ascii_case("wayland")) {
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

    use crate::http::test_server::serve_once;

    fn png_response(w: u32, h: u32) -> String {
        let img = image::RgbaImage::new(w, h);
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, buf.get_ref());
        serde_json::json!({"image_data": b64, "width": w, "height": h, "format": "png"}).to_string()
    }

    #[test]
    fn test_clamp_to_u32_nan() {
        assert_eq!(clamp_to_u32(f32::NAN), 0);
    }

    #[test]
    fn test_region_spec_normalizes_and_clips() {
        // Negative size → normalized
        let spec = RegionSpec::from_rect(&Rect::new(100.0, 100.0, -50.0, -30.0)).unwrap();
        assert_eq!(
            spec,
            RegionSpec {
                x: 50,
                y: 70,
                width: 50,
                height: 30
            }
        );
        // Partly off-screen → clipped, not shifted
        let spec = RegionSpec::from_rect(&Rect::new(-10.0, -5.0, 100.0, 50.0)).unwrap();
        assert_eq!(
            spec,
            RegionSpec {
                x: 0,
                y: 0,
                width: 90,
                height: 45
            }
        );
        // Fractional edges round outward
        let spec = RegionSpec::from_rect(&Rect::new(0.5, 0.5, 1.0, 1.0)).unwrap();
        assert_eq!(
            spec,
            RegionSpec {
                x: 0,
                y: 0,
                width: 2,
                height: 2
            }
        );
    }

    #[test]
    fn test_region_spec_rejects_invalid() {
        for r in [
            Rect::new(0.0, 0.0, 0.0, 10.0),
            Rect::new(-100.0, 0.0, 50.0, 10.0), // entirely off-screen
            Rect::new(f32::NAN, 0.0, 10.0, 10.0),
            Rect::new(0.0, 0.0, f32::INFINITY, 10.0),
        ] {
            let err = RegionSpec::from_rect(&r).unwrap_err();
            assert!(matches!(err, SelahError::InvalidRegion(_)), "{r:?}: {err}");
        }
    }

    #[tokio::test]
    async fn test_capture_window_rejected_without_request() {
        // No server: if a request were sent, this would fail with an Api error instead
        let client = CaptureClient::new("http://127.0.0.1:9");
        let err = client
            .capture(&CaptureRegion::Window("firefox".into()), ImageFormat::Png)
            .await
            .unwrap_err();
        assert!(matches!(err, SelahError::InvalidRegion(_)), "{err}");
    }

    #[tokio::test]
    async fn test_capture_monitor_empty_id_rejected() {
        let client = CaptureClient::new("http://127.0.0.1:9");
        let err = client
            .capture_monitor(" ", ImageFormat::Png)
            .await
            .unwrap_err();
        assert!(matches!(err, SelahError::InvalidRegion(_)));
    }

    #[tokio::test]
    async fn test_capture_region_request_and_response() {
        let (url, rx) = serve_once(200, png_response(3, 2));
        let client = CaptureClient::new(&format!("{url}/"));
        let resp = client
            .capture(
                &CaptureRegion::Rect(Rect::new(10.0, 20.0, -5.0, 8.0)),
                ImageFormat::Jpeg,
            )
            .await
            .unwrap();
        assert_eq!((resp.width, resp.height), (3, 2));
        let png = crate::decode_image_data(&resp.image_data).unwrap();
        assert_eq!(image::load_from_memory(&png).unwrap().width(), 3);

        let req = rx.recv().unwrap();
        assert_eq!(req.request_line, "POST /v1/screen/capture HTTP/1.1");
        let body = req.json();
        assert_eq!(body["format"], "jpg");
        assert_eq!(body["agent_id"], "selah");
        assert_eq!(
            body["region"],
            serde_json::json!({"x": 5, "y": 20, "width": 5, "height": 8})
        );
        assert!(body.get("monitor").is_none());
    }

    #[tokio::test]
    async fn test_capture_monitor_request() {
        let (url, rx) = serve_once(200, png_response(1, 1));
        let client = CaptureClient::new(&url);
        client
            .capture_monitor("HDMI-A-1", ImageFormat::Png)
            .await
            .unwrap();
        let body = rx.recv().unwrap().json();
        assert_eq!(body["monitor"], "HDMI-A-1");
        assert!(body.get("region").is_none());
    }

    #[tokio::test]
    async fn test_capture_error_status() {
        let (url, _rx) = serve_once(403, r#"{"error":"permission denied"}"#);
        let err = CaptureClient::new(&url).capture_full().await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("daimon capture 403"), "{msg}");
        assert!(msg.contains("permission denied"), "{msg}");
    }

    #[tokio::test]
    async fn test_list_monitors() {
        let (url, rx) = serve_once(
            200,
            r#"[{"id":"DP-1","name":"Main","width":2560,"height":1440,"x":0,"y":0,"primary":true}]"#,
        );
        let monitors = CaptureClient::new(&url).list_monitors().await.unwrap();
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].id, "DP-1");
        assert!(monitors[0].primary);
        assert_eq!(
            rx.recv().unwrap().request_line,
            "GET /v1/screen/monitors HTTP/1.1"
        );
    }

    #[test]
    fn test_session_type_from_env() {
        use std::ffi::OsStr;
        let w = Some(OsStr::new("wayland-0"));
        assert_eq!(session_type_from(w, None), SessionType::Wayland);
        // Empty WAYLAND_DISPLAY does not count
        assert_eq!(
            session_type_from(Some(OsStr::new("")), None),
            SessionType::X11
        );
        assert_eq!(
            session_type_from(None, Some(OsStr::new("Wayland"))),
            SessionType::Wayland
        );
        assert_eq!(
            session_type_from(None, Some(OsStr::new("x11"))),
            SessionType::X11
        );
        assert_eq!(session_type_from(None, None), SessionType::X11);
    }
}
