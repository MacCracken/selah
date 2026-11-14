//! Annotation engine for Selah.

use crate::core::{Annotation, AnnotationKind, Color, push_xml_escaped};
use crate::error::SelahError;
use crate::geometry::Rect;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits, RgbaImage};
use ranga::blend::{BlendMode, blend_row};
use ranga::pixel::{PixelBuffer, PixelFormat};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::io::Cursor;
use std::path::Path;
use uuid::Uuid;

/// Maximum width or height (pixels) accepted when decoding an image.
///
/// Generous enough for multi-monitor 8K captures; anything larger is treated
/// as hostile (decompression bomb) and rejected before pixel allocation.
pub const MAX_IMAGE_DIMENSION: u32 = 32_768;

/// Maximum size (bytes) of a decoded image buffer — both the decoder's native
/// buffer and the 8-bit RGBA working copy used for drawing (`width × height × 4`)
/// must fit. Checked from the image header, before any pixels are allocated.
pub const MAX_DECODE_ALLOC: u64 = 512 * 1024 * 1024;

/// Maximum size of an annotation layer file accepted by
/// [`AnnotationCanvas::load_from_file`].
pub const MAX_LAYER_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Annotation layer format version written by [`AnnotationCanvas::save_to_file`].
pub const LAYER_VERSION: u32 = 1;

/// Stroke width (pixels) for outlines, lines and ellipses.
const STROKE_WIDTH: u32 = 2;

/// Highlight overlay opacity (≈ 30%).
const HIGHLIGHT_OPACITY: u8 = 76;

/// Upper bound on ellipse sample points, so absurd radii cannot stall rendering.
const MAX_ELLIPSE_STEPS: f64 = (1u32 << 20) as f64;

/// Decoder limits applied to every image selah loads.
fn decode_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    limits
}

/// Decode encoded image bytes, enforcing [`MAX_IMAGE_DIMENSION`] and
/// [`MAX_DECODE_ALLOC`].
pub(crate) fn decode_image(source: &[u8]) -> Result<DynamicImage, SelahError> {
    decode_image_with_limits(source, decode_limits())
}

fn decode_image_with_limits(source: &[u8], limits: Limits) -> Result<DynamicImage, SelahError> {
    let load_err =
        |e: image::ImageError| SelahError::AnnotationError(format!("failed to load image: {e}"));
    let max_alloc = limits.max_alloc.unwrap_or(u64::MAX);
    let mut reader = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|e| SelahError::AnnotationError(format!("failed to load image: {e}")))?;
    reader.limits(limits);
    // `into_decoder` reads only the header and enforces the dimension limits.
    let decoder = reader.into_decoder().map_err(load_err)?;

    // The decoder's own allocation limit covers only its native buffer; a
    // small grayscale image can still expand 4× when converted to RGBA for
    // drawing. Budget the larger of the two up front.
    let (w, h) = decoder.dimensions();
    let rgba_bytes = u64::from(w) * u64::from(h) * 4;
    let needed = decoder.total_bytes().max(rgba_bytes);
    if needed > max_alloc {
        return Err(SelahError::AnnotationError(format!(
            "failed to load image: {w}x{h} needs {needed} bytes, limit {max_alloc}"
        )));
    }
    DynamicImage::from_decoder(decoder).map_err(load_err)
}

#[inline]
fn to_codec_format(format: crate::core::ImageFormat) -> image::ImageFormat {
    match format {
        crate::core::ImageFormat::Png => image::ImageFormat::Png,
        crate::core::ImageFormat::Jpeg => image::ImageFormat::Jpeg,
        crate::core::ImageFormat::Bmp => image::ImageFormat::Bmp,
        crate::core::ImageFormat::WebP => image::ImageFormat::WebP,
    }
}

/// Encode an image in `format`.
///
/// JPEG has no alpha channel (the encoder rejects RGBA input), so alpha is
/// dropped for JPEG; every other format is written as 8-bit RGBA.
fn encode_image(
    img: DynamicImage,
    format: crate::core::ImageFormat,
) -> Result<Vec<u8>, SelahError> {
    let mut out = Cursor::new(Vec::new());
    let codec = to_codec_format(format);
    let result = match format {
        crate::core::ImageFormat::Jpeg => img.into_rgb8().write_to(&mut out, codec),
        _ => img.into_rgba8().write_to(&mut out, codec),
    };
    result.map_err(|e| SelahError::AnnotationError(format!("failed to encode image: {e}")))?;
    Ok(out.into_inner())
}

/// Convert image data from one format to another.
///
/// # Example
///
/// ```
/// let img = image::RgbaImage::new(2, 2);
/// let mut png = std::io::Cursor::new(Vec::new());
/// img.write_to(&mut png, image::ImageFormat::Png).unwrap();
///
/// let jpeg = selah::convert_format(png.get_ref(), selah::ImageFormat::Jpeg).unwrap();
/// assert_eq!(&jpeg[..2], &[0xFF, 0xD8]); // JPEG SOI marker
/// ```
pub fn convert_format(
    source: &[u8],
    target: crate::core::ImageFormat,
) -> Result<Vec<u8>, SelahError> {
    let img = decode_image(source)?;
    tracing::debug!(
        width = img.width(),
        height = img.height(),
        target = %target,
        "converting image format"
    );
    encode_image(img, target)
}

/// Serializable annotation layer for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationLayer {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub created: chrono::DateTime<chrono::Utc>,
    pub annotations: Vec<Annotation>,
}

/// Canvas that holds annotations on a screenshot.
#[derive(Debug, Clone)]
pub struct AnnotationCanvas {
    /// Width of the underlying image.
    pub width: u32,
    /// Height of the underlying image.
    pub height: u32,
    /// List of annotations.
    annotations: Vec<Annotation>,
}

