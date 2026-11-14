//! `annotate`: rendering, the SVG overlay, the layer file both ways, and the facade (port plan §3 M4).
//!
//! An annotation in a row is a group of tokens. For `render`: `e:AnnotationKind.<K> g:<x> g:<y> g:<w> g:<h> i:<r>
//! i:<g> i:<b> i:<a>`, the rect given as `Rect::new`'s f32 arguments. For `to_svg` and `layer_json` the group starts
//! with the id `h:<16 bytes>` and ends with the text, `s:<text>` or `N`.
//!
//! Rows:
//! - `render A p:fixtures/<src> <groups> e:ImageFormat.Png` → `i:<w> i:<h> c:<crc32 of the RGBA>` or `E:<variant>`.
//!   selah's own `annotate_image`, its PNG decoded back. Every kind × a grid of rect classes × three colours on
//!   images from 1x1 to 64x64 (one colour on 512x512), and overlapping groups.
//! - `to_svg A i:<w> i:<h> <groups>` → `s:<svg>`.
//! - `layer_json A i:<w> i:<h> i:<secs> i:<nanos> <groups>` → `s:<the layer file>`: serde_json::to_string_pretty of
//!   AnnotationLayer, as save_to_file writes it, at a fixed `created`. For a canvas with a non-finite region the
//!   expectation is the port's corrected answer, `E:AnnotationError` (ADR 0002 #19: Rust writes a file it cannot read).
//! - `layer_load A s:<json>` → `i:<w> i:<h> i:<n> s:<annotation JSON>…` or `E:<variant>`: load_from_file on a file
//!   holding the text, each annotation then written back with serde_json::to_string.
//! - `decode_image_data A s:<base64>` → `h:<bytes>` or `E:Api`.
//! - `redact_image A p:fixtures/<src> <targets> e:ImageFormat.Png` → `i:<w> i:<h> c:<crc32> i:<suggestions>` or
//!   `E:<variant>`; targets `N` (all) or a list of `s:<RedactionTarget JSON>`. Every fixture but the JPEGs (their
//!   pixels are class B) and the 16.8 MP one.

use crate::fixtures::Fixture;
use crate::m_decode::variant;
use crate::{Out, Row, emit_tsv, esc, tg, th, ts};
use selah::{Annotation, AnnotationCanvas, AnnotationKind as K, Color, ImageFormat, Rect, RedactionTarget};

const KINDS: [(K, &str); 7] = [
    (K::Arrow, "Arrow"),
    (K::Rectangle, "Rectangle"),
    (K::Circle, "Circle"),
    (K::Text, "Text"),
    (K::Highlight, "Highlight"),
    (K::Redaction, "Redaction"),
    (K::FreeForm, "FreeForm"),
];

const COLORS: [[u8; 4]; 3] = [[255, 0, 0, 255], [0, 128, 255, 128], [10, 200, 30, 0]];

/// The rect classes, as `Rect::new` arguments, for a w x h image.
fn rects(w: f32, h: f32) -> Vec<[f32; 4]> {
    let (nan, inf) = (f32::NAN, f32::INFINITY);
    vec![
        [1.0, 1.0, w - 2.0, h - 2.0],
        [0.0, 0.0, w, h],
        [2.5, 1.25, w / 2.0 + 0.75, h / 2.0 + 0.5],
        [w - 1.0, h - 1.0, -(w - 2.0), -(h - 2.0)],
        [-5.0, -3.0, 10.0, 8.0],
        [w - 3.0, h - 2.0, 10.0, 10.0],
        [w + 5.0, h + 5.0, 4.0, 4.0],
        [-20.0, -20.0, 5.0, 5.0],
        [nan, 0.0, 5.0, 5.0],
        [0.0, nan, 5.0, 5.0],
        [0.0, 0.0, nan, 5.0],
        [0.0, 0.0, 5.0, nan],
        [-inf, 0.0, inf, 3.0],
        [0.0, 0.0, inf, inf],
        [-inf, -inf, 10.0, 10.0],
        [0.0, 0.0, -inf, 4.0],
        [-1e20, 0.0, 2e20, 100.0],
        [0.0, -1e30, 4.0, 2e30],
        [1e38, 1e38, 1e38, 1e38],
        [-3e38, -3e38, 3e38, 3e38],
        [2.0, 2.0, 0.0, 0.0],
        [1.2, 1.2, 0.1, 0.1],
        [w / 2.0, h / 2.0, 0.5, 0.5],
        [1e-40, 0.0, 3.0, 3.0],
        [w, 0.0, -w, h],
        [-100.0, h / 2.0, 300.0, 0.0],
        [w / 2.0, -50.0, 0.0, h + 100.0],
        [0.49, 0.51, 1.98, 1.02],
        [-0.0, -0.0, 3.0, -0.0],
    ]
}

