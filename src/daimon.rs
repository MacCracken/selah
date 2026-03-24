//! Daimon and Hoosh integration for Selah.
//!
//! Provides client configuration and API access for:
//! - **Daimon**: Agent lifecycle, screen capture orchestration
//! - **Hoosh**: LLM vision inference for real OCR
//!
//! Feature-gated behind `ai`.

use crate::SelahError;
use serde::{Deserialize, Serialize};

/// Configuration for connecting to the daimon agent runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaimonConfig {
    /// Daimon API endpoint.
    pub endpoint: String,
    /// Optional API key for authentication.
    pub api_key: Option<String>,
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

/// Client for daimon agent runtime interactions.
///
/// Handles agent registration, heartbeat, and orchestration of screen
/// capture workflows through the daimon API.
#[derive(Debug, Clone)]
pub struct DaimonClient {
    config: DaimonConfig,
    client: reqwest::Client,
}

impl DaimonClient {
    /// Create a new daimon client with default configuration.
    pub fn new() -> Self {
        Self::with_config(DaimonConfig::default())
    }

    /// Create a daimon client with custom configuration.
    pub fn with_config(config: DaimonConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self { config, client }
    }

    /// Get the configured endpoint.
    pub fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    /// Register this agent with daimon.
    ///
    /// Returns the assigned agent ID on success.
    pub async fn register_agent(&self, name: &str) -> Result<String, SelahError> {
        let url = format!("{}/v1/agents/register", self.config.endpoint);
        let body = serde_json::json!({
            "name": name,
            "type": "selah",
            "capabilities": ["screenshot", "annotate", "ocr", "redact"]
        });

        let resp = self.client.post(&url).json(&body).send().await
            .map_err(|e| SelahError::Api(format!("daimon register failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(SelahError::Api(format!("daimon register {status}: {text}")));
        }

        let result: serde_json::Value = resp.json().await
            .map_err(|e| SelahError::Api(format!("daimon register parse: {e}")))?;

        result["agent_id"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| SelahError::Api("daimon register: missing agent_id".into()))
    }

    /// Send a heartbeat to daimon.
    pub async fn heartbeat(&self, agent_id: &str) -> Result<(), SelahError> {
        let url = format!("{}/v1/agents/{agent_id}/heartbeat", self.config.endpoint);
        let resp = self.client.post(&url).send().await
            .map_err(|e| SelahError::Api(format!("daimon heartbeat failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(SelahError::Api(format!("daimon heartbeat {status}: {text}")));
        }

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
    client: reqwest::Client,
}

impl HooshClient {
    /// Create a new hoosh client with default configuration.
    pub fn new() -> Self {
        Self::with_config(HooshConfig::default())
    }

    /// Create a hoosh client with custom configuration.
    pub fn with_config(config: HooshConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap_or_default();
        Self { config, client }
    }

    /// Get the configured endpoint.
    pub fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    /// Extract text from an image using LLM vision.
    ///
    /// Sends the image to hoosh for real OCR via a vision model.
    /// Returns extracted text with bounding boxes.
    pub async fn ocr(&self, image_data: &[u8]) -> Result<crate::OcrResult, SelahError> {
        let b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            image_data,
        );

        let url = format!("{}/v1/vision/ocr", self.config.endpoint);
        let body = serde_json::json!({
            "image": b64,
            "task": "ocr",
        });

        let resp = self.client.post(&url).json(&body).send().await
            .map_err(|e| SelahError::Api(format!("hoosh OCR failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(SelahError::Api(format!("hoosh OCR {status}: {text}")));
        }

        let result: serde_json::Value = resp.json().await
            .map_err(|e| SelahError::Api(format!("hoosh OCR parse: {e}")))?;

        let text = result["text"].as_str().unwrap_or("").to_string();
        let confidence = result["confidence"].as_f64().unwrap_or(0.0);

        let bounding_boxes: Vec<crate::Rect> = result["regions"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        Some(crate::Rect::new(
                            r["x"].as_f64()? as f32,
                            r["y"].as_f64()? as f32,
                            r["width"].as_f64()? as f32,
                            r["height"].as_f64()? as f32,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(crate::OcrResult {
            text,
            confidence,
            bounding_boxes,
            is_stub: false,
        })
    }
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
}