impl AnnotationCanvas {
    /// Create a new empty canvas.
    ///
    /// # Example
    ///
    /// ```
    /// use selah::{AnnotationCanvas, Annotation, AnnotationKind, Rect, Color};
    ///
    /// let mut canvas = AnnotationCanvas::new(1920, 1080);
    /// canvas.add_annotation(Annotation::new(
    ///     AnnotationKind::Rectangle,
    ///     Rect::new(10.0, 10.0, 100.0, 50.0),
    ///     Color::RED,
    /// ));
    /// assert_eq!(canvas.count(), 1);
    /// ```
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            annotations: Vec::new(),
        }
    }

    /// Add an annotation to the canvas.
    pub fn add_annotation(&mut self, annotation: Annotation) -> Uuid {
        let id = annotation.id;
        self.annotations.push(annotation);
        id
    }

    /// Remove an annotation by ID. Returns true if found and removed.
    pub fn remove_annotation(&mut self, id: Uuid) -> bool {
        let len_before = self.annotations.len();
        self.annotations.retain(|a| a.id != id);
        self.annotations.len() < len_before
    }

    /// Clear all annotations.
    pub fn clear(&mut self) {
        self.annotations.clear();
    }

    /// Get all annotations.
    #[must_use]
    pub fn get_annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Number of annotations.
    #[must_use]
    pub fn count(&self) -> usize {
        self.annotations.len()
    }

    /// Apply a redaction (black filled rectangle) at the given region.
    pub fn apply_redaction(&mut self, rect: Rect) -> Uuid {
        let annotation = Annotation::new(AnnotationKind::Redaction, rect, Color::BLACK);
        self.add_annotation(annotation)
    }

    /// Save the canvas annotations to a JSON file.
    pub fn save_to_file(&self, path: &Path) -> Result<(), SelahError> {
        let layer = AnnotationLayer {
            version: LAYER_VERSION,
            width: self.width,
            height: self.height,
            created: chrono::Utc::now(),
            annotations: self.annotations.clone(),
        };
        let json = serde_json::to_string_pretty(&layer).map_err(|e| {
            SelahError::AnnotationError(format!("failed to serialize annotations: {e}"))
        })?;
        std::fs::write(path, json)?;
        tracing::debug!(path = %path.display(), annotations = self.annotations.len(), "saved annotation layer");
        Ok(())
    }

    /// Load a canvas from a previously saved JSON file.
    ///
    /// Files larger than [`MAX_LAYER_FILE_BYTES`] or written by a newer
    /// layer format than [`LAYER_VERSION`] are rejected.
    pub fn load_from_file(path: &Path) -> Result<Self, SelahError> {
        let json = crate::fsutil::read_limited(path, MAX_LAYER_FILE_BYTES)?;
        let layer: AnnotationLayer = serde_json::from_slice(&json).map_err(|e| {
            SelahError::AnnotationError(format!("failed to deserialize annotations: {e}"))
        })?;
        if layer.version > LAYER_VERSION {
            return Err(SelahError::AnnotationError(format!(
                "unsupported annotation layer version {} (this build supports up to {LAYER_VERSION})",
                layer.version
            )));
        }
        tracing::debug!(path = %path.display(), annotations = layer.annotations.len(), "loaded annotation layer");
        Ok(Self {
            width: layer.width,
            height: layer.height,
            annotations: layer.annotations,
        })
    }

    /// Render annotations onto a source image, returning the modified image as encoded bytes.
    ///
    /// Supports redaction (black fill), highlight (semi-transparent overlay),
    /// rectangles, circles, arrows, and text placeholders drawn with pixel ops.
    ///
    /// Area annotations with negative width/height are normalized first, and
    /// redactions cover every pixel the region touches (fractional edges are
    /// rounded outward). A redaction whose region contains NaN is rejected
    /// rather than silently skipped. Images larger than
    /// [`MAX_IMAGE_DIMENSION`] in either axis are rejected.
    pub fn render_to_image(
        source: &[u8],
        annotations: &[Annotation],
        format: crate::core::ImageFormat,
    ) -> Result<Vec<u8>, SelahError> {
        if let Some(bad) = annotations
            .iter()
            .find(|a| a.kind == AnnotationKind::Redaction && rect_has_nan(&a.position))
        {
            return Err(SelahError::AnnotationError(format!(
                "redaction {} has a non-numeric (NaN) region",
                bad.id
            )));
        }

        let rgba = decode_image(source)?.into_rgba8();
        let (w, h) = rgba.dimensions();
        let _span = tracing::debug_span!(
            "render_to_image",
            width = w,
            height = h,
            annotations = annotations.len(),
            format = %format
        )
        .entered();

        // Hand the decoded pixels to ranga for drawing (no copy)
        let mut buf = PixelBuffer::new(rgba.into_raw(), w, h, PixelFormat::Rgba8)
            .map_err(|e| SelahError::AnnotationError(format!("pixel buffer error: {e}")))?;

        for ann in annotations {
            draw_annotation(&mut buf, ann);
        }

        let rgba = RgbaImage::from_raw(w, h, buf.into_data())
            .ok_or_else(|| SelahError::AnnotationError("buffer size mismatch".into()))?;
        encode_image(DynamicImage::ImageRgba8(rgba), format)
    }

    /// Render all annotations as an SVG overlay string.
    ///
    /// Text is XML-escaped, area shapes are normalized (SVG rejects negative
    /// sizes), non-finite coordinates are written as `0`, and each arrow gets
    /// its own arrowhead marker in its stroke color.
    #[must_use]
    pub fn to_svg(&self) -> String {
        let mut svg = String::with_capacity(96 + self.annotations.len() * 160);
        // `write!` into a String cannot fail.
        let _ = write!(
            svg,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"#,
            self.width, self.height
        );
        svg.push('\n');

        for ann in &self.annotations {
            let _ = write_svg_element(&mut svg, ann);
            svg.push('\n');
        }

        svg.push_str("</svg>");
        svg
    }
}