fn ann(kind: &K, r: [f32; 4], c: [u8; 4], id: u128, text: Option<&str>) -> Annotation {
    let mut a = Annotation::new(kind.clone(), Rect::new(r[0], r[1], r[2], r[3]), Color { r: c[0], g: c[1], b: c[2], a: c[3] });
    a.id = uuid::Uuid::from_u128(id);
    a.text = text.map(str::to_string);
    a
}

fn group(kname: &str, r: [f32; 4], c: [u8; 4]) -> Vec<String> {
    let mut g = vec![format!("e:AnnotationKind.{kname}")];
    g.extend(r.iter().map(|v| tg(*v)));
    g.extend(c.iter().map(|v| format!("i:{v}")));
    g
}

fn full_group(a: &Annotation, kname: &str, r: [f32; 4]) -> Vec<String> {
    let mut g = vec![th(a.id.as_bytes())];
    g.extend(group(kname, r, [a.color.r, a.color.g, a.color.b, a.color.a]));
    g.push(a.text.as_deref().map_or("N".to_string(), ts));
    g
}

fn crc(bytes: &[u8]) -> String {
    let mut c = flate2::Crc::new();
    c.update(bytes);
    format!("c:{:08x}", c.sum())
}

/// selah's annotate_image to PNG, decoded back: (w, h, rgba).
fn render(src: &[u8], anns: &[Annotation]) -> Result<(u32, u32, Vec<u8>), selah::SelahError> {
    let png = selah::annotate_image(src, anns, ImageFormat::Png)?;
    let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png).expect("PNG").to_rgba8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
}

fn render_row(rows: &mut Vec<Row>, file: &str, src: &[u8], groups: Vec<Vec<String>>, anns: &[Annotation], note: String) {
    let mut inputs = vec![format!("p:fixtures/{file}")];
    for g in groups {
        inputs.extend(g);
    }
    inputs.push("e:ImageFormat.Png".to_string());
    let outputs = match render(src, anns) {
        Ok((w, h, px)) => vec![format!("i:{w}"), format!("i:{h}"), crc(&px)],
        Err(e) => vec![format!("E:{}", variant(&e))],
    };
    rows.push(Row { func: "render".into(), class: "A", inputs, outputs, note });
}

