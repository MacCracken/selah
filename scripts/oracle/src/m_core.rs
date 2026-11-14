//! `core`: selah's core types (port plan §3 M1).
//!
//! Rows (enum values as `e:<Enum>.<Variant>`; a refusal as `E:<SelahError variant> s:<its message>`, the message
//! being the variant's own `String`, which is what the port's `detail_out` carries):
//! - `AnnotationKind::fmt e → s`, `AnnotationKind::from_str s → e | E`, `AnnotationKind::to_json e → s`,
//!   `AnnotationKind::from_json s → e | E:serde s`;
//! - `ImageFormat::extension|mime_type|fmt e → s`, `ImageFormat::from_str s → e | E`, `ImageFormat::from_path s → e | N`,
//!   `ImageFormat::to_json e → s`, `ImageFormat::from_json s → e | E:serde s`;
//! - `xml_escape s → s`; `derive_output_path s s → s`;
//! - `Color::fmt i×4 → s`, `Color::to_css i×4 → s`, `Color::to_json i×4 → s`, `Color::from_json s → i×4 | E:serde s`;
//! - `RedactionTarget::fmt|key e [s] → s`, `RedactionTarget::to_json e [s] → s`, `RedactionTarget::from_json s → e [s] | E:serde s`;
//! - `CaptureSource::fmt` (FullScreen; Region with Rect::new's four args; Window with its id) → s;
//! - `Annotation::to_json h:<id> e g×4 i×4 (s|N) → s`, `Annotation::from_json s → h e g×4 i×4 (s|N) | E:serde s`.

use crate::{Out, Row, Xs, emit_tsv, tg, th, ti, ts};
use selah::{Annotation, AnnotationKind, CaptureSource, Color, ImageFormat, Rect, RedactionTarget, SelahError};

/// The variant name and its message.
pub fn err_parts(e: &SelahError) -> (String, String) {
    match e {
        SelahError::CaptureFailed(m) => ("CaptureFailed".into(), m.clone()),
        SelahError::InvalidRegion(m) => ("InvalidRegion".into(), m.clone()),
        SelahError::UnsupportedFormat(m) => ("UnsupportedFormat".into(), m.clone()),
        SelahError::AnnotationError(m) => ("AnnotationError".into(), m.clone()),
        SelahError::Api(m) => ("Api".into(), m.clone()),
        SelahError::Io(io) => ("Io".into(), io.to_string()),
        other => ("Unknown".into(), other.to_string()),
    }
}

fn err_toks(e: &SelahError) -> Vec<String> {
    let (v, m) = err_parts(e);
    vec![format!("E:{v}"), ts(&m)]
}

const KINDS: [(AnnotationKind, &str); 7] = [
    (AnnotationKind::Arrow, "Arrow"),
    (AnnotationKind::Rectangle, "Rectangle"),
    (AnnotationKind::Circle, "Circle"),
    (AnnotationKind::Text, "Text"),
    (AnnotationKind::Highlight, "Highlight"),
    (AnnotationKind::Redaction, "Redaction"),
    (AnnotationKind::FreeForm, "FreeForm"),
];
const FORMATS: [(ImageFormat, &str); 4] =
    [(ImageFormat::Png, "Png"), (ImageFormat::Jpeg, "Jpeg"), (ImageFormat::Bmp, "Bmp"), (ImageFormat::WebP, "WebP")];

fn kind_tok(k: &AnnotationKind) -> String {
    let name = KINDS.iter().find(|(x, _)| x == k).map_or("?", |(_, n)| n);
    format!("e:AnnotationKind.{name}")
}
fn fmt_tok(f: ImageFormat) -> String {
    let name = FORMATS.iter().find(|(x, _)| *x == f).map_or("?", |(_, n)| n);
    format!("e:ImageFormat.{name}")
}
fn target_toks(t: &RedactionTarget) -> Vec<String> {
    match t {
        RedactionTarget::Email => vec!["e:RedactionTarget.Email".into()],
        RedactionTarget::Phone => vec!["e:RedactionTarget.Phone".into()],
        RedactionTarget::CreditCard => vec!["e:RedactionTarget.CreditCard".into()],
        RedactionTarget::IpAddress => vec!["e:RedactionTarget.IpAddress".into()],
        RedactionTarget::Custom(s) => vec!["e:RedactionTarget.Custom".into(), ts(s)],
        _ => vec!["e:RedactionTarget.?".into()],
    }
}
fn color_toks(c: &Color) -> Vec<String> {
    vec![ti(c.r.into()), ti(c.g.into()), ti(c.b.into()), ti(c.a.into())]
}
fn layout(rc: &Rect) -> Vec<String> {
    let h = rc.as_hisab();
    vec![tg(h.min.x), tg(h.min.y), tg(h.max.x), tg(h.max.y)]
}
fn annotation_toks(a: &Annotation) -> Vec<String> {
    let mut v = vec![th(a.id.as_bytes()), kind_tok(&a.kind)];
    v.extend(layout(&a.position));
    v.extend(color_toks(&a.color));
    v.push(a.text.as_deref().map_or("N".into(), ts));
    v
}

