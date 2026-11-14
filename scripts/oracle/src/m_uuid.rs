//! `uuid`: how selah writes and reads ids (port plan §3 M1). selah's ids are uuid 1.29.2 `Uuid`s: written
//! hyphenated (serde, `Display`) or simple (SVG arrow markers), read through serde, which takes `Uuid::parse_str`'s forms.
//! Rows: `Uuid::fmt h → s`, `Uuid::simple h → s`, `Uuid::to_json h → s`, `Uuid::from_json s → h | E:serde s`.

use crate::{Out, Row, Xs, emit_tsv, th, ts};
use uuid::Uuid;

pub fn emit(out: &mut Out) {
    let mut rows = Vec::new();
    let mut add = |f: &str, inputs: Vec<String>, outputs: Vec<String>| {
        rows.push(Row { func: f.into(), class: "A", inputs, outputs, note: String::new() });
    };
    let mut ids = vec![Uuid::nil(), Uuid::max()];
    let mut r = Xs(0x0_1D5);
    for _ in 0..40 {
        let a = r.next();
        let b = r.next();
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&a.to_be_bytes());
        bytes[8..].copy_from_slice(&b.to_be_bytes());
        ids.push(Uuid::from_bytes(bytes));
    }
    for id in &ids {
        add("Uuid::fmt", vec![th(id.as_bytes())], vec![ts(&id.to_string())]);
        add("Uuid::simple", vec![th(id.as_bytes())], vec![ts(&id.simple().to_string())]);
        add("Uuid::to_json", vec![th(id.as_bytes())], vec![ts(&serde_json::to_string(id).expect("json"))]);
    }
    let docs = [
        r#""12345678-9abc-4def-8123-456789abcdef""#,
        r#""12345678-9ABC-4DEF-8123-456789ABCDEF""#,
        r#""123456789abc4def8123456789abcdef""#,
        r#""{12345678-9abc-4def-8123-456789abcdef}""#,
        r#""urn:uuid:12345678-9abc-4def-8123-456789abcdef""#,
        r#""URN:UUID:12345678-9abc-4def-8123-456789abcdef""#,
        r#""12345678-9abc-4def-8123-456789abcde""#,
        r#""12345678-9abc-4def-8123-456789abcdeff""#,
        r#""1234567-89abc-4def-8123-456789abcdef""#,
        r#""12345678-9abc-4def-8123-456789abcdeg""#,
        r#""12345678 9abc 4def 8123 456789abcdef""#,
        r#""{12345678-9abc-4def-8123-456789abcdef""#,
        r#""""#,
        r#"" 12345678-9abc-4def-8123-456789abcdef""#,
        r#""00000000-0000-0000-0000-000000000000""#,
        "12",
        "null",
        r#"[1,2]"#,
    ];
    for d in docs {
        let o = match serde_json::from_str::<Uuid>(d) {
            Ok(u) => vec![th(u.as_bytes())],
            Err(e) => vec!["E:serde".into(), ts(&e.to_string())],
        };
        add("Uuid::from_json", vec![ts(d)], o);
    }
    emit_tsv(out, "uuid", "h:<16 bytes> for an id", &rows);
}
