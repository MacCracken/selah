//! The image fixtures under `tests/golden/fixtures/`: the decode corpus (port plan §2 step 5, §3 M3).
//!
//! Every fixture is built here, deterministically, from seeded xorshift patterns and pinned encoders,
//! so `--check` can rebuild and compare them byte for byte. They are inputs, not oracle outputs: the
//! oracle's verdict on each one (pixels or error) is `m_decode`'s job.
//!
//! Producers:
//! - `png` 0.18.1 for every colour type and bit depth it can write (palette, sub-byte, 16-bit, tRNS);
//! - a hand-written writer for Adam7 interlace (png 0.18 cannot write it) and for malformed PNGs;
//! - `image` 0.25.10's own BMP, JPEG and WebP encoders (what selah 0.30.0 itself writes);
//! - a hand-written BMP writer for the variants `image` cannot write (palette depths, RLE, bitfields,
//!   top-down, V4 headers);
//! - `jpeg-encoder` 0.6.1 for chroma subsampling, progressive, restart intervals and CMYK.

use crate::Out;
use image::ExtendedColorType;
use image::ImageEncoder;

pub struct Fixture {
    /// File name under `tests/golden/fixtures/`.
    pub file: String,
    /// `png`, `bmp`, `jpeg`, `webp`, `gif` or `other` — sets the decode row's class.
    pub kind: &'static str,
    pub bytes: Vec<u8>,
    /// How it was made, for people.
    pub how: String,
    /// An ADR 0002 row the port is expected to diverge under (`0002#1`), if any.
    pub adr: Option<&'static str>,
}

// ---------------------------------------------------------------- patterns

struct Xs(u64);
impl Xs {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn byte(&mut self) -> u8 {
        (self.next() >> 32) as u8
    }
}

/// `n` seeded noise bytes.
fn noise(n: usize, seed: u64) -> Vec<u8> {
    let mut r = Xs(seed | 1);
    (0..n).map(|_| r.byte()).collect()
}

/// A smooth gradient with one hard diagonal edge, `ch` channels: what a JPEG encoder sees in a
/// screenshot, rather than noise (which no lossy codec keeps close).
fn grad(w: u32, h: u32, ch: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h) as usize * ch);
    for y in 0..h {
        for x in 0..w {
            let edge = if x * h > y * w { 60u32 } else { 0 };
            let base = [
                (x * 255 / w.max(1)) as u8,
                (y * 255 / h.max(1)) as u8,
                ((x + y) * 127 / (w + h).max(1) + edge) as u8,
                (200 + (x % 7) * 8) as u8,
            ];
            v.extend_from_slice(&base[..ch]);
        }
    }
    v
}

/// Sub-byte samples (each < 2^depth), packed MSB-first, rows byte-aligned.
fn pack(samples: &[u8], w: usize, h: usize, depth: usize) -> Vec<u8> {
    let row_bytes = (w * depth).div_ceil(8);
    let mut out = vec![0u8; row_bytes * h];
    for y in 0..h {
        for x in 0..w {
            let v = samples[y * w + x] as usize & ((1 << depth) - 1);
            let bit = x * depth;
            out[y * row_bytes + bit / 8] |= (v << (8 - depth - bit % 8)) as u8;
        }
    }
    out
}

// ---------------------------------------------------------------- PNG

fn png_enc(
    w: u32,
    h: u32,
    color: png::ColorType,
    depth: png::BitDepth,
    data: &[u8],
    palette: Option<Vec<u8>>,
    trns: Option<Vec<u8>>,
) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut e = png::Encoder::new(&mut buf, w, h);
        e.set_color(color);
        e.set_depth(depth);
        if let Some(p) = palette {
            e.set_palette(p);
        }
        if let Some(t) = trns {
            e.set_trns(t);
        }
        let mut wr = e.write_header().expect("png header");
        wr.write_image_data(data).expect("png data");
    }
    buf
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut c = flate2::Crc::new();
    c.update(bytes);
    c.sum()
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

