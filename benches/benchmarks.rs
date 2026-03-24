//! Benchmarks for selah core operations.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use selah::{
    Annotation, AnnotationCanvas, AnnotationKind, Color, ImageFormat, Rect,
    suggest_redactions, xml_escape,
};

fn make_png(width: u32, height: u32) -> Vec<u8> {
    use image::{ImageFormat, RgbaImage};
    let mut img = RgbaImage::new(width, height);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba([128, 128, 128, 255]);
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png).unwrap();
    buf.into_inner()
}

fn bench_render_to_image(c: &mut Criterion) {
    let png = make_png(64, 64);
    let annotations = vec![
        Annotation::new(
            AnnotationKind::Redaction,
            Rect::new(0.0, 0.0, 32.0, 32.0),
            Color::BLACK,
        ),
        Annotation::new(
            AnnotationKind::Rectangle,
            Rect::new(10.0, 10.0, 44.0, 44.0),
            Color::RED,
        ),
        Annotation::new(
            AnnotationKind::Highlight,
            Rect::new(20.0, 20.0, 30.0, 30.0),
            Color::YELLOW,
        ),
    ];

    c.bench_function("render_to_image_64x64", |b| {
        b.iter(|| {
            AnnotationCanvas::render_to_image(
                black_box(&png),
                black_box(&annotations),
                ImageFormat::Png,
            )
            .unwrap()
        });
    });
}

fn bench_suggest_redactions(c: &mut Criterion) {
    let clean_text = "This is a perfectly normal sentence with no sensitive data at all.";
    let pii_text = "Contact user@example.com at 192.168.1.100 or call 555-123-4567. Card: 4111111111111111";
    let long_text = clean_text.repeat(100);

    c.bench_function("suggest_redactions_clean", |b| {
        b.iter(|| suggest_redactions(black_box(clean_text)));
    });

    c.bench_function("suggest_redactions_pii", |b| {
        b.iter(|| suggest_redactions(black_box(pii_text)));
    });

    c.bench_function("suggest_redactions_long", |b| {
        b.iter(|| suggest_redactions(black_box(&long_text)));
    });
}

fn bench_rect_operations(c: &mut Criterion) {
    c.bench_function("rect_new", |b| {
        b.iter(|| Rect::new(black_box(10.0), black_box(20.0), black_box(100.0), black_box(50.0)));
    });

    let r = Rect::new(10.0, 10.0, 100.0, 50.0);
    let p = hisab::Vec2::new(55.0, 35.0);
    c.bench_function("rect_contains_point", |b| {
        b.iter(|| r.contains_point(black_box(p)));
    });

    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b_rect = Rect::new(50.0, 50.0, 100.0, 100.0);
    c.bench_function("rect_intersects", |b| {
        b.iter(|| a.intersects(black_box(&b_rect)));
    });
}

fn bench_xml_escape(c: &mut Criterion) {
    let plain = "Hello World 1234567890";
    let special = r#"<script>alert("xss")</script> & 'more' <entities>"#;
    let long_plain = plain.repeat(100);

    c.bench_function("xml_escape_plain", |b| {
        b.iter(|| xml_escape(black_box(plain)));
    });

    c.bench_function("xml_escape_special", |b| {
        b.iter(|| xml_escape(black_box(special)));
    });

    c.bench_function("xml_escape_long", |b| {
        b.iter(|| xml_escape(black_box(&long_plain)));
    });
}

criterion_group!(
    benches,
    bench_render_to_image,
    bench_suggest_redactions,
    bench_rect_operations,
    bench_xml_escape,
);
criterion_main!(benches);
