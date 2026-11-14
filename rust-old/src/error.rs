//! Error types for Selah.

/// Errors in Selah.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SelahError {
    /// A screen capture operation failed (e.g. the capture backend timed out or
    /// the clipboard tool exited with an error).
    #[error("capture failed: {0}")]
    CaptureFailed(String),
    /// The requested capture region is invalid (e.g. negative dimensions or
    /// coordinates outside the screen bounds).
    #[error("invalid region: {0}")]
    InvalidRegion(String),
    /// The requested image format is not supported (e.g. "tiff").
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    /// An error occurred while creating or applying an annotation (e.g. invalid
    /// color or a rendering failure).
    #[error("annotation error: {0}")]
    AnnotationError(String),
    /// An I/O error occurred (e.g. writing a screenshot to disk).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// A remote API call failed (e.g. the daimon capture service returned an
    /// error or was unreachable).
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
