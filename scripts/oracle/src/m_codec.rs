//! `codec`: `convert_format` verdicts and the encoders' round trips (port plan §3 M3; §4 classes A and R).
//!
//! Rows:
//! - `convert_format A p:fixtures/<file> e:ImageFormat.<T>` → `i:<w> i:<h>` (the converted image's dimensions,
//!   read back with `image`) or `E:<variant>`. Every fixture of at most 1 MP, against every target, through
//!   `selah::convert_format` itself. The pixels are covered by `decode` and `encode`.
//! - `encode R p:decode/<file>.rgba i:<w> i:<h> e:ImageFormat.<T>`: the oracle's decoded pixels of a fixture as
//!   the encoders' input, so both sides encode the same image.
//!   - PNG and BMP → `b:1`: lossless, so decode(encode(px)) must equal px.
//!   - JPEG → `i:<mean × 1000> i:<max>`: Rust selah's own JPEG of the pixels (`convert_format` from a lossless
//!     PNG of them), decoded by `image`, against the pixels: the mean and the largest absolute error over RGB.
//!     The port's encoder must be no less accurate (ADR 0003; port-parity.md §5).
//!   - WebP → `b:1` with `adr=0002#1`: the port refuses it.
//!
//! `--reverse <dir>` (in main.rs) is the other half of class R: Rust decodes the files the Cyrius encoders wrote.

use crate::fixtures::Fixture;
use crate::m_decode::{BIG_RGBA, variant};
use crate::{Out, Row, emit_tsv};
use selah::ImageFormat as F;

pub const TARGETS: [(F, &str); 4] = [(F::Png, "Png"), (F::Jpeg, "Jpeg"), (F::Bmp, "Bmp"), (F::WebP, "WebP")];

/// A lossless PNG of RGBA8 pixels.
pub fn png_of(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_raw(w, h, rgba.to_vec()).expect("sized");
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).expect("png");
    buf.into_inner()
}

/// The mean (× 1000, truncated) and the largest absolute error of `got` against `want` over the RGB channels.
pub fn err_vs(got: &[u8], want: &[u8]) -> (u64, u64) {
    let (mut sum, mut mx, mut n) = (0u64, 0u64, 0u64);
    for (i, (a, b)) in got.iter().zip(want).enumerate() {
        if i % 4 == 3 {
            continue;
        }
        let d = u64::from(a.abs_diff(*b));
        sum += d;
        mx = mx.max(d);
        n += 1;
    }
    if n == 0 { (0, 0) } else { (sum * 1000 / n, mx) }
}

/// The squared-error sum and the sample count of `got` against `want` over RGB, for PSNR.
pub fn sse(got: &[u8], want: &[u8]) -> (u64, u64) {
    let (mut s, mut n) = (0u64, 0u64);
    for (i, (a, b)) in got.iter().zip(want).enumerate() {
        if i % 4 == 3 {
            continue;
        }
        let d = u64::from(a.abs_diff(*b));
        s += d * d;
        n += 1;
    }
    (s, n)
}

/// PSNR in dB from a squared-error sum over n samples (100 for an exact match).
pub fn psnr(s: u64, n: u64) -> f64 {
    if s == 0 || n == 0 { 100.0 } else { 10.0 * (255.0f64 * 255.0 / (s as f64 / n as f64)).log10() }
}

/// Rust selah's JPEG of the pixels, decoded by `image`.
pub fn rust_jpeg_px(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let jpg = selah::convert_format(&png_of(rgba, w, h), F::Jpeg).expect("selah encodes JPEG");
    image::load_from_memory_with_format(&jpg, image::ImageFormat::Jpeg).expect("decodes").to_rgba8().into_raw()
}

/// Rust selah's JPEG of the pixels, decoded by `image`: its error against them.
pub fn rust_jpeg_err(rgba: &[u8], w: u32, h: u32) -> (u64, u64) {
    let jpg = selah::convert_format(&png_of(rgba, w, h), F::Jpeg).expect("selah encodes JPEG");
    let back = image::load_from_memory_with_format(&jpg, image::ImageFormat::Jpeg).expect("decodes").to_rgba8();
    err_vs(back.as_raw(), rgba)
}

