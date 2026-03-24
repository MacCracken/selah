//! Annotation engine for Selah.

use crate::core::{Annotation, AnnotationKind, Color, xml_escape};
use crate::error::SelahError;
use crate::geometry::Rect;
use image::ImageFormat;
use image::RgbaImage;
use ranga::pixel::{PixelBuffer, PixelFormat};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

/// Convert image data from one format to another.
pub fn convert_format(
    source: &[u8],
    target: crate::core::ImageFormat,
) -> Result<Vec<u8>, SelahError> {
    let img = image::load_from_memory(source)
        .map_err(|e| SelahError::AnnotationError(format!("failed to load image: {e}")))?;
    let rgba = img.to_rgba8();

    let image_format = match target {
        crate::core::ImageFormat::Png => ImageFormat::Png,
        crate::core::ImageFormat::Jpeg => ImageFormat::Jpeg,
        crate::core::ImageFormat::Bmp => ImageFormat::Bmp,
        crate::core::ImageFormat::WebP => ImageFormat::WebP,
    };

    let mut buf = std::io::Cursor::new(Vec::new());
    rgba.write_to(&mut buf, image_format)
        .map_err(|e| SelahError::AnnotationError(format!("failed to encode image: {e}")))?;
    Ok(buf.into_inner())
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
    pub fn get_annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Number of annotations.
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
            version: 1,
            width: self.width,
            height: self.height,
            created: chrono::Utc::now(),
            annotations: self.annotations.clone(),
        };
        let json = serde_json::to_string_pretty(&layer).map_err(|e| {
            SelahError::AnnotationError(format!("failed to serialize annotations: {e}"))
        })?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load a canvas from a previously saved JSON file.
    pub fn load_from_file(path: &Path) -> Result<Self, SelahError> {
        let json = std::fs::read_to_string(path)?;
        let layer: AnnotationLayer = serde_json::from_str(&json).map_err(|e| {
            SelahError::AnnotationError(format!("failed to deserialize annotations: {e}"))
        })?;
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
    pub fn render_to_image(
        source: &[u8],
        annotations: &[Annotation],
        format: crate::core::ImageFormat,
    ) -> Result<Vec<u8>, SelahError> {
        let img = image::load_from_memory(source)
            .map_err(|e| SelahError::AnnotationError(format!("failed to load image: {e}")))?;
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());

        // Convert image::RgbaImage to ranga::PixelBuffer for drawing
        let mut buf = PixelBuffer::new(rgba.into_raw(), w, h, PixelFormat::Rgba8)
            .map_err(|e| SelahError::AnnotationError(format!("pixel buffer error: {e}")))?;

        for ann in annotations {
            let pos = &ann.position;
            let color = &ann.color;

            match ann.kind {
                AnnotationKind::Redaction => {
                    Self::fill_rect(&mut buf, pos, &Color::BLACK);
                }
                AnnotationKind::Highlight => {
                    Self::blend_rect(&mut buf, pos, color, 0.3);
                }
                AnnotationKind::Rectangle => {
                    Self::stroke_rect(&mut buf, pos, color, 2);
                }
                AnnotationKind::Circle => {
                    Self::stroke_ellipse(&mut buf, pos, color, 2);
                }
                AnnotationKind::Arrow => {
                    Self::draw_line(
                        &mut buf,
                        pos.x() as i32,
                        pos.y() as i32,
                        (pos.x() + pos.width()) as i32,
                        (pos.y() + pos.height()) as i32,
                        color,
                        2,
                    );
                }
                AnnotationKind::Text | AnnotationKind::FreeForm => {
                    // Text rendering without a font rasterizer: draw a colored underline bar
                    Self::fill_rect(
                        &mut buf,
                        &Rect::new(pos.x(), pos.y() + pos.height() - 2.0, pos.width(), 2.0),
                        color,
                    );
                }
            }
        }

        // Convert ranga PixelBuffer back to image::RgbaImage for encoding
        let raw = buf.data;
        let rgba = RgbaImage::from_raw(w, h, raw)
            .ok_or_else(|| SelahError::AnnotationError("buffer size mismatch".into()))?;

        let image_format = match format {
            crate::core::ImageFormat::Png => ImageFormat::Png,
            crate::core::ImageFormat::Jpeg => ImageFormat::Jpeg,
            crate::core::ImageFormat::Bmp => ImageFormat::Bmp,
            crate::core::ImageFormat::WebP => ImageFormat::WebP,
        };

        let mut out = std::io::Cursor::new(Vec::new());
        rgba.write_to(&mut out, image_format)
            .map_err(|e| SelahError::AnnotationError(format!("failed to encode image: {e}")))?;
        Ok(out.into_inner())
    }

    /// Fill a rectangle with a solid color.
    fn fill_rect(buf: &mut PixelBuffer, rect: &Rect, color: &Color) {
        let (iw, ih) = (buf.width as i32, buf.height as i32);
        let x0 = (rect.x() as i32).max(0);
        let y0 = (rect.y() as i32).max(0);
        let x1 = ((rect.x() + rect.width()) as i32).min(iw);
        let y1 = ((rect.y() + rect.height()) as i32).min(ih);
        let pixel = [color.r, color.g, color.b, color.a];
        for y in y0..y1 {
            for x in x0..x1 {
                buf.set_rgba(x as u32, y as u32, pixel);
            }
        }
    }

    /// Blend a semi-transparent rectangle over existing pixels.
    fn blend_rect(buf: &mut PixelBuffer, rect: &Rect, color: &Color, opacity: f32) {
        let (iw, ih) = (buf.width as i32, buf.height as i32);
        let x0 = (rect.x() as i32).max(0);
        let y0 = (rect.y() as i32).max(0);
        let x1 = ((rect.x() + rect.width()) as i32).min(iw);
        let y1 = ((rect.y() + rect.height()) as i32).min(ih);
        let opacity_u8 = (opacity * 255.0) as u8;
        let overlay = [color.r, color.g, color.b, 255];
        for y in y0..y1 {
            for x in x0..x1 {
                let existing = buf.get_rgba(x as u32, y as u32).unwrap_or([0, 0, 0, 0]);
                let blended = ranga::blend::blend_pixel(
                    overlay,
                    existing,
                    ranga::blend::BlendMode::Normal,
                    opacity_u8,
                );
                buf.set_rgba(x as u32, y as u32, blended);
            }
        }
    }

    /// Draw a rectangle outline.
    fn stroke_rect(buf: &mut PixelBuffer, rect: &Rect, color: &Color, thickness: u32) {
        let t = thickness as f32;
        // Top edge
        Self::fill_rect(buf, &Rect::new(rect.x(), rect.y(), rect.width(), t), color);
        // Bottom edge
        Self::fill_rect(
            buf,
            &Rect::new(rect.x(), rect.y() + rect.height() - t, rect.width(), t),
            color,
        );
        // Left edge
        Self::fill_rect(buf, &Rect::new(rect.x(), rect.y(), t, rect.height()), color);
        // Right edge
        Self::fill_rect(
            buf,
            &Rect::new(rect.x() + rect.width() - t, rect.y(), t, rect.height()),
            color,
        );
    }

    /// Draw an ellipse outline inscribed in the given rect.
    fn stroke_ellipse(buf: &mut PixelBuffer, rect: &Rect, color: &Color, thickness: u32) {
        let cx = rect.x() + rect.width() / 2.0;
        let cy = rect.y() + rect.height() / 2.0;
        let rx = rect.width() / 2.0;
        let ry = rect.height() / 2.0;
        let pixel = [color.r, color.g, color.b, color.a];
        let (iw, ih) = (buf.width as i32, buf.height as i32);

        // Sample the ellipse outline with enough points
        let steps = ((rx + ry) * 4.0) as i32;
        for i in 0..steps {
            let angle = 2.0 * std::f64::consts::PI * (i as f64) / (steps as f64);
            let ex = cx as f64 + rx as f64 * angle.cos();
            let ey = cy as f64 + ry as f64 * angle.sin();
            // Draw a small filled square for thickness
            for dy in 0..thickness as i32 {
                for dx in 0..thickness as i32 {
                    let px = ex as i32 + dx - thickness as i32 / 2;
                    let py = ey as i32 + dy - thickness as i32 / 2;
                    if px >= 0 && px < iw && py >= 0 && py < ih {
                        buf.set_rgba(px as u32, py as u32, pixel);
                    }
                }
            }
        }
    }

    /// Draw a line using Bresenham's algorithm with thickness.
    fn draw_line(
        buf: &mut PixelBuffer,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: &Color,
        thickness: u32,
    ) {
        let pixel = [color.r, color.g, color.b, color.a];
        let (iw, ih) = (buf.width as i32, buf.height as i32);
        let half_t = thickness as i32 / 2;

        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut cx = x0;
        let mut cy = y0;

        loop {
            for ty in -half_t..=half_t {
                for tx in -half_t..=half_t {
                    let px = cx + tx;
                    let py = cy + ty;
                    if px >= 0 && px < iw && py >= 0 && py < ih {
                        buf.set_rgba(px as u32, py as u32, pixel);
                    }
                }
            }

            if cx == x1 && cy == y1 {
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

    /// Render all annotations as an SVG overlay string.
    pub fn to_svg(&self) -> String {
        let mut svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"#,
            self.width, self.height
        );
        svg.push('\n');

        for ann in &self.annotations {
            let pos = &ann.position;
            let color = ann.color.to_css();

            match ann.kind {
                AnnotationKind::Rectangle => {
                    svg.push_str(&format!(
                        r#"  <rect x="{}" y="{}" width="{}" height="{}" stroke="{}" fill="none" stroke-width="2"/>"#,
                        pos.x(), pos.y(), pos.width(), pos.height(), color
                    ));
                }
                AnnotationKind::Circle => {
                    let cx = pos.x() + pos.width() / 2.0;
                    let cy = pos.y() + pos.height() / 2.0;
                    let rx = pos.width() / 2.0;
                    let ry = pos.height() / 2.0;
                    svg.push_str(&format!(
                        r#"  <ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}" stroke="{color}" fill="none" stroke-width="2"/>"#,
                    ));
                }
                AnnotationKind::Arrow => {
                    let x2 = pos.x() + pos.width();
                    let y2 = pos.y() + pos.height();
                    svg.push_str(&format!(
                        r#"  <line x1="{}" y1="{}" x2="{x2}" y2="{y2}" stroke="{color}" stroke-width="2" marker-end="url(#arrow)"/>"#,
                        pos.x(), pos.y()
                    ));
                }
                AnnotationKind::Text => {
                    let text = ann.text.as_deref().unwrap_or("");
                    let escaped = xml_escape(text);
                    svg.push_str(&format!(
                        r#"  <text x="{}" y="{}" fill="{color}" font-size="16">{escaped}</text>"#,
                        pos.x(),
                        pos.y() + 16.0
                    ));
                }
                AnnotationKind::Highlight => {
                    svg.push_str(&format!(
                        r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{color}" opacity="0.3"/>"#,
                        pos.x(), pos.y(), pos.width(), pos.height()
                    ));
                }
                AnnotationKind::Redaction => {
                    svg.push_str(&format!(
                        r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="black"/>"#,
                        pos.x(),
                        pos.y(),
                        pos.width(),
                        pos.height()
                    ));
                }
                AnnotationKind::FreeForm => {
                    // Freeform rendered as a small filled rect placeholder
                    svg.push_str(&format!(
                        r#"  <rect x="{}" y="{}" width="{}" height="{}" stroke="{color}" fill="none" stroke-width="2" stroke-dasharray="4"/>"#,
                        pos.x(), pos.y(), pos.width(), pos.height()
                    ));
                }
            }
            svg.push('\n');
        }

        svg.push_str("</svg>");
        svg
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
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
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
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
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
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
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
}
