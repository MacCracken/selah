//! `system`: the history file, the HTTP layer, the capture and hoosh clients' Rust-contract paths, and the session
//! and clipboard helpers (port plan §3 M5). The clients are driven for real: each row's request goes to a one-shot
//! loopback server (Rust's own `http::test_server::serve_once`, copied: it is `#[cfg(test)]`), on a current-thread
//! tokio runtime.
//!
//! Rows (all class A unless marked):
//! - `history_line A h:<line>` → serde_json::from_slice::<HistoryEntry>: `h:<id> s:<path> i:<secs> i:<nanos>
//!   s:<source> i:<w> i:<h> s:<format>` or `E:serde`. Lines with a member serde ignores but bayan refuses (a
//!   duplicate key, a lone surrogate, invalid UTF-8) are tagged `adr=0002#21`.
//! - `history_id A h:<line>` → the private EntryId's read (`{ id: Uuid }`, the same derive): `h:<id>` or `E:serde`.
//! - `history_json A h:<id> s:<path> i:<secs> i:<nanos> s:<source> i:<w> i:<h> s:<format>` → `s:<line>`.
//! - `history_list A h:<file> i:<limit> <since>` → `i:<n> h:<id>…` (newest first); `<since>` is `N` or
//!   `i:<secs> i:<nanos>`; limit -1 is usize::MAX. HistoryStore::list over a file holding the bytes.
//! - `history_get A h:<file> h:<id>` → `h:<id> s:<source>` or `N`.
//! - `history_delete A h:<file> h:<id>` → `b:<found> h:<the file after>`.
//! - `region A g:<x> g:<y> g:<w> g:<h> e:ImageFormat.<F>` → CaptureClient::capture(Rect): `s:<request body>` or
//!   `E:<variant> s:<message>`. WebP rows are `adr=0002#1` (the port refuses them).
//! - `monitor_req A s:<id> e:ImageFormat.<F>` → capture_monitor: `s:<request body>` or `E:<variant> s:<message>`.
//! - `capture_http A i:<status> h:<body>` → capture_full against that answer: `c:<crc32 of image_data> i:<len>
//!   i:<w> i:<h> s:<format>`, `E:Api s:<message>` for a status error, or `E:Api s:<prefix>` ending in
//!   "invalid response" for a refused body (the rest of that text is serde's, ADR 0002 #14).
//! - `monitors_http A i:<status> h:<body>` → list_monitors: `i:<n>` then `s:<id> s:<name> i:<w> i:<h> i:<x> i:<y>
//!   b:<primary>` each, or the errors as above.
//! - `ocr_answer A h:<body>` → HooshClient::ocr against that answer: `s:<text> f:<confidence> i:<n> g:<min x>
//!   g:<min y> g:<max x> g:<max y>…` or `E:Api s:<prefix>`. The port reads the same JSON from the model's content.
//! - `agent_path A s:<agent id>` → heartbeat's request path (validate_agent_id, then endpoint_url's encoding):
//!   `s:<path>` or `E:Api s:<message>`.
//! - `session A <wayland> <xdg>` (`s:` or `N` each) → detect_session_type under those variables:
//!   `e:SessionType.<S>`.
//! - `mime A h:<bytes>` → copy_to_clipboard's MIME type: `s:<mime>`.
//! - `exit_status A i:<raw wait status>` → ExitStatus's Display: `s:<text>`.
//! - `rect_debug A g:<x> g:<y> g:<w> g:<h>` → `{rect:?}`: `s:<text>`.
//! - `daimon_config A h:<json>` → DaimonConfig: `s:<endpoint> <key>` (`s:` or `N`) or `E:serde`; and
//!   `daimon_config_json A s:<endpoint> <key>` → `s:<json>`; `hoosh_config A h:<json>` → `s:<endpoint>` or `E:serde`.
//! - `wire W …` → the retargeted requests (D2), Cyrius-only: the expected bytes are this file's constants, taken from
//!   daimon's `api_register_agent` and the OpenAI chat-completions shape hoosh forwards.

use crate::m_decode::variant;
use crate::{Out, Row, emit_tsv, tb, tf, tg, th, ti, ts};
use selah::{CaptureClient, CaptureRegion, DaimonConfig, HistoryEntry, HistoryStore, HooshClient, HooshConfig};
use selah::{ImageFormat, Rect};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

/// A request the one-shot server saw.
pub struct Captured {
    pub request_line: String,
    pub body: Vec<u8>,
}

/// Serve exactly one request with `status` and `body` (Rust's serve_once).
pub fn serve_once(status: u16, body: Vec<u8>) -> (String, mpsc::Receiver<Captured>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let Ok((stream, _)) = listener.accept() else { return };
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() {
            return;
        }
        let mut len = 0usize;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                return;
            }
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some((k, v)) = line.split_once(':')
                && k.trim().eq_ignore_ascii_case("content-length")
            {
                len = v.trim().parse().unwrap_or(0);
            }
        }
        let mut req_body = vec![0; len];
        let _ = reader.read_exact(&mut req_body);
        let mut stream = stream;
        let head = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(&body);
        let _ = tx.send(Captured { request_line: request_line.trim_end().to_string(), body: req_body });
    });
    (url, rx)
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime")
}

