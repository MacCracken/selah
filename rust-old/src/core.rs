//! Core types and primitives for the Selah screenshot tool.

use crate::error::SelahError;
use crate::geometry::Rect;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
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
#[non_exhaustive]
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
                write!(
                    f,
                    "region ({}x{} at {},{})",
                    r.width(),
                    r.height(),
                    r.x(),
                    r.y()
                )
            }
            CaptureSource::Window(id) => write!(f, "window {id}"),
        }
    }
}

/// Capture region specification.
#[non_exhaustive]
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
    ///
    /// # Example
    ///
    /// ```
    /// use selah::{Annotation, AnnotationKind, Rect, Color};
    ///
    /// let ann = Annotation::new(
    ///     AnnotationKind::Rectangle,
    ///     Rect::new(10.0, 20.0, 100.0, 50.0),
    ///     Color::RED,
    /// );
    /// assert!(ann.text.is_none());
    /// ```
    #[must_use]
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
    #[must_use]
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
#[non_exhaustive]
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

impl std::str::FromStr for AnnotationKind {
    type Err = SelahError;

    /// Parse a kind name case-insensitively (`"rectangle"`, `"Rectangle"`,
    /// `"rect"`, `"freeform"`, …).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "arrow" => Ok(AnnotationKind::Arrow),
            "rectangle" | "rect" => Ok(AnnotationKind::Rectangle),
            "circle" | "ellipse" => Ok(AnnotationKind::Circle),
            "text" => Ok(AnnotationKind::Text),
            "highlight" => Ok(AnnotationKind::Highlight),
            "redaction" | "redact" => Ok(AnnotationKind::Redaction),
            "freeform" | "free_form" | "free-form" => Ok(AnnotationKind::FreeForm),
            other => Err(SelahError::AnnotationError(format!(
                "unknown annotation kind {other:?} (use arrow, rectangle, circle, text, highlight, redaction, or freeform)"
            ))),
        }
    }
}

/// Supported image formats.
#[non_exhaustive]
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
    ///
    /// # Example
    ///
    /// ```
    /// use selah::ImageFormat;
    ///
    /// assert_eq!(ImageFormat::Png.extension(), "png");
    /// assert_eq!("jpg".parse::<ImageFormat>().unwrap(), ImageFormat::Jpeg);
    /// ```
    #[must_use]
    #[inline]
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Bmp => "bmp",
            ImageFormat::WebP => "webp",
        }
    }

    /// MIME type for this format.
    #[must_use]
    #[inline]
    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::WebP => "image/webp",
        }
    }

    /// Infer the format from a path's file extension (case-insensitive).
    ///
    /// # Example
    ///
    /// ```
    /// use selah::ImageFormat;
    /// use std::path::Path;
    ///
    /// assert_eq!(ImageFormat::from_path(Path::new("shot.JPEG")), Some(ImageFormat::Jpeg));
    /// assert_eq!(ImageFormat::from_path(Path::new("notes.txt")), None);
    /// ```
    #[must_use]
    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        path.extension()?.to_str()?.parse().ok()
    }
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.extension())
    }
}

impl std::str::FromStr for ImageFormat {
    type Err = SelahError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "png" => Ok(ImageFormat::Png),
            "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
            "bmp" => Ok(ImageFormat::Bmp),
            "webp" => Ok(ImageFormat::WebP),
            other => Err(SelahError::UnsupportedFormat(format!(
                "{other} (use png, jpg, bmp, or webp)"
            ))),
        }
    }
}

/// Escape a string for safe inclusion in XML/SVG content.
///
/// Escapes the five XML special characters and drops characters that are not
/// allowed anywhere in an XML 1.0 document (C0 controls other than tab, LF and
/// CR, plus U+FFFE/U+FFFF), so the output is always well-formed.
///
/// # Example
///
/// ```
/// assert_eq!(selah::xml_escape("<script>alert('xss')</script>"),
///            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;");
/// ```
#[must_use]
pub fn xml_escape(s: &str) -> String {
    // Fast path: most text needs no escaping at all.
    if !s.bytes().any(is_xml_special) {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len() + s.len() / 8);
    push_xml_escaped(&mut out, s);
    out
}

