//! `decode`: what selah 0.30.0 makes of each fixture (port plan §2 step 5; §3 M3's grid).
//!
//! The oracle's verdict is taken through selah's own public path, so its limits, format guessing and
//! RGBA conversion are the ones measured: `selah::annotate_image(bytes, &[], ImageFormat::Png)` decodes
//! exactly as `render_to_image` does, draws nothing, and re-encodes losslessly as PNG, which is decoded
//! back to RGBA8 here. So a row's pixels are selah's decoded pixels, bit for bit.
//!
//! Row: `decode <class> p:fixtures/<file> → i:<w> i:<h> p:decode/<file>.rgba` on success (`c:<crc32>` in place
//! of the blob when the RGBA is over 4 MiB), or
//! `→ E:<SelahError variant>` with the message in the note. Class A for PNG, BMP and every refusal;
//! class B for a decoded JPEG (the IDCTs differ; port plan §4).

use crate::fixtures::Fixture;

/// Decoded pixels larger than this are written as a `c:` CRC-32 token instead of a `p:` blob.
pub const BIG_RGBA: usize = 4 << 20;
use crate::{Out, Row, emit_tsv};

pub fn variant(e: &selah::SelahError) -> String {
    let dbg = format!("{e:?}");
    dbg.split(['(', ' ', '{']).next().unwrap_or("Unknown").to_string()
}

pub fn emit(out: &mut Out, fixtures: &[Fixture]) {
    let mut rows = Vec::new();
    for f in fixtures {
        let input = format!("p:fixtures/{}", f.file);
        let adr = f.adr.map(|a| format!(" adr={a}")).unwrap_or_default();
        match selah::annotate_image(&f.bytes, &[], selah::ImageFormat::Png) {
            Ok(png) => {
                let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
                    .expect("selah's own PNG output decodes")
                    .to_rgba8();
                let (w, h) = img.dimensions();
                let rgba = img.into_raw();
                // Pixels over BIG_RGBA bytes are carried as their CRC-32 rather than a blob in the repository.
                let pixels = if rgba.len() > BIG_RGBA {
                    let mut c = flate2::Crc::new();
                    c.update(&rgba);
                    format!("c:{:08x}", c.sum())
                } else {
                    let blob = format!("decode/{}.rgba", f.file);
                    out.insert(blob.clone(), rgba);
                    format!("p:{blob}")
                };
                rows.push(Row {
                    func: "decode".into(),
                    class: if f.kind == "jpeg" { "B" } else { "A" },
                    inputs: vec![input],
                    outputs: vec![format!("i:{w}"), format!("i:{h}"), pixels],
                    note: format!("{}: {}{adr}", f.kind, f.how),
                });
            }
            Err(e) => rows.push(Row {
                func: "decode".into(),
                class: "A",
                inputs: vec![input],
                outputs: vec![format!("E:{}", variant(&e))],
                note: format!("{}: {} -- refused: {}{adr}", f.kind, f.how, e),
            }),
        }
    }
    emit_tsv(out, "decode", "decode: E:<variant> on refusal, the message in the note; adr=0002#N marks an accepted divergence", &rows);
}
