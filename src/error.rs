//! Error types for Selah.

/// Errors in Selah.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SelahError {
    #[error("capture failed: {0}")]
    CaptureFailed(String),
    #[error("invalid region: {0}")]
    InvalidRegion(String),
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("annotation error: {0}")]
    AnnotationError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("API error: {0}")]
    Api(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selah_error_display_capture_failed() {
        let e = SelahError::CaptureFailed("timeout".into());
        assert_eq!(e.to_string(), "capture failed: timeout");
    }

    #[test]
    fn test_selah_error_display_invalid_region() {
        let e = SelahError::InvalidRegion("negative width".into());
        assert_eq!(e.to_string(), "invalid region: negative width");
    }

    #[test]
    fn test_selah_error_display_unsupported_format() {
        let e = SelahError::UnsupportedFormat("tiff".into());
        assert_eq!(e.to_string(), "unsupported format: tiff");
    }

    #[test]
    fn test_selah_error_display_annotation() {
        let e = SelahError::AnnotationError("bad color".into());
        assert_eq!(e.to_string(), "annotation error: bad color");
    }

    #[test]
    fn test_selah_error_display_api() {
        let e = SelahError::Api("connection refused".into());
        assert_eq!(e.to_string(), "API error: connection refused");
    }

    #[test]
    fn test_selah_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let e: SelahError = io_err.into();
        assert!(e.to_string().contains("file not found"));
    }
}
