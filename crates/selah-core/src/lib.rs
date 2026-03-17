//! selah-core — Core types and primitives for the Selah screenshot tool.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A captured screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screenshot {
    /// Unique identifier.
    pub id: Uuid,
    /// Raw image data (encoded bytes).
    pub data: Vec<u8>,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// When the screenshot was taken.
    pub timestamp: DateTime<Utc>,
    /// Where the capture came from.
    pub source: CaptureSource,
    /// Image format.
    pub format: ImageFormat,
}

/// Source of a screen capture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CaptureSource {
    /// Full screen capture.
    FullScreen,
    /// A specific region was captured.
    Region(Rect),
    /// A specific window was captured.
    Window(String),
}

impl std::fmt::Display for CaptureSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureSource::FullScreen => write!(f, "full screen"),
            CaptureSource::Region(r) => {
                write!(f, "region ({}x{} at {},{})", r.width, r.height, r.x, r.y)
            }
            CaptureSource::Window(id) => write!(f, "window {id}"),
        }
    }
}

/// Capture region specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CaptureRegion {
    /// Capture the entire screen.
    FullScreen,
    /// Capture a rectangular region.
    Rect(Rect),
    /// Capture a window by its ID.
    Window(String),
}

/// A single annotation on a screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    /// Unique identifier.
    pub id: Uuid,
    /// The kind of annotation.
    pub kind: AnnotationKind,
    /// Position of the annotation.
    pub position: Rect,
    /// Color of the annotation.
    pub color: Color,
    /// Optional text content.
    pub text: Option<String>,
}

impl Annotation {
    /// Create a new annotation.
    pub fn new(kind: AnnotationKind, position: Rect, color: Color) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            position,
            color,
            text: None,
        }
    }

    /// Create a new annotation with text.
    pub fn with_text(kind: AnnotationKind, position: Rect, color: Color, text: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            position,
            color,
            text: Some(text),
        }
    }
}

/// Types of annotations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AnnotationKind {
    Arrow,
    Rectangle,
    Circle,
    Text,
    Highlight,
    Redaction,
    FreeForm,
}

impl std::fmt::Display for AnnotationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnnotationKind::Arrow => write!(f, "arrow"),
            AnnotationKind::Rectangle => write!(f, "rectangle"),
            AnnotationKind::Circle => write!(f, "circle"),
            AnnotationKind::Text => write!(f, "text"),
            AnnotationKind::Highlight => write!(f, "highlight"),
            AnnotationKind::Redaction => write!(f, "redaction"),
            AnnotationKind::FreeForm => write!(f, "freeform"),
        }
    }
}

/// Supported image formats.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub enum ImageFormat {
    #[default]
    Png,
    Jpeg,
    Bmp,
    WebP,
}

impl ImageFormat {
    /// File extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Bmp => "bmp",
            ImageFormat::WebP => "webp",
        }
    }

    /// MIME type for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::WebP => "image/webp",
        }
    }
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.extension())
    }
}

/// RGBA color.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const RED: Color = Color {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const GREEN: Color = Color {
        r: 0,
        g: 255,
        b: 0,
        a: 255,
    };
    pub const BLUE: Color = Color {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    };
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const YELLOW: Color = Color {
        r: 255,
        g: 255,
        b: 0,
        a: 128,
    };

    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// CSS rgba string.
    pub fn to_css(&self) -> String {
        format!(
            "rgba({},{},{},{:.2})",
            self.r,
            self.g,
            self.b,
            self.a as f64 / 255.0
        )
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::RED
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "#{:02x}{:02x}{:02x}{:02x}",
            self.r, self.g, self.b, self.a
        )
    }
}

/// A 2D point.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Euclidean distance to another point.
    pub fn distance_to(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

impl Default for Point {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

/// A rectangle defined by position and size.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if a point is inside this rectangle.
    pub fn contains(&self, point: &Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    /// Area of the rectangle.
    pub fn area(&self) -> f64 {
        self.width * self.height
    }

    /// Center point of the rectangle.
    pub fn center(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Check if this rectangle intersects another.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}

impl Default for Rect {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }
}

/// A connected display monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monitor {
    /// Unique identifier (e.g. "HDMI-A-1").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Resolution width in pixels.
    pub width: u32,
    /// Resolution height in pixels.
    pub height: u32,
    /// X offset in the virtual display layout.
    pub x: i32,
    /// Y offset in the virtual display layout.
    pub y: i32,
    /// Whether this is the primary display.
    pub primary: bool,
}

/// Types of sensitive data that can be redacted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RedactionTarget {
    Email,
    Phone,
    CreditCard,
    IpAddress,
    Custom(String),
}