fn ihdr(w: u32, h: u32, depth: u8, color: u8, interlace: u8) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(&w.to_be_bytes());
    d.extend_from_slice(&h.to_be_bytes());
    d.extend_from_slice(&[depth, color, 0, 0, interlace]);
    d
}

fn zlib(raw: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(6));
    z.write_all(raw).expect("zlib");
    z.finish().expect("zlib")
}

/// An Adam7-interlaced PNG: `samples` holds `ch` samples per pixel, each < 2^depth, unpacked.
fn png_adam7(w: usize, h: usize, depth: usize, color: u8, ch: usize, samples: &[u8], plte: Option<&[u8]>) -> Vec<u8> {
    const PASSES: [(usize, usize, usize, usize); 7] =
        [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)];
    let mut raw = Vec::new();
    for (x0, y0, dx, dy) in PASSES {
        let pw = if w > x0 { (w - x0).div_ceil(dx) } else { 0 };
        let ph = if h > y0 { (h - y0).div_ceil(dy) } else { 0 };
        if pw == 0 || ph == 0 {
            continue;
        }
        for py in 0..ph {
            let y = y0 + py * dy;
            let mut row = Vec::with_capacity(pw * ch);
            for px in 0..pw {
                let x = x0 + px * dx;
                row.extend_from_slice(&samples[(y * w + x) * ch..(y * w + x) * ch + ch]);
            }
            raw.push(0); // filter type None
            if depth == 8 {
                raw.extend_from_slice(&row);
            } else {
                raw.extend_from_slice(&pack(&row, pw * ch, 1, depth));
            }
        }
    }
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &ihdr(w as u32, h as u32, depth as u8, color, 1));
    if let Some(p) = plte {
        chunk(&mut out, b"PLTE", p);
    }
    chunk(&mut out, b"IDAT", &zlib(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// A PNG whose header claims `w`×`h` but whose IDAT holds a single row: what a decoder must refuse from the
/// header alone, before allocating.
fn png_header_only(w: u32, h: u32) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &ihdr(w, h, 8, 6, 0));
    chunk(&mut out, b"IDAT", &zlib(&[0u8; 5]));
    chunk(&mut out, b"IEND", &[]);
    out
}

// ---------------------------------------------------------------- BMP

struct Bmp<'a> {
    w: i32,
    /// Negative: top-down.
    h: i32,
    bpp: u16,
    compression: u32,
    /// BGRX entries.
    palette: &'a [[u8; 4]],
    /// RGB(A) masks for BI_BITFIELDS.
    masks: Option<[u32; 4]>,
    /// 40 (INFO) or 108 (V4).
    header: u32,
    /// Pixel data exactly as stored (rows already in file order and padded, or RLE).
    data: Vec<u8>,
}

fn bmp(b: Bmp<'_>) -> Vec<u8> {
    let mut dib = Vec::new();
    dib.extend_from_slice(&b.header.to_le_bytes());
    dib.extend_from_slice(&b.w.to_le_bytes());
    dib.extend_from_slice(&b.h.to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&b.bpp.to_le_bytes());
    dib.extend_from_slice(&b.compression.to_le_bytes());
    dib.extend_from_slice(&(b.data.len() as u32).to_le_bytes());
    dib.extend_from_slice(&2835u32.to_le_bytes());
    dib.extend_from_slice(&2835u32.to_le_bytes());
    dib.extend_from_slice(&(b.palette.len() as u32).to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    let mut after_info = Vec::new();
    if b.header >= 108 {
        let m = b.masks.unwrap_or([0; 4]);
        for v in m {
            dib.extend_from_slice(&v.to_le_bytes());
        }
        dib.extend_from_slice(&0x7352_4742u32.to_le_bytes()); // LCS_sRGB
        dib.extend_from_slice(&[0u8; 36 + 12]); // endpoints, gamma
    } else if let Some(m) = b.masks {
        // INFO header + BI_BITFIELDS: three masks follow the header.
        for v in &m[..3] {
            after_info.extend_from_slice(&v.to_le_bytes());
        }
    }
    let mut pal = Vec::new();
    for e in b.palette {
        pal.extend_from_slice(e);
    }
    let offset = 14 + dib.len() + after_info.len() + pal.len();
    let size = offset + b.data.len();
    let mut out = b"BM".to_vec();
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(offset as u32).to_le_bytes());
    out.extend_from_slice(&dib);
    out.extend_from_slice(&after_info);
    out.extend_from_slice(&pal);
    out.extend_from_slice(&b.data);
    out
}

