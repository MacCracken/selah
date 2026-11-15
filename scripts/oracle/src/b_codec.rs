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
    m5(root);
    m6();
}

/// M6: the MCP server's calls, through Rust's stdio server (oracle --mcp-serve), so each figure includes a pipe round
/// trip: selah's build_dispatcher is private, and only the server can be timed. tests/mcp/mcp.bcyr times the same
/// requests through bote's stdio loop (`*_stdio`). `mcp_stdio_floor` is a method neither server has, answered -32601:
/// the round trip with almost no work. Inputs from tests/golden/mcp/.
fn m6() {
    let fx = crate::m_mcp::fixtures();
    let dir = crate::m_mcp::session_dir(9999, &fx);
    let mut s = crate::m_mcp::Server::start(&dir, "http://127.0.0.1:9");
    time("mcp_stdio_floor", 2000, || s.call(r#"{"jsonrpc":"2.0","id":9,"method":"selah/none"}"#).len());
    time("mcp_tools_list_stdio", 2000, || s.call(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#).len());
    let shot = dir.join("shot.png").to_string_lossy().into_owned();
    let out = dir.join("out.png").to_string_lossy().into_owned();
    let req = serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "selah_annotate",
        "arguments": { "image_path": shot, "output": out, "annotations": [
            { "kind": "redaction", "position": { "x": 0, "y": 0, "width": 4, "height": 4 } },
            { "kind": "rectangle", "position": { "x": 1, "y": 1, "width": 5, "height": 5 } },
            { "kind": "text", "position": { "x": 0, "y": 0, "width": 8, "height": 8 }, "text": "hi" } ] } } })
    .to_string();
    time("mcp_annotate_8x8_stdio", 200, || s.call(&req).len());
    time("mcp_history_list_stdio", 2000, || {
        s.call(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"selah_history","arguments":{}}}"#).len()
    });
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}

/// M5's entry i, as tests/selah.bcyr builds it.
fn hentry(i: u64) -> selah::HistoryEntry {
    selah::HistoryEntry {
        id: uuid::Uuid::from_u128(u128::from(i)),
        path: format!("/home/u/Pictures/shot-{i}.png"),
        timestamp: chrono::DateTime::from_timestamp(1_763_164_800 + i as i64, ((i * 7919) % 1_000_000_000) as u32)
            .expect("in range"),
        source: "full screen".to_string(),
        width: 1920,
        height: 1080,
        format: "png".to_string(),
    }
}

/// A fresh store under build/bench/ holding entries 0..n, written as serde_json writes them.
fn hstore(root: &std::path::Path, name: &str, n: u64) -> selah::HistoryStore {
    let dir = root.join("build/bench");
    std::fs::create_dir_all(&dir).expect("bench dir");
    let path = dir.join(format!("rust_{name}.jsonl"));
    let mut all = String::new();
    for i in 0..n {
        all.push_str(&serde_json::to_string(&hentry(i)).expect("json"));
        all.push('\n');
    }
    std::fs::write(&path, all).expect("write");
    selah::HistoryStore::open(path)
}

/// M5: history and the capture answer's parse, on the inputs tests/selah.bcyr builds.
fn m5(root: &std::path::Path) {
    let rec = hstore(root, "history_record", 0);
    let mut i = 0u64;
    time("history_record", 2000, || {
        rec.record(hentry(i)).expect("record");
        i += 1;
        1
    });
    let s = hstore(root, "history_1000", 1000);
    time("history_list_1000", 200, || s.list(10, None).expect("list").len());
    let last = hentry(999).id;
    time("history_get_1000", 200, || usize::from(s.get(last).expect("get").is_some()));
    let miss = hentry(5000).id;
    time("history_delete_miss_1000", 200, || usize::from(s.delete(miss).expect("delete")));
    let alpha = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut body = String::from("{\"image_data\":\"");
    for k in 0..1_048_576usize {
        body.push(alpha[(k * 7) % 64] as char);
    }
    body.push_str("\",\"width\":1920,\"height\":1080,\"format\":\"png\"}");
    time("capture_parse_1mb", 50, || {
        serde_json::from_slice::<selah::CaptureResponse>(body.as_bytes()).expect("parse").width as usize
    });
}