fn crc(bytes: &[u8]) -> String {
    let mut h = flate2::Crc::new();
    h.update(bytes);
    format!("c:{:08x}", h.sum())
}

fn row(rows: &mut Vec<Row>, func: &str, class: &'static str, inputs: Vec<String>, outputs: Vec<String>, note: &str) {
    rows.push(Row { func: func.to_string(), class, inputs, outputs, note: note.to_string() });
}

fn err_tokens(e: &selah::SelahError) -> Vec<String> {
    vec![format!("E:{}", variant(e)), ts(&e.to_string())]
}

/// An error whose tail is serde's text: the Display up to and including "invalid response".
fn err_prefix(e: &selah::SelahError) -> Vec<String> {
    let s = e.to_string();
    match s.find("invalid response") {
        Some(i) => vec![format!("E:{}", variant(e)), ts(&s[..i + "invalid response".len()])],
        None => err_tokens(e),
    }
}

fn fmt_tok(f: ImageFormat) -> String {
    format!("e:ImageFormat.{f:?}")
}

// ── history ─────────────────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct EntryId {
    id: uuid::Uuid,
}

fn entry_tokens(e: &HistoryEntry) -> Vec<String> {
    vec![
        th(e.id.as_bytes()),
        ts(&e.path),
        ti(e.timestamp.timestamp()),
        ti(i64::from(e.timestamp.timestamp_subsec_nanos())),
        ts(&e.source),
        ti(i64::from(e.width)),
        ti(i64::from(e.height)),
        ts(&e.format),
    ]
}

fn entry(id: u128, path: &str, secs: i64, nanos: u32, source: &str, w: u32, h: u32, format: &str) -> HistoryEntry {
    HistoryEntry {
        id: uuid::Uuid::from_u128(id),
        path: path.to_string(),
        timestamp: chrono::DateTime::from_timestamp(secs, nanos).expect("in range"),
        source: source.to_string(),
        width: w,
        height: h,
        format: format.to_string(),
    }
}

fn line_of(e: &HistoryEntry) -> String {
    serde_json::to_string(e).expect("serializes")
}

const ID1: &str = "00112233-4455-6677-8899-aabbccddeeff";

