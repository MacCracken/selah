//! selah-annotate — Annotation engine for Selah.

use selah_core::{Annotation, AnnotationKind, Color, Rect};
use uuid::Uuid;

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
                        pos.x, pos.y, pos.width, pos.height, color
                    ));
                }
                AnnotationKind::Circle => {
                    let cx = pos.x + pos.width / 2.0;
                    let cy = pos.y + pos.height / 2.0;
                    let rx = pos.width / 2.0;
                    let ry = pos.height / 2.0;
                    svg.push_str(&format!(
                        r#"  <ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}" stroke="{color}" fill="none" stroke-width="2"/>"#,
                    ));
                }
                AnnotationKind::Arrow => {
                    let x2 = pos.x + pos.width;
                    let y2 = pos.y + pos.height;
                    svg.push_str(&format!(
                        r#"  <line x1="{}" y1="{}" x2="{x2}" y2="{y2}" stroke="{color}" stroke-width="2" marker-end="url(#arrow)"/>"#,
                        pos.x, pos.y
                    ));
                }
                AnnotationKind::Text => {
                    let text = ann.text.as_deref().unwrap_or("");
                    svg.push_str(&format!(
                        r#"  <text x="{}" y="{}" fill="{color}" font-size="16">{text}</text>"#,
                        pos.x,
                        pos.y + 16.0
                    ));
                }
                AnnotationKind::Highlight => {
                    svg.push_str(&format!(
                        r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{color}" opacity="0.3"/>"#,
                        pos.x, pos.y, pos.width, pos.height
                    ));
                }
                AnnotationKind::Redaction => {
                    svg.push_str(&format!(
                        r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="black"/>"#,
                        pos.x, pos.y, pos.width, pos.height
                    ));
                }
                AnnotationKind::FreeForm => {
                    // Freeform rendered as a small filled rect placeholder
                    svg.push_str(&format!(
                        r#"  <rect x="{}" y="{}" width="{}" height="{}" stroke="{color}" fill="none" stroke-width="2" stroke-dasharray="4"/>"#,
                        pos.x, pos.y, pos.width, pos.height
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
}