/// Rows of `bytes_per_row` unpadded bytes, padded to 4 and stored bottom-up unless `top_down`.
fn bmp_rows(rows: Vec<Vec<u8>>, top_down: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let order: Vec<&Vec<u8>> = if top_down { rows.iter().collect() } else { rows.iter().rev().collect() };
    for r in order {
        out.extend_from_slice(r);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    out
}

fn palette(n: usize, seed: u64) -> Vec<[u8; 4]> {
    let b = noise(n * 3, seed);
    (0..n).map(|i| [b[i * 3], b[i * 3 + 1], b[i * 3 + 2], 0]).collect()
}

/// BI_RLE8 / BI_RLE4, encoded mode only: runs of one index, end-of-line after each row, end-of-bitmap.
/// Rows are given top to bottom and stored bottom-up.
fn rle(idx: &[u8], w: usize, h: usize, four: bool) -> Vec<u8> {
    let mut out = Vec::new();
    for y in (0..h).rev() {
        let row = &idx[y * w..y * w + w];
        let mut x = 0;
        while x < w {
            let v = row[x];
            let mut n = 1;
            while x + n < w && row[x + n] == v && n < 255 {
                n += 1;
            }
            out.push(n as u8);
            out.push(if four { (v << 4) | v } else { v });
            x += n;
        }
        out.extend_from_slice(&[0, 0]);
    }
    out.extend_from_slice(&[0, 1]);
    out
}

// ---------------------------------------------------------------- image / jpeg-encoder

fn image_enc(kind: &str, w: u32, h: u32, data: &[u8], color: ExtendedColorType, q: u8) -> Vec<u8> {
    let mut buf = Vec::new();
    match kind {
        "bmp" => image::codecs::bmp::BmpEncoder::new(&mut buf).write_image(data, w, h, color),
        "jpeg" => image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, q).write_image(data, w, h, color),
        "webp" => image::codecs::webp::WebPEncoder::new_lossless(&mut buf).write_image(data, w, h, color),
        _ => unreachable!(),
    }
    .expect("image encode");
    buf
}

fn jpeg(
    w: u16,
    h: u16,
    data: &[u8],
    color: jpeg_encoder::ColorType,
    q: u8,
    sampling: jpeg_encoder::SamplingFactor,
    progressive: bool,
    restart: u16,
) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut e = jpeg_encoder::Encoder::new(&mut buf, q);
    e.set_sampling_factor(sampling);
    e.set_progressive(progressive);
    if restart > 0 {
        e.set_restart_interval(restart);
    }
    e.encode(data, w, h, color).expect("jpeg-encoder");
    buf
}

// ---------------------------------------------------------------- the corpus