impl std::fmt::Display for RedactionTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RedactionTarget::Email => write!(f, "email"),
            RedactionTarget::Phone => write!(f, "phone"),
            RedactionTarget::CreditCard => write!(f, "credit card"),
            RedactionTarget::IpAddress => write!(f, "IP address"),
            RedactionTarget::Custom(s) => write!(f, "custom: {s}"),
        }
    }
}

/// Errors in selah-core.
#[derive(Debug, thiserror::Error)]
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
    fn test_color_default_is_red() {
        let c = Color::default();
        assert_eq!(c, Color::RED);
    }

    #[test]
    fn test_color_display() {
        let c = Color::new(255, 128, 0, 255);
        assert_eq!(c.to_string(), "#ff8000ff");
    }

    #[test]
    fn test_color_to_css() {
        let c = Color::new(255, 0, 0, 255);
        assert_eq!(c.to_css(), "rgba(255,0,0,1.00)");
    }

    #[test]
    fn test_image_format_extension() {
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
        assert_eq!(ImageFormat::Bmp.extension(), "bmp");
        assert_eq!(ImageFormat::WebP.extension(), "webp");
    }

    #[test]
    fn test_image_format_mime() {
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
    }

    #[test]
    fn test_image_format_default_is_png() {
        assert_eq!(ImageFormat::default(), ImageFormat::Png);
    }

    #[test]
    fn test_image_format_display() {
        assert_eq!(ImageFormat::Png.to_string(), "png");
        assert_eq!(ImageFormat::WebP.to_string(), "webp");
    }

    #[test]
    fn test_rect_contains_point() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(r.contains(&Point::new(50.0, 30.0)));
        assert!(r.contains(&Point::new(10.0, 10.0))); // on edge
        assert!(r.contains(&Point::new(110.0, 60.0))); // on far edge
        assert!(!r.contains(&Point::new(5.0, 30.0))); // outside left
        assert!(!r.contains(&Point::new(50.0, 65.0))); // outside bottom
    }

    #[test]
    fn test_rect_area() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        assert_eq!(r.area(), 5000.0);
    }

    #[test]
    fn test_rect_center() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        let c = r.center();
        assert_eq!(c.x, 50.0);
        assert_eq!(c.y, 25.0);
    }

    #[test]
    fn test_rect_intersects() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);
        let c = Rect::new(200.0, 200.0, 50.0, 50.0);
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn test_rect_default_is_zero() {
        let r = Rect::default();
        assert_eq!(r.x, 0.0);
        assert_eq!(r.area(), 0.0);
    }

    #[test]
    fn test_point_distance() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(3.0, 4.0);
        assert!((a.distance_to(&b) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_point_default() {
        let p = Point::default();
        assert_eq!(p.x, 0.0);
        assert_eq!(p.y, 0.0);
    }

    #[test]
    fn test_capture_source_display() {
        assert_eq!(CaptureSource::FullScreen.to_string(), "full screen");
        let r = Rect::new(10.0, 20.0, 800.0, 600.0);
        assert_eq!(
            CaptureSource::Region(r).to_string(),
            "region (800x600 at 10,20)"
        );
        assert_eq!(
            CaptureSource::Window("abc".into()).to_string(),
            "window abc"
        );
    }

    #[test]
    fn test_annotation_kind_display() {
        assert_eq!(AnnotationKind::Arrow.to_string(), "arrow");
        assert_eq!(AnnotationKind::Redaction.to_string(), "redaction");
        assert_eq!(AnnotationKind::FreeForm.to_string(), "freeform");
    }

    #[test]
    fn test_redaction_target_display() {
        assert_eq!(RedactionTarget::Email.to_string(), "email");
        assert_eq!(RedactionTarget::Phone.to_string(), "phone");
        assert_eq!(
            RedactionTarget::Custom("SSN".into()).to_string(),
            "custom: SSN"
        );
    }

    #[test]
    fn test_annotation_new() {
        let ann = Annotation::new(
            AnnotationKind::Arrow,
            Rect::new(0.0, 0.0, 50.0, 50.0),
            Color::RED,
        );
        assert_eq!(ann.kind, AnnotationKind::Arrow);
        assert!(ann.text.is_none());
    }

    #[test]
    fn test_annotation_with_text() {
        let ann = Annotation::with_text(
            AnnotationKind::Text,
            Rect::new(0.0, 0.0, 200.0, 30.0),
            Color::BLACK,
            "Hello".into(),
        );
        assert_eq!(ann.kind, AnnotationKind::Text);
        assert_eq!(ann.text.as_deref(), Some("Hello"));
    }
}
