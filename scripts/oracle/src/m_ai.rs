//! `ai`: PII detection, the stub OCR and smart-crop suggestions (port plan §3 M2). Rows:
//! - `extract_text_regions h:<bytes> → s:<text> f:<confidence> b:<is_stub>` (its bounding boxes are always empty);
//! - `suggest_redactions s:<text> → i:<n>`, then per suggestion `e:RedactionTarget.<V> s:<matched> f:<confidence>`
//!   (its region is always `Rect::default()`);
//! - `suggest_smart_crop i:<w> i:<h> → i:<n>`, then per suggestion the region's layout `g×4`, `s:<reason>`,
//!   `f:<confidence>`;
//! - `char::is_alphanumeric i:<code point> → b` (a cross-check of src/ai_data.cyr's lookup);
//! - `OcrResult::to_json`, `RedactionSuggestion::to_json` (`s:<text> i:<k>`: the k-th suggestion of the text),
//!   `SmartCropSuggestion::to_json` → `s:<json>`, and
//!   `OcrResult::from_json s:<json> → s:<text> f:<confidence> i:<boxes> g×4… b:<is_stub> | E:serde s`.

use crate::fixtures::Fixture;
use crate::{Out, Row, Xs, emit_tsv, tb, tf, tg, th, ti, ts};
use selah::{OcrResult, Rect, RedactionTarget, SmartCropSuggestion};

fn target_tok(t: &RedactionTarget) -> String {
    match t {
        RedactionTarget::Email => "e:RedactionTarget.Email",
        RedactionTarget::Phone => "e:RedactionTarget.Phone",
        RedactionTarget::CreditCard => "e:RedactionTarget.CreditCard",
        RedactionTarget::IpAddress => "e:RedactionTarget.IpAddress",
        _ => "e:RedactionTarget.Custom",
    }
    .into()
}

fn layout(rc: &Rect) -> Vec<String> {
    let h = rc.as_hisab();
    vec![tg(h.min.x), tg(h.min.y), tg(h.max.x), tg(h.max.y)]
}

/// A random digit string of `len` digits whose Luhn checksum passes (or, with `valid` false, fails).
fn luhn_number(r: &mut Xs, len: usize, valid: bool) -> String {
    let mut d: Vec<u32> = (0..len - 1).map(|_| (r.next() % 10) as u32).collect();
    if d[0] == 0 {
        d[0] = 4;
    }
    // Luhn: double every second digit from the right, the check digit not doubled.
    let mut sum = 0;
    for (i, &x) in d.iter().rev().enumerate() {
        let v = if i % 2 == 0 { x * 2 } else { x };
        sum += if v > 9 { v - 9 } else { v };
    }
    let mut check = (10 - sum % 10) % 10;
    if !valid {
        check = (check + 1 + (r.next() % 9) as u32) % 10;
    }
    d.push(check);
    d.iter().map(|x| char::from_digit(*x, 10).expect("digit")).collect()
}

/// Splits a digit string into groups of the given sizes (the remainder as the last group).
fn grouped(digits: &str, sizes: &[usize], sep: &str) -> String {
    let mut out = Vec::new();
    let mut i = 0;
    for &s in sizes {
        if i + s >= digits.len() {
            break;
        }
        out.push(&digits[i..i + s]);
        i += s;
    }
    out.push(&digits[i..]);
    out.join(sep)
}

fn pick<'a>(r: &mut Xs, v: &[&'a str]) -> &'a str {
    v[(r.next() % v.len() as u64) as usize]
}

