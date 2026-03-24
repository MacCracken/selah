//! Selah — AI-native screenshot capture, annotation, and redaction library for AGNOS.

pub mod core;
pub mod geometry;
pub mod error;
pub mod capture;
pub mod history;
pub mod annotate;
pub mod ai;
pub mod daimon;
#[cfg(any(feature = "mcp", test))]
pub mod mcp;

// Re-exports for convenience
pub use core::{
    Annotation, AnnotationKind, CaptureRegion, CaptureSource, Color, ImageFormat,
    Monitor, RedactionTarget, Screenshot, xml_escape, derive_output_path,
};
pub use geometry::Rect;
pub use error::SelahError;
pub use capture::{CaptureClient, CaptureResponse};
pub use history::{HistoryStore, HistoryEntry};
pub use annotate::{AnnotationCanvas, AnnotationLayer, convert_format};
pub use ai::{OcrResult, RedactionSuggestion, SmartCropSuggestion, extract_text_regions, suggest_redactions, suggest_smart_crop};
pub use daimon::{DaimonClient, DaimonConfig, HooshClient, HooshConfig};
#[cfg(feature = "mcp")]
pub use mcp::run_server as run_mcp_server;

// Re-export hisab::Vec2 as Point for convenience
pub use hisab::Vec2;

/// Decode base64-encoded image data from a capture response.
pub fn decode_image_data(base64_data: &str) -> Result<Vec<u8>, SelahError> {
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, base64_data)
        .map_err(|e| SelahError::Api(format!("failed to decode image data: {e}")))
}

/// Apply annotations to an image and return the encoded bytes.
pub fn annotate_image(
    source: &[u8],
    annotations: &[Annotation],
    format: ImageFormat,
) -> Result<Vec<u8>, SelahError> {
    AnnotationCanvas::render_to_image(source, annotations, format)
}

/// Detect PII in an image and produce redacted bytes.
/// Returns (redacted image bytes, list of suggestions).
pub fn redact_image(
    source: &[u8],
    targets: Option<&[RedactionTarget]>,
    format: ImageFormat,
) -> Result<(Vec<u8>, Vec<RedactionSuggestion>), SelahError> {
    let ocr = extract_text_regions(source);
    let mut suggestions = suggest_redactions(&ocr.text);

    // Filter zero-size regions
    suggestions.retain(|s| s.region.area() > 0.0);

    // Filter by requested targets
    if let Some(targets) = targets {
        suggestions.retain(|s| targets.contains(&s.target_type));
    }

    let annotations: Vec<Annotation> = suggestions.iter()
        .map(|s| Annotation::new(AnnotationKind::Redaction, s.region, Color::BLACK))
        .collect();

    let result = AnnotationCanvas::render_to_image(source, &annotations, format)?;
    Ok((result, suggestions))
}