pub fn build() -> Vec<Fixture> {
    use png::{BitDepth as D, ColorType as C};
    let mut v: Vec<Fixture> = Vec::new();
    let mut add = |file: &str, kind: &'static str, bytes: Vec<u8>, how: &str, adr: Option<&'static str>| {
        v.push(Fixture { file: file.to_string(), kind, bytes, how: how.to_string(), adr });
    };
    let (w, h) = (33u32, 17u32);
    let n = (w * h) as usize;

    // PNG through the png crate.
    add("png-rgba8-1x1.png", "png", png_enc(1, 1, C::Rgba, D::Eight, &[200, 10, 30, 128], None, None), "png 0.18.1 RGBA8", None);
    add("png-rgba8-7x3.png", "png", png_enc(7, 3, C::Rgba, D::Eight, &noise(84, 1), None, None), "png 0.18.1 RGBA8 noise", None);
    add("png-rgba8-33x17.png", "png", png_enc(w, h, C::Rgba, D::Eight, &noise(n * 4, 2), None, None), "png 0.18.1 RGBA8 noise", None);
    add("png-rgba8-64x64-grad.png", "png", png_enc(64, 64, C::Rgba, D::Eight, &grad(64, 64, 4), None, None), "png 0.18.1 RGBA8 gradient", None);
    add("png-rgb8-33x17.png", "png", png_enc(w, h, C::Rgb, D::Eight, &noise(n * 3, 3), None, None), "png 0.18.1 RGB8", None);
    add("png-gray8-33x17.png", "png", png_enc(w, h, C::Grayscale, D::Eight, &noise(n, 4), None, None), "png 0.18.1 gray 8", None);
    add("png-graya8-33x17.png", "png", png_enc(w, h, C::GrayscaleAlpha, D::Eight, &noise(n * 2, 5), None, None), "png 0.18.1 gray+alpha 8", None);
    for (depth, d) in [(1usize, D::One), (2, D::Two), (4, D::Four)] {
        let s = noise(n, 10 + depth as u64);
        add(&format!("png-gray{depth}-33x17.png"), "png", png_enc(w, h, C::Grayscale, d, &pack(&s, 33, 17, depth), None, None), &format!("png 0.18.1 gray {depth}-bit"), None);
        let pal: Vec<u8> = palette(1 << depth, 20 + depth as u64).iter().flat_map(|e| [e[2], e[1], e[0]]).collect();
        add(&format!("png-pal{depth}-33x17.png"), "png", png_enc(w, h, C::Indexed, d, &pack(&s, 33, 17, depth), Some(pal), None), &format!("png 0.18.1 palette {depth}-bit"), None);
    }
    let pal8: Vec<u8> = palette(256, 28).iter().flat_map(|e| [e[2], e[1], e[0]]).collect();
    let idx8 = noise(n, 29);
    add("png-pal8-33x17.png", "png", png_enc(w, h, C::Indexed, D::Eight, &idx8, Some(pal8.clone()), None), "png 0.18.1 palette 8-bit", None);
    let trns: Vec<u8> = (0..256u32).map(|i| ((i * 37) % 256) as u8).collect();
    add("png-pal8-trns-33x17.png", "png", png_enc(w, h, C::Indexed, D::Eight, &idx8, Some(pal8), Some(trns)), "png 0.18.1 palette 8-bit + per-entry tRNS", None);
    let rgb = noise(n * 3, 30);
    let key = [rgb[0], rgb[1], rgb[2]];
    add("png-rgb8-trns-33x17.png", "png", png_enc(w, h, C::Rgb, D::Eight, &rgb, None, Some(vec![0, key[0], 0, key[1], 0, key[2]])), "png 0.18.1 RGB8 + tRNS key (first pixel's colour)", None);
    let g = noise(n, 31);
    add("png-gray8-trns-33x17.png", "png", png_enc(w, h, C::Grayscale, D::Eight, &g, None, Some(vec![0, g[0]])), "png 0.18.1 gray 8 + tRNS key", None);
    for (name, c, ch) in [("rgb16", C::Rgb, 3usize), ("rgba16", C::Rgba, 4), ("gray16", C::Grayscale, 1), ("graya16", C::GrayscaleAlpha, 2)] {
        add(&format!("png-{name}-33x17.png"), "png", png_enc(w, h, c, D::Sixteen, &noise(n * ch * 2, 40 + ch as u64), None, None), &format!("png 0.18.1 {name}"), None);
    }
    // Adam7, hand-written (png 0.18 writes no interlace).
    add("png-adam7-rgba8-33x17.png", "png", png_adam7(33, 17, 8, 6, 4, &noise(n * 4, 50), None), "hand-written Adam7, RGBA8, filter 0", None);
    let pal4: Vec<u8> = palette(16, 51).iter().flat_map(|e| [e[2], e[1], e[0]]).collect();
    add("png-adam7-pal4-33x17.png", "png", png_adam7(33, 17, 4, 3, 1, &noise(n, 52).iter().map(|b| b & 15).collect::<Vec<_>>(), Some(&pal4)), "hand-written Adam7, palette 4-bit", None);
    add("png-adam7-gray1-5x5.png", "png", png_adam7(5, 5, 1, 0, 1, &noise(25, 53).iter().map(|b| b & 1).collect::<Vec<_>>(), None), "hand-written Adam7, gray 1-bit, 5x5 (passes 2, 4, 6 partly empty)", None);
    // Malformed PNG.
    let good = png_enc(w, h, C::Rgba, D::Eight, &noise(n * 4, 2), None, None);
    add("png-truncated.png", "png", good[..good.len() / 2].to_vec(), "png-rgba8-33x17.png cut in half (mid-IDAT)", None);
    let mut badcrc = good.clone();
    let l = badcrc.len();
    badcrc[l - 13] ^= 0xFF; // the IDAT CRC's last byte (IEND is the last 12 bytes)
    add("png-idat-badcrc.png", "png", badcrc, "png-rgba8-33x17.png with the IDAT CRC corrupted", None);
    add("png-too-wide-40000x1.png", "png", png_header_only(40000, 1), "IHDR 40000x1 (over MAX_IMAGE_DIMENSION 32768), one-row IDAT", None);
    add("png-too-big-20000x20000.png", "png", png_header_only(20000, 20000), "IHDR 20000x20000 RGBA8 (1.6 GB, over MAX_DECODE_ALLOC 512 MiB), tiny IDAT", None);

    // BMP through image (what selah 0.30.0 writes).
    add("bmp-image-rgb8-33x17.bmp", "bmp", image_enc("bmp", w, h, &noise(n * 3, 60), ExtendedColorType::Rgb8, 0), "image 0.25.10 BmpEncoder, RGB8", None);
    add("bmp-image-rgba8-33x17.bmp", "bmp", image_enc("bmp", w, h, &noise(n * 4, 61), ExtendedColorType::Rgba8, 0), "image 0.25.10 BmpEncoder, RGBA8", None);
    add("bmp-image-l8-33x17.bmp", "bmp", image_enc("bmp", w, h, &noise(n, 62), ExtendedColorType::L8, 0), "image 0.25.10 BmpEncoder, L8", None);
    // BMP, hand-written.
    let px = noise(n * 3, 63);
    let rows24 = |top: bool| bmp_rows((0..17).map(|y| px[y * 99..y * 99 + 99].to_vec()).collect(), top);
    add("bmp-24-bottomup-33x17.bmp", "bmp", bmp(Bmp { w: 33, h: 17, bpp: 24, compression: 0, palette: &[], masks: None, header: 40, data: rows24(false) }), "hand-written BI_RGB 24-bit, bottom-up", None);
    add("bmp-24-topdown-33x17.bmp", "bmp", bmp(Bmp { w: 33, h: -17, bpp: 24, compression: 0, palette: &[], masks: None, header: 40, data: rows24(true) }), "hand-written BI_RGB 24-bit, top-down", None);
    for depth in [1usize, 4, 8] {
        let p = palette(1 << depth, 70 + depth as u64);
        let idx: Vec<u8> = noise(n, 75 + depth as u64).iter().map(|b| b & ((1u16 << depth) - 1) as u8).collect();
        let rows: Vec<Vec<u8>> = (0..17).map(|y| pack(&idx[y * 33..y * 33 + 33], 33, 1, depth)).collect();
        add(&format!("bmp-pal{depth}-33x17.bmp"), "bmp", bmp(Bmp { w: 33, h: 17, bpp: depth as u16, compression: 0, palette: &p, masks: None, header: 40, data: bmp_rows(rows, false) }), &format!("hand-written BI_RGB {depth}-bit palette"), None);
    }
    let idx_runs: Vec<u8> = (0..n).map(|i| ((i / 5) % 7) as u8).collect();
    add("bmp-rle8-33x17.bmp", "bmp", bmp(Bmp { w: 33, h: 17, bpp: 8, compression: 1, palette: &palette(256, 80), masks: None, header: 40, data: rle(&idx_runs, 33, 17, false) }), "hand-written BI_RLE8, encoded runs", None);
    add("bmp-rle4-33x17.bmp", "bmp", bmp(Bmp { w: 33, h: 17, bpp: 4, compression: 2, palette: &palette(16, 81), masks: None, header: 40, data: rle(&idx_runs, 33, 17, true) }), "hand-written BI_RLE4, encoded runs", None);
    let w16 = noise(n * 2, 82);
    let rows16: Vec<Vec<u8>> = (0..17).map(|y| w16[y * 66..y * 66 + 66].to_vec()).collect();
    add("bmp-16-555-33x17.bmp", "bmp", bmp(Bmp { w: 33, h: 17, bpp: 16, compression: 0, palette: &[], masks: None, header: 40, data: bmp_rows(rows16.clone(), false) }), "hand-written BI_RGB 16-bit (5-5-5)", None);
    add("bmp-16-565-33x17.bmp", "bmp", bmp(Bmp { w: 33, h: 17, bpp: 16, compression: 3, palette: &[], masks: Some([0xF800, 0x07E0, 0x001F, 0]), header: 40, data: bmp_rows(rows16, false) }), "hand-written BI_BITFIELDS 16-bit 5-6-5, INFO header + 3 masks", None);
    let w32 = noise(n * 4, 83);
    let rows32: Vec<Vec<u8>> = (0..17).map(|y| w32[y * 132..y * 132 + 132].to_vec()).collect();
    add("bmp-32-rgb-33x17.bmp", "bmp", bmp(Bmp { w: 33, h: 17, bpp: 32, compression: 0, palette: &[], masks: None, header: 40, data: bmp_rows(rows32.clone(), false) }), "hand-written BI_RGB 32-bit (the fourth byte is not alpha by spec; noise here)", None);
    add("bmp-32-v4-alpha-33x17.bmp", "bmp", bmp(Bmp { w: 33, h: 17, bpp: 32, compression: 3, palette: &[], masks: Some([0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000]), header: 108, data: bmp_rows(rows32, false) }), "hand-written V4 BI_BITFIELDS 32-bit with an alpha mask", None);
    let mut bad_dib = bmp(Bmp { w: 33, h: 17, bpp: 24, compression: 0, palette: &[], masks: None, header: 40, data: rows24(false) });
    bad_dib[14..18].copy_from_slice(&77u32.to_le_bytes());
    add("bmp-bad-dib-size.bmp", "bmp", bad_dib, "bmp-24-bottomup with DIB header size 77 (no such header)", None);

    // JPEG through image (what selah 0.30.0 itself writes).
    add("jpeg-image-q75-rgb-33x17-grad.jpg", "jpeg", image_enc("jpeg", w, h, &grad(w, h, 3), ExtendedColorType::Rgb8, 75), "image 0.25.10 JpegEncoder q75, RGB8 gradient", None);
    add("jpeg-image-q95-rgb-64x64-grad.jpg", "jpeg", image_enc("jpeg", 64, 64, &grad(64, 64, 3), ExtendedColorType::Rgb8, 95), "image 0.25.10 JpegEncoder q95, RGB8 gradient", None);
    add("jpeg-image-q50-gray-33x17-grad.jpg", "jpeg", image_enc("jpeg", w, h, &grad(w, h, 1), ExtendedColorType::L8, 50), "image 0.25.10 JpegEncoder q50, L8 gradient", None);
    add("jpeg-image-q75-rgb-33x17-noise.jpg", "jpeg", image_enc("jpeg", w, h, &noise(n * 3, 90), ExtendedColorType::Rgb8, 75), "image 0.25.10 JpegEncoder q75, RGB8 noise (worst case for any IDCT difference)", None);
    // JPEG through jpeg-encoder.
    use jpeg_encoder::{ColorType as J, SamplingFactor as S};
    let g64 = grad(64, 64, 3);
    for (name, s) in [("420", S::F_2_2), ("422", S::F_2_1), ("444", S::F_1_1), ("440", S::F_1_2)] {
        add(&format!("jpeg-enc-{name}-64x64.jpg"), "jpeg", jpeg(64, 64, &g64, J::Rgb, 85, s, false, 0), &format!("jpeg-encoder 0.6.1 q85, {name} subsampling, baseline"), None);
    }
    add("jpeg-enc-420-33x17.jpg", "jpeg", jpeg(33, 17, &grad(w, h, 3), J::Rgb, 85, S::F_2_2, false, 0), "jpeg-encoder 0.6.1 q85, 4:2:0, odd size (partial MCUs)", None);
    add("jpeg-enc-420-1x1.jpg", "jpeg", jpeg(1, 1, &[10, 200, 90], J::Rgb, 85, S::F_2_2, false, 0), "jpeg-encoder 0.6.1 q85, 4:2:0, 1x1", None);
    add("jpeg-enc-restart-64x64.jpg", "jpeg", jpeg(64, 64, &g64, J::Rgb, 85, S::F_2_2, false, 2), "jpeg-encoder 0.6.1 q85, 4:2:0, restart interval 2 MCUs", None);
    add("jpeg-enc-gray-64x64.jpg", "jpeg", jpeg(64, 64, &grad(64, 64, 1), J::Luma, 85, S::F_1_1, false, 0), "jpeg-encoder 0.6.1 q85, grayscale", None);
    add("jpeg-enc-progressive-64x64.jpg", "jpeg", jpeg(64, 64, &g64, J::Rgb, 85, S::F_2_2, true, 0), "jpeg-encoder 0.6.1 q85, 4:2:0, progressive (SOF2)", Some("0002#2"));
    add("jpeg-enc-cmyk-33x17.jpg", "jpeg", jpeg(33, 17, &grad(w, h, 4), J::Cmyk, 85, S::F_1_1, false, 0), "jpeg-encoder 0.6.1 q85, CMYK (Adobe APP14)", None);
    let good_jpeg = jpeg(64, 64, &g64, J::Rgb, 85, S::F_2_2, false, 0);
    add("jpeg-truncated.jpg", "jpeg", good_jpeg[..good_jpeg.len() / 2].to_vec(), "jpeg-enc-420-64x64 cut in half (mid-scan)", None);

    // WebP and GIF.
    add("webp-lossless-rgba8-7x3.webp", "webp", image_enc("webp", 7, 3, &noise(84, 95), ExtendedColorType::Rgba8, 0), "image 0.25.10 WebPEncoder lossless, RGBA8", Some("0002#1"));
    add("gif-1x1.gif", "gif", b"GIF89a\x01\x00\x01\x00\x80\x00\x00\xff\xff\xff\x00\x00\x00!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;".to_vec(), "a minimal valid GIF89a, 1x1 (selah refuses GIF on both sides)", None);

    // Not images.
    add("other-empty.bin", "other", Vec::new(), "zero bytes", None);
    add("other-noise-64.bin", "other", noise(64, 99), "64 noise bytes", None);
    add("other-text.txt", "other", b"not an image\n".to_vec(), "plain text", None);
    v
}

pub fn emit(out: &mut Out, fixtures: &[Fixture]) {
    let mut index = String::from("# fixture<TAB>kind<TAB>bytes<TAB>how\n");
    for f in fixtures {
        out.insert(format!("fixtures/{}", f.file), f.bytes.clone());
        index.push_str(&format!("{}\t{}\t{}\t{}\n", f.file, f.kind, f.bytes.len(), f.how));
    }
    out.insert("fixtures/INDEX.tsv".to_string(), index.into_bytes());
}
