//! `mcp`: the MCP server, as a client sees it (port plan §3 M6). Rust 0.30.0's server is only reachable through
//! `selah::run_mcp_server` on stdio, so the oracle runs itself as that server (`oracle --mcp-serve <api_url>`), one
//! child per session, in a fresh temp directory `D` (its working directory, with `XDG_DATA_HOME` = `D/xdg`), and talks
//! JSON-RPC over the pipes.
//!
//! Fixtures (`tests/golden/mcp/`): the files every session's `D` starts with, written here so both sides read the same
//! bytes. Each session also gets `D/d.png` (a directory), `D/link.png` → `victim.txt` (a symlink) and the history file
//! `D/xdg/selah/history.jsonl` (copied from `mcp/history.jsonl`).
//!
//! Rows:
//! - `tool A s:<name>` → `s:<description> s:<inputSchema>`: from tools/list, sorted by name (bote 0.92 lists them in
//!   HashMap order, which changes between runs), the schema re-serialized with sorted keys (its properties are a
//!   HashMap too). Rust's bote 0.92 sends no annotations, so they have no row (ADR 0002 #24).
//! - `session A <requests> <daimon>` → one `s:` per response, then a `c:` per checked file. `<requests>` is one `s:`
//!   holding a JSON array of JSON-RPC requests, `$DIR` standing for `D`; `<daimon>` is `N`, or `i:<status> h:<body>`
//!   for the one capture a session may make. A response is re-serialized with sorted keys, `D` written `$DIR`, and,
//!   after a capture, its time and any entry id written `$NOW` and `$ID`. A checked file (`f:` in the request list's
//!   second element) is `c:<crc32 of its RGBA>` (decoded by `image` from its content) or `N` when absent.
//!   The note says how to compare: empty is exact; `verdict` compares only refused or not (Rust's bote refused the
//!   arguments against the schema with JSON-RPC −32602, where bote 3.8.15 answers a tool error, per MCP 2025-11-25
//!   SEP-1303; or a transport text differs, ADR 0002 #22); `prefix` compares the one response's message up to the
//!   end of the given text (the rest is serde's, ADR 0002 #14); `adr=0002#N` passes when the port refuses.

use crate::{Out, Row, emit_tsv, th, ti, ts};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// The fixture files, by name.
pub(crate) fn fixtures() -> Vec<(&'static str, Vec<u8>)> {
    let png = |w: u32, h: u32, f: &dyn Fn(u32, u32) -> [u8; 4]| {
        let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba(f(x, y)));
        let mut c = std::io::Cursor::new(Vec::new());
        img.write_to(&mut c, image::ImageFormat::Png).expect("png");
        c.into_inner()
    };
    let white = |_: u32, _: u32| [255, 255, 255, 255];
    let in_png = png(4, 4, &white);
    let mut leaky = in_png.clone();
    leaky.extend_from_slice(b"\x00contact admin@example.com\x00");
    let mut pii = in_png.clone();
    pii.extend_from_slice(
        b"\x00mail bob.smith@corp.example.org call 555-123-4567 card 4111111111111111 ip 192.168.1.100 x@y.co\x00",
    );
    let grad = |x: u32, y: u32| [(x * 40) as u8, (y * 60) as u8, 100, 255];
    let rgb = image::RgbImage::from_fn(6, 4, |x, y| image::Rgb([(x * 40) as u8, (y * 60) as u8, 100]));
    let mut jpg = std::io::Cursor::new(Vec::new());
    rgb.write_to(&mut jpg, image::ImageFormat::Jpeg).expect("jpeg");
    let bmp_img = image::RgbaImage::from_fn(5, 3, |x, y| image::Rgba(grad(x, y)));
    let mut bmp = std::io::Cursor::new(Vec::new());
    bmp_img.write_to(&mut bmp, image::ImageFormat::Bmp).expect("bmp");
    let mut webp = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_fn(4, 4, |x, y| image::Rgba(grad(x, y)))
        .write_to(&mut webp, image::ImageFormat::WebP)
        .expect("webp");
    vec![
        ("in.png", in_png.clone()),
        ("shot.png", png(8, 8, &white)),
        ("grad.png", png(7, 5, &grad)),
        ("leaky.png", leaky),
        ("pii.png", pii),
        ("photo.jpg", jpg.into_inner()),
        ("pic.bmp", bmp.into_inner()),
        ("art.webp", webp.into_inner()),
        ("noext", in_png),
        ("prev.png", png(1, 1, &white)),
        ("id_ed25519", b"-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXk=\n".to_vec()),
        (".bashrc", b"export SAFE=1\n".to_vec()),
        ("victim.txt", b"keep me".to_vec()),
        ("empty.png", Vec::new()),
    ]
}