/// History lines, each with its ADR tag when serde accepts what bayan refuses.
fn history_lines() -> Vec<(Vec<u8>, &'static str)> {
    let base = |rest: &str| format!("{{\"id\":\"{ID1}\",\"path\":\"/p.png\",\"timestamp\":\"2026-11-14T10:00:00Z\",{rest}}}");
    let ok_rest = "\"source\":\"full screen\",\"width\":1920,\"height\":1080,\"format\":\"png\"";
    let mut v: Vec<(Vec<u8>, &'static str)> = Vec::new();
    let mut s = |t: String| v.push((t.into_bytes(), ""));
    s(base(ok_rest));
    s(format!("  {}  ", base(ok_rest)));
    s(format!("{}\r", base(ok_rest)));
    s(base(&format!("{ok_rest},\"extra\":[1,{{\"a\":null}}]")));
    s(base("\"source\":\"s\",\"width\":0,\"height\":4294967295,\"format\":\"\""));
    s(base("\"source\":\"s\",\"width\":4294967296,\"height\":1,\"format\":\"png\""));
    s(base("\"source\":\"s\",\"width\":-1,\"height\":1,\"format\":\"png\""));
    s(base("\"source\":\"s\",\"width\":-0,\"height\":1,\"format\":\"png\""));
    s(base("\"source\":\"s\",\"width\":1.0,\"height\":1,\"format\":\"png\""));
    s(base("\"source\":\"s\",\"width\":1e2,\"height\":1,\"format\":\"png\""));
    s(base("\"source\":\"s\",\"width\":\"1\",\"height\":1,\"format\":\"png\""));
    s(base("\"source\":\"s\",\"width\":1,\"height\":1"));
    s(base("\"source\":null,\"width\":1,\"height\":1,\"format\":\"png\""));
    s(base("\"source\":\"tab\\there \\u00e9 \\ud83d\\ude00 \\\"q\\\"\",\"width\":1,\"height\":1,\"format\":\"png\""));
    s(base("\"source\":\"s\",\"width\":1,\"height\":1,\"format\":\"png\",\"big\":18446744073709551616"));
    s(base("\"source\":\"s\",\"width\":1,\"height\":1,\"format\":\"png\",\"neg\":-9223372036854775809"));
    s(base("\"source\":\"s\",\"width\":1,\"height\":1,\"format\":\"png\",\"huge\":1e400"));
    // The id in each of Uuid::parse_str's forms, and not.
    for id in [
        "00112233445566778899AABBCCDDEEFF",
        "{00112233-4455-6677-8899-aabbccddeeff}",
        "URN:UUID:00112233-4455-6677-8899-aabbccddeeff",
        "00112233-4455-6677-8899-aabbccddeef",
        "00112233-4455-6677-8899_aabbccddeeff",
    ] {
        s(format!(
            "{{\"id\":\"{id}\",\"path\":\"p\",\"timestamp\":\"2026-11-14T10:00:00Z\",{ok_rest}}}"
        ));
    }
    // Timestamps as chrono reads them.
    for ts in [
        "2026-11-14T10:00:00.5Z",
        "2026-11-14T10:00:00.123456789Z",
        "2026-11-14T10:00:00.1234567891Z",
        "2026-11-14t10:00:00z",
        "2026-11-14 10:00:00+02:00",
        "2026-11-14T10:00:00-0130",
        "2026-11-14T23:59:60Z",
        "2026-02-30T10:00:00Z",
        "2026-11-14T10:00:00",
        "+12026-11-14T10:00:00Z",
        "1969-12-31T23:59:59.999999999Z",
    ] {
        s(format!(
            "{{\"id\":\"{ID1}\",\"path\":\"p\",\"timestamp\":\"{ts}\",{ok_rest}}}"
        ));
    }
    // The array form serde's derive also reads.
    s(format!("[\"{ID1}\",\"p\",\"2026-11-14T10:00:00Z\",\"s\",1,2,\"png\"]"));
    s(format!("[\"{ID1}\",\"p\",\"2026-11-14T10:00:00Z\",\"s\",1,2]"));
    s(format!("[\"{ID1}\"]"));
    // Malformed and torn.
    s("{not json}".to_string());
    s(format!("{{\"id\":\"{ID1}\",\"path\":\"p"));
    s(format!("{} x", base(ok_rest)));
    s("null".to_string());
    s("[]".to_string());
    s(format!("{{\"id\":\"{ID1}\"}}"));
    s(format!("{{\"id\":\"{ID1}\",\"id\":\"{ID1}\"}}"));
    drop(s);
    // Accepted by serde (the defect sits in a member it ignores), refused by bayan (ADR 0002 #21).
    let adr = "adr=0002#21";
    v.push((base(&format!("{ok_rest},\"x\":1,\"x\":2")).into_bytes(), adr));
    v.push((base(&format!("{ok_rest},\"x\":\"\\ud800\"")).into_bytes(), adr));
    let mut bad = base(&format!("{ok_rest},\"x\":\"..\"")).into_bytes();
    let at = bad.len() - 3;
    bad[at] = 0xff;
    v.push((bad, adr));
    // Invalid UTF-8 in a field serde keeps: refused by both.
    let mut bad2 = base(ok_rest).into_bytes();
    let at2 = bad2.iter().position(|&b| b == b'f').expect("in source");
    bad2[at2] = 0xc3;
    v.push((bad2, ""));
    v
}

fn history_file(lines: &[&[u8]]) -> Vec<u8> {
    let mut f = Vec::new();
    for l in lines {
        f.extend_from_slice(l);
        f.push(b'\n');
    }
    f
}

fn temp_store(tag: &str, n: usize, content: &[u8]) -> (HistoryStore, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("selah_oracle_history_{tag}_{n}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("history.jsonl");
    std::fs::write(&path, content).expect("write");
    (HistoryStore::open(path), dir)
}

fn emit_history(rows: &mut Vec<Row>) {
    for (line, adr) in history_lines() {
        let note = if adr.is_empty() { String::new() } else { adr.to_string() };
        let trimmed = line.trim_ascii();
        let outs = match serde_json::from_slice::<HistoryEntry>(trimmed) {
            Ok(e) => entry_tokens(&e),
            Err(_) => vec!["E:serde".to_string()],
        };
        row(rows, "history_line", "A", vec![th(&line)], outs, &note);
        let id = match serde_json::from_slice::<EntryId>(&line) {
            Ok(e) => vec![th(e.id.as_bytes())],
            Err(_) => vec!["E:serde".to_string()],
        };
        row(rows, "history_id", "A", vec![th(&line)], id, &note);
    }

    // Writing: the line serde_json writes for an entry.
    let entries = [
        entry(1, "/tmp/a.png", 1_763_164_800, 0, "full screen", 1920, 1080, "png"),
        entry(2, "/tmp/a \"b\"\\c\n\u{7f}\u{1}é😀.png", 0, 1, "region (5x5 at 1,2)", 0, 4_294_967_295, "jpg"),
        entry(3, "", -1, 999_999_999, "", 1, 1, ""),
        entry(4, "p", 1_763_164_800, 120_000_000, "window 0x1", 7, 8, "bmp"),
        entry(5, "p", 1_763_164_800, 123_456_000, "s", 7, 8, "webp"),
        entry(6, "p", 1_763_164_800, 123_456_789, "s", 7, 8, "png"),
        entry(7, "p", 253_402_300_799, 0, "s", 7, 8, "png"),
        entry(8, "p", -62_135_596_800, 0, "s", 7, 8, "png"),
        entry(9, "p", 253_402_300_800, 5, "s", 7, 8, "png"),
        entry(10, "p", -62_167_219_201, 0, "s", 7, 8, "png"),
    ];
    for e in &entries {
        row(rows, "history_json", "A", entry_tokens(e), vec![ts(&line_of(e))], "");
    }
    // A leap second, which only reading can make.
    let leap: HistoryEntry = serde_json::from_str(&format!(
        "{{\"id\":\"{ID1}\",\"path\":\"p\",\"timestamp\":\"2016-12-31T23:59:60.5Z\",\"source\":\"s\",\"width\":1,\
         \"height\":1,\"format\":\"png\"}}"
    ))
    .expect("leap second");
    row(rows, "history_json", "A", entry_tokens(&leap), vec![ts(&line_of(&leap))], "leap second");

    // list / get / delete over Rust-written lines with some foreign ones mixed in.
    let ts0 = 1_763_164_800i64;
    let mk = |id: u128, off: i64, nanos: u32, src: &str| line_of(&entry(id, "/s.png", ts0 + off, nanos, src, 4, 3, "png"));
    let l: Vec<String> = vec![
        mk(1, 3, 0, "e0"),
        mk(2, 1, 0, "e1"),
        mk(3, 5, 0, "e2"),
        mk(4, 5, 0, "e3"),
        mk(5, 2, 0, "e4"),
        mk(6, 5, 1, "e5"),
        mk(7, -3600, 0, "old"),
    ];
    let lb: Vec<&[u8]> = l.iter().map(|s| s.as_bytes()).collect();
    let mut files: Vec<Vec<u8>> = vec![
        history_file(&lb),
        Vec::new(),
        history_file(&[lb[0], b"{not json}", b"\xff\xfe garbage", b"", b"   ", lb[1], lb[2]]),
        {
            let mut f = history_file(&[lb[0], lb[1]]);
            f.extend_from_slice(b"{\"id\":\"0000");
            f
        },
        history_file(&[lb[3], lb[3], lb[4]]),
        {
            let mut f = history_file(&[lb[0]]);
            f.extend_from_slice(lb[1]);
            f
        },
        history_file(&[
            format!("[\"{}\",\"p\",\"2026-11-14T10:00:00Z\",\"arr\",1,2,\"png\"]", uuid::Uuid::from_u128(8)).as_bytes(),
            lb[0],
            format!("{}\x0c", l[1]).as_bytes(),
            format!("  {}\r", l[2]).as_bytes(),
        ]),
    ];
    // A bigger file, so the bounded heap is exercised past its first growth.
    let big: Vec<String> = (0..200u128)
        .map(|i| mk(100 + i, ((i * 7919) % 97) as i64, (i % 3) as u32, &format!("b{i}")))
        .collect();
    let bb: Vec<&[u8]> = big.iter().map(|s| s.as_bytes()).collect();
    files.push(history_file(&bb));

    let sinces: [Option<(i64, u32)>; 4] = [None, Some((ts0 + 2, 0)), Some((ts0 + 5, 1)), Some((ts0 + 5, 2))];
    for (fi, content) in files.iter().enumerate() {
        for limit in [0i64, 1, 3, 10, -1] {
            for since in &sinces {
                let (store, dir) = temp_store("list", fi, content);
                let lim = if limit < 0 { usize::MAX } else { limit as usize };
                let s = since.map(|(a, b)| chrono::DateTime::from_timestamp(a, b).expect("since"));
                let got = store.list(lim, s).expect("list");
                let mut outs = vec![ti(got.len() as i64)];
                outs.extend(got.iter().map(|e| th(e.id.as_bytes())));
                let mut ins = vec![th(content), ti(limit)];
                match since {
                    None => ins.push("N".to_string()),
                    Some((a, b)) => {
                        ins.push(ti(*a));
                        ins.push(ti(i64::from(*b)));
                    }
                }
                row(rows, "history_list", "A", ins, outs, "");
                let _ = std::fs::remove_dir_all(&dir);
            }
        }
        for id in [1u128, 3, 4, 8, 100, 150, 999] {
            let uid = uuid::Uuid::from_u128(id);
            let (store, dir) = temp_store("get", fi, content);
            let outs = match store.get(uid).expect("get") {
                Some(e) => vec![th(e.id.as_bytes()), ts(&e.source)],
                None => vec!["N".to_string()],
            };
            row(rows, "history_get", "A", vec![th(content), th(uid.as_bytes())], outs, "");
            let _ = std::fs::remove_dir_all(&dir);
            let (store, dir) = temp_store("del", fi, content);
            let found = store.delete(uid).expect("delete");
            let after = std::fs::read(store.path()).unwrap_or_default();
            let after_tok = if after.len() > 4096 { crc(&after) } else { th(&after) };
            row(rows, "history_delete", "A", vec![th(content), th(uid.as_bytes())], vec![tb(found), after_tok], "");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

// ── capture ─────────────────────────────────────────────────────────────────────────

fn rect_grid() -> Vec<[f32; 4]> {
    let (nan, inf) = (f32::NAN, f32::INFINITY);
    vec![
        [100.0, 100.0, -50.0, -30.0],
        [-10.0, -5.0, 100.0, 50.0],
        [0.5, 0.5, 1.0, 1.0],
        [0.0, 0.0, 0.0, 10.0],
        [-100.0, 0.0, 50.0, 10.0],
        [nan, 0.0, 10.0, 10.0],
        [0.0, 0.0, inf, 10.0],
        [10.0, 20.0, -5.0, 8.0],
        [0.0, 0.0, 1920.0, 1080.0],
        [0.25, 0.75, 0.5, 0.25],
        [0.0, 0.0, 0.001, 0.001],
        [-0.5, -0.5, 1.0, 1.0],
        [-0.5, -0.5, 0.5, 0.5],
        [4_294_967_000.0, 0.0, 1000.0, 1.0],
        [4_294_967_296.0, 0.0, 10.0, 10.0],
        [1e20, 1e20, 1e20, 1e20],
        [-1e20, -1e20, 2e20, 2e20],
        [3e38, 0.0, 3e38, 1.0],
        [-inf, 0.0, 1.0, 1.0],
        [0.0, -inf, 1.0, 1.0],
        [0.0, 0.0, -inf, 1.0],
        [0.0, 0.0, 1.0, nan],
        [16_777_216.0, 0.0, 1.0, 1.0],
        [16_777_215.5, 0.0, 0.25, 1.0],
        [1e-30, 1e-30, 1e-30, 1e-30],
        [-0.0, -0.0, 3.0, -0.0],
        [1.0, 1.0, -1.5, -1.5],
        [2.5, 3.5, 0.0, 1.0],
        [7.0, 7.0, 1e-7, 1e-7],
        [123.456, 789.012, 345.678, 901.234],
        [0.1, 0.2, 0.3, -0.7],
        [-5.0, 2.0, 4.0, 3.0],
        [65535.5, 65535.5, 2.0, 2.0],
    ]
}

fn emit_capture(rows: &mut Vec<Row>) {
    let rt = rt();
    let ok_body = br#"{"image_data":"AAAA","width":1,"height":1,"format":"png"}"#.to_vec();
    let formats = [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Bmp, ImageFormat::WebP];
    for r in rect_grid() {
        let rect = Rect::new(r[0], r[1], r[2], r[3]);
        let g = vec![tg(r[0]), tg(r[1]), tg(r[2]), tg(r[3])];
        row(rows, "rect_debug", "A", g.clone(), vec![ts(&format!("{rect:?}"))], "");
        for (fi, f) in formats.iter().enumerate() {
            if fi > 0 && r != [10.0, 20.0, -5.0, 8.0] && r != [0.0, 0.0, 0.0, 10.0] {
                continue;
            }
            let (url, rx) = serve_once(200, ok_body.clone());
            let client = CaptureClient::new(&url);
            let res = rt.block_on(client.capture(&CaptureRegion::Rect(rect), *f));
            let outs = match res {
                Ok(_) => {
                    let req = rx.recv().expect("request");
                    vec![ts(&String::from_utf8(req.body).expect("utf-8"))]
                }
                Err(e) => err_tokens(&e),
            };
            let note = if *f == ImageFormat::WebP { "adr=0002#1" } else { "" };
            let mut ins = g.clone();
            ins.push(fmt_tok(*f));
            row(rows, "region", "A", ins, outs, note);
        }
    }

    // capture_monitor: ids, blank ids (Unicode whitespace), escaping.
    let ids = [
        "HDMI-A-1", "DP-1", " ", "", "\t\n", "\u{3000}", "\u{85}", "\u{a0}x", "a\"b\\c", "é😀", "\u{1}", "x\u{2028}",
    ];
    for id in ids {
        let (url, rx) = serve_once(200, ok_body.clone());
        let client = CaptureClient::new(&url);
        let res = rt.block_on(client.capture_monitor(id, ImageFormat::Png));
        let outs = match res {
            Ok(_) => vec![ts(&String::from_utf8(rx.recv().expect("request").body).expect("utf-8"))],
            Err(e) => err_tokens(&e),
        };
        row(rows, "monitor_req", "A", vec![ts(id), fmt_tok(ImageFormat::Png)], outs, "");
    }

    // The capture answer and the HTTP status text.
    let mut long = vec![b'x'; 600];
    long[511] = 0xc3;
    long[512] = 0xa9;
    let bodies: Vec<(u16, Vec<u8>)> = vec![
        (200, ok_body.clone()),
        (200, br#"{"image_data":"","width":0,"height":4294967295,"format":"","extra":{"a":[1,2]}}"#.to_vec()),
        (200, br#"["iVBORw0KGgo=",3,2,"png"]"#.to_vec()),
        (200, br#"{"image_data":"x","width":-1,"height":1,"format":"png"}"#.to_vec()),
        (200, br#"{"image_data":"x","width":1.0,"height":1,"format":"png"}"#.to_vec()),
        (200, br#"{"image_data":"x","width":1,"height":1}"#.to_vec()),
        (200, br#"{"image_data":null,"width":1,"height":1,"format":"png"}"#.to_vec()),
        (200, b"not json".to_vec()),
        (200, b"".to_vec()),
        (201, ok_body.clone()),
        (299, ok_body.clone()),
        (403, br#"{"error":"permission denied"}"#.to_vec()),
        (404, b"".to_vec()),
        (418, b"short and stout".to_vec()),
        (429, br#"{"error":"rate limited"}"#.to_vec()),
        (500, vec![b'x'; 10_000]),
        (500, vec![b'y'; 512]),
        (500, vec![b'y'; 513]),
        (502, long.clone()),
        (503, b"a\xe0\x80b\xf0\x9f\x98c\xffd \xed\xa0\x80 \xc3".to_vec()),
        (599, b"odd".to_vec()),
        (400, "naïve ünïcode".as_bytes().to_vec()),
    ];
    for (status, body) in &bodies {
        let (url, _rx) = serve_once(*status, body.clone());
        let client = CaptureClient::new(&url);
        let outs = match rt.block_on(client.capture_full()) {
            Ok(c) => vec![crc(c.image_data.as_bytes()), ti(c.image_data.len() as i64), ti(i64::from(c.width)),
                ti(i64::from(c.height)), ts(&c.format)],
            Err(e) => err_prefix(&e),
        };
        row(rows, "capture_http", "A", vec![ti(i64::from(*status)), th(body)], outs, "");
    }

    let monitor_bodies: Vec<(u16, Vec<u8>)> = vec![
        (200, br#"[{"id":"DP-1","name":"Main","width":2560,"height":1440,"x":0,"y":0,"primary":true}]"#.to_vec()),
        (200, br#"[]"#.to_vec()),
        (200, br#"[{"id":"a","name":"b","width":1,"height":2,"x":-2147483648,"y":2147483647,"primary":false},["c","d",3,4,-5,6,true]]"#.to_vec()),
        (200, br#"[{"id":"a","name":"b","width":1,"height":2,"x":2147483648,"y":0,"primary":false}]"#.to_vec()),
        (200, br#"[{"id":"a","name":"b","width":1,"height":2,"x":-0,"y":0,"primary":false}]"#.to_vec()),
        (200, br#"[{"id":"a","name":"b","width":1,"height":2,"x":0,"y":0,"primary":1}]"#.to_vec()),
        (200, br#"[{"id":"a","name":"b","width":1,"height":2,"x":0,"y":0}]"#.to_vec()),
        (200, br#"{"id":"a"}"#.to_vec()),
        (200, b"[".to_vec()),
        (500, b"boom".to_vec()),
    ];
    for (status, body) in &monitor_bodies {
        let (url, _rx) = serve_once(*status, body.clone());
        let client = CaptureClient::new(&url);
        let outs = match rt.block_on(client.list_monitors()) {
            Ok(ms) => {
                let mut o = vec![ti(ms.len() as i64)];
                for m in &ms {
                    o.extend([ts(&m.id), ts(&m.name), ti(i64::from(m.width)), ti(i64::from(m.height)),
                        ti(i64::from(m.x)), ti(i64::from(m.y)), tb(m.primary)]);
                }
                o
            }
            Err(e) => err_prefix(&e),
        };
        row(rows, "monitors_http", "A", vec![ti(i64::from(*status)), th(body)], outs, "");
    }
}

// ── hoosh (the answer's reading) and agent ids ──────────────────────────────────────

fn emit_ocr(rows: &mut Vec<Row>) {
    let rt = rt();
    let answers: Vec<(&[u8], &str)> = vec![
        (br#"{"text":"hi","confidence":7.5,"regions":[{"x":1,"y":2,"width":3,"height":4},{"x":1e300,"y":0,"width":1,"height":1},{"x":0,"y":0,"width":0,"height":0}]}"#, ""),
        (br#"{"text":"t","confidence":-1,"regions":[]}"#, ""),
        (br#"{"text":"t","confidence":0.25}"#, ""),
        (br#"{"text":"t","confidence":"high"}"#, ""),
        (br#"{"text":"t","confidence":null}"#, ""),
        (br#"{"text":"t","confidence":-0}"#, ""),
        (br#"{"text":"t","confidence":-0.0}"#, ""),
        (br#"{"text":"t","confidence":1e-320}"#, ""),
        (br#"{"text":"t","confidence":18446744073709551615}"#, ""),
        (br#"{"text":"t","confidence":1e400}"#, ""),
        (br#"{"text":"t","regions":[{"x":"1","y":2,"width":3,"height":4}]}"#, ""),
        (br#"{"text":"t","regions":[{"x":10,"y":10,"width":-5,"height":5}]}"#, ""),
        (br#"{"text":"t","regions":[{"x":0.1,"y":0.2,"width":0.3,"height":-0.7}]}"#, ""),
        (br#"{"text":"t","regions":[{"x":16777217,"y":0,"width":1,"height":1}]}"#, ""),
        (br#"{"text":"t","regions":[{"x":3.4028235677973366e38,"y":0,"width":1,"height":1}]}"#, ""),
        (br#"{"text":"t","regions":[{"x":-1e38,"y":0,"width":2e38,"height":1}]}"#, ""),
        (br#"{"text":"t","regions":[{"x":1e-46,"y":0,"width":1e-46,"height":1}]}"#, ""),
        (br#"{"text":"t","regions":[[1,2,3,4],5,null,{"x":1,"y":1,"width":1,"height":1,"z":9}]}"#, ""),
        (br#"{"text":"t","regions":{"x":1}}"#, ""),
        (br#"{"text":5,"confidence":0.5}"#, ""),
        (br#"{"text":"line 1\nline 2 \u00e9 \ud83d\ude00"}"#, ""),
        (br#"["text","confidence"]"#, ""),
        (br#""just a string""#, ""),
        (br#"42"#, ""),
        (br#"{}"#, ""),
        (br#"not json"#, ""),
        (br#"{"text":"t""#, ""),
        (br#"{"text":"a","text":"b"}"#, "adr=0002#21"),
    ];
    for (body, adr) in answers {
        let (url, _rx) = serve_once(200, body.to_vec());
        let client = HooshClient::with_config(HooshConfig { endpoint: url });
        let outs = match rt.block_on(client.ocr(b"img")) {
            Ok(o) => {
                let mut v = vec![ts(&o.text), tf(o.confidence), ti(o.bounding_boxes.len() as i64)];
                for b in &o.bounding_boxes {
                    let h = b.as_hisab();
                    v.extend([tg(h.min.x), tg(h.min.y), tg(h.max.x), tg(h.max.y)]);
                }
                v
            }
            Err(e) => err_prefix(&e),
        };
        row(rows, "ocr_answer", "A", vec![th(body)], outs, adr);
    }

    let ids = [
        "agent-123", "x/../../admin", "a b", "a%20b", "q?x#y", "{}`<>\"\\", "é😀", ".", "..", "", "a\nb", "a\tb",
        "\u{7f}", "\u{85}", "\u{9f}x", "\u{a0}", "\u{2028}", "...", "./a", "a/", "/", "%", "~-._!$&'()*+,;=:@",
    ];
    let mut owned: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
    owned.push("a".repeat(256));
    owned.push("a".repeat(257));
    owned.push(format!("{}\n", "é".repeat(40)));
    for id in &owned {
        let (url, rx) = serve_once(200, Vec::new());
        let client = selah::DaimonClient::with_config(DaimonConfig { endpoint: url, api_key: None });
        let outs = match rt.block_on(client.heartbeat(id)) {
            Ok(()) => {
                let line = rx.recv().expect("request").request_line;
                let path = line.split(' ').nth(1).unwrap_or("").to_string();
                vec![ts(&path)]
            }
            Err(e) => err_tokens(&e),
        };
        row(rows, "agent_path", "A", vec![ts(id)], outs, "");
    }
}

// ── environment, MIME, exit status, configs ─────────────────────────────────────────

fn emit_misc(rows: &mut Vec<Row>) {
    let vals: [Option<&str>; 5] = [None, Some(""), Some("wayland-0"), Some("Wayland"), Some("x11")];
    for w in &vals {
        for x in &vals {
            // SAFETY: the oracle sets these before any thread of its own reads the environment (this runs first).
            unsafe {
                match w {
                    Some(v) => std::env::set_var("WAYLAND_DISPLAY", v),
                    None => std::env::remove_var("WAYLAND_DISPLAY"),
                }
                match x {
                    Some(v) => std::env::set_var("XDG_SESSION_TYPE", v),
                    None => std::env::remove_var("XDG_SESSION_TYPE"),
                }
            }
            let st = selah::capture::detect_session_type();
            let tok = |o: &Option<&str>| o.map(ts).unwrap_or_else(|| "N".to_string());
            row(rows, "session", "A", vec![tok(w), tok(x)], vec![format!("e:SessionType.{st:?}")], "");
        }
    }
    unsafe {
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("XDG_SESSION_TYPE");
    }

    let mimes: Vec<&[u8]> = vec![
        b"\x89PNG\r\n\x1a\n....", b"\x89PNG\r\n\x1a", b"\xff\xd8\xff\xe0", b"\xff\xd8", b"GIF89a", b"GIF87a..", b"GIF8",
        b"RIFF\x10\x00\x00\x00WEBPVP8 ", b"RIFF\x10\x00\x00\x00WEBQ", b"RIFF\x10\x00\x00", b"MM\x00*", b"II*\x00",
        b"II*", b"DDS ", b"BM", b"B", b"\x00\x00\x01\x00", b"\x00\x00\x02\x00", b"#?RADIANCE\n",
        b"\x00\x00\x00\x1cftypavif", b"\x00\x00\x00\x1cftypavis", b"v/1\x01", b"qoif", b"P1", b"P7", b"P8", b"P",
        b"farbfeld", b"", b"text", b"\x89PNG\r\n\x1a\x0a",
    ];
    for m in mimes {
        let mime = image::guess_format(m).map(|f| f.to_mime_type()).unwrap_or("image/png");
        row(rows, "mime", "A", vec![th(m)], vec![ts(mime)], "");
    }

    use std::os::unix::process::ExitStatusExt;
    let mut raws: Vec<i32> = vec![0, 1 << 8, 2 << 8, 127 << 8, 255 << 8];
    for sig in 1..=31 {
        raws.push(sig);
    }
    raws.extend([9 | 0x80, 11 | 0x80, 6 | 0x80]);
    for raw in raws {
        let st = std::process::ExitStatus::from_raw(raw);
        row(rows, "exit_status", "A", vec![ti(i64::from(raw))], vec![ts(&st.to_string())], "");
    }

    let dconf: Vec<&[u8]> = vec![
        br#"{"endpoint":"http://localhost:8090","api_key":"secret"}"#,
        br#"{"endpoint":"http://x","api_key":null}"#,
        br#"{"endpoint":"http://x"}"#,
        br#"{"api_key":"k"}"#,
        br#"{"endpoint":5}"#,
        br#"{"endpoint":"http://x","api_key":7}"#,
        br#"["http://x","k"]"#,
        br#"["http://x"]"#,
        br#"{"endpoint":"http://x","extra":1}"#,
    ];
    for j in dconf {
        let outs = match serde_json::from_slice::<DaimonConfig>(j) {
            Ok(c) => vec![ts(&c.endpoint), c.api_key.as_deref().map(ts).unwrap_or_else(|| "N".to_string())],
            Err(_) => vec!["E:serde".to_string()],
        };
        row(rows, "daimon_config", "A", vec![th(j)], outs, "");
    }
    for (ep, key) in [("http://localhost:8090", Some("secret")), ("http://x/\"y\"", None), ("", Some(""))] {
        let c = DaimonConfig { endpoint: ep.to_string(), api_key: key.map(str::to_string) };
        let k = key.map(ts).unwrap_or_else(|| "N".to_string());
        row(rows, "daimon_config_json", "A", vec![ts(ep), k], vec![ts(&serde_json::to_string(&c).expect("json"))], "");
    }
    let hconf: Vec<&[u8]> = vec![
        br#"{"endpoint":"http://localhost:8088"}"#,
        br#"{"endpoint":"http://h","model":"m"}"#,
        br#"{}"#,
        br#"{"endpoint":null}"#,
    ];
    for j in hconf {
        let outs = match serde_json::from_slice::<HooshConfig>(j) {
            Ok(c) => vec![ts(&c.endpoint)],
            Err(_) => vec!["E:serde".to_string()],
        };
        row(rows, "hoosh_config", "A", vec![th(j)], outs, "");
    }
}

// ── class W: the retargeted requests (D2) ───────────────────────────────────────────

fn emit_wire(rows: &mut Vec<Row>) {
    // daimon 2.5.6 api_register_agent reads `name` and `type` (System | User | Service) and answers 201 with the id.
    for name in ["selah", "sel\"ah\n"] {
        let mut body = String::from("{\"name\":");
        body.push_str(&serde_json::to_string(name).expect("json"));
        body.push_str(",\"type\":\"Service\"}");
        row(rows, "wire_register", "W", vec![ts(name)], vec![ts("POST /v1/agents"), ts(&body)],
            "daimon src/api_agent.cyr api_register_agent");
    }
    // The OpenAI chat-completions shape hoosh forwards to OpenAI-compatible providers.
    for (model, img) in [("vision-model", b"img".as_slice()), ("m\"x", b"\x89PNG\r\n\x1a\n\x00".as_slice())] {
        let mime = image::guess_format(img).map(|f| f.to_mime_type()).unwrap_or("image/png");
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, img);
        let js = |v: &str| serde_json::to_string(v).expect("json");
        let body = format!(
            "{{\"model\":{},\"messages\":[{{\"role\":\"user\",\"content\":[{{\"type\":\"text\",\"text\":{}}},\
             {{\"type\":\"image_url\",\"image_url\":{{\"url\":\"data:{mime};base64,{b64}\"}}}}]}}]}}",
            js(model),
            js(OCR_PROMPT)
        );
        row(rows, "wire_ocr", "W", vec![ts(model), th(img)],
            vec![ts("POST /v1/chat/completions"), ts(&body)],
            "hoosh /v1/chat/completions, OpenAI content parts");
    }
}

/// The OCR instruction the port sends (its src/daimon.cyr `_sl_ocr_prompt`).
const OCR_PROMPT: &str = "Extract all of the text in this image. Answer with only a JSON object and nothing else: \
{\"text\": the text, lines separated by \\n, \"confidence\": your confidence from 0 to 1, \"regions\": [{\"x\": left, \
\"y\": top, \"width\": w, \"height\": h} in pixels of this image, one per line of text]}.";

pub fn emit(out: &mut Out) {
    let mut rows = Vec::new();
    emit_misc(&mut rows);
    emit_history(&mut rows);
    emit_capture(&mut rows);
    emit_ocr(&mut rows);
    emit_wire(&mut rows);
    emit_tsv(
        out,
        "system",
        "history, the HTTP layer, the capture and hoosh clients, session and clipboard helpers (see m_system.rs)",
        &rows,
    );
}

/// The reverse oracle for history: Rust lists each history file the Cyrius harness wrote (build/reverse/history-N.jsonl)
/// and compares the ids, newest first, with the harness's `.ids` file. Returns the number that differ.
pub fn reverse_history(dir: &std::path::Path) -> usize {
    let mut bad = 0usize;
    let mut n = 0usize;
    let mut i = 0usize;
    loop {
        let file = dir.join(format!("history-{i}.jsonl"));
        let want = dir.join(format!("history-{i}.ids"));
        if !file.exists() {
            break;
        }
        n += 1;
        let store = HistoryStore::open(file.clone());
        let got = store.list(usize::MAX, None).map(|v| {
            v.iter().map(|e| e.id.to_string()).collect::<Vec<_>>().join("\n")
        });
        let want_s = std::fs::read_to_string(&want).unwrap_or_default();
        match got {
            Ok(g) if g.trim() == want_s.trim() => {}
            Ok(g) => {
                eprintln!("history {}: Rust lists\n{g}\nthe port expected\n{want_s}", file.display());
                bad += 1;
            }
            Err(e) => {
                eprintln!("history {}: Rust refuses: {e}", file.display());
                bad += 1;
            }
        }
        i += 1;
    }
    println!("reverse history: {}/{n} Cyrius-written history files read back by Rust", n - bad);
    bad
}
