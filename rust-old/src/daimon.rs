//! Daimon and Hoosh integration for Selah.
//!
//! Provides client configuration and API access for:
//! - **Daimon**: Agent lifecycle, screen capture orchestration
//! - **Hoosh**: LLM vision inference for real OCR

use crate::SelahError;
use crate::geometry::Rect;
use crate::http;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for connecting to the daimon agent runtime.
#[derive(Clone, Serialize, Deserialize)]
pub struct DaimonConfig {
    /// Daimon API endpoint.
    pub endpoint: String,
    /// Optional API key, sent as an `Authorization: Bearer` header.
    pub api_key: Option<String>,
}

impl std::fmt::Debug for DaimonConfig {
    // Never print the API key (e.g. in logs or panic messages).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DaimonConfig")
            .field("endpoint", &self.endpoint)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl Default for DaimonConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8090".to_string(),
            api_key: None,
        }
    }
}

/// Configuration for connecting to hoosh LLM inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooshConfig {
    /// Hoosh API endpoint.
    pub endpoint: String,
}

impl Default for HooshConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8088".to_string(),
        }
    }
}

/// Maximum accepted agent ID length.
const MAX_AGENT_ID_LEN: usize = 256;

/// Build `endpoint` + path `segments`, percent-encoding each segment so
/// caller-supplied values (like agent IDs) cannot alter the path.
fn endpoint_url(endpoint: &str, segments: &[&str]) -> Result<reqwest::Url, SelahError> {
    let mut url = reqwest::Url::parse(endpoint)
        .map_err(|e| SelahError::Api(format!("invalid endpoint {endpoint:?}: {e}")))?;
    url.path_segments_mut()
        .map_err(|()| SelahError::Api(format!("endpoint {endpoint:?} cannot have a path")))?
        .pop_if_empty()
        .extend(segments);
    Ok(url)
}

fn validate_agent_id(agent_id: &str) -> Result<(), SelahError> {
    if agent_id.is_empty()
        || agent_id == "."
        || agent_id == ".."
        || agent_id.len() > MAX_AGENT_ID_LEN
        || agent_id.chars().any(char::is_control)
    {
        return Err(SelahError::Api(format!(
            "invalid agent id {:?}",
            http::truncate(agent_id, 64)
        )));
    }
    Ok(())
}

/// Client for daimon agent runtime interactions.
///
/// Handles agent registration, heartbeat, and orchestration of screen
/// capture workflows through the daimon API.
#[derive(Debug, Clone)]
pub struct DaimonClient {
    config: DaimonConfig,
    http: http::HttpClient,
}

impl DaimonClient {
    /// Create a new daimon client with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(DaimonConfig::default())
    }

    /// Create a daimon client with custom configuration.
    #[must_use]
    pub fn with_config(mut config: DaimonConfig) -> Self {
        config.endpoint = config.endpoint.trim_end_matches('/').to_string();
        let http = http::HttpClient::new(Duration::from_secs(30));
        Self { config, http }
    }

    /// Get the configured endpoint.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    fn post(&self, segments: &[&str]) -> Result<reqwest::RequestBuilder, SelahError> {
        let url = endpoint_url(&self.config.endpoint, segments)?;
        let req = self.http.client()?.post(url);
        Ok(match &self.config.api_key {
            Some(key) => req.bearer_auth(key),
            None => req,
        })
    }

    /// Register this agent with daimon.
    ///
    /// Returns the assigned agent ID on success.
    pub async fn register_agent(&self, name: &str) -> Result<String, SelahError> {
        #[derive(Deserialize)]
        struct RegisterResponse {
            agent_id: String,
        }

        let body = serde_json::json!({
            "name": name,
            "type": "selah",
            "capabilities": ["screenshot", "annotate", "ocr", "redact"]
        });

        let resp = self
            .post(&["v1", "agents", "register"])?
            .json(&body)
            .send()
            .await
            .map_err(|e| SelahError::Api(format!("daimon register failed: {e}")))?;
        let resp = http::check_status(resp, "daimon register").await?;
        let RegisterResponse { agent_id } =
            http::read_json(resp, http::MAX_API_RESPONSE_BYTES, "daimon register").await?;
        validate_agent_id(&agent_id)?;
        tracing::info!(%agent_id, "registered with daimon");
        Ok(agent_id)
    }

    /// Send a heartbeat to daimon.
    pub async fn heartbeat(&self, agent_id: &str) -> Result<(), SelahError> {
        validate_agent_id(agent_id)?;
        let resp = self
            .post(&["v1", "agents", agent_id, "heartbeat"])?
            .send()
            .await
            .map_err(|e| SelahError::Api(format!("daimon heartbeat failed: {e}")))?;
        http::check_status(resp, "daimon heartbeat").await?;
        tracing::debug!(%agent_id, "daimon heartbeat ok");
        Ok(())
    }
}

impl Default for DaimonClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Client for hoosh LLM vision inference.
///
/// Used for real OCR via vision models, replacing the stub byte-scanner.
#[derive(Debug, Clone)]
pub struct HooshClient {
    config: HooshConfig,
    http: http::HttpClient,
}

