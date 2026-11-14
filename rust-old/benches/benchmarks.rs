//! Benchmarks for selah core operations.

use criterion::{Criterion, criterion_group, criterion_main};
use selah::{
    Annotation, AnnotationCanvas, AnnotationKind, Color, ImageFormat, Rect, extract_text_regions,
    suggest_redactions, xml_escape,
};
use std::hint::black_box;

fn make_image(width: u32, height: u32, format: image::ImageFormat) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(width, height, image::Rgba([128, 128, 128, 255]));
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, format).unwrap();
    buf.into_inner()
}

fn make_png(width: u32, height: u32) -> Vec<u8> {
    make_image(width, height, image::ImageFormat::Png)
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
    let pii_text =
        "Contact user@example.com at 192.168.1.100 or call 555-123-4567. Card: 4111111111111111";
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
        b.iter(|| {
            Rect::new(
                black_box(10.0),
                black_box(20.0),
                black_box(100.0),
                black_box(50.0),
            )
        });
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

/// Drawing-heavy render: BMP in/out keeps codec cost low so pixel ops dominate.
fn bench_render_drawing(c: &mut Criterion) {
    let bmp = make_image(512, 512, image::ImageFormat::Bmp);
    let annotations = vec![
        Annotation::new(
            AnnotationKind::Redaction,
            Rect::new(0.0, 0.0, 256.0, 256.0),
            Color::BLACK,
        ),
        Annotation::new(
            AnnotationKind::Highlight,
            Rect::new(64.0, 64.0, 384.0, 384.0),
            Color::YELLOW,
        ),
        Annotation::new(
            AnnotationKind::Rectangle,
            Rect::new(10.0, 10.0, 490.0, 490.0),
            Color::RED,
        ),
        Annotation::new(
            AnnotationKind::Circle,
            Rect::new(100.0, 100.0, 300.0, 200.0),
            Color::GREEN,
        ),
        Annotation::new(
            AnnotationKind::Arrow,
            Rect::new(0.0, 511.0, 511.0, -511.0),
            Color::BLUE,
        ),
    ];

    c.bench_function("render_to_image_512_bmp", |b| {
        b.iter(|| {
            AnnotationCanvas::render_to_image(
                black_box(&bmp),
                black_box(&annotations),
                ImageFormat::Bmp,
            )
            .unwrap()
        });
    });
}

fn bench_to_svg(c: &mut Criterion) {
    let mut canvas = AnnotationCanvas::new(1920, 1080);
    let kinds = [
        AnnotationKind::Rectangle,
        AnnotationKind::Circle,
        AnnotationKind::Arrow,
        AnnotationKind::Highlight,
        AnnotationKind::Redaction,
    ];
    for i in 0..100 {
        let f = i as f32;
        canvas.add_annotation(Annotation::new(
            kinds[i % kinds.len()].clone(),
            Rect::new(f * 10.0, f * 5.0, 100.0, 50.0),
            Color::RED,
        ));
    }
    c.bench_function("to_svg_100", |b| {
        b.iter(|| black_box(&canvas).to_svg());
    });
}

fn bench_extract_text_regions(c: &mut Criterion) {
    // 64 KiB of mixed binary + ASCII runs, similar to PNG metadata chunks.
    let data: Vec<u8> = (0..65_536u32)
        .map(|i| {
            if (i / 64) % 2 == 0 {
                b'a' + (i % 26) as u8
            } else {
                (i % 31) as u8
            }
        })
        .collect();
    c.bench_function("extract_text_regions_64k", |b| {
        b.iter(|| extract_text_regions(black_box(&data)));
    });
}

criterion_group!(
    benches,
    bench_render_to_image,
    bench_suggest_redactions,
    bench_rect_operations,
    bench_xml_escape,
    bench_render_drawing,
    bench_to_svg,
    bench_extract_text_regions,
);
criterion_main!(benches);