/// The history every session starts with (serde_json lines, and one that is not).
fn history_file() -> Vec<u8> {
    let mut s = String::new();
    for (i, (secs, nanos, src)) in [
        (1_763_164_800i64, 0u32, "full screen"),
        (1_763_164_860, 500_000_000, "region (10x20 at 1,2)"),
        (1_763_168_400, 123_456_789, "window 7"),
        (1_700_000_000, 0, "full screen"),
    ]
    .iter()
    .enumerate()
    {
        let e = selah::HistoryEntry {
            id: uuid::Uuid::from_u128(0x1111_0000 + i as u128),
            path: format!("/home/u/Pictures/shot-{i}.png"),
            timestamp: chrono::DateTime::from_timestamp(*secs, *nanos).expect("time"),
            source: (*src).to_string(),
            width: 1920 + i as u32,
            height: 1080,
            format: "png".to_string(),
        };
        s.push_str(&serde_json::to_string(&e).expect("json"));
        s.push('\n');
    }
    s.push_str("{not json}\n");
    s.into_bytes()
}

/// A fresh session directory with the fixtures. Returns it, canonicalized.
pub(crate) fn session_dir(n: usize, fx: &[(&str, Vec<u8>)]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("selah_oracle_mcp_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("xdg/selah")).expect("dir");
    for (name, bytes) in fx {
        std::fs::write(d.join(name), bytes).expect("fixture");
    }
    std::fs::create_dir(d.join("d.png")).expect("d.png");
    std::os::unix::fs::symlink("victim.txt", d.join("link.png")).expect("symlink");
    std::fs::write(d.join("xdg/selah/history.jsonl"), history_file()).expect("history");
    std::fs::canonicalize(&d).expect("canonical")
}

/// One Rust MCP server, a child of the oracle.
pub(crate) struct Server {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
}

impl Server {
    pub(crate) fn start(dir: &Path, api_url: &str) -> Self {
        let mut child = Command::new(std::env::current_exe().expect("exe"))
            .args(["--mcp-serve", api_url])
            .current_dir(dir)
            .env("XDG_DATA_HOME", dir.join("xdg"))
            .env("HOME", dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the Rust MCP server");
        let reader = BufReader::new(child.stdout.take().expect("stdout"));
        Server { child, reader }
    }

    pub(crate) fn call(&mut self, req: &str) -> String {
        let stdin = self.child.stdin.as_mut().expect("stdin");
        // One write, the newline with it: two would wake the server twice (and cost the bench a read).
        let mut out = Vec::with_capacity(req.len() + 1);
        out.extend_from_slice(req.as_bytes());
        out.push(b'\n');
        stdin.write_all(&out).expect("write");
        stdin.flush().expect("flush");
        let mut line = String::new();
        self.reader.read_line(&mut line).expect("read");
        line.trim_end().to_string()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let _ = self.child.wait();
    }
}

/// Runs the server for the oracle's own child process (`oracle --mcp-serve <url>`).
pub fn serve(api_url: &str) {
    if let Err(e) = selah::run_mcp_server(api_url) {
        eprintln!("mcp server: {e}");
        std::process::exit(1);
    }
}

/// Replaces the value after every `\"<key>\":\"` (inside the escaped payload text) with `mask`.
fn mask_values(s: &str, key: &str, mask: &str) -> String {
    let pat = format!("\\\"{key}\\\":\\\"");
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find(&pat) {
        out.push_str(&rest[..i + pat.len()]);
        rest = &rest[i + pat.len()..];
        let end = rest.find("\\\"").unwrap_or(rest.len());
        out.push_str(mask);
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// A response as a row token: re-serialized with sorted keys, `D` as `$DIR`, and after a capture the times and ids
/// masked.
fn canonical(line: &str, dir: &str, after_capture: bool) -> String {
    let v: Value = serde_json::from_str(line).expect("a JSON-RPC response");
    let mut s = v.to_string().replace(dir, "$DIR");
    if after_capture {
        s = mask_values(&s, "timestamp", "$NOW");
        s = mask_values(&s, "id", "$ID");
    }
    s
}

fn crc(bytes: &[u8]) -> String {
    let mut h = flate2::Crc::new();
    h.update(bytes);
    format!("c:{:08x}", h.sum())
}

/// A tools/call request.
fn call(id: u64, tool: &str, args: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": tool, "arguments": args } })
}

/// A session: the requests, the files to check after, the daimon's answer, the comparison note.
struct Session {
    requests: Vec<Value>,
    files: Vec<&'static str>,
    daimon: Option<(u16, Vec<u8>)>,
    note: &'static str,
}

fn one(tool: &str, args: Value, note: &'static str) -> Session {
    Session { requests: vec![call(1, tool, args)], files: vec![], daimon: None, note }
}

fn checked(tool: &str, args: Value, files: Vec<&'static str>) -> Session {
    Session { requests: vec![call(1, tool, args)], files, daimon: None, note: "" }
}

fn capture_answer(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x * 50) as u8, (y * 70) as u8, 9, 255]));
    let mut c = std::io::Cursor::new(Vec::new());
    img.write_to(&mut c, image::ImageFormat::Png).expect("png");
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, c.get_ref());
    json!({ "image_data": b64, "width": w, "height": h, "format": "png" }).to_string().into_bytes()
}