pub fn emit(out: &mut Out) {
    let mut rows = Vec::new();
    let mut add = |f: &str, inputs: Vec<String>, outputs: Vec<String>| {
        rows.push(Row { func: f.into(), class: "A", inputs, outputs, note: String::new() });
    };

    // AnnotationKind
    for (k, _) in &KINDS {
        add("AnnotationKind::fmt", vec![kind_tok(k)], vec![ts(&k.to_string())]);
        add("AnnotationKind::to_json", vec![kind_tok(k)], vec![ts(&serde_json::to_string(k).expect("json"))]);
    }
    let kind_inputs = [
        "arrow", "rectangle", "rect", "circle", "ellipse", "text", "highlight", "redaction", "redact", "freeform", "free_form",
        "free-form", "ARROW", "ReCt", "FreeForm", "FREE-FORM", "Ellipse", "", " arrow", "arrow ", "free form", "line",
        "squiggle\"q", "tab\there", "nl\nx", "back\\slash", "nul\0x", "esc\x1bx", "del\x7fx", "über", "ARROWß", "émoji😀",
    ];
    for s in kind_inputs {
        let o = match s.parse::<AnnotationKind>() {
            Ok(k) => vec![kind_tok(&k)],
            Err(e) => err_toks(&e),
        };
        add("AnnotationKind::from_str", vec![ts(s)], o);
    }
    for s in [r#""Arrow""#, r#""FreeForm""#, r#""arrow""#, r#""Rect""#, "3", "null", r#"{"Arrow":null}"#] {
        let o = match serde_json::from_str::<AnnotationKind>(s) {
            Ok(k) => vec![kind_tok(&k)],
            Err(e) => vec!["E:serde".into(), ts(&e.to_string())],
        };
        add("AnnotationKind::from_json", vec![ts(s)], o);
    }

    // ImageFormat
    for (f, _) in FORMATS {
        add("ImageFormat::extension", vec![fmt_tok(f)], vec![ts(f.extension())]);
        add("ImageFormat::mime_type", vec![fmt_tok(f)], vec![ts(f.mime_type())]);
        add("ImageFormat::fmt", vec![fmt_tok(f)], vec![ts(&f.to_string())]);
        add("ImageFormat::to_json", vec![fmt_tok(f)], vec![ts(&serde_json::to_string(&f).expect("json"))]);
    }
    for s in ["png", "PNG", "Png", "jpg", "JPG", "jpeg", "JPEG", "Jpeg", "bmp", "BMP", "webp", "WEBP", "WebP", "gif", "", "tiff", " png", "png ", "pn", "jpe"] {
        let o = match s.parse::<ImageFormat>() {
            Ok(f) => vec![fmt_tok(f)],
            Err(e) => err_toks(&e),
        };
        add("ImageFormat::from_str", vec![ts(s)], o);
    }
    let paths = [
        "a.png", "a.PNG", "shot.jpeg", "shot.jpg", "s.Jpg", "b.bmp", "w.webp", "w.WEBP", "a.gif", "a", "a.tar.png", ".png",
        "..png", "dir.png/file", "a.png/", "", "/", "a.", "dir/a.bmp", "/abs/x/y.jpeg", "./rel.png", "a.png.", "a .png",
        "über.png", "a\\b.png",
    ];
    for p in paths {
        let o = match ImageFormat::from_path(std::path::Path::new(p)) {
            Some(f) => vec![fmt_tok(f)],
            None => vec!["N".into()],
        };
        add("ImageFormat::from_path", vec![ts(p)], o);
    }
    for s in [r#""Png""#, r#""WebP""#, r#""png""#, r#""Webp""#, "0"] {
        let o = match serde_json::from_str::<ImageFormat>(s) {
            Ok(f) => vec![fmt_tok(f)],
            Err(e) => vec!["E:serde".into(), ts(&e.to_string())],
        };
        add("ImageFormat::from_json", vec![ts(s)], o);
    }

    // xml_escape
    let mut xs: Vec<String> = (0u32..0x80).map(|c| char::from_u32(c).expect("ascii").to_string()).collect();
    xs.extend(
        [
            "", "plain text", "a & b < c > d \"e\" 'f'", "<script>alert('x')</script>", "tab\tnl\ncr\r", "\u{FFFE}", "\u{FFFF}",
            "\u{FFFD}", "\u{FEFF}bom", "\u{FB01}ligature", "\u{F000}", "\u{EFFF}", "x\u{FFFE}y\u{FFFF}z", "é中😀", "&amp;",
            "&#x27;", "\0\x01\x1f\x7f", "ends with &", "<<>>", "mix\u{1}ed\u{FFFF}<é>\u{FEFF}",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    xs.push("long & text ".repeat(50));
    for s in &xs {
        add("xml_escape", vec![ts(s)], vec![ts(&selah::xml_escape(s))]);
    }

    // derive_output_path
    let inputs = [
        "a.png", "dir/a.png", "/abs/dir/file.tar.gz", "noext", ".hidden", "dir/.hidden", "a.", "dir/", "", "a/b/", "..", ".",
        "a..b", "x.PNG", "über/naïve.jpeg", "/", "a.png/", "./b.bmp", "c:\\win\\d.png",
    ];
    for i in inputs {
        for suffix in ["annotated", "redacted", ""] {
            add("derive_output_path", vec![ts(i), ts(suffix)], vec![ts(&selah::derive_output_path(i, suffix))]);
        }
    }

    // Color
    let mut colors = vec![Color::RED, Color::GREEN, Color::BLUE, Color::BLACK, Color::WHITE, Color::YELLOW, Color::new(0, 0, 0, 0)];
    let mut r = Xs(0xC0_10_75);
    for _ in 0..40 {
        let b = r.next();
        colors.push(Color::new(b as u8, (b >> 8) as u8, (b >> 16) as u8, (b >> 24) as u8));
    }
    for a in 0..=255u8 {
        colors.push(Color::new(12, 34, 56, a));
    }
    for c in &colors {
        add("Color::fmt", color_toks(c), vec![ts(&c.to_string())]);
        add("Color::to_css", color_toks(c), vec![ts(&c.to_css())]);
        add("Color::to_json", color_toks(c), vec![ts(&serde_json::to_string(c).expect("json"))]);
    }
    for s in [
        r#"{"r":255,"g":0,"b":0,"a":255}"#,
        r#"{"a":1,"b":2,"g":3,"r":4}"#,
        r#"{"r":256,"g":0,"b":0,"a":255}"#,
        r#"{"r":-1,"g":0,"b":0,"a":255}"#,
        r#"{"r":1.0,"g":0,"b":0,"a":255}"#,
        r#"{"r":1e2,"g":0,"b":0,"a":255}"#,
        r#"{"r":1,"g":0,"b":0}"#,
        r#"{"r":1,"g":0,"b":0,"a":255,"z":9}"#,
        r#"{"r":"1","g":0,"b":0,"a":255}"#,
        r#"[1,2,3,4]"#,
    ] {
        let o = match serde_json::from_str::<Color>(s) {
            Ok(c) => color_toks(&c),
            Err(e) => vec!["E:serde".into(), ts(&e.to_string())],
        };
        add("Color::from_json", vec![ts(s)], o);
    }

    // RedactionTarget
    let targets = [
        RedactionTarget::Email,
        RedactionTarget::Phone,
        RedactionTarget::CreditCard,
        RedactionTarget::IpAddress,
        RedactionTarget::Custom("ssn".into()),
        RedactionTarget::Custom(String::new()),
        RedactionTarget::Custom("a \"quoted\" é\n".into()),
    ];
    for t in &targets {
        add("RedactionTarget::fmt", target_toks(t), vec![ts(&t.to_string())]);
        add("RedactionTarget::key", target_toks(t), vec![ts(t.key())]);
        add("RedactionTarget::to_json", target_toks(t), vec![ts(&serde_json::to_string(t).expect("json"))]);
    }
    for s in [r#""Email""#, r#""CreditCard""#, r#"{"Custom":"x"}"#, r#""email""#, r#""Custom""#, r#"{"Custom":5}"#, r#"{"Email":null}"#] {
        let o = match serde_json::from_str::<RedactionTarget>(s) {
            Ok(t) => target_toks(&t),
            Err(e) => vec!["E:serde".into(), ts(&e.to_string())],
        };
        add("RedactionTarget::from_json", vec![ts(s)], o);
    }

    // CaptureSource::fmt
    add("CaptureSource::fmt", vec!["e:CaptureSource.FullScreen".into()], vec![ts(&CaptureSource::FullScreen.to_string())]);
    for id in ["0x1a2b", "", "win dow"] {
        add("CaptureSource::fmt", vec!["e:CaptureSource.Window".into(), ts(id)], vec![ts(&CaptureSource::Window(id.into()).to_string())]);
    }
    let rects: [[f32; 4]; 6] = [
        [0.0, 0.0, 1920.0, 1080.0],
        [10.5, -3.25, 0.1, 0.2],
        [100.0, 50.0, -40.0, -30.0],
        [f32::MAX, 0.0, 1.0, f32::MIN_POSITIVE],
        [f32::NAN, f32::INFINITY, -0.0, 1e-45],
        [16_777_216.0, 0.0, 1.0, 3.0],
    ];
    for a in rects {
        let s = CaptureSource::Region(Rect::new(a[0], a[1], a[2], a[3])).to_string();
        let mut ins = vec!["e:CaptureSource.Region".to_string()];
        ins.extend(a.iter().map(|&x| tg(x)));
        add("CaptureSource::fmt", ins, vec![ts(&s)]);
    }

    // Annotation JSON
    let ids = [
        uuid::Uuid::from_bytes([0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0x4d, 0xef, 0x81, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]),
        uuid::Uuid::nil(),
        uuid::Uuid::max(),
    ];
    let texts: [Option<&str>; 5] = [None, Some(""), Some("hello"), Some("q\"b\\s/ \n\t\r\x01\x1f\x7f é😀"), Some("</script>")];
    let mut i = 0usize;
    for (k, _) in &KINDS {
        for t in texts {
            let a = Annotation {
                id: ids[i % ids.len()],
                kind: k.clone(),
                position: Rect::new(10.5, -2.0, 100.0 / 3.0, -0.0),
                color: colors[i % colors.len()],
                text: t.map(str::to_string),
            };
            add("Annotation::to_json", annotation_toks(&a), vec![ts(&serde_json::to_string(&a).expect("json"))]);
            i += 1;
        }
    }
    let docs = [
        r#"{"id":"12345678-9abc-4def-8123-456789abcdef","kind":"Rectangle","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":4},"text":null}"#,
        r#"{"id":"12345678-9abc-4def-8123-456789abcdef","kind":"Text","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":4},"text":"hi \u00e9 \ud83d\ude00 \"q\" \\ \/ \n"}"#,
        r#"{"id":"12345678-9abc-4def-8123-456789abcdef","kind":"Arrow","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":4}}"#,
        r#"{"id":"123456789ABC4DEF8123456789ABCDEF","kind":"Arrow","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":4},"text":null}"#,
        r#"{"id":"{12345678-9abc-4def-8123-456789abcdef}","kind":"Arrow","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":4},"text":null}"#,
        r#"{"id":"urn:uuid:12345678-9abc-4def-8123-456789abcdef","kind":"Arrow","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":4},"text":null}"#,
        r#"{"id":"not-a-uuid","kind":"Arrow","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":4},"text":null}"#,
        r#"{"id":"12345678-9abc-4def-8123-456789abcdef","kind":"arrow","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":4},"text":null}"#,
        r#"{"id":"12345678-9abc-4def-8123-456789abcdef","kind":"Arrow","position":{"x":1e39,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":4},"text":null}"#,
        r#"{"id":"12345678-9abc-4def-8123-456789abcdef","kind":"Arrow","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":400},"text":null}"#,
        r#"{"id":"12345678-9abc-4def-8123-456789abcdef","kind":"Arrow","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":4},"text":7}"#,
        r#"{"kind":"Arrow","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":4},"text":null}"#,
        r#"{"id":"12345678-9abc-4def-8123-456789abcdef","kind":"Arrow","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":4},"text":null,"extra":[1,2]}"#,
        r#"{"id":"12345678-9abc-4def-8123-456789abcdef","kind":"Arrow","position":{"x":1,"y":2,"width":3,"height":4},"color":{"r":1,"g":2,"b":3,"a":4},"text":"\ud800"}"#,
    ];
    for d in docs {
        let o = match serde_json::from_str::<Annotation>(d) {
            Ok(a) => annotation_toks(&a),
            Err(e) => vec!["E:serde".into(), ts(&e.to_string())],
        };
        add("Annotation::from_json", vec![ts(d)], o);
    }

    emit_tsv(out, "core", "enum values as e:<Enum>.<Variant>; a refusal as E:<variant> s:<message>; N is None", &rows);
}