pub fn emit(out: &mut Out, fixtures: &[Fixture]) {
    let mut rows = Vec::new();
    for f in fixtures {
        if f.file.contains("4100x4100") {
            continue; // over 1 MP: its verdict follows `decode`, and four conversions of 16.8 MP are minutes
        }
        let src_adr = f.adr;
        for (fmt, name) in TARGETS {
            let input = vec![format!("p:fixtures/{}", f.file), format!("e:ImageFormat.{name}")];
            match selah::convert_format(&f.bytes, fmt) {
                Ok(bytes) => {
                    let img = image::load_from_memory(&bytes).expect("the converted bytes decode");
                    let adr = if fmt == F::WebP { Some("0002#1") } else { src_adr };
                    let adr = adr.map(|a| format!(" adr={a}")).unwrap_or_default();
                    rows.push(Row {
                        func: "convert_format".into(),
                        class: "A",
                        inputs: input,
                        outputs: vec![format!("i:{}", img.width()), format!("i:{}", img.height())],
                        note: format!("{} -> {name}{adr}", f.file),
                    });
                }
                Err(e) => rows.push(Row {
                    func: "convert_format".into(),
                    class: "A",
                    inputs: input,
                    outputs: vec![format!("E:{}", variant(&e))],
                    note: format!("{} -> {name} -- refused: {e}", f.file),
                }),
            }
        }
    }

    // The encoders, on every decoded fixture's pixels.
    for f in fixtures {
        let Ok(png) = selah::annotate_image(&f.bytes, &[], F::Png) else { continue };
        let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png).expect("decodes").to_rgba8();
        let (w, h) = img.dimensions();
        let rgba = img.into_raw();
        if rgba.len() > BIG_RGBA {
            continue;
        }
        for (fmt, name) in TARGETS {
            let inputs = vec![
                format!("p:decode/{}.rgba", f.file),
                format!("i:{w}"),
                format!("i:{h}"),
                format!("e:ImageFormat.{name}"),
            ];
            let (outputs, note) = match fmt {
                F::Jpeg => {
                    let (mean, mx) = rust_jpeg_err(&rgba, w, h);
                    (
                        vec![format!("i:{mean}"), format!("i:{mx}")],
                        format!("{}: Rust's q75 JPEG, decoded by image: mean {}.{:03} max {mx}", f.file, mean / 1000, mean % 1000),
                    )
                }
                F::WebP => (vec!["b:1".to_string()], format!("{}: lossless adr=0002#1", f.file)),
                _ => (vec!["b:1".to_string()], format!("{}: lossless", f.file)),
            };
            rows.push(Row { func: "encode".into(), class: "R", inputs, outputs, note });
        }
    }
    emit_tsv(
        out,
        "codec",
        "convert_format: dimensions or E:<variant>; encode: b:1 lossless, or Rust's JPEG error i:<mean x 1000> i:<max>",
        &rows,
    );
}