pub fn emit(out: &mut Out, fixtures: &[Fixture]) {
    let mut rows = Vec::new();
    let find = |name: &str| fixtures.iter().find(|f| f.file == name).expect("fixture").bytes.clone();

    // ---- render
    for (file, w, h, colors) in [
        ("png-rgba8-1x1.png", 1.0, 1.0, 3usize),
        ("png-rgba8-7x3.png", 7.0, 3.0, 3),
        ("png-rgba8-33x17.png", 33.0, 17.0, 3),
        ("png-rgba8-64x64-grad.png", 64.0, 64.0, 3),
        ("png-rgba8-512x512-grad.png", 512.0, 512.0, 1),
    ] {
        let src = find(file);
        for (ri, r) in rects(w, h).into_iter().enumerate() {
            for (kind, kname) in KINDS.iter() {
                for c in COLORS.iter().take(colors) {
                    let a = ann(kind, r, *c, 1, None);
                    let note = format!("{file}: {kname} rect #{ri} colour {c:?}");
                    render_row(&mut rows, file, &src, vec![group(kname, r, *c)], &[a], note);
                }
            }
        }
        // Overlapping annotations, drawn in order.
        let sets: [&[(K, &str, [f32; 4], [u8; 4])]; 4] = [
            &[(K::Highlight, "Highlight", [0.0, 0.0, w, h], [255, 255, 0, 255]), (K::Rectangle, "Rectangle", [1.0, 1.0, w - 2.0, h - 2.0], [255, 0, 0, 255])],
            &[(K::Redaction, "Redaction", [0.0, 0.0, w / 2.0, h], [9, 9, 9, 9]), (K::Highlight, "Highlight", [0.0, 0.0, w, h / 2.0], [0, 0, 255, 255]), (K::Highlight, "Highlight", [w / 4.0, 0.0, w / 2.0, h], [0, 255, 0, 255])],
            &[(K::Circle, "Circle", [0.0, 0.0, w, h], [0, 0, 0, 255]), (K::Arrow, "Arrow", [0.0, h, w, -h], [255, 255, 255, 255]), (K::Text, "Text", [1.0, 0.0, w - 1.0, h], [5, 6, 7, 8])],
            &[(K::FreeForm, "FreeForm", [0.0, 0.0, w, h], [100, 100, 100, 100]), (K::Redaction, "Redaction", [f32::NAN, 0.0, 1.0, 1.0], [0, 0, 0, 255])],
        ];
        for (si, set) in sets.iter().enumerate() {
            let groups = set.iter().map(|(_, kn, r, c)| group(kn, *r, *c)).collect();
            let anns: Vec<Annotation> = set.iter().map(|(k, _, r, c)| ann(k, *r, *c, 1, None)).collect();
            render_row(&mut rows, file, &src, groups, &anns, format!("{file}: overlapping set {si}"));
        }
    }
    // Sources the facade refuses, with an annotation (the NaN check runs first).
    for file in ["other-text.txt", "gif-1x1.gif"] {
        let src = find(file);
        for (k, kn, r) in [(K::Rectangle, "Rectangle", [0.0, 0.0, 1.0, 1.0]), (K::Redaction, "Redaction", [f32::NAN, 0.0, 1.0, 1.0])] {
            let a = ann(&k, r, [1, 2, 3, 4], 1, None);
            render_row(&mut rows, file, &src, vec![group(kn, r, [1, 2, 3, 4])], &[a], format!("{file}: refused source, {kn}"));
        }
    }

    // ---- to_svg and layer_json
    let texts: [Option<&str>; 6] = [None, Some(""), Some("hello"), Some("<script>&\"'</script>"), Some("tab\tnl\ncr\rnul\u{0}del\u{7f}"), Some("Ünïcödé ✓ 日本")];
    let mut canvases: Vec<(u32, u32, Vec<Annotation>, Vec<String>)> = Vec::new();
    let mut id = 0x0123_4567_89ab_cdef_0011_2233_4455_6677u128;
    for (ri, r) in rects(64.0, 48.0).into_iter().enumerate() {
        for (kind, kname) in KINDS.iter() {
            for (ci, c) in COLORS.iter().take(2).enumerate() {
                let text = if *kind == K::Text { texts[(ri + ci) % texts.len()] } else { None };
                id = id.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let a = ann(kind, r, *c, id, text);
                let g = full_group(&a, kname, r);
                canvases.push((64, 48, vec![a], g));
            }
        }
    }
    for t in texts {
        id = id.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let a = ann(&K::Text, [3.0, 4.0, 50.0, 10.0], [0, 0, 0, 255], id, t);
        let g = full_group(&a, "Text", [3.0, 4.0, 50.0, 10.0]);
        canvases.push((1920, 1080, vec![a], g));
    }
    // A few multi-annotation canvases, and an empty one.
    canvases.push((0, 0, vec![], vec![]));
    for n in [2usize, 5] {
        let mut anns = Vec::new();
        let mut gs = Vec::new();
        for i in 0..n {
            let (kind, kname) = &KINDS[i % KINDS.len()];
            let r = [i as f32 * 3.5, 1.0, 10.0 - i as f32, 4.25];
            id = id.wrapping_add(0x1000_0000_0000_0001);
            let a = ann(kind, r, [i as u8, 2, 3, 255], id, if *kind == K::Text { Some("x&y") } else { None });
            gs.extend(full_group(&a, kname, r));
            anns.push(a);
        }
        canvases.push((800, 600, anns, gs));
    }
    for (w, h, anns, g) in &canvases {
        let mut canvas = AnnotationCanvas::new(*w, *h);
        for a in anns {
            canvas.add_annotation(a.clone());
        }
        let mut inputs = vec![format!("i:{w}"), format!("i:{h}")];
        inputs.extend(g.iter().cloned());
        rows.push(Row { func: "to_svg".into(), class: "A", inputs, outputs: vec![ts(&canvas.to_svg())], note: String::new() });
    }
    let times: [(i64, u32); 6] = [(0, 0), (1_763_100_000, 123_456_789), (1_763_100_000, 500_000_000), (-86_401, 1), (253_402_300_799, 999_999_999), (1_483_228_799, 1_000_000_000)];
    let mut layer_texts = Vec::new();
    for (i, (w, h, anns, g)) in canvases.iter().enumerate() {
        if i % 7 != 0 && anns.len() <= 1 {
            continue; // a sample of the single-annotation canvases; every multi-annotation one
        }
        let (secs, nanos) = times[i % times.len()];
        let created = chrono::DateTime::from_timestamp(secs, nanos).expect("in range");
        let layer = selah::AnnotationLayer { version: selah::annotate::LAYER_VERSION, width: *w, height: *h, created, annotations: anns.clone() };
        let json = serde_json::to_string_pretty(&layer).expect("serialize");
        let mut inputs = vec![format!("i:{w}"), format!("i:{h}"), format!("i:{secs}"), format!("i:{nanos}")];
        inputs.extend(g.iter().cloned());
        // Rust defect fixed in the port (ADR 0002 #19): a non-finite x, y, width or height is written as `null`, which
        // load_from_file refuses, so Rust saves a layer it cannot read back. The corrected expectation is a refusal.
        let unreadable = anns.iter().any(|a| {
            let p = &a.position;
            !(p.x().is_finite() && p.y().is_finite() && p.width().is_finite() && p.height().is_finite())
        });
        let (outputs, note) = if unreadable {
            (vec!["E:AnnotationError".to_string()], "Rust defect fixed: ADR 0002 #19 (Rust writes null, then refuses it)".to_string())
        } else {
            (vec![ts(&json)], String::new())
        };
        rows.push(Row { func: "layer_json".into(), class: "A", inputs, outputs, note });
        layer_texts.push(json);
    }

    // ---- layer_load: what Rust wrote, then hand-written variants.
    let ok_ann = r#"{"id":"00000000-0000-0000-0000-000000000000","kind":"Redaction","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":0,"g":0,"b":0,"a":255},"text":null}"#;
    let mut variants: Vec<String> = layer_texts.clone();
    for v in [
        format!(r#"{{"version":1,"width":10,"height":20,"created":"2026-01-01T00:00:00Z","annotations":[{ok_ann}]}}"#),
        format!(r#"[1,10,20,"2026-01-01T00:00:00Z",[{ok_ann}]]"#),
        format!(r#"{{"version":1,"width":10,"height":20,"created":"2026-01-01T00:00:00Z","annotations":[{ok_ann}],"extra":true}}"#),
        format!(r#"{{"annotations":[],"created":"2026-01-01T00:00:00+05:30","height":2,"width":1,"version":0}}"#),
        r#"{"version":2,"width":1,"height":1,"created":"2026-01-01T00:00:00Z","annotations":[]}"#.to_string(),
        r#"{"version":4294967295,"width":1,"height":1,"created":"2026-01-01T00:00:00Z","annotations":[]}"#.to_string(),
        r#"{"version":4294967296,"width":1,"height":1,"created":"2026-01-01T00:00:00Z","annotations":[]}"#.to_string(),
        r#"{"version":1.0,"width":1,"height":1,"created":"2026-01-01T00:00:00Z","annotations":[]}"#.to_string(),
        r#"{"version":1,"width":-1,"height":1,"created":"2026-01-01T00:00:00Z","annotations":[]}"#.to_string(),
        r#"{"version":1,"width":1,"height":1,"created":"yesterday","annotations":[]}"#.to_string(),
        r#"{"version":1,"width":1,"height":1,"created":"2026-01-01T00:00:00Z","annotations":{}}"#.to_string(),
        r#"{"version":1,"width":1,"height":1,"created":"2026-01-01T00:00:00Z"}"#.to_string(),
        r#"{"width":1,"height":1,"created":"2026-01-01T00:00:00Z","annotations":[]}"#.to_string(),
        r#"{"version":1,"version":1,"width":1,"height":1,"created":"2026-01-01T00:00:00Z","annotations":[]}"#.to_string(),
        r#"{"version":1,"width":1,"height":1,"created":"2026-01-01T00:00:00Z","annotations":[{"id":"00000000-0000-0000-0000-000000000000","kind":"Redaction","position":{"x":1e39,"y":0,"width":1,"height":1},"color":{"r":0,"g":0,"b":0,"a":255},"text":null}]}"#.to_string(),
        r#"{"version":1,"width":1,"height":1,"created":"2026-01-01T00:00:00Z","annotations":[{"id":"x","kind":"Redaction","position":{"x":1,"y":0,"width":1,"height":1},"color":{"r":0,"g":0,"b":0,"a":255},"text":null}]}"#.to_string(),
        r#"[1,1,1,"2026-01-01T00:00:00Z"]"#.to_string(),
        "[]".to_string(),
        "null".to_string(),
        "".to_string(),
        "{".to_string(),
        r#"{"version":1,"width":1,"height":1,"created":"2026-01-01T00:00:00Z","annotations":[]} trailing"#.to_string(),
        " \n {\"version\":1,\"width\":1,\"height\":1,\"created\":\"2026-01-01T00:00:00Z\",\"annotations\":[]} \n".to_string(),
    ] {
        variants.push(v);
    }
    let dir = std::env::temp_dir().join(format!("selah-oracle-layers-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    for (i, v) in variants.iter().enumerate() {
        let path = dir.join(format!("layer-{i}.json"));
        std::fs::write(&path, v).expect("write layer");
        let outputs = match AnnotationCanvas::load_from_file(&path) {
            Ok(c) => {
                let mut o = vec![format!("i:{}", c.width), format!("i:{}", c.height), format!("i:{}", c.count())];
                o.extend(c.get_annotations().iter().map(|a| ts(&serde_json::to_string(a).expect("json"))));
                o
            }
            Err(e) => vec![format!("E:{}", variant(&e))],
        };
        rows.push(Row { func: "layer_load".into(), class: "A", inputs: vec![ts(v)], outputs, note: String::new() });
    }
    std::fs::remove_dir_all(&dir).ok();

    // ---- decode_image_data
    let mut b64: Vec<String> = Vec::new();
    for n in 0..10usize {
        let bytes: Vec<u8> = (0..n).map(|i| (i * 37 + 200) as u8).collect();
        b64.push(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes));
    }
    for s in ["QQ", "QQ=", "QQ===", "QUI", "QUI==", "QQ==QQ==", "=QQ=", "Q Q==", "QQ==\n", " QQ==", "QR==", "QUJ=",
        "-_8=", "+/8=", "QUJD\r\n", "QUJDRA", "QUJDRA=", "Q", "====", "QUJ\u{e9}", "QUJD====", "AAAA", "////", "QUJDRA=="] {
        b64.push(s.to_string());
    }
    for s in &b64 {
        let outputs = match selah::decode_image_data(s) {
            Ok(b) => vec![th(&b)],
            Err(e) => vec![format!("E:{}", variant(&e))],
        };
        rows.push(Row { func: "decode_image_data".into(), class: "A", inputs: vec![ts(s)], outputs, note: esc(s) });
    }

    // ---- redact_image
    let target_sets: [Option<Vec<RedactionTarget>>; 4] = [
        None,
        Some(vec![RedactionTarget::Email]),
        Some(vec![RedactionTarget::Custom("x".into()), RedactionTarget::IpAddress]),
        Some(vec![]),
    ];
    for f in fixtures {
        // A JPEG source's pixels are class B (`decode` covers them), so a CRC of them says nothing here.
        if f.file.contains("4100x4100") || f.kind == "jpeg" {
            continue;
        }
        let adr = f.adr.map(|a| format!(" adr={a}")).unwrap_or_default();
        for set in &target_sets {
            let mut inputs = vec![format!("p:fixtures/{}", f.file)];
            match set {
                None => inputs.push("N".to_string()),
                Some(ts_) => inputs.extend(ts_.iter().map(|t| ts(&serde_json::to_string(t).expect("json")))),
            }
            inputs.push("e:ImageFormat.Png".to_string());
            let outputs = match selah::redact_image(&f.bytes, set.as_deref(), ImageFormat::Png) {
                Ok((png, sugg)) => {
                    let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png).expect("PNG").to_rgba8();
                    let (w, h) = img.dimensions();
                    vec![format!("i:{w}"), format!("i:{h}"), crc(img.as_raw()), format!("i:{}", sugg.len())]
                }
                Err(e) => vec![format!("E:{}", variant(&e))],
            };
            rows.push(Row { func: "redact_image".into(), class: "A", inputs, outputs, note: format!("{}{adr}", f.file) });
        }
    }

    emit_tsv(out, "annotate", "render / to_svg / layer_json / layer_load / decode_image_data / redact_image; see m_annotate.rs", &rows);
}
