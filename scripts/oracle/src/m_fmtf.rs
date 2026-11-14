//! `fmtf`: float text as Rust prints it (port plan §3 M1).
//!
//! selah writes f32 rect coordinates in SVG through `Display` (`{x}`), and every float in its JSON through
//! serde_json 1.0.154 (zmij). Both print the shortest digits that read back, positioned differently. Rows:
//! - `f32::fmt g:<bits> → s:<text>` and `f32::to_json g:<bits> → s:<text>`;
//! - `f64::fmt f:<bits> → s:<text>` and `f64::to_json f:<bits> → s:<text>`.
//!
//! The grid: every binade (2^e for every exponent, and its neighbours), every power of ten in range and its
//! neighbours, integers 0..=1100, the subnormal and range edges, ±0, ±∞, NaN, and a seeded sweep of bit patterns.

use crate::{Out, Row, Xs, emit_tsv, tf, tg, ts};

fn f32_grid() -> Vec<f32> {
    let mut v: Vec<f32> = vec![0.0, -0.0, f32::INFINITY, f32::NEG_INFINITY, f32::NAN, f32::MAX, f32::MIN, f32::MIN_POSITIVE, f32::EPSILON];
    for bits in [1u32, 2, 3, 0x007F_FFFF, 0x0080_0000, 0x0080_0001, 0x7F7F_FFFE] {
        v.push(f32::from_bits(bits));
    }
    for e in 0u32..=254 {
        let b = e << 23;
        for d in [0u32, 1, 2] {
            v.push(f32::from_bits(b + d));
        }
        if b > 0 {
            v.push(f32::from_bits(b - 1));
        }
    }
    for p in -45i32..=38 {
        let x: f32 = format!("1e{p}").parse().expect("f32 literal");
        v.push(x);
        v.push(f32::from_bits(x.to_bits().wrapping_add(1)));
        v.push(f32::from_bits(x.to_bits().wrapping_sub(1)));
    }
    for i in 0..=1100u32 {
        v.push(i as f32);
    }
    for x in [0.1f32, 0.2, 0.3, 0.7, 2.5, 33.3, 100.25, 1920.0, 1080.0, 1e15, 1e16, 1e17, 0.0001, 0.00001, 0.000001] {
        v.push(x);
        v.push(-x);
    }
    let mut r = Xs(0x5E1A_F32);
    for _ in 0..3000 {
        let b = (r.next() >> 32) as u32;
        v.push(f32::from_bits(b));
    }
    v
}

fn f64_grid() -> Vec<f64> {
    let mut v: Vec<f64> = vec![0.0, -0.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN, f64::MAX, f64::MIN, f64::MIN_POSITIVE, f64::EPSILON];
    for bits in [1u64, 2, 0x000F_FFFF_FFFF_FFFF, 0x0010_0000_0000_0000] {
        v.push(f64::from_bits(bits));
    }
    for e in (0u64..=2046).step_by(3) {
        let b = e << 52;
        v.push(f64::from_bits(b));
        v.push(f64::from_bits(b + 1));
        if b > 0 {
            v.push(f64::from_bits(b - 1));
        }
    }
    for p in -324i32..=308 {
        let x: f64 = format!("1e{p}").parse().expect("f64 literal");
        v.push(x);
        v.push(f64::from_bits(x.to_bits().wrapping_add(1)));
    }
    // 2^-25: the tie Rust's core printer rounds up and zmij rounds to even (impetus ADR 0010).
    v.push(2f64.powi(-25));
    for x in [0.1f64, 0.2, 0.3, 0.95, 0.7, 2.5, 1.0 / 3.0, 100.0, 1e15, 1e16, 1e17, 1e-5, 1e-6, 1e-7] {
        v.push(x);
        v.push(-x);
    }
    for a in 0..=255u32 {
        v.push(f64::from(a) / 255.0);
    }
    let mut r = Xs(0x5E1A_F64);
    for _ in 0..3000 {
        v.push(f64::from_bits(r.next()));
    }
    v
}

pub fn emit(out: &mut Out) {
    let mut rows = Vec::new();
    for x in f32_grid() {
        rows.push(Row { func: "f32::fmt".into(), class: "A", inputs: vec![tg(x)], outputs: vec![ts(&format!("{x}"))], note: String::new() });
        let j = serde_json::to_string(&x).expect("f32 json");
        rows.push(Row { func: "f32::to_json".into(), class: "A", inputs: vec![tg(x)], outputs: vec![ts(&j)], note: String::new() });
    }
    for x in f64_grid() {
        rows.push(Row { func: "f64::fmt".into(), class: "A", inputs: vec![tf(x)], outputs: vec![ts(&format!("{x}"))], note: String::new() });
        let j = serde_json::to_string(&x).expect("f64 json");
        rows.push(Row { func: "f64::to_json".into(), class: "A", inputs: vec![tf(x)], outputs: vec![ts(&j)], note: String::new() });
    }
    emit_tsv(out, "fmtf", "s:<text> as Rust Display (fmt) or serde_json 1.0.154 (to_json) writes the float", &rows);
}