impl HooshClient {
    /// Create a new hoosh client with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(HooshConfig::default())
    }

    /// Create a hoosh client with custom configuration.
    #[must_use]
    pub fn with_config(mut config: HooshConfig) -> Self {
        config.endpoint = config.endpoint.trim_end_matches('/').to_string();
        let http = http::HttpClient::new(Duration::from_secs(60));
        Self { config, http }
    }

    /// Get the configured endpoint.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    /// Extract text from an image using LLM vision.
    ///
    /// Sends the image to hoosh for real OCR via a vision model.
    /// Returns extracted text with bounding boxes. Confidence is clamped to
    /// `0.0..=1.0`; non-finite or empty regions are dropped and negative
    /// sizes are normalized.
    pub async fn ocr(&self, image_data: &[u8]) -> Result<crate::OcrResult, SelahError> {
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, image_data);
        let url = endpoint_url(&self.config.endpoint, &["v1", "vision", "ocr"])?;
        let body = serde_json::json!({
            "image": b64,
            "task": "ocr",
        });

        let resp = self
            .http
            .client()?
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| SelahError::Api(format!("hoosh OCR failed: {e}")))?;
        let resp = http::check_status(resp, "hoosh OCR").await?;
        let result: serde_json::Value =
            http::read_json(resp, http::MAX_API_RESPONSE_BYTES, "hoosh OCR").await?;

        let text = result["text"].as_str().unwrap_or_default().to_string();
        let confidence = result["confidence"]
            .as_f64()
            .filter(|c| c.is_finite())
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);

        let bounding_boxes: Vec<Rect> = result["regions"]
            .as_array()
            .map(|arr| arr.iter().filter_map(parse_region).collect())
            .unwrap_or_default();

        tracing::debug!(
            text_bytes = text.len(),
            regions = bounding_boxes.len(),
            confidence,
            "hoosh OCR complete"
        );
        Ok(crate::OcrResult {
            text,
            confidence,
            bounding_boxes,
            is_stub: false,
        })
    }
}

/// Parse one `{x, y, width, height}` OCR region, rejecting unusable boxes.
fn parse_region(r: &serde_json::Value) -> Option<Rect> {
    let rect = Rect::new(
        r["x"].as_f64()? as f32,
        r["y"].as_f64()? as f32,
        r["width"].as_f64()? as f32,
        r["height"].as_f64()? as f32,
    );
    (rect.is_finite() && rect.area() > 0.0).then(|| rect.normalized())
}

impl Default for HooshClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daimon_config_default() {
        let config = DaimonConfig::default();
        assert_eq!(config.endpoint, "http://localhost:8090");
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_hoosh_config_default() {
        let config = HooshConfig::default();
        assert_eq!(config.endpoint, "http://localhost:8088");
    }

    #[test]
    fn test_daimon_client_endpoint() {
        let client = DaimonClient::new();
        assert_eq!(client.endpoint(), "http://localhost:8090");
    }

    #[test]
    fn test_daimon_client_custom_config() {
        let config = DaimonConfig {
            endpoint: "http://custom:9090".to_string(),
            api_key: Some("key123".to_string()),
        };
        let client = DaimonClient::with_config(config);
        assert_eq!(client.endpoint(), "http://custom:9090");
    }

    #[test]
    fn test_hoosh_client_endpoint() {
        let client = HooshClient::new();
        assert_eq!(client.endpoint(), "http://localhost:8088");
    }

    #[test]
    fn test_daimon_config_serde_roundtrip() {
        let config = DaimonConfig {
            endpoint: "http://localhost:8090".into(),
            api_key: Some("secret".into()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: DaimonConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.endpoint, config.endpoint);
        assert_eq!(parsed.api_key, config.api_key);
    }

    #[test]
    fn test_hoosh_config_serde_roundtrip() {
        let config = HooshConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: HooshConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.endpoint, config.endpoint);
    }

    #[test]
    fn test_daimon_client_default_trait() {
        let client = DaimonClient::default();
        assert_eq!(client.endpoint(), "http://localhost:8090");
    }

    #[test]
    fn test_hoosh_client_default_trait() {
        let client = HooshClient::default();
        assert_eq!(client.endpoint(), "http://localhost:8088");
    }

    #[test]
    fn test_hoosh_client_custom_config() {
        let config = HooshConfig {
            endpoint: "http://custom:9999".to_string(),
        };
        let client = HooshClient::with_config(config);
        assert_eq!(client.endpoint(), "http://custom:9999");
    }

    use crate::http::test_server::serve_once;

    #[test]
    fn test_daimon_config_debug_redacts_api_key() {
        let config = DaimonConfig {
            endpoint: "http://x".into(),
            api_key: Some("super-secret".into()),
        };
        let dbg = format!("{config:?}");
        assert!(!dbg.contains("super-secret"), "{dbg}");
        assert!(dbg.contains("<redacted>"));
        let client = DaimonClient::with_config(config);
        assert!(!format!("{client:?}").contains("super-secret"));
    }

