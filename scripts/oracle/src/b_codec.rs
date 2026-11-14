//! `--bench`: Rust selah 0.30.0's codec paths, timed on the image `tests/selah.bcyr` builds, so
//! `docs/benchmarks-rust-v-cyrius.md` can set M3's codec benches (which have no criterion counterpart) beside Rust.
//!
//! The image is 512 x 512 RGBA, `(x ^ y, 3x + y, 5y ^ x, 255) mod 256`. The decode benches read the bytes the Cyrius
//! bench wrote (`build/bench/image512.png` / `.jpg`), so both sides decode the same files. The paths are selah's:
//! decode is `image`'s guessed-format decode to RGBA8 (selah's `decode_image` plus the RGBA conversion every caller
//! makes); encode is `DynamicImage::write_to` as selah's `encode_image` calls it (RGB8 for JPEG); convert is
//! `selah::convert_format`.

use std::hint::black_box;
use std::io::Cursor;
use std::time::Instant;

fn image512() -> image::RgbaImage {
    image::RgbaImage::from_fn(512, 512, |x, y| {
        image::Rgba([((x ^ y) & 255) as u8, ((x * 3 + y) & 255) as u8, (((y * 5) ^ x) & 255) as u8, 255])
    })
}

/// Runs `f` n times after two warm-ups and prints the mean, in the Cyrius bench's format.
fn time<F: FnMut() -> usize>(name: &str, n: u32, mut f: F) {
    black_box(f());
    black_box(f());
    let t = Instant::now();
    for _ in 0..n {
        black_box(f());
    }
    let per = t.elapsed().as_secs_f64() / f64::from(n);
    println!("  {name}: {:.3}ms avg [{n} iters] (Rust selah 0.30.0)", per * 1e3);
}

fn encode(img: &image::DynamicImage, fmt: image::ImageFormat) -> usize {
    let mut out = Cursor::new(Vec::new());
    match fmt {
        image::ImageFormat::Jpeg => img.to_rgb8().write_to(&mut out, fmt),
        _ => img.to_rgba8().write_to(&mut out, fmt),
    }
    .expect("encode");
    out.into_inner().len()
}

fn decode(bytes: &[u8]) -> usize {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .expect("guess")
        .decode()
        .expect("decode")
        .to_rgba8()
        .len()
}

pub fn run(root: &std::path::Path) {
    let img = image::DynamicImage::ImageRgba8(image512());
    time("encode_png_512", 20, || encode(&img, image::ImageFormat::Png));
    time("encode_bmp_512", 200, || encode(&img, image::ImageFormat::Bmp));
    time("encode_jpeg_512", 20, || encode(&img, image::ImageFormat::Jpeg));
    let read = |name: &str| std::fs::read(root.join("build/bench").join(name)).ok();
    match (read("image512.png"), read("image512.jpg")) {
        (Some(png), Some(jpg)) => {
            time("decode_png_512", 20, || decode(&png));
            time("decode_jpeg_512", 20, || decode(&jpg));
            time("convert_format_png_to_jpeg_512", 10, || {
                selah::convert_format(&png, selah::ImageFormat::Jpeg).expect("convert").len()
            });
        }
        _ => eprintln!("oracle --bench: build/bench/image512.{{png,jpg}} missing: run `cyrius bench` first"),
    }
}
