//! Integration tests for the selah library.

use selah::*;

/// Helper: create a minimal PNG image in memory with the given dimensions and color.
fn make_png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    use image::{ImageFormat, RgbaImage};
    let mut img = RgbaImage::new(width, height);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba(rgba);
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png).unwrap();
    buf.into_inner()
}

#[test]
fn test_annotate_image_roundtrip() {
    let png = make_png(4, 4, [255, 0, 0, 255]);

    let annotations = vec![Annotation::new(
        AnnotationKind::Rectangle,
        Rect::new(0.0, 0.0, 4.0, 4.0),
        Color::BLUE,
    )];

    let result = annotate_image(&png, &annotations, ImageFormat::Png).unwrap();

    // Verify output is valid PNG
    let img = image::load_from_memory(&result).unwrap().to_rgba8();
    assert_eq!(img.width(), 4);
    assert_eq!(img.height(), 4);
}

#[test]
fn test_annotate_image_with_redaction() {
    let png = make_png(2, 2, [255, 255, 255, 255]);

    let annotations = vec![Annotation::new(
        AnnotationKind::Redaction,
        Rect::new(0.0, 0.0, 2.0, 2.0),
        Color::BLACK,
    )];

    let result = annotate_image(&png, &annotations, ImageFormat::Png).unwrap();
    let img = image::load_from_memory(&result).unwrap().to_rgba8();

    // All pixels should be black after redaction
    for pixel in img.pixels() {
        assert_eq!(pixel.0, [0, 0, 0, 255]);
    }
}

#[test]
fn test_redact_image_with_no_pii() {
    // A small solid-color image has no embedded text that resembles PII
    let png = make_png(2, 2, [128, 128, 128, 255]);

    let (result_bytes, suggestions) = redact_image(&png, None, ImageFormat::Png).unwrap();

    // No PII should be detected in a solid gray image
    assert!(suggestions.is_empty());

    // Output should still be a valid image
    let img = image::load_from_memory(&result_bytes).unwrap().to_rgba8();
    assert_eq!(img.width(), 2);
    assert_eq!(img.height(), 2);
}

#[test]
fn test_geometry_serde_backward_compat() {
    // Old format used f64 values
    let json =
        r#"{"x": 10.123456789012345, "y": 20.987654321098765, "width": 100.0, "height": 50.0}"#;
    let r: Rect = serde_json::from_str(json).unwrap();
    assert!((r.x() - 10.1234).abs() < 0.01);
    assert!((r.y() - 20.9876).abs() < 0.01);
    assert_eq!(r.width(), 100.0);
    assert_eq!(r.height(), 50.0);
}

#[test]
fn test_decode_image_data_valid() {
    use base64::Engine;
    let data = vec![1, 2, 3, 4];
    let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
    let decoded = decode_image_data(&encoded).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_decode_image_data_invalid() {
    let result = decode_image_data("not-valid-base64!!!");
    assert!(result.is_err());
}

#[test]
fn test_annotation_canvas_svg_workflow() {
    let mut canvas = AnnotationCanvas::new(800, 600);

    canvas.add_annotation(Annotation::new(
        AnnotationKind::Rectangle,
        Rect::new(10.0, 10.0, 100.0, 50.0),
        Color::RED,
    ));
    canvas.add_annotation(Annotation::with_text(
        AnnotationKind::Text,
        Rect::new(50.0, 50.0, 200.0, 30.0),
        Color::BLACK,
        "Hello".into(),
    ));
    canvas.apply_redaction(Rect::new(300.0, 100.0, 150.0, 40.0));

    let svg = canvas.to_svg();
    assert!(svg.contains("<svg"));
    assert!(svg.contains("</svg>"));
    assert!(svg.contains("<rect"));
    assert!(svg.contains("<text"));
    assert!(svg.contains("Hello"));
    assert!(svg.contains("fill=\"black\""));
    assert_eq!(canvas.count(), 3);
}

#[test]
fn test_suggest_redactions_and_smart_crop_together() {
    let text = "Contact user@example.com at 192.168.1.1";
    let redactions = suggest_redactions(text);
    assert!(redactions.len() >= 2);

    let crops = suggest_smart_crop(1920, 1080);
    assert_eq!(crops.len(), 5);
}

#[test]
fn test_extract_text_and_suggest_redactions_pipeline() {
    // Simulate the pipeline: extract text -> find PII
    // Use spaces to separate tokens so the PII detector can find them.
    // The byte scanner extracts runs of 4+ printable ASCII chars.
    let data = b"\x00\x00 user@example.com \x00\x00 192.168.1.100 \x00\x00";
    let ocr = extract_text_regions(data);
    assert!(!ocr.text.is_empty());

    let suggestions = suggest_redactions(&ocr.text);
    let has_email = suggestions
        .iter()
        .any(|s| s.target_type == RedactionTarget::Email);
    let has_ip = suggestions
        .iter()
        .any(|s| s.target_type == RedactionTarget::IpAddress);
    assert!(has_email);
    assert!(has_ip);
}

#[test]
fn test_convert_format_roundtrip() {
    let png = make_png(2, 2, [0, 255, 0, 255]);
    let bmp = convert_format(&png, ImageFormat::Bmp).unwrap();
    assert!(!bmp.is_empty());
    // BMP magic bytes
    assert_eq!(&bmp[0..2], b"BM");
}

#[test]
fn test_history_store_integration() {
    use selah::HistoryStore;

    let path = std::env::temp_dir().join(format!(
        "selah_integration_test_{}.jsonl",
        uuid::Uuid::new_v4()
    ));
    let store = HistoryStore::open(path.clone());

    let entry = HistoryEntry {
        id: uuid::Uuid::new_v4(),
        path: "/tmp/test.png".into(),
        timestamp: chrono::Utc::now(),
        source: "full screen".into(),
        width: 1920,
        height: 1080,
        format: "png".into(),
    };
    let id = entry.id;
    store.record(entry).unwrap();

    let got = store.get(id).unwrap().unwrap();
    assert_eq!(got.width, 1920);

    let entries = store.list(10, None).unwrap();
    assert_eq!(entries.len(), 1);

    store.delete(id).unwrap();
    assert!(store.get(id).unwrap().is_none());

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_vec2_re_export() {
    // Vec2 should be usable via selah::Vec2
    let p = Vec2::new(10.0, 20.0);
    let r = Rect::new(0.0, 0.0, 100.0, 100.0);
    assert!(r.contains_point(p));
}