    #[test]
    fn test_endpoint_trailing_slash_trimmed() {
        let client = DaimonClient::with_config(DaimonConfig {
            endpoint: "http://localhost:8090/".into(),
            api_key: None,
        });
        assert_eq!(client.endpoint(), "http://localhost:8090");
        let hoosh = HooshClient::with_config(HooshConfig {
            endpoint: "http://localhost:8088//".into(),
        });
        assert_eq!(hoosh.endpoint(), "http://localhost:8088");
    }

    #[test]
    fn test_endpoint_url_encodes_segments() {
        let url = endpoint_url(
            "http://h:1/base/",
            &["v1", "agents", "a/b?c#d", "heartbeat"],
        )
        .unwrap();
        assert_eq!(
            url.as_str(),
            "http://h:1/base/v1/agents/a%2Fb%3Fc%23d/heartbeat"
        );
        assert!(endpoint_url("not a url", &["v1"]).is_err());
        assert!(endpoint_url("mailto:x@y.z", &["v1"]).is_err());
    }

    #[test]
    fn test_validate_agent_id() {
        assert!(validate_agent_id("agent-123").is_ok());
        for bad in ["", ".", "..", "a\nb"] {
            assert!(validate_agent_id(bad).is_err(), "{bad:?}");
        }
        assert!(validate_agent_id(&"a".repeat(MAX_AGENT_ID_LEN + 1)).is_err());
    }

    #[test]
    fn test_parse_region() {
        let ok =
            parse_region(&serde_json::json!({"x": 10, "y": 10, "width": -5, "height": 5})).unwrap();
        assert_eq!((ok.x(), ok.width()), (5.0, 5.0));
        assert!(
            parse_region(&serde_json::json!({"x": 1e300, "y": 0, "width": 1, "height": 1}))
                .is_none()
        );
        assert!(
            parse_region(&serde_json::json!({"x": 0, "y": 0, "width": 0, "height": 1})).is_none()
        );
        assert!(parse_region(&serde_json::json!({"x": 0, "y": 0})).is_none());
    }

    #[tokio::test]
    async fn test_register_agent_sends_bearer_token() {
        let (url, rx) = serve_once(200, r#"{"agent_id":"agent-42"}"#);
        let client = DaimonClient::with_config(DaimonConfig {
            endpoint: url,
            api_key: Some("tok".into()),
        });
        assert_eq!(client.register_agent("selah").await.unwrap(), "agent-42");
        let req = rx.recv().unwrap();
        assert_eq!(req.request_line, "POST /v1/agents/register HTTP/1.1");
        assert_eq!(req.header("authorization"), Some("Bearer tok"));
        assert_eq!(req.json()["type"], "selah");
    }

    #[tokio::test]
    async fn test_register_agent_missing_id() {
        let (url, _rx) = serve_once(200, r#"{"status":"ok"}"#);
        let client = DaimonClient::with_config(DaimonConfig {
            endpoint: url,
            api_key: None,
        });
        let err = client
            .register_agent("selah")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("agent_id"), "{err}");
    }

    #[tokio::test]
    async fn test_heartbeat_encodes_agent_id_and_omits_auth() {
        let (url, rx) = serve_once(200, "");
        let client = DaimonClient::with_config(DaimonConfig {
            endpoint: url,
            api_key: None,
        });
        client.heartbeat("x/../../admin").await.unwrap();
        let req = rx.recv().unwrap();
        assert_eq!(
            req.request_line,
            "POST /v1/agents/x%2F..%2F..%2Fadmin/heartbeat HTTP/1.1"
        );
        assert!(req.header("authorization").is_none());
    }

    #[tokio::test]
    async fn test_heartbeat_error_status() {
        let (url, _rx) = serve_once(404, "no such agent");
        let client = DaimonClient::with_config(DaimonConfig {
            endpoint: url,
            api_key: None,
        });
        let err = client.heartbeat("a").await.unwrap_err().to_string();
        assert!(
            err.contains("daimon heartbeat 404") && err.contains("no such agent"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn test_hoosh_ocr_sanitizes_response() {
        let (url, rx) = serve_once(
            200,
            r#"{"text":"hi","confidence":7.5,"regions":[
                {"x":1,"y":2,"width":3,"height":4},
                {"x":1e300,"y":0,"width":1,"height":1},
                {"x":0,"y":0,"width":0,"height":0}
            ]}"#,
        );
        let client = HooshClient::with_config(HooshConfig { endpoint: url });
        let result = client.ocr(b"img").await.unwrap();
        assert_eq!(result.text, "hi");
        assert_eq!(result.confidence, 1.0);
        assert_eq!(result.bounding_boxes.len(), 1);
        assert!(!result.is_stub);
        let req = rx.recv().unwrap();
        assert_eq!(req.request_line, "POST /v1/vision/ocr HTTP/1.1");
        assert_eq!(req.json()["image"], "aW1n"); // base64("img")
    }
}