/// Map non-finite values to 0 so SVG attributes stay parseable.
#[inline]
fn num(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

#[inline]
fn rect_has_nan(r: &Rect) -> bool {
    r.x().is_nan() || r.y().is_nan() || r.width().is_nan() || r.height().is_nan()
}

fn write_svg_element(svg: &mut String, ann: &Annotation) -> std::fmt::Result {
    let color = ann.color.css();
    let n = ann.position.normalized();
    let (x, y, w, h) = (num(n.x()), num(n.y()), num(n.width()), num(n.height()));

    match ann.kind {
        AnnotationKind::Rectangle => write!(
            svg,
            r#"  <rect x="{x}" y="{y}" width="{w}" height="{h}" stroke="{color}" fill="none" stroke-width="{STROKE_WIDTH}"/>"#
        ),
        AnnotationKind::Circle => {
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            let (rx, ry) = (w / 2.0, h / 2.0);
            write!(
                svg,
                r#"  <ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}" stroke="{color}" fill="none" stroke-width="{STROKE_WIDTH}"/>"#
            )
        }
        AnnotationKind::Arrow => {
            // Arrows keep their signed size: direction matters.
            let p = &ann.position;
            let (x1, y1) = (num(p.x()), num(p.y()));
            let (x2, y2) = (num(p.x() + p.width()), num(p.y() + p.height()));
            let id = ann.id.simple();
            write!(
                svg,
                r#"  <defs><marker id="arrow-{id}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto"><path d="M0,0 L10,5 L0,10 z" fill="{color}"/></marker></defs>"#
            )?;
            write!(
                svg,
                r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{color}" stroke-width="{STROKE_WIDTH}" marker-end="url(#arrow-{id})"/>"#
            )
        }
        AnnotationKind::Text => {
            write!(
                svg,
                r#"  <text x="{x}" y="{}" fill="{color}" font-size="16">"#,
                y + 16.0
            )?;
            push_xml_escaped(svg, ann.text.as_deref().unwrap_or(""));
            svg.push_str("</text>");
            Ok(())
        }
        AnnotationKind::Highlight => write!(
            svg,
            r#"  <rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{color}" opacity="0.3"/>"#
        ),
        AnnotationKind::Redaction => write!(
            svg,
            r#"  <rect x="{x}" y="{y}" width="{w}" height="{h}" fill="black"/>"#
        ),
        AnnotationKind::FreeForm => write!(
            svg,
            r#"  <rect x="{x}" y="{y}" width="{w}" height="{h}" stroke="{color}" fill="none" stroke-width="{STROKE_WIDTH}" stroke-dasharray="4"/>"#
        ),
    }
}

// ---------------------------------------------------------------------------
// Rasterization
// ---------------------------------------------------------------------------

#[inline]
fn rgba(color: &Color) -> [u8; 4] {
    [color.r, color.g, color.b, color.a]
}

fn draw_annotation(buf: &mut PixelBuffer, ann: &Annotation) {
    let pos = &ann.position;
    let color = &ann.color;
    match ann.kind {
        AnnotationKind::Redaction => fill_rect(buf, pos, Color::BLACK),
        AnnotationKind::Highlight => blend_rect(buf, pos, color, HIGHLIGHT_OPACITY),
        AnnotationKind::Rectangle => stroke_rect(buf, pos, color, STROKE_WIDTH),
        AnnotationKind::Circle => stroke_ellipse(buf, pos, color, STROKE_WIDTH),
        AnnotationKind::Arrow => draw_line(
            buf,
            (f64::from(pos.x()), f64::from(pos.y())),
            (
                f64::from(pos.x() + pos.width()),
                f64::from(pos.y() + pos.height()),
            ),
            color,
            STROKE_WIDTH,
        ),
        AnnotationKind::Text | AnnotationKind::FreeForm => {
            // Text rendering without a font rasterizer: draw a colored underline bar
            let n = pos.normalized();
            let underline = Rect::new(n.x(), n.y() + n.height() - 2.0, n.width(), 2.0);
            fill_rect(buf, &underline, *color);
        }
    }
}

/// Pixel bounds `[x0, x1) × [y0, y1)` of every pixel `rect` touches,
/// clipped to the image. Fractional edges round outward (floor/ceil) so a
/// partially covered pixel is included — required for redaction to be
/// leak-free. Float→int casts saturate, so huge or infinite coordinates clamp
/// to the image edge and NaN yields an empty span.
#[inline]
fn pixel_bounds(rect: &Rect, width: u32, height: u32) -> Option<(usize, usize, usize, usize)> {
    let n = rect.normalized();
    let r = n.as_hisab();
    let clamp = |v: f32, max: u32| (v as i64).clamp(0, i64::from(max)) as usize;
    let x0 = clamp(r.min.x.floor(), width);
    let y0 = clamp(r.min.y.floor(), height);
    let x1 = clamp(r.max.x.ceil(), width);
    let y1 = clamp(r.max.y.ceil(), height);
    (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
}

/// Mutable RGBA8 rows `y0..y1`, each sliced to columns `x0..x1`.
#[inline]
fn row_spans(
    buf: &mut PixelBuffer,
    (x0, y0, x1, y1): (usize, usize, usize, usize),
) -> impl Iterator<Item = &mut [u8]> {
    let stride = buf.width() as usize * 4;
    buf.data_mut()
        .chunks_exact_mut(stride)
        .skip(y0)
        .take(y1 - y0)
        .map(move |row| &mut row[x0 * 4..x1 * 4])
}

/// Fill a rectangle with a solid color (no blending).
fn fill_rect(buf: &mut PixelBuffer, rect: &Rect, color: Color) {
    let Some(bounds) = pixel_bounds(rect, buf.width(), buf.height()) else {
        return;
    };
    let px = rgba(&color);
    for row in row_spans(buf, bounds) {
        for p in row.as_chunks_mut::<4>().0 {
            *p = px;
        }
    }
}

/// Blend an opaque `color` over existing pixels at `opacity` (row-wise SIMD).
fn blend_rect(buf: &mut PixelBuffer, rect: &Rect, color: &Color, opacity: u8) {
    let Some(bounds @ (x0, _, x1, _)) = pixel_bounds(rect, buf.width(), buf.height()) else {
        return;
    };
    let overlay = [color.r, color.g, color.b, 255].repeat(x1 - x0);
    for row in row_spans(buf, bounds) {
        blend_row(&overlay, row, BlendMode::Normal, opacity);
    }
}

/// Draw a rectangle outline.
fn stroke_rect(buf: &mut PixelBuffer, rect: &Rect, color: &Color, thickness: u32) {
    let n = rect.normalized();
    let t = thickness as f32;
    let (x, y, w, h) = (n.x(), n.y(), n.width(), n.height());
    let c = *color;
    fill_rect(buf, &Rect::new(x, y, w, t), c); // top
    fill_rect(buf, &Rect::new(x, y + h - t, w, t), c); // bottom
    fill_rect(buf, &Rect::new(x, y, t, h), c); // left
    fill_rect(buf, &Rect::new(x + w - t, y, t, h), c); // right
}

/// Fill a `size`×`size` square with top-left corner at (`x`, `y`), clipped.
#[inline]
fn stamp(buf: &mut PixelBuffer, x: i64, y: i64, size: u32, px: [u8; 4]) {
    let (w, h) = (i64::from(buf.width()), i64::from(buf.height()));
    let size = i64::from(size.max(1));
    let (x0, y0) = (x.clamp(0, w), y.clamp(0, h));
    let (x1, y1) = (
        x.saturating_add(size).clamp(0, w),
        y.saturating_add(size).clamp(0, h),
    );
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let bounds = (x0 as usize, y0 as usize, x1 as usize, y1 as usize);
    for row in row_spans(buf, bounds) {
        for p in row.as_chunks_mut::<4>().0 {
            *p = px;
        }
    }
}

/// Draw an ellipse outline inscribed in the given rect.
fn stroke_ellipse(buf: &mut PixelBuffer, rect: &Rect, color: &Color, thickness: u32) {
    let n = rect.normalized();
    if !n.is_finite() {
        return;
    }
    // Skip ellipses whose bounding box (plus stroke) misses the image entirely.
    let t = thickness as f32;
    let reach = Rect::new(
        n.x() - t,
        n.y() - t,
        n.width() + 2.0 * t,
        n.height() + 2.0 * t,
    );
    if pixel_bounds(&reach, buf.width(), buf.height()).is_none() {
        return;
    }

    let rx = f64::from(n.width()) / 2.0;
    let ry = f64::from(n.height()) / 2.0;
    let cx = f64::from(n.x()) + rx;
    let cy = f64::from(n.y()) + ry;
    let px = rgba(color);
    let half = i64::from(thickness / 2);
    // Sample points far off-canvas are clamped (in f64, before the integer
    // cast) to just outside it, so huge radii cannot overflow i64 math.
    let (lo_x, hi_x) = (-f64::from(thickness) - 1.0, f64::from(buf.width()) + 1.0);
    let (lo_y, hi_y) = (-f64::from(thickness) - 1.0, f64::from(buf.height()) + 1.0);

    // ~4 samples per unit of (rx + ry) keeps the outline gap-free; capped so
    // huge radii stay bounded (they are mostly off-canvas anyway).
    let steps = ((rx + ry) * 4.0).ceil().clamp(0.0, MAX_ELLIPSE_STEPS) as u32;
    for i in 0..steps {
        let angle = std::f64::consts::TAU * f64::from(i) / f64::from(steps);
        let ex = (cx + rx * angle.cos()).clamp(lo_x, hi_x) as i64;
        let ey = (cy + ry * angle.sin()).clamp(lo_y, hi_y) as i64;
        stamp(buf, ex - half, ey - half, thickness, px);
    }
}

/// Clip segment `a`–`b` to the box `[min, max]` (Liang–Barsky).
/// Returns `None` if the segment misses the box or has non-finite endpoints.
fn clip_line(
    (x0, y0): (f64, f64),
    (x1, y1): (f64, f64),
    (xmin, ymin): (f64, f64),
    (xmax, ymax): (f64, f64),
) -> Option<((f64, f64), (f64, f64))> {
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
        return None;
    }
    let (dx, dy) = (x1 - x0, y1 - y0);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-dx, x0 - xmin),
        (dx, xmax - x0),
        (-dy, y0 - ymin),
        (dy, ymax - y0),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None; // parallel to and outside this edge
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return None;
                }
                t0 = t0.max(r);
            } else {
                if r < t0 {
                    return None;
                }
                t1 = t1.min(r);
            }
        }
    }
    Some(((x0 + t0 * dx, y0 + t0 * dy), (x0 + t1 * dx, y0 + t1 * dy)))
}

