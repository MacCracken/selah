//! Shared HTTP helpers: client construction, status checks, bounded bodies.

use crate::error::SelahError;
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::Duration;

/// Maximum bytes of an error response body included in an error message.
const MAX_ERROR_BODY: usize = 512;

/// Maximum size of a capture response (base64 screenshots can be large).
pub(crate) const MAX_CAPTURE_RESPONSE_BYTES: usize = 256 * 1024 * 1024;

/// Maximum size of other JSON API responses (registration, monitors, OCR).
pub(crate) const MAX_API_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const USER_AGENT: &str = concat!("selah/", env!("CARGO_PKG_VERSION"));

/// A configured reqwest client, or the reason one could not be built.
///
/// Building only fails if the TLS backend or resolver cannot be initialised.
/// Rather than panicking in an infallible constructor (as
/// `reqwest::Client::new()` would), the error is kept and surfaced as an
/// [`SelahError::Api`] on the first request.
#[derive(Debug, Clone)]
pub(crate) struct HttpClient(Result<reqwest::Client, Arc<str>>);

impl HttpClient {
    /// Build a client with a request timeout, connect timeout and user agent.
    pub(crate) fn new(timeout: Duration) -> Self {
        let built = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(CONNECT_TIMEOUT)
            .user_agent(USER_AGENT)
            .build()
            .map_err(|e| {
                tracing::warn!(error = %e, "failed to build HTTP client");
                Arc::from(e.to_string())
            });
        Self(built)
    }

    /// Wrap a caller-supplied client.
    pub(crate) fn from_client(client: reqwest::Client) -> Self {
        Self(Ok(client))
    }

    /// The client, or an error explaining why it is unavailable.
    #[inline]
    pub(crate) fn client(&self) -> Result<&reqwest::Client, SelahError> {
        self.0
            .as_ref()
            .map_err(|e| SelahError::Api(format!("HTTP client unavailable: {e}")))
    }
}

/// Pass `resp` through on success; otherwise return an [`SelahError::Api`]
/// with the status and a truncated, lossily-decoded body.
pub(crate) async fn check_status(
    resp: reqwest::Response,
    context: &str,
) -> Result<reqwest::Response, SelahError> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = read_prefix(resp, MAX_ERROR_BODY + 1).await;
    let text = String::from_utf8_lossy(&body);
    let shown = truncate(&text, MAX_ERROR_BODY);
    let ellipsis = if shown.len() < text.len() { "…" } else { "" };
    tracing::warn!(%status, context, "API request failed");
    Err(SelahError::Api(format!(
        "{context} {status}: {shown}{ellipsis}"
    )))
}

/// Read and deserialize a JSON body, refusing bodies larger than `max_bytes`.
pub(crate) async fn read_json<T: DeserializeOwned>(
    mut resp: reqwest::Response,
    max_bytes: usize,
    context: &str,
) -> Result<T, SelahError> {
    let too_large = |len: u64| {
        SelahError::Api(format!(
            "{context}: response too large ({len} bytes, limit {max_bytes})"
        ))
    };
    if let Some(len) = resp.content_length()
        && len > max_bytes as u64
    {
        return Err(too_large(len));
    }

    let mut body = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| SelahError::Api(format!("{context}: failed to read response: {e}")))?
    {
        if body.len() + chunk.len() > max_bytes {
            return Err(too_large((body.len() + chunk.len()) as u64));
        }
        body.extend_from_slice(&chunk);
    }

    serde_json::from_slice(&body)
        .map_err(|e| SelahError::Api(format!("{context}: invalid response: {e}")))
}

/// Read at most `max` bytes of the body, ignoring transport errors.
async fn read_prefix(mut resp: reqwest::Response, max: usize) -> Vec<u8> {
    let mut buf = Vec::new();
    while buf.len() < max {
        match resp.chunk().await {
            Ok(Some(chunk)) => {
                let take = (max - buf.len()).min(chunk.len());
                buf.extend_from_slice(&chunk[..take]);
            }
            _ => break,
        }
    }
    buf
}

/// Truncate `s` to at most `max` bytes on a char boundary.
#[inline]
pub(crate) fn truncate(s: &str, max: usize) -> &str {
    &s[..s.floor_char_boundary(max)]
}