/// Bytes that may need escaping or removal: the five XML specials, C0
/// controls other than tab/LF/CR, and `0xEF` (lead byte of U+FFFE/U+FFFF).
static XML_SPECIAL: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 0x20 {
        t[i] = true;
        i += 1;
    }
    t[b'\t' as usize] = false;
    t[b'\n' as usize] = false;
    t[b'\r' as usize] = false;
    t[b'&' as usize] = true;
    t[b'<' as usize] = true;
    t[b'>' as usize] = true;
    t[b'"' as usize] = true;
    t[b'\'' as usize] = true;
    t[0xEF] = true;
    t
};

#[inline]
fn is_xml_special(b: u8) -> bool {
    XML_SPECIAL[usize::from(b)]
}

/// Append the XML-escaped form of `s` to `out` (see [`xml_escape`]).
///
/// Runs of ordinary bytes are copied in bulk via a table-driven scan; only
/// flagged characters are handled one at a time.
#[inline]
pub(crate) fn push_xml_escaped(out: &mut String, mut rest: &str) {
    while !rest.is_empty() {
        let run = rest.bytes().position(is_xml_special).unwrap_or(rest.len());
        // Flagged bytes are ASCII or a UTF-8 lead byte, so `run` is always a
        // char boundary; `split_at_checked` keeps this panic-free regardless.
        if run > 0
            && let Some((plain, tail)) = rest.split_at_checked(run)
        {
            out.push_str(plain);
            rest = tail;
            continue;
        }
        let mut chars = rest.chars();
        match chars.next() {
            Some('&') => out.push_str("&amp;"),
            Some('<') => out.push_str("&lt;"),
            Some('>') => out.push_str("&gt;"),
            Some('"') => out.push_str("&quot;"),
            Some('\'') => out.push_str("&#x27;"),
            // Not allowed anywhere in XML 1.0: C0 controls except tab/LF/CR,
            // and the non-characters U+FFFE / U+FFFF.
            Some(c) if (c < ' ' && !matches!(c, '\t' | '\n' | '\r')) => {}
            Some('\u{FFFE}' | '\u{FFFF}') => {}
            Some(c) => out.push(c),
            None => {}
        }
        rest = chars.as_str();
    }
}

/// Generate a default output path by appending a suffix before the file extension.
///
/// e.g. `derive_output_path("photo.png", "annotated")` -> `"photo_annotated.png"`
///
/// # Example
///
/// ```
/// assert_eq!(selah::derive_output_path("photo.png", "annotated"), "photo_annotated.png");
/// ```
#[must_use]
pub fn derive_output_path(input: &str, suffix: &str) -> String {
    let p = std::path::Path::new(input);
    let stem = p.file_stem().unwrap_or_default().to_string_lossy();
    let ext = p.extension().unwrap_or_default().to_string_lossy();
    if ext.is_empty() {
        format!("{stem}_{suffix}")
    } else {
        format!("{stem}_{suffix}.{ext}")
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

    /// Create a color from RGBA components.
    #[must_use]
    #[inline]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// CSS rgba string.
    #[must_use]
    pub fn to_css(&self) -> String {
        self.css().to_string()
    }

    /// Allocation-free CSS `rgba()` formatter, for use with `write!`.
    #[inline]
    pub(crate) fn css(&self) -> impl fmt::Display + '_ {
        CssColor(self)
    }
}

struct CssColor<'a>(&'a Color);