/// Draw a line using Bresenham's algorithm with thickness.
///
/// The segment is clipped to the image (plus stroke margin) first, so the
/// iteration count is bounded by the image size no matter how far outside
/// the canvas the endpoints lie, and integer math cannot overflow.
fn draw_line(
    buf: &mut PixelBuffer,
    from: (f64, f64),
    to: (f64, f64),
    color: &Color,
    thickness: u32,
) {
    let margin = f64::from(thickness) + 1.0;
    let max = (
        f64::from(buf.width()) + margin,
        f64::from(buf.height()) + margin,
    );
    let Some(((x0, y0), (x1, y1))) = clip_line(from, to, (-margin, -margin), max) else {
        return;
    };

    let px = rgba(color);
    let half = i64::from(thickness / 2);
    let side = thickness / 2 * 2 + 1; // square brush covering [-half, half]

    let (mut cx, mut cy) = (x0 as i64, y0 as i64);
    let (ex, ey) = (x1 as i64, y1 as i64);
    let dx = (ex - cx).abs();
    let dy = -(ey - cy).abs();
    let sx = if cx < ex { 1 } else { -1 };
    let sy = if cy < ey { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        stamp(buf, cx - half, cy - half, side, px);
        if cx == ex && cy == ey {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            cx += sx;
        }
        if e2 <= dx {
            err += dx;
            cy += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_canvas() -> AnnotationCanvas {
        AnnotationCanvas::new(1920, 1080)
    }

    #[test]
    fn test_new_canvas_empty() {
        let canvas = make_canvas();
        assert_eq!(canvas.count(), 0);
        assert_eq!(canvas.width, 1920);
        assert_eq!(canvas.height, 1080);
    }

    #[test]
    fn test_add_annotation() {
        let mut canvas = make_canvas();
        let ann = Annotation::new(
            AnnotationKind::Rectangle,
            Rect::new(10.0, 10.0, 100.0, 50.0),
            Color::RED,
        );
        canvas.add_annotation(ann);
        assert_eq!(canvas.count(), 1);
    }

    #[test]
    fn test_remove_annotation() {
        let mut canvas = make_canvas();
        let ann = Annotation::new(
            AnnotationKind::Arrow,
            Rect::new(0.0, 0.0, 50.0, 50.0),
            Color::BLUE,
        );
        let id = canvas.add_annotation(ann);
        assert_eq!(canvas.count(), 1);
        assert!(canvas.remove_annotation(id));
        assert_eq!(canvas.count(), 0);
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut canvas = make_canvas();
        assert!(!canvas.remove_annotation(Uuid::new_v4()));
    }

    #[test]
    fn test_clear() {
        let mut canvas = make_canvas();
        for _ in 0..5 {
            canvas.add_annotation(Annotation::new(
                AnnotationKind::Circle,
                Rect::new(0.0, 0.0, 30.0, 30.0),
                Color::GREEN,
            ));
        }
        assert_eq!(canvas.count(), 5);
        canvas.clear();
        assert_eq!(canvas.count(), 0);
    }

    #[test]
    fn test_get_annotations() {
        let mut canvas = make_canvas();
        canvas.add_annotation(Annotation::new(
            AnnotationKind::Text,
            Rect::new(0.0, 0.0, 200.0, 30.0),
            Color::BLACK,
        ));
        let anns = canvas.get_annotations();
        assert_eq!(anns.len(), 1);
        assert_eq!(anns[0].kind, AnnotationKind::Text);
    }

    #[test]
    fn test_apply_redaction() {
        let mut canvas = make_canvas();
        let id = canvas.apply_redaction(Rect::new(100.0, 200.0, 300.0, 50.0));
        let anns = canvas.get_annotations();
        assert_eq!(anns.len(), 1);
        assert_eq!(anns[0].id, id);
        assert_eq!(anns[0].kind, AnnotationKind::Redaction);
        assert_eq!(anns[0].color, Color::BLACK);
    }

    #[test]
    fn test_svg_output_contains_svg_tag() {
        let canvas = make_canvas();
        let svg = canvas.to_svg();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"1920\""));
        assert!(svg.contains("height=\"1080\""));
    }

    #[test]
    fn test_svg_rectangle() {
        let mut canvas = make_canvas();
        canvas.add_annotation(Annotation::new(
            AnnotationKind::Rectangle,
            Rect::new(10.0, 20.0, 100.0, 50.0),
            Color::RED,
        ));
        let svg = canvas.to_svg();
        assert!(svg.contains("<rect"));
        assert!(svg.contains("x=\"10\""));
        assert!(svg.contains("stroke=\"rgba(255,0,0,1.00)\""));
    }

    #[test]
    fn test_svg_redaction() {
        let mut canvas = make_canvas();
        canvas.apply_redaction(Rect::new(0.0, 0.0, 200.0, 30.0));
        let svg = canvas.to_svg();
        assert!(svg.contains("fill=\"black\""));
    }

    #[test]
    fn test_svg_text() {
        let mut canvas = make_canvas();
        canvas.add_annotation(Annotation::with_text(
            AnnotationKind::Text,
            Rect::new(50.0, 50.0, 200.0, 30.0),
            Color::WHITE,
            "Hello World".into(),
        ));
        let svg = canvas.to_svg();
        assert!(svg.contains("<text"));
        assert!(svg.contains("Hello World"));
    }

    #[test]
    fn test_save_and_load_annotations() {
        let mut canvas = make_canvas();
        canvas.add_annotation(Annotation::new(
            AnnotationKind::Rectangle,
            Rect::new(10.0, 20.0, 100.0, 50.0),
            Color::RED,
        ));
        canvas.add_annotation(Annotation::with_text(
            AnnotationKind::Text,
            Rect::new(50.0, 50.0, 200.0, 30.0),
            Color::WHITE,
            "Test".into(),
        ));

        let path = std::env::temp_dir().join(format!("selah_test_layer_{}.json", Uuid::new_v4()));
        canvas.save_to_file(&path).unwrap();

        let loaded = AnnotationCanvas::load_from_file(&path).unwrap();
        assert_eq!(loaded.width, 1920);
        assert_eq!(loaded.height, 1080);
        assert_eq!(loaded.count(), 2);

        let anns = loaded.get_annotations();
        assert_eq!(anns[0].kind, AnnotationKind::Rectangle);
        assert_eq!(anns[1].kind, AnnotationKind::Text);
        assert_eq!(anns[1].text.as_deref(), Some("Test"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_save_format_version() {
        let canvas = make_canvas();
        let path =
            std::env::temp_dir().join(format!("selah_test_layer_ver_{}.json", Uuid::new_v4()));
        canvas.save_to_file(&path).unwrap();

        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["width"], 1920);
        assert_eq!(json["height"], 1080);
        assert!(json["created"].is_string());
        assert!(json["annotations"].is_array());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_convert_format_png_to_bmp() {
        // Create a minimal 1x1 PNG in memory
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let png_bytes = buf.into_inner();

        let bmp_bytes = convert_format(&png_bytes, crate::core::ImageFormat::Bmp).unwrap();
        assert!(!bmp_bytes.is_empty());
        // BMP files start with "BM"
        assert_eq!(&bmp_bytes[0..2], b"BM");
    }

    #[test]
    fn test_svg_text_xss_safe() {
        let mut canvas = make_canvas();
        canvas.add_annotation(Annotation::with_text(
            AnnotationKind::Text,
            Rect::new(0.0, 0.0, 200.0, 30.0),
            Color::BLACK,
            "<script>alert('xss')</script>".into(),
        ));
        let svg = canvas.to_svg();
        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&lt;script&gt;"));
    }

    #[test]
    fn test_svg_all_annotation_kinds() {
        let mut canvas = make_canvas();
        canvas.add_annotation(Annotation::new(
            AnnotationKind::Arrow,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Color::RED,
        ));
        canvas.add_annotation(Annotation::new(
            AnnotationKind::Circle,
            Rect::new(50.0, 50.0, 80.0, 80.0),
            Color::GREEN,
        ));
        canvas.add_annotation(Annotation::new(
            AnnotationKind::Highlight,
            Rect::new(10.0, 10.0, 200.0, 30.0),
            Color::YELLOW,
        ));
        canvas.add_annotation(Annotation::new(
            AnnotationKind::FreeForm,
            Rect::new(0.0, 0.0, 50.0, 50.0),
            Color::BLUE,
        ));
        let svg = canvas.to_svg();
        assert!(svg.contains("<line"));
        assert!(svg.contains("<ellipse"));
        assert!(svg.contains("opacity=\"0.3\""));
        assert!(svg.contains("stroke-dasharray"));
    }

    #[test]
    fn test_render_to_image_small_png() {
        // Create a 4x4 red PNG
        let mut img = RgbaImage::new(4, 4);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgba([255, 0, 0, 255]);
        }
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let png_bytes = buf.into_inner();

        // Add a black redaction covering the top-left 2x2 area
        let annotations = vec![Annotation::new(
            AnnotationKind::Redaction,
            Rect::new(0.0, 0.0, 2.0, 2.0),
            Color::BLACK,
        )];

        let result = AnnotationCanvas::render_to_image(
            &png_bytes,
            &annotations,
            crate::core::ImageFormat::Png,
        )
        .unwrap();

        // Verify output is valid PNG
        let output_img = image::load_from_memory(&result).unwrap().to_rgba8();
        assert_eq!(output_img.width(), 4);
        assert_eq!(output_img.height(), 4);

        // Top-left pixel should be black (redacted)
        let p = output_img.get_pixel(0, 0);
        assert_eq!(p.0, [0, 0, 0, 255]);

        // Bottom-right pixel should still be red
        let p = output_img.get_pixel(3, 3);
        assert_eq!(p.0, [255, 0, 0, 255]);
    }

    #[test]
    fn test_render_to_image_with_multiple_annotations() {
        let mut img = RgbaImage::new(10, 10);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgba([255, 255, 255, 255]);
        }
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        let png_bytes = buf.into_inner();

        let annotations = vec![
            Annotation::new(
                AnnotationKind::Rectangle,
                Rect::new(1.0, 1.0, 8.0, 8.0),
                Color::RED,
            ),
            Annotation::new(
                AnnotationKind::Highlight,
                Rect::new(3.0, 3.0, 4.0, 4.0),
                Color::YELLOW,
            ),
        ];

        let result = AnnotationCanvas::render_to_image(
            &png_bytes,
            &annotations,
            crate::core::ImageFormat::Png,
        )
        .unwrap();

        let output_img = image::load_from_memory(&result).unwrap();
        assert_eq!(output_img.width(), 10);
        assert_eq!(output_img.height(), 10);
    }

    #[test]
    fn test_svg_text_with_no_text() {
        let mut canvas = make_canvas();
        canvas.add_annotation(Annotation::new(
            AnnotationKind::Text,
            Rect::new(0.0, 0.0, 100.0, 20.0),
            Color::BLACK,
        ));
        let svg = canvas.to_svg();
        // Should produce a <text> element with empty content
        assert!(svg.contains("<text"));
    }

    // -- hardening tests ---------------------------------------------------

    fn solid_png(w: u32, h: u32, px: [u8; 4]) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, image::Rgba(px));
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    fn render(src: &[u8], anns: &[Annotation]) -> RgbaImage {
        let out =
            AnnotationCanvas::render_to_image(src, anns, crate::core::ImageFormat::Png).unwrap();
        image::load_from_memory(&out).unwrap().to_rgba8()
    }

    const WHITE: [u8; 4] = [255, 255, 255, 255];
    const BLACK: [u8; 4] = [0, 0, 0, 255];

    #[test]
    fn test_redaction_negative_size_is_normalized() {
        // Same region as (0,0)-(4,4), expressed from the opposite corner
        let src = solid_png(6, 6, WHITE);
        let img = render(
            &src,
            &[Annotation::new(
                AnnotationKind::Redaction,
                Rect::new(4.0, 4.0, -4.0, -4.0),
                Color::BLACK,
            )],
        );
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(img.get_pixel(x, y).0, BLACK, "pixel ({x},{y}) leaked");
            }
        }
        assert_eq!(img.get_pixel(5, 5).0, WHITE);
    }

    #[test]
    fn test_redaction_fractional_edges_round_outward() {
        let src = solid_png(6, 6, WHITE);
        let img = render(
            &src,
            &[Annotation::new(
                AnnotationKind::Redaction,
                Rect::new(0.5, 0.5, 1.0, 1.0),
                Color::BLACK,
            )],
        );
        // The region touches pixels 0 and 1 on both axes
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            assert_eq!(img.get_pixel(x, y).0, BLACK, "pixel ({x},{y}) leaked");
        }
        assert_eq!(img.get_pixel(2, 2).0, WHITE);
    }

    #[test]
    fn test_redaction_infinite_extent_clamps_to_edges() {
        let src = solid_png(4, 4, WHITE);
        let img = render(
            &src,
            &[Annotation::new(
                AnnotationKind::Redaction,
                Rect::new(2.0, 0.0, f32::INFINITY, 4.0),
                Color::BLACK,
            )],
        );
        assert_eq!(img.get_pixel(3, 3).0, BLACK);
        assert_eq!(img.get_pixel(1, 1).0, WHITE);
    }

    #[test]
    fn test_redaction_nan_region_rejected() {
        let src = solid_png(4, 4, WHITE);
        let err = AnnotationCanvas::render_to_image(
            &src,
            &[Annotation::new(
                AnnotationKind::Redaction,
                Rect::new(f32::NAN, 0.0, 2.0, 2.0),
                Color::BLACK,
            )],
            crate::core::ImageFormat::Png,
        )
        .unwrap_err();
        assert!(err.to_string().contains("NaN"));
    }

    #[test]
    fn test_huge_coordinates_terminate_without_panic() {
        // Previously: i32 overflow in Bresenham and ~billions of loop iterations
        let src = solid_png(16, 16, WHITE);
        let anns = [
            Annotation::new(
                AnnotationKind::Arrow,
                Rect::new(-1e30, -1e30, 2e30, 2e30),
                Color::RED,
            ),
            Annotation::new(
                AnnotationKind::Arrow,
                Rect::new(f32::MAX, 0.0, f32::MIN, 0.0),
                Color::RED,
            ),
            Annotation::new(
                AnnotationKind::Circle,
                Rect::new(-1e9, -1e9, 2e9, 2e9),
                Color::GREEN,
            ),
            Annotation::new(
                AnnotationKind::Circle,
                Rect::new(1e9, 1e9, 10.0, 10.0),
                Color::GREEN,
            ),
            Annotation::new(
                AnnotationKind::Rectangle,
                Rect::new(f32::NAN, 0.0, 1e38, f32::INFINITY),
                Color::BLUE,
            ),
            Annotation::new(
                AnnotationKind::Highlight,
                Rect::new(-1e38, -1e38, f32::INFINITY, f32::INFINITY),
                Color::YELLOW,
            ),
        ];
        let start = std::time::Instant::now();
        let img = render(&src, &anns);
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        // The diagonal arrow crosses the canvas and must be visible
        assert_eq!(img.get_pixel(8, 8).0[0], 255);
        assert_ne!(img.get_pixel(8, 8).0, WHITE);
    }

    #[test]
    fn test_arrow_from_offscreen_draws_visible_part() {
        let src = solid_png(10, 10, WHITE);
        let img = render(
            &src,
            &[Annotation::new(
                AnnotationKind::Arrow,
                Rect::new(-100.0, 5.0, 200.0, 0.0),
                Color::RED,
            )],
        );
        for x in 0..10 {
            assert_eq!(img.get_pixel(x, 5).0, [255, 0, 0, 255], "x={x}");
        }
        assert_eq!(img.get_pixel(5, 0).0, WHITE);
    }

    #[test]
    fn test_arrow_negative_direction_draws() {
        let src = solid_png(10, 10, WHITE);
        let img = render(
            &src,
            &[Annotation::new(
                AnnotationKind::Arrow,
                Rect::new(9.0, 9.0, -9.0, -9.0),
                Color::BLUE,
            )],
        );
        assert_eq!(img.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(img.get_pixel(5, 5).0, [0, 0, 255, 255]);
    }

    #[test]
    fn test_highlight_blends_over_background() {
        let src = solid_png(4, 4, WHITE);
        let img = render(
            &src,
            &[Annotation::new(
                AnnotationKind::Highlight,
                Rect::new(0.0, 0.0, 4.0, 4.0),
                Color::new(255, 255, 0, 255),
            )],
        );
        let [r, g, b, a] = img.get_pixel(1, 1).0;
        assert_eq!((r, g, a), (255, 255, 255));
        // ~30% yellow over white: blue drops to ~70%
        assert!((170..=185).contains(&b), "blue = {b}");
    }

    #[test]
    fn test_circle_outline_drawn() {
        let src = solid_png(20, 20, WHITE);
        let img = render(
            &src,
            &[Annotation::new(
                AnnotationKind::Circle,
                Rect::new(2.0, 2.0, 16.0, 16.0),
                Color::GREEN,
            )],
        );
        assert_eq!(img.get_pixel(10, 2).0, [0, 255, 0, 255]); // top of the circle
        assert_eq!(img.get_pixel(10, 10).0, WHITE); // center untouched
    }

    #[test]
    fn test_rectangle_negative_size_outline() {
        let src = solid_png(10, 10, WHITE);
        let img = render(
            &src,
            &[Annotation::new(
                AnnotationKind::Rectangle,
                Rect::new(9.0, 9.0, -8.0, -8.0),
                Color::RED,
            )],
        );
        assert_eq!(img.get_pixel(1, 1).0, [255, 0, 0, 255]);
        assert_eq!(img.get_pixel(5, 5).0, WHITE);
    }

    #[test]
    fn test_render_to_jpeg_and_webp() {
        // JPEG encoding of RGBA used to fail outright
        let src = solid_png(8, 8, [10, 20, 30, 255]);
        let anns = [Annotation::new(
            AnnotationKind::Redaction,
            Rect::new(0.0, 0.0, 4.0, 4.0),
            Color::BLACK,
        )];
        let jpg =
            AnnotationCanvas::render_to_image(&src, &anns, crate::core::ImageFormat::Jpeg).unwrap();
        assert_eq!(&jpg[..2], &[0xFF, 0xD8]);
        assert_eq!(image::load_from_memory(&jpg).unwrap().width(), 8);

        let webp =
            AnnotationCanvas::render_to_image(&src, &anns, crate::core::ImageFormat::WebP).unwrap();
        assert_eq!(&webp[8..12], b"WEBP");
    }

    #[test]
    fn test_convert_format_all_targets() {
        let src = solid_png(3, 3, [1, 2, 3, 255]);
        for fmt in [
            crate::core::ImageFormat::Png,
            crate::core::ImageFormat::Jpeg,
            crate::core::ImageFormat::Bmp,
            crate::core::ImageFormat::WebP,
        ] {
            let out = convert_format(&src, fmt).unwrap();
            let img = image::load_from_memory(&out).unwrap();
            assert_eq!((img.width(), img.height()), (3, 3), "{fmt}");
        }
    }

    #[test]
    fn test_decode_rejects_invalid_data() {
        let err =
            convert_format(b"definitely not an image", crate::core::ImageFormat::Png).unwrap_err();
        assert!(err.to_string().contains("failed to load image"));
    }

    #[test]
    fn test_decode_limits_enforced() {
        let src = solid_png(8, 8, WHITE);
        let mut limits = Limits::default();
        limits.max_image_width = Some(4);
        assert!(decode_image_with_limits(&src, limits).is_err());
        assert!(decode_image(&src).is_ok());
    }

    #[test]
    fn test_decode_rejects_oversized_header() {
        // Hand-crafted 54-byte BMP claiming 40000x40000 RGBA (6.4 GB of pixels).
        // Must be rejected from the header alone, without allocating.
        let mut bmp = Vec::new();
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&54u32.to_le_bytes()); // file size
        bmp.extend_from_slice(&[0; 4]); // reserved
        bmp.extend_from_slice(&54u32.to_le_bytes()); // pixel data offset
        bmp.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER size
        bmp.extend_from_slice(&40_000i32.to_le_bytes()); // width
        bmp.extend_from_slice(&40_000i32.to_le_bytes()); // height
        bmp.extend_from_slice(&1u16.to_le_bytes()); // planes
        bmp.extend_from_slice(&32u16.to_le_bytes()); // bpp
        bmp.extend_from_slice(&[0; 24]); // compression .. important colors
        let err = AnnotationCanvas::render_to_image(&bmp, &[], crate::core::ImageFormat::Png)
            .unwrap_err();
        assert!(err.to_string().contains("failed to load image"), "{err}");
    }

    #[test]
    fn test_huge_circle_partially_on_canvas_no_overflow() {
        // Passes the finiteness and on-canvas checks, but sample points land
        // at ±1e20 — previously saturated to i64::MAX and overflowed in `stamp`.
        let src = solid_png(16, 16, WHITE);
        let img = render(
            &src,
            &[Annotation::new(
                AnnotationKind::Circle,
                Rect::new(-1e20, 0.0, 2e20, 100.0),
                Color::GREEN,
            )],
        );
        assert_eq!(img.dimensions(), (16, 16));
    }

    #[test]
    fn test_stamp_saturates_at_extremes() {
        let mut buf = PixelBuffer::new(vec![0; 4 * 4 * 4], 4, 4, PixelFormat::Rgba8).unwrap();
        stamp(&mut buf, i64::MAX - 1, 0, 3, [1, 2, 3, 4]);
        stamp(&mut buf, i64::MIN, i64::MIN, 3, [1, 2, 3, 4]);
        assert!(buf.data().iter().all(|&b| b == 0));
        stamp(&mut buf, -1, -1, 3, [9, 9, 9, 9]);
        assert_eq!(buf.get_rgba(1, 1), Some([9, 9, 9, 9]));
    }

    #[test]
    fn test_decode_budgets_rgba_expansion() {
        // A grayscale image's native buffer fits the budget, but its RGBA
        // working copy (4×) does not: must be rejected up front.
        let img = image::GrayImage::new(64, 64); // 4 KiB native, 16 KiB RGBA
        let mut png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let mut limits = Limits::default();
        limits.max_alloc = Some(8 * 1024);
        let err = decode_image_with_limits(png.get_ref(), limits).unwrap_err();
        assert!(err.to_string().contains("needs 16384 bytes"), "{err}");
        let mut limits = Limits::default();
        limits.max_alloc = Some(16 * 1024);
        assert!(decode_image_with_limits(png.get_ref(), limits).is_ok());
    }

    #[test]
    fn test_clip_line() {
        let min = (0.0, 0.0);
        let max = (10.0, 10.0);
        // Fully inside: unchanged
        assert_eq!(
            clip_line((1.0, 1.0), (5.0, 5.0), min, max),
            Some(((1.0, 1.0), (5.0, 5.0)))
        );
        // Crossing: clipped to the box
        assert_eq!(
            clip_line((-10.0, 5.0), (20.0, 5.0), min, max),
            Some(((0.0, 5.0), (10.0, 5.0)))
        );
        // Entirely outside
        assert_eq!(clip_line((20.0, 0.0), (30.0, 10.0), min, max), None);
        assert_eq!(clip_line((-5.0, -1.0), (15.0, -1.0), min, max), None);
        // Non-finite endpoints
        assert_eq!(clip_line((f64::NAN, 0.0), (1.0, 1.0), min, max), None);
        assert_eq!(clip_line((0.0, 0.0), (f64::INFINITY, 1.0), min, max), None);
    }

    #[test]
    fn test_svg_arrow_has_matching_marker() {
        let mut canvas = make_canvas();
        let id = canvas.add_annotation(Annotation::new(
            AnnotationKind::Arrow,
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Color::RED,
        ));
        let svg = canvas.to_svg();
        let marker = format!("id=\"arrow-{}\"", id.simple());
        let reference = format!("url(#arrow-{})", id.simple());
        assert!(svg.contains(&marker), "{svg}");
        assert!(svg.contains(&reference), "{svg}");
        assert!(svg.contains("<marker"));
    }

    #[test]
    fn test_svg_negative_rect_normalized() {
        let mut canvas = make_canvas();
        canvas.apply_redaction(Rect::new(100.0, 100.0, -50.0, -30.0));
        let svg = canvas.to_svg();
        assert!(
            svg.contains(r#"x="50" y="70" width="50" height="30""#),
            "{svg}"
        );
        assert!(!svg.contains("width=\"-"));
    }

    #[test]
    fn test_svg_non_finite_written_as_zero() {
        let mut canvas = make_canvas();
        canvas.add_annotation(Annotation::new(
            AnnotationKind::Rectangle,
            Rect::new(f32::NAN, 1.0, f32::INFINITY, 2.0),
            Color::RED,
        ));
        let svg = canvas.to_svg();
        assert!(!svg.contains("NaN") && !svg.contains("inf"), "{svg}");
    }

    #[test]
    fn test_svg_text_strips_control_chars() {
        let mut canvas = make_canvas();
        canvas.add_annotation(Annotation::with_text(
            AnnotationKind::Text,
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Color::BLACK,
            "a\u{0}b".into(),
        ));
        assert!(canvas.to_svg().contains(">ab</text>"));
    }

    #[test]
    fn test_load_rejects_future_layer_version() {
        let path =
            std::env::temp_dir().join(format!("selah_test_layer_future_{}.json", Uuid::new_v4()));
        let json = serde_json::json!({
            "version": LAYER_VERSION + 1,
            "width": 1,
            "height": 1,
            "created": chrono::Utc::now(),
            "annotations": []
        });
        std::fs::write(&path, json.to_string()).unwrap();
        let err = AnnotationCanvas::load_from_file(&path).unwrap_err();
        assert!(
            err.to_string()
                .contains("unsupported annotation layer version")
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_rejects_invalid_rect() {
        let path =
            std::env::temp_dir().join(format!("selah_test_layer_badrect_{}.json", Uuid::new_v4()));
        let json = r#"{"version":1,"width":1,"height":1,"created":"2026-01-01T00:00:00Z",
            "annotations":[{"id":"00000000-0000-0000-0000-000000000000","kind":"Redaction",
            "position":{"x":1e39,"y":0,"width":1,"height":1},
            "color":{"r":0,"g":0,"b":0,"a":255},"text":null}]}"#;
        std::fs::write(&path, json).unwrap();
        assert!(AnnotationCanvas::load_from_file(&path).is_err());
        std::fs::remove_file(&path).ok();
    }
}