#[cfg(test)]
pub(crate) mod test_server {
    //! Minimal one-shot HTTP/1.1 server for exercising client code paths.

    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    /// A request captured by the mock server.
    #[derive(Debug)]
    pub struct Captured {
        pub request_line: String,
        pub headers: Vec<(String, String)>,
        pub body: Vec<u8>,
    }

    impl Captured {
        pub fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        }

        pub fn json(&self) -> serde_json::Value {
            serde_json::from_slice(&self.body).unwrap()
        }
    }

    /// Serve exactly one request with `status` and `body`, returning the base
    /// URL and a receiver for the captured request.
    pub fn serve_once(status: u16, body: impl Into<Vec<u8>>) -> (String, mpsc::Receiver<Captured>) {
        let body = body.into();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let mut headers = Vec::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let line = line.trim_end();
                if line.is_empty() {
                    break;
                }
                if let Some((k, v)) = line.split_once(':') {
                    headers.push((k.trim().to_string(), v.trim().to_string()));
                }
            }
            let len = headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                .and_then(|(_, v)| v.parse::<usize>().ok())
                .unwrap_or(0);
            let mut req_body = vec![0; len];
            reader.read_exact(&mut req_body).unwrap();

            let mut stream = stream;
            let head = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
            let _ = tx.send(Captured {
                request_line: request_line.trim_end().to_string(),
                headers,
                body: req_body,
            });
        });
        (url, rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_client_error_surfaces_as_api_error() {
        let broken = HttpClient(Err(Arc::from("no TLS backend")));
        let err = broken.client().unwrap_err().to_string();
        assert!(
            err.contains("HTTP client unavailable: no TLS backend"),
            "{err}"
        );
        assert!(HttpClient::new(Duration::from_secs(1)).client().is_ok());
    }

    #[test]
    fn test_truncate_char_boundary() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello", 3), "hel");
        // 'é' is two bytes; never split it
        assert_eq!(truncate("héllo", 2), "h");
        assert_eq!(truncate("", 0), "");
    }

    #[tokio::test]
    async fn test_check_status_truncates_error_body() {
        let (url, _rx) = test_server::serve_once(500, "x".repeat(10_000));
        let resp = HttpClient::new(Duration::from_secs(5))
            .client()
            .unwrap()
            .get(&url)
            .send()
            .await
            .unwrap();
        let err = check_status(resp, "test").await.unwrap_err().to_string();
        assert!(err.starts_with("API error: test 500"), "{err}");
        assert!(
            err.len() < 600,
            "error message not truncated: {} bytes",
            err.len()
        );
        assert!(err.ends_with('…'));
    }

    #[tokio::test]
    async fn test_read_json_enforces_limit() {
        let (url, _rx) = test_server::serve_once(200, format!("\"{}\"", "a".repeat(1000)));
        let resp = HttpClient::new(Duration::from_secs(5))
            .client()
            .unwrap()
            .get(&url)
            .send()
            .await
            .unwrap();
        let err = read_json::<String>(resp, 100, "test").await.unwrap_err();
        assert!(err.to_string().contains("too large"), "{err}");
    }

    #[tokio::test]
    async fn test_read_json_ok_and_sends_user_agent() {
        let (url, rx) = test_server::serve_once(200, r#"{"a":1}"#);
        let resp = HttpClient::new(Duration::from_secs(5))
            .client()
            .unwrap()
            .get(&url)
            .send()
            .await
            .unwrap();
        let v: serde_json::Value = read_json(resp, 100, "test").await.unwrap();
        assert_eq!(v["a"], 1);
        let req = rx.recv().unwrap();
        assert!(req.header("user-agent").unwrap().starts_with("selah/"));
    }

    #[tokio::test]
    async fn test_read_json_invalid() {
        let (url, _rx) = test_server::serve_once(200, "not json");
        let resp = HttpClient::new(Duration::from_secs(5))
            .client()
            .unwrap()
            .get(&url)
            .send()
            .await
            .unwrap();
        let err = read_json::<serde_json::Value>(resp, 100, "ctx")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("ctx: invalid response"), "{err}");
    }
}