fn sessions() -> Vec<Session> {
    let d = |p: &str| format!("$DIR/{p}");
    let r = |x: f64, y: f64, w: f64, h: f64| json!({ "x": x, "y": y, "width": w, "height": h });
    let mut v = vec![
        // ── selah_annotate ──
        checked("selah_annotate", json!({ "image_path": d("shot.png"), "annotations": [
            { "kind": "redaction", "position": r(0.0, 0.0, 4.0, 4.0) },
            { "kind": "Rectangle", "position": r(1.0, 1.0, 5.0, 5.0), "color": { "r": 0, "g": 0, "b": 255, "a": 255 } },
            { "kind": "text", "position": r(0.0, 0.0, 8.0, 8.0), "text": "hi" } ] }), vec!["shot_annotated.png"]),
        checked("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [], "output": d("out.jpg") }),
            vec![]),
        checked("selah_annotate", json!({ "image_path": d("grad.png"), "annotations": [
            { "kind": "rect", "position": r(1.5, 0.5, 3.0, 2.0), "color": { "r": 10, "g": 200, "b": 30, "a": 128 } },
            { "kind": "ELLIPSE", "position": r(0.0, 0.0, 7.0, 5.0) },
            { "kind": "highlight", "position": r(2.0, 2.0, -2.0, -2.0), "color": { "r": 255, "g": 255, "b": 0, "a": 128 } },
            { "kind": "arrow", "position": r(0.0, 4.0, 6.0, -4.0), "id": "ignored" },
            { "kind": "free-form", "position": r(1.0, 1.0, 2.0, 2.0) },
            { "kind": "redact", "position": r(5.0, 3.0, 9.0, 9.0) } ] }), vec!["grad_annotated.png"]),
        checked("selah_annotate", json!({ "image_path": d("pic.bmp"), "annotations": [
            { "kind": "circle", "position": r(0.0, 0.0, 5.0, 3.0) } ] }), vec!["pic_annotated.bmp"]),
        checked("selah_annotate", json!({ "image_path": d("photo.jpg"), "annotations": [], "output": d("p.png") }),
            vec![]),
        checked("selah_annotate", json!({ "image_path": d("noext"), "annotations": [] }), vec!["noext_annotated.png"]),
        checked("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [], "output": d("prev.png") }),
            vec!["prev.png"]),
        checked("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [], "output": "rel/x.png" }),
            vec![]),
        one("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [], "output": d("art.webp") }),
            "adr=0002#1"),
        one("selah_annotate", json!({ "image_path": d("art.webp"), "annotations": [] }), "adr=0002#1"),
        one("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [
            { "kind": "blob", "position": r(0.0, 0.0, 1.0, 1.0) } ] }), ""),
        one("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [
            { "kind": "arrow", "position": { "x": 1e39, "y": 0, "width": 1, "height": 1 } } ] }),
            "prefix:invalid annotation 0"),
        one("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [
            { "kind": "arrow", "position": r(0.0, 0.0, 1.0, 1.0), "color": { "r": 1, "g": 2, "b": 3 } } ] }),
            "prefix:invalid annotation 0"),
        one("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [
            { "kind": "arrow", "position": r(0.0, 0.0, 1.0, 1.0) },
            { "kind": "arrow", "position": r(0.0, 0.0, 1.0, 1.0), "color": { "r": 300, "g": 2, "b": 3, "a": 4 } } ] }),
            "prefix:invalid annotation 1"),
        one("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [
            { "kind": "text", "position": r(0.0, 0.0, 1.0, 1.0), "text": null } ] }), ""),
        one("selah_annotate", json!({ "image_path": d("in.png"), "annotations": {} }), "verdict"),
        one("selah_annotate", json!({ "annotations": [] }), "verdict"),
        one("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [], "output": "../../evil.png" }), ""),
        one("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [], "output": d(".bashrc") }), ""),
        one("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [], "output": d("d.png") }), ""),
        one("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [], "output": d("link.png") }), ""),
        one("selah_annotate", json!({ "image_path": d("in.png"), "annotations": [], "output": "" }), ""),
        one("selah_annotate", json!({ "image_path": d("id_ed25519"), "annotations": [], "output": d("o.png") }), ""),
        one("selah_annotate", json!({ "image_path": d("missing.png"), "annotations": [] }), ""),
        one("selah_annotate", json!({ "image_path": "$DIR", "annotations": [], "output": d("o.png") }), ""),
        one("selah_annotate", json!({ "image_path": d("empty.png"), "annotations": [] }), ""),
        one("selah_annotate", json!({ "image_path": "a/../in.png", "annotations": [] }), ""),
        one("selah_annotate", json!({ "image_path": d("in\u{0}.png"), "annotations": [] }), ""),
        // ── selah_ocr ──
        one("selah_ocr", json!({ "image_path": d("in.png") }), ""),
        one("selah_ocr", json!({ "image_path": d("leaky.png"), "engine": "local" }), ""),
        one("selah_ocr", json!({ "image_path": d("pii.png") }), ""),
        one("selah_ocr", json!({ "image_path": d("photo.jpg") }), ""),
        one("selah_ocr", json!({ "image_path": d("in.png"), "engine": "magic" }), "verdict"),
        one("selah_ocr", json!({ "image_path": d("in.png"), "engine": "hoosh" }), "adr=0002#24"),
        one("selah_ocr", json!({ "image_path": "../etc/passwd" }), ""),
        one("selah_ocr", json!({ "image_path": "$DIR" }), ""),
        one("selah_ocr", json!({ "image_path": d("id_ed25519") }), ""),
        one("selah_ocr", json!({}), "verdict"),
        one("selah_ocr", json!({ "image_path": 5 }), "verdict"),
        // ── selah_redact ──
        checked("selah_redact", json!({ "image_path": d("leaky.png") }), vec!["leaky_redacted.png"]),
        one("selah_redact", json!({ "image_path": d("pii.png") }), ""),
        one("selah_redact", json!({ "image_path": d("pii.png"), "targets": ["email"] }), ""),
        one("selah_redact", json!({ "image_path": d("pii.png"), "targets": ["phone", "ip_address"] }), ""),
        one("selah_redact", json!({ "image_path": d("pii.png"), "targets": [] }), ""),
        one("selah_redact", json!({ "image_path": d("pii.png"), "targets": null }), "verdict"),
        one("selah_redact", json!({ "image_path": d("pii.png"), "targets": ["ssn"] }), "verdict"),
        one("selah_redact", json!({ "image_path": d("pii.png"), "targets": "email" }), "verdict"),
        checked("selah_redact", json!({ "image_path": d("in.png"), "output": d("prev.png") }), vec!["prev.png"]),
        one("selah_redact", json!({ "image_path": d("in.png"), "output": d("d.png") }), ""),
        one("selah_redact", json!({ "image_path": d("in.png"), "output": "a/../../b.png" }), ""),
        checked("selah_redact", json!({ "image_path": d("pic.bmp") }), vec!["pic_redacted.bmp"]),
        one("selah_redact", json!({ "image_path": d("photo.jpg"), "output": d("r.jpeg") }), ""),
        one("selah_redact", json!({ "image_path": d("id_ed25519"), "output": d("o.png") }), ""),
        one("selah_redact", json!({}), "verdict"),
        // ── selah_history ──
        one("selah_history", json!({}), ""),
        one("selah_history", json!({ "limit": 2 }), ""),
        one("selah_history", json!({ "limit": 0 }), ""),
        one("selah_history", json!({ "limit": 1000 }), ""),
        one("selah_history", json!({ "limit": 1001 }), "verdict"),
        one("selah_history", json!({ "limit": -1 }), "verdict"),
        one("selah_history", json!({ "limit": 2.5 }), "verdict"),
        one("selah_history", json!({ "since": "2025-11-15T00:00:30Z" }), ""),
        one("selah_history", json!({ "since": "2025-11-15T00:01:00.5Z" }), ""),
        one("selah_history", json!({ "since": "2025-11-15T00:01:00.500000001Z" }), ""),
        one("selah_history", json!({ "since": "2025-11-15T02:01:00+02:00" }), ""),
        one("selah_history", json!({ "since": "2025-11-15 00:00:00z" }), ""),
        one("selah_history", json!({ "since": "2000-01-01T00:00:00Z", "limit": 3 }), ""),
    ];
    for since in [
        "yesterday",
        "2025-11-15",
        "2025-11-15T00:00:00",
        "2025-11-15T00:00:00Zx",
        "2025-13-01T00:00:00Z",
        "2025-02-29T00:00:00Z",
        "2025-11-15T24:00:00Z",
        "2025-11-15T23:60:00Z",
        "2025-11-15T00:00:60Z",
        "2016-12-31T23:59:60Z",
        "2025-11-15X00:00:00Z",
        "2025-11-15T00:00:00.Z",
        "2025-11-15T00:00:00.1234567891Z",
        "2025-11-15T00:00:00+0200",
        "2025-11-15T00:00:00+02:6",
        "2025-11-15T00:00:00+02:60",
        "2025-11-15T00:00:00+99:00",
        "2025-11-15T00:00:00\u{2212}02:00",
        "2025-11-15T00:00:00+02",
        "+2025-11-15T00:00:00Z",
        "2025-11-15t00:00:00+00:00",
        "0000-01-01T00:00:00Z",
    ] {
        v.push(one("selah_history", json!({ "since": since }), ""));
    }
    // ── selah_capture (against a one-shot daimon) ──
    let ok = Some((200u16, capture_answer(3, 2)));
    let mk = |args: Value, daimon: Option<(u16, Vec<u8>)>, note: &'static str, files: Vec<&'static str>| Session {
        requests: vec![call(1, "selah_capture", args), call(2, "selah_history", json!({ "limit": 1 }))],
        files,
        daimon,
        note,
    };
    v.push(mk(json!({}), ok.clone(), "", vec!["screenshot.png"]));
    v.push(mk(json!({ "output": d("cap.png"), "region": r(1.0, 2.0, 3.0, 4.0) }), ok.clone(), "", vec!["cap.png"]));
    v.push(mk(json!({ "output": d("cap.bmp"), "region": r(0.5, 0.5, -10.0, 7.25) }), ok.clone(), "", vec!["cap.bmp"]));
    v.push(mk(json!({ "format": "JPG" }), ok.clone(), "", vec![]));
    v.push(mk(json!({ "output": d("a.png"), "format": "jpg" }), ok.clone(), "", vec![]));
    v.push(mk(json!({ "format": "tiff" }), ok.clone(), "verdict", vec![]));
    v.push(mk(json!({ "output": "../x.png" }), ok.clone(), "", vec![]));
    v.push(mk(json!({ "output": "notes.txt" }), ok.clone(), "", vec![]));
    v.push(mk(json!({ "output": d("d.png") }), ok.clone(), "", vec![]));
    v.push(mk(json!({ "region": r(0.0, 0.0, 0.0, 10.0) }), ok.clone(), "", vec![]));
    v.push(mk(json!({ "region": { "x": 0 } }), ok.clone(), "verdict", vec![]));
    v.push(mk(json!({ "region": { "x": 1e39, "y": 0, "width": 1, "height": 1 } }), ok.clone(),
        "prefix:invalid region", vec![]));
    v.push(mk(json!({ "format": "webp" }), ok.clone(), "adr=0002#1", vec![]));
    v.push(mk(json!({}), Some((403, br#"{"error":"permission denied"}"#.to_vec())), "", vec![]));
    v.push(mk(json!({}), Some((200, b"not json".to_vec())), "prefix:API error: daimon capture: invalid response",
        vec![]));
    v.push(mk(json!({}), Some((200, br#"{"image_data":"@@@","width":1,"height":1,"format":"png"}"#.to_vec())),
        "prefix:API error: failed to decode image data", vec![]));
    v
}

pub fn emit(out: &mut Out) {
    let fx = fixtures();
    for (name, bytes) in &fx {
        out.insert(format!("mcp/{name}"), bytes.clone());
    }
    out.insert("mcp/history.jsonl".to_string(), history_file());
    let mut rows = Vec::new();

    // tools/list, from a server with nothing to do.
    {
        let dir = session_dir(0, &fx);
        let mut s = Server::start(&dir, "http://127.0.0.1:9");
        let list: Value = serde_json::from_str(&s.call(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#))
            .expect("tools/list");
        // bote 0.92 keeps its registry in a HashMap, so the order of tools/list changes from run to run: the
        // contract is the set, and the rows go out sorted by name.
        let mut tools = list["result"]["tools"].as_array().expect("tools").clone();
        tools.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        for t in &tools {
            rows.push(Row {
                func: "tool".into(),
                class: "A",
                inputs: vec![ts(t["name"].as_str().expect("name"))],
                outputs: vec![ts(t["description"].as_str().expect("description")), ts(&t["inputSchema"].to_string())],
                note: String::new(),
            });
        }
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }

    for (n, sess) in sessions().into_iter().enumerate() {
        let dir = session_dir(n + 1, &fx);
        let dir_s = dir.to_string_lossy().into_owned();
        let (api, rx) = match &sess.daimon {
            Some((status, body)) => {
                let (u, rx) = crate::m_system::serve_once(*status, body.clone());
                (u, Some(rx))
            }
            None => ("http://127.0.0.1:9".to_string(), None),
        };
        let mut server = Server::start(&dir, &api);
        let mut outs = Vec::new();
        let mut captured = false;
        for req in &sess.requests {
            let text = req.to_string().replace("$DIR", &dir_s);
            let resp = server.call(&text);
            if req["params"]["name"] == "selah_capture" {
                captured = true;
            }
            outs.push(ts(&canonical(&resp, &dir_s, captured)));
        }
        drop(server);
        drop(rx);
        for f in &sess.files {
            // By content, not by extension: a capture saves daimon's bytes under whatever name it was given.
            let decoded = std::fs::read(dir.join(f)).ok().and_then(|b| image::load_from_memory(&b).ok());
            outs.push(match decoded {
                Some(img) => crc(img.to_rgba8().as_raw()),
                None => "N".to_string(),
            });
        }
        let reqs = Value::Array(sess.requests.clone()).to_string();
        let files = Value::Array(sess.files.iter().map(|f| json!(f)).collect()).to_string();
        let daimon = match &sess.daimon {
            Some((status, body)) => vec![ti(i64::from(*status)), th(body)],
            None => vec!["N".to_string()],
        };
        let mut inputs = vec![ts(&reqs), ts(&files)];
        inputs.extend(daimon);
        // Rust's bote refused the arguments against the schema (JSON-RPC -32602): bote 3.8.15 answers that as a tool
        // error (SEP-1303) with its own text, so only the verdict compares.
        let schema_refusal = outs.iter().any(|o| o.contains("\"code\":-32602"));
        let note = if schema_refusal && sess.note.is_empty() { "verdict" } else { sess.note };
        rows.push(Row { func: "session".into(), class: "A", inputs, outputs: outs, note: note.to_string() });
        let _ = std::fs::remove_dir_all(&dir);
    }
    emit_tsv(out, "mcp", "the MCP server over stdio: tools/list and sessions of tools/call (see m_mcp.rs)", &rows);
}