/// One PII-shaped (or nearly) fragment.
fn fragment(r: &mut Xs) -> String {
    let wraps = ["", "", "", "<>", "()", "\"\"", "«»", "[]"];
    let trail = ["", "", "", ".", ",", ";", ":", "!", "?", ")", "]", "\"", "'", "...", ".)"];
    let lead = ["", "", "", "(", "<", "card:", "ip=", "tel:", "#", "+"];
    let words = ["call", "email", "the", "card", "at", "and", "é", "日本", "—", "x", "ID", "2024", "12/30", "v1.2.3"];
    let w = pick(r, &wraps);
    let core = match r.next() % 9 {
        0 => {
            let locals = ["user", "a", "first.last", "x_y+z", "ünï", "o'neil", "1", ""];
            let domains = ["example", "corp", "例え", "a-b", "x", ""];
            let tlds = ["com", "io", "c", "123", "co.uk", "jp", "公司", "c0m", ""];
            let at = if r.next() % 9 == 0 { "@@" } else { "@" };
            let dot = if r.next() % 8 == 0 { "" } else { "." };
            format!("{}{}{}{}{}", pick(r, &locals), at, pick(r, &domains), dot, pick(r, &tlds))
        }
        1 | 2 => {
            let len = 12 + (r.next() % 9) as usize;
            let valid = r.next() % 3 != 0;
            let n = luhn_number(r, len, valid);
            match r.next() % 6 {
                0 => n,
                1 => grouped(&n, &[4, 4, 4, 4, 4], "-"),
                2 => grouped(&n, &[4, 4, 4, 4, 4], " "),
                3 => grouped(&n, &[4, 6, 5], " "),
                4 => grouped(&n, &[4, 3, 6, 6], pick(r, &[" ", "  ", "\t"])),
                _ => grouped(&n, &[4, 4, 4], " "),
            }
        }
        3 | 4 => {
            let parts = 3 + (r.next() % 3) as usize;
            let oct: Vec<String> = (0..parts)
                .map(|_| match r.next() % 7 {
                    0 => format!("{:03}", r.next() % 300),
                    1 => "0004".into(),
                    _ => (r.next() % 300).to_string(),
                })
                .collect();
            oct.join(".")
        }
        5 | 6 => {
            let d = |r: &mut Xs, k: usize| -> String { (0..k).map(|_| char::from_digit((r.next() % 10) as u32, 10).unwrap()).collect() };
            match r.next() % 6 {
                0 => format!("({}) {}-{}", d(r, 3), d(r, 3), d(r, 4)),
                1 => format!("{}-{}-{}", d(r, 3), d(r, 3), d(r, 4)),
                2 => format!("+{}-{}-{}-{}", d(r, 1), d(r, 3), d(r, 3), d(r, 4)),
                3 => format!("({}){}.{}", d(r, 3), d(r, 3), d(r, 4)),
                4 => {
                    let k = 8 + (r.next() % 9) as usize;
                    d(r, k)
                }
                _ => format!("{}.{}.{}#{}", d(r, 3), d(r, 3), d(r, 4), d(r, 2)),
            }
        }
        7 => pick(r, &words).to_string(),
        _ => format!("{}{}", pick(r, &lead), pick(r, &words)),
    };
    let (a, b) = if w.is_empty() { ("", "") } else { w.split_at(w.chars().next().unwrap().len_utf8()) };
    format!("{}{}{}{}{}", pick(r, &lead), a, core, b, pick(r, &trail))
}

fn sep(r: &mut Xs) -> &'static str {
    pick(r, &[" ", " ", " ", " ", "  ", "\t", "\n", "\u{a0}", "\u{3000}", "\u{2028}", " \r\n "])
}