impl fmt::Display for CssColor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = self.0;
        write!(
            f,
            "rgba({},{},{},{:.2})",
            c.r,
            c.g,
            c.b,
            f64::from(c.a) / 255.0
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
#[non_exhaustive]
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

impl RedactionTarget {
    /// Stable machine-readable key (`email`, `phone`, `credit_card`,
    /// `ip_address`, `custom`), as used by the MCP `selah_redact` tool.
    #[must_use]
    #[inline]
    pub fn key(&self) -> &'static str {
        match self {
            RedactionTarget::Email => "email",
            RedactionTarget::Phone => "phone",
            RedactionTarget::CreditCard => "credit_card",
            RedactionTarget::IpAddress => "ip_address",
            RedactionTarget::Custom(_) => "custom",
        }
    }
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
        assert!(r.contains_point(hisab::Vec2::new(50.0, 30.0)));
        assert!(r.contains_point(hisab::Vec2::new(10.0, 10.0))); // on edge
        assert!(r.contains_point(hisab::Vec2::new(110.0, 60.0))); // on far edge
        assert!(!r.contains_point(hisab::Vec2::new(5.0, 30.0))); // outside left
        assert!(!r.contains_point(hisab::Vec2::new(50.0, 65.0))); // outside bottom
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
        assert_eq!(r.x(), 0.0);
        assert_eq!(r.area(), 0.0);
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

    #[test]
    fn test_image_format_from_str() {
        assert_eq!("png".parse::<ImageFormat>().unwrap(), ImageFormat::Png);
        assert_eq!("jpg".parse::<ImageFormat>().unwrap(), ImageFormat::Jpeg);
        assert_eq!("jpeg".parse::<ImageFormat>().unwrap(), ImageFormat::Jpeg);
        assert_eq!("bmp".parse::<ImageFormat>().unwrap(), ImageFormat::Bmp);
        assert_eq!("webp".parse::<ImageFormat>().unwrap(), ImageFormat::WebP);
        assert_eq!("PNG".parse::<ImageFormat>().unwrap(), ImageFormat::Png);
        assert!("tiff".parse::<ImageFormat>().is_err());
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("hello"), "hello");
        assert_eq!(xml_escape("<script>"), "&lt;script&gt;");
        assert_eq!(xml_escape("a&b"), "a&amp;b");
        assert_eq!(xml_escape(r#"x"y'z"#), "x&quot;y&#x27;z");
    }

    #[test]
    fn test_derive_output_path() {
        assert_eq!(
            derive_output_path("photo.png", "annotated"),
            "photo_annotated.png"
        );
        assert_eq!(
            derive_output_path("photo.png", "redacted"),
            "photo_redacted.png"
        );
        assert_eq!(derive_output_path("noext", "out"), "noext_out");
        assert_eq!(
            derive_output_path("/tmp/img.jpg", "annotated"),
            "img_annotated.jpg"
        );
    }

    #[test]
    fn test_image_format_from_str_invalid() {
        let err = "tiff".parse::<ImageFormat>().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("tiff"));
        assert!(msg.contains("png"));
    }

    #[test]
    fn test_xml_escape_all_special_chars() {
        assert_eq!(
            xml_escape(r#"<a href="x">&'test'</a>"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#x27;test&#x27;&lt;/a&gt;"
        );
    }

    #[test]
    fn test_xml_escape_strips_xml_illegal_chars() {
        assert_eq!(xml_escape("a\u{0}b\u{1b}c"), "abc");
        assert_eq!(xml_escape("tab\tnl\ncr\r"), "tab\tnl\ncr\r");
        assert_eq!(xml_escape("x\u{FFFE}y\u{FFFF}z"), "xyz");
        // U+FFFD and other EF-prefixed chars are legal and kept
        assert_eq!(xml_escape("\u{FFFD}\u{FF01}"), "\u{FFFD}\u{FF01}");
    }

    #[test]
    fn test_xml_escape_multibyte_preserved() {
        assert_eq!(xml_escape("héllo <wörld> 日本"), "héllo &lt;wörld&gt; 日本");
    }

    #[test]
    fn test_image_format_from_path() {
        use std::path::Path;
        assert_eq!(
            ImageFormat::from_path(Path::new("a/b.JPG")),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(
            ImageFormat::from_path(Path::new("x.webp")),
            Some(ImageFormat::WebP)
        );
        assert_eq!(ImageFormat::from_path(Path::new("noext")), None);
        assert_eq!(ImageFormat::from_path(Path::new("x.tiff")), None);
    }

    #[test]
    fn test_annotation_kind_from_str_roundtrip() {
        for kind in [
            AnnotationKind::Arrow,
            AnnotationKind::Rectangle,
            AnnotationKind::Circle,
            AnnotationKind::Text,
            AnnotationKind::Highlight,
            AnnotationKind::Redaction,
            AnnotationKind::FreeForm,
        ] {
            assert_eq!(kind.to_string().parse::<AnnotationKind>().unwrap(), kind);
        }
        assert_eq!(
            "Rectangle".parse::<AnnotationKind>().unwrap(),
            AnnotationKind::Rectangle
        );
        assert_eq!(
            "FreeForm".parse::<AnnotationKind>().unwrap(),
            AnnotationKind::FreeForm
        );
        assert!("blob".parse::<AnnotationKind>().is_err());
    }

    #[test]
    fn test_redaction_target_key() {
        assert_eq!(RedactionTarget::Email.key(), "email");
        assert_eq!(RedactionTarget::Phone.key(), "phone");
        assert_eq!(RedactionTarget::CreditCard.key(), "credit_card");
        assert_eq!(RedactionTarget::IpAddress.key(), "ip_address");
        assert_eq!(RedactionTarget::Custom("x".into()).key(), "custom");
    }

    #[test]
    fn test_xml_escape_empty() {
        assert_eq!(xml_escape(""), "");
    }

    #[test]
    fn test_xml_escape_no_special() {
        assert_eq!(xml_escape("plain text 123"), "plain text 123");
    }

    #[test]
    fn test_derive_output_path_dotfile() {
        assert_eq!(derive_output_path(".hidden", "out"), ".hidden_out");
    }

    #[test]
    fn test_derive_output_path_multiple_dots() {
        assert_eq!(
            derive_output_path("file.backup.png", "annotated"),
            "file.backup_annotated.png"
        );
    }

    #[test]
    fn test_capture_source_display_all_variants() {
        assert_eq!(CaptureSource::FullScreen.to_string(), "full screen");
        assert_eq!(
            CaptureSource::Window("firefox".into()).to_string(),
            "window firefox"
        );
        let r = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let s = CaptureSource::Region(r).to_string();
        assert!(s.contains("1920"));
        assert!(s.contains("1080"));
    }

    #[test]
    fn test_annotation_with_text_has_correct_fields() {
        let ann = Annotation::with_text(
            AnnotationKind::Text,
            Rect::new(10.0, 20.0, 200.0, 30.0),
            Color::BLUE,
            "Hello <World>".into(),
        );
        assert_eq!(ann.kind, AnnotationKind::Text);
        assert_eq!(ann.color, Color::BLUE);
        assert_eq!(ann.text, Some("Hello <World>".into()));
        assert_eq!(ann.position.x(), 10.0);
    }

    #[test]
    fn test_annotation_new_generates_unique_ids() {
        let a = Annotation::new(AnnotationKind::Arrow, Rect::default(), Color::RED);
        let b = Annotation::new(AnnotationKind::Arrow, Rect::default(), Color::RED);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn test_color_new_and_constants() {
        let c = Color::new(1, 2, 3, 4);
        assert_eq!(c.r, 1);
        assert_eq!(c.g, 2);
        assert_eq!(c.b, 3);
        assert_eq!(c.a, 4);
        assert_eq!(Color::WHITE, Color::new(255, 255, 255, 255));
        assert_eq!(Color::BLACK, Color::new(0, 0, 0, 255));
    }

    #[test]
    fn test_color_to_css_semi_transparent() {
        let c = Color::new(0, 128, 255, 128);
        let css = c.to_css();
        assert!(css.starts_with("rgba(0,128,255,"));
        assert!(css.contains("0.50"));
    }

    #[test]
    fn test_redaction_target_display_all_variants() {
        assert_eq!(RedactionTarget::CreditCard.to_string(), "credit card");
        assert_eq!(RedactionTarget::IpAddress.to_string(), "IP address");
    }

    #[test]
    fn test_annotation_kind_display_all() {
        assert_eq!(AnnotationKind::Rectangle.to_string(), "rectangle");
        assert_eq!(AnnotationKind::Circle.to_string(), "circle");
        assert_eq!(AnnotationKind::Text.to_string(), "text");
        assert_eq!(AnnotationKind::Highlight.to_string(), "highlight");
    }

    #[test]
    fn test_image_format_bmp_and_webp_mime() {
        assert_eq!(ImageFormat::Bmp.mime_type(), "image/bmp");
        assert_eq!(ImageFormat::WebP.mime_type(), "image/webp");
    }
}
