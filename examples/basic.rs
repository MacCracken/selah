//! Basic usage of the selah screenshot library.
//!
//! Run with: `cargo run --example basic`

use selah::{
    Annotation, AnnotationCanvas, AnnotationKind, Color, ImageFormat, Rect,
    suggest_redactions, suggest_smart_crop, xml_escape,
};

fn main() {
    // Create an annotation canvas (1920x1080)
    let mut canvas = AnnotationCanvas::new(1920, 1080);

    // Add a rectangle annotation
    let rect_id = canvas.add_annotation(Annotation::new(
        AnnotationKind::Rectangle,
        Rect::new(100.0, 100.0, 400.0, 200.0),
        Color::RED,
    ));
    println!("Added rectangle annotation: {rect_id}");

    // Add a text annotation
    canvas.add_annotation(Annotation::with_text(
        AnnotationKind::Text,
        Rect::new(120.0, 320.0, 300.0, 30.0),
        Color::BLACK,
        "Important area".into(),
    ));

    // Add a highlight
    canvas.add_annotation(Annotation::new(
        AnnotationKind::Highlight,
        Rect::new(500.0, 100.0, 200.0, 100.0),
        Color::YELLOW,
    ));

    // Add a redaction
    canvas.apply_redaction(Rect::new(800.0, 200.0, 300.0, 50.0));

    println!("Canvas has {} annotations", canvas.count());

    // Render to SVG
    let svg = canvas.to_svg();
    println!("\nSVG output ({} bytes):", svg.len());
    println!("{svg}");

    // Demonstrate xml_escape for safe text inclusion
    let unsafe_text = "<script>alert('xss')</script>";
    let safe_text = xml_escape(unsafe_text);
    println!("\nXML escaped: {safe_text}");

    // Demonstrate PII detection
    let text = "Contact john@example.com or call 555-123-4567. Server: 10.0.0.1";
    let suggestions = suggest_redactions(text);
    println!("\nPII detected in text:");
    for s in &suggestions {
        println!("  - {} (confidence: {:.0}%): {}", s.target_type, s.confidence * 100.0, s.matched_text);
    }

    // Demonstrate smart crop suggestions
    let crops = suggest_smart_crop(1920, 1080);
    println!("\nSmart crop suggestions:");
    for c in &crops {
        println!(
            "  - {} (confidence: {:.0}%): {}x{} at ({}, {})",
            c.reason,
            c.confidence * 100.0,
            c.region.width(),
            c.region.height(),
            c.region.x(),
            c.region.y()
        );
    }

    // Demonstrate ImageFormat parsing
    let format: ImageFormat = "png".parse().unwrap();
    println!("\nFormat: {} (MIME: {})", format, format.mime_type());
}