/// `--reverse <dir>`: Rust decodes what the Cyrius encoders wrote. For each `<stem>.png` / `.bmp` / `.jpg` in
/// `dir` beside `<stem>.rgba` (the pixels encoded) and `<stem>.dims` (`<w> <h>`):
/// - every file must decode, at the right size;
/// - PNG and BMP must decode to exactly the pixels;
/// - JPEG is measured against Rust's own JPEG of the same pixels, both decoded by `image`: PSNR and mean error, per
///   file (with SELAH_REVERSE_VERBOSE) and over the corpus. Not gated per file: two correct encoders differ there by
///   rounding noise; the encoder is gated on the exact mathematics in the parity harness (port-parity.md §5).
///
/// Returns the number of failures.
pub fn reverse(dir: &std::path::Path) -> usize {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .expect("read the reverse directory")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("png" | "bmp" | "jpg")))
        .collect();
    names.sort();
    let (mut ok, mut bad) = (0usize, 0usize);
    let mut worst_margin = i64::MIN;
    // Corpus totals over the JPEGs: the sum of the per-file means (Cyrius, Rust), and better / equal / worse.
    let (mut sum_c, mut sum_r, mut better, mut equal, mut worse) = (0u64, 0u64, 0usize, 0usize, 0usize);
    let (mut sse_c, mut sse_r, mut sse_n, mut worst_psnr) = (0u64, 0u64, 0u64, f64::INFINITY);
    for p in &names {
        // <fixture file>.<ext> beside <fixture file>.rgba and .dims (the fixture's name keeps its own extension).
        let stem = p.with_extension("").display().to_string();
        let rgba = std::fs::read(format!("{stem}.rgba")).expect("the .rgba beside it");
        let dims = std::fs::read_to_string(format!("{stem}.dims")).expect("the .dims beside it");
        let mut it = dims.split_whitespace().map(|t| t.parse::<u32>().expect("dims"));
        let (w, h) = (it.next().expect("w"), it.next().expect("h"));
        let bytes = std::fs::read(p).expect("read");
        let got = match image::load_from_memory(&bytes) {
            Ok(i) => i.to_rgba8(),
            Err(e) => {
                eprintln!("FAILED   {}: Rust cannot decode it: {e}", p.display());
                bad += 1;
                continue;
            }
        };
        if got.dimensions() != (w, h) {
            eprintln!("FAILED   {}: {:?}, expected {w}x{h}", p.display(), got.dimensions());
            bad += 1;
            continue;
        }
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "jpg" {
            let (cm, cx) = err_vs(got.as_raw(), &rgba);
            let rpx = rust_jpeg_px(&rgba, w, h);
            let (rm, rx) = err_vs(&rpx, &rgba);
            let (cs, cn) = sse(got.as_raw(), &rgba);
            let (rs, rn) = sse(&rpx, &rgba);
            sse_c += cs;
            sse_r += rs;
            sse_n += cn;
            let dp = psnr(cs, cn) - psnr(rs, rn);
            worst_psnr = worst_psnr.min(dp);
            if std::env::var_os("SELAH_REVERSE_VERBOSE").is_some() {
                println!("  {}: PSNR Cyrius {:.3} Rust {:.3} ({:+.3} dB); mean {} vs {}; max {cx} vs {rx}",
                    p.file_name().unwrap_or_default().to_string_lossy(), psnr(cs, cn), psnr(rs, rn), dp, cm, rm);
            }
            let margin = cm as i64 - rm as i64;
            worst_margin = worst_margin.max(margin);
            sum_c += cm;
            sum_r += rm;
            match cm.cmp(&rm) {
                std::cmp::Ordering::Less => better += 1,
                std::cmp::Ordering::Equal => equal += 1,
                std::cmp::Ordering::Greater => worse += 1,
            }
        } else if got.as_raw() != &rgba {
            eprintln!("FAILED   {}: Rust decodes different pixels", p.display());
            bad += 1;
            continue;
        }
        ok += 1;
    }
    println!(
        "oracle --reverse: {ok} ok, {bad} FAILED of {} files; JPEG, measured: worst file's mean error vs Rust's {}{}",
        names.len(),
        if worst_margin < 0 { "-" } else { "+" },
        format!("{}.{:03}", worst_margin.unsigned_abs() / 1000, worst_margin.unsigned_abs() % 1000),
    );
    let jn = (better + equal + worse).max(1) as u64;
    println!(
        "oracle --reverse: JPEG over {} files: mean of the means, Cyrius {}.{:03} vs Rust {}.{:03}; Cyrius better on {better}, equal on {equal}, worse on {worse}",
        better + equal + worse,
        sum_c / jn / 1000, sum_c / jn % 1000, sum_r / jn / 1000, sum_r / jn % 1000,
    );
    println!(
        "oracle --reverse: JPEG corpus PSNR Cyrius {:.3} dB vs Rust {:.3} dB; worst file {:+.3} dB",
        psnr(sse_c, sse_n), psnr(sse_r, sse_n), worst_psnr
    );
    bad
}