pub fn emit(out: &mut Out, fixtures: &[Fixture]) {
    let mut rows = Vec::new();
    let mut add = |f: &str, inputs: Vec<String>, outputs: Vec<String>| {
        rows.push(Row { func: f.into(), class: "A", inputs, outputs, note: String::new() });
    };

    // extract_text_regions
    let mut blobs: Vec<Vec<u8>> = vec![
        b"....Hello World!....".to_vec(),
        Vec::new(),
        (0..100).map(|i| (i * 7 % 20) as u8).collect(),
        b"\x00abcd\x01\x02efgh\x00ab\x00ijkl".to_vec(),
        b"abc".to_vec(),
        b"abcd".to_vec(),
        b"abc\x00defg\x7fhij~ \x1f    ".to_vec(),
        (0u8..=255).collect(),
    ];
    for f in fixtures {
        blobs.push(f.bytes.clone());
    }
    let mut r = Xs(0x0C_7EE7);
    for k in 0..120 {
        let len = (r.next() % (64 << (k % 6))) as usize;
        let dense = r.next() % 4;
        blobs.push(
            (0..len)
                .map(|_| {
                    let b = (r.next() >> 24) as u8;
                    if dense > 0 && b % 4 != 0 { 0x20 + b % 0x5F } else { b }
                })
                .collect(),
        );
    }
    let mut ocrs: Vec<OcrResult> = Vec::new();
    for b in &blobs {
        let o = selah::extract_text_regions(b);
        add("extract_text_regions", vec![th(b)], vec![ts(&o.text), tf(o.confidence), tb(o.is_stub)]);
        ocrs.push(o);
    }

    // suggest_redactions
    let mut texts: Vec<String> = [
        "contact user@example.com for info",
        "call 555-123-4567 now",
        "id 5551234567 here",
        "server at 192.168.1.100 is down",
        "not an IP: 999.999.999.999",
        "card: 4111111111111111",
        "id: 1234567890123",
        "this is perfectly clean text with no PII",
        "bad email: user@localhost",
        "test user@example.x",
        "test user@example.123",
        "card: 4111-1111-1111-1111",
        "addr 0.0.0.0 here",
        "addr 255.255.255.255 here",
        "not ip: 192.168.1",
        "email user@test.com phone 555-123-4567 ip 10.0.0.1",
        "mail me at user@example.com.",
        "<admin@corp.io>, (ops@corp.io)",
        "server is 10.0.0.1.",
        "host=192.168.0.7,",
        "ip 192.168.100.200",
        "1.2.3.4.5",
        "1.2.3.0004",
        "card 4111 1111 1111 1111 exp 12/30",
        "amex 3782 822463 10005.",
        "1234 5678 9012 3456",
        "2020 2021",
        "card:4111111111111111.",
        "3782-822463-10005",
        "call (555) 123-4567 today",
        "call 555-123-4567.",
        "10.0.0.1 then a@b.co then 555-123-4567",
        "ünïcödé@例え.jp (５５５) ４１１１ — 🎉 user@例子.公司 «a@b.cd»",
        "«a@b.cd»",
        "",
        "   ",
        "a@b.cd@e.fg",
        "@example.com",
        "x@.com",
        "4111 1111 1111 1111 1111",
        "4111 1111 1111 1111 111",
        "4111\u{a0}1111\u{a0}1111\u{a0}1111",
        "4111 1111. 1111 1111",
        "(555)  123-4567",
        "x(555) 123-4567,",
        "(555) 123-45678",
        "+1 (555) 123-4567",
        "ⓐ@example.com",
        "́a@b.co",
        "٣a@b.co",
        "a@b.coⅫ",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut r = Xs(0xA1_9117);
    for _ in 0..3000 {
        let n = 1 + (r.next() % 7) as usize;
        let mut s = String::new();
        for i in 0..n {
            if i > 0 {
                s.push_str(sep(&mut r));
            }
            s.push_str(&fragment(&mut r));
        }
        texts.push(s);
    }
    for t in &texts {
        let sug = selah::suggest_redactions(t);
        let mut o = vec![ti(sug.len() as i64)];
        for s in &sug {
            o.push(target_tok(&s.target_type));
            o.push(ts(&s.matched_text));
            o.push(tf(s.confidence));
        }
        add("suggest_redactions", vec![ts(t)], o);
    }
    for t in texts.iter().take(400) {
        for (k, s) in selah::suggest_redactions(t).iter().enumerate() {
            add("RedactionSuggestion::to_json", vec![ts(t), ti(k as i64)], vec![ts(&serde_json::to_string(s).expect("json"))]);
        }
    }

    // suggest_smart_crop
    let mut dims: Vec<(u32, u32)> = vec![
        (0, 0), (0, 5), (5, 0), (1, 1), (2, 2), (3, 3), (7, 5), (100, 50), (1920, 1080), (3840, 2160), (1080, 1920),
        (16_777_216, 1), (16_777_217, 3), (16_777_219, 16_777_221), (u32::MAX, u32::MAX), (u32::MAX, 1),
    ];
    let mut r = Xs(0xC2_0F);
    for k in 0..200u32 {
        let m = 1u64 << (4 + k % 29);
        dims.push(((r.next() % m) as u32, (r.next() % m) as u32));
    }
    let mut crops: Vec<SmartCropSuggestion> = Vec::new();
    for (w, h) in dims {
        let sug = selah::suggest_smart_crop(w, h);
        let mut o = vec![ti(sug.len() as i64)];
        for s in &sug {
            o.extend(layout(&s.region));
            o.push(ts(&s.reason));
            o.push(tf(s.confidence));
        }
        add("suggest_smart_crop", vec![ti(w.into()), ti(h.into())], o);
        crops.extend(sug);
    }
    for c in crops.iter().take(200) {
        let mut ins = layout(&c.region);
        ins.push(ts(&c.reason));
        ins.push(tf(c.confidence));
        add("SmartCropSuggestion::to_json", ins, vec![ts(&serde_json::to_string(c).expect("json"))]);
    }

    // char::is_alphanumeric, against the generated table's lookup
    let mut cps: Vec<u32> = vec![0, 0x2F, 0x30, 0x39, 0x3A, 0x40, 0x41, 0x5A, 0x5B, 0x60, 0x61, 0x7A, 0x7B, 0xAA, 0xB2, 0xBA, 0x300, 0x345, 0x660, 0x2160, 0x24B6, 0x24E9, 0x3007, 0xFF15, 0x10FFFF, 0xE0000];
    let mut r = Xs(0xA1_7A);
    for _ in 0..3000 {
        cps.push((r.next() % 0x11_0000) as u32);
    }
    for _ in 0..1000 {
        cps.push((r.next() % 0x3_0000) as u32);
    }
    for cp in cps {
        if let Some(c) = char::from_u32(cp) {
            add("char::is_alphanumeric", vec![ti(cp.into())], vec![tb(c.is_alphanumeric())]);
        }
    }

    // OcrResult JSON
    ocrs.push(OcrResult {
        text: "a \"b\"\n\u{e9}".into(),
        confidence: 0.875,
        bounding_boxes: vec![Rect::new(1.0, 2.0, 3.5, -4.25), Rect::new(0.1, 0.2, 0.3, 0.4)],
        is_stub: false,
    });
    for o in ocrs.iter() {
        let mut ins = vec![ts(&o.text), tf(o.confidence), ti(o.bounding_boxes.len() as i64)];
        for b in &o.bounding_boxes {
            ins.extend(layout(b));
        }
        ins.push(tb(o.is_stub));
        add("OcrResult::to_json", ins, vec![ts(&serde_json::to_string(o).expect("json"))]);
    }
    let docs = [
        r#"{"text":"hi","confidence":0.5,"bounding_boxes":[],"is_stub":true}"#,
        r#"{"text":"hi","confidence":0.5,"bounding_boxes":[]}"#,
        r#"{"text":"hi","confidence":1,"bounding_boxes":[{"x":1,"y":2,"width":3,"height":4}],"is_stub":false}"#,
        r#"{"text":"hi","confidence":0.5,"bounding_boxes":[[1,2,3,4]],"is_stub":false}"#,
        r#"{"text":"hi","confidence":0.5,"bounding_boxes":[{"x":1e39,"y":2,"width":3,"height":4}]}"#,
        r#"{"text":"hi","confidence":0.5}"#,
        r#"{"confidence":0.5,"bounding_boxes":[]}"#,
        r#"{"text":"hi","confidence":"0.5","bounding_boxes":[]}"#,
        r#"{"text":"hi","confidence":0.5,"bounding_boxes":[],"is_stub":1}"#,
        r#"["hi",0.5,[],true]"#,
        r#"["hi",0.5,[]]"#,
    ];
    for d in docs {
        let o = match serde_json::from_str::<OcrResult>(d) {
            Ok(o) => {
                let mut v = vec![ts(&o.text), tf(o.confidence), ti(o.bounding_boxes.len() as i64)];
                for b in &o.bounding_boxes {
                    v.extend(layout(b));
                }
                v.push(tb(o.is_stub));
                v
            }
            Err(e) => vec!["E:serde".into(), ts(&e.to_string())],
        };
        add("OcrResult::from_json", vec![ts(d)], o);
    }

    emit_tsv(out, "ai", "a list as i:<n> then each element's tokens; rects as their layout", &rows);
}
