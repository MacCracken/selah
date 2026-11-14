//! `oracle` — drives the frozen Rust selah 0.30.0 in `rust-old/` and writes the Cyrius port's golden corpus
//! under `tests/golden/` (port plan §2 step 5). Format 1 is specified in `scripts/oracle/README.md`.
//!
//! ```sh
//! cargo run --release --locked --manifest-path scripts/oracle/Cargo.toml             # write
//! cargo run --release --locked --manifest-path scripts/oracle/Cargo.toml -- --check  # verify; exit 1 on any difference
//! cargo run --release --locked --manifest-path scripts/oracle/Cargo.toml -- --reverse build/reverse
//!     # class R's reverse oracle: Rust decodes what the Cyrius encoders wrote (cyrius test writes the directory)
//! cargo run --release --locked --manifest-path scripts/oracle/Cargo.toml -- --bench
//!     # Rust's numbers for the codec benches (after `cyrius bench`, which writes build/bench/)
//! ```

mod b_codec;
mod fixtures;
mod m_annotate;
mod m_codec;
mod m_core;
mod m_decode;
mod m_error;
mod m_fmtf;
mod m_geometry;
mod m_rfc3339;
mod m_system;
mod m_uuid;
mod m_ai;
mod t_ai;
mod t_debug;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every file the oracle owns, keyed by its path under `tests/golden/`.
pub type Out = BTreeMap<String, Vec<u8>>;

/// Generated Cyrius sources (port plan: tables are generated, never typed), keyed by their path from the repo root.
pub type SrcOut = BTreeMap<String, Vec<u8>>;

/// The provenance line every `.tsv` carries.
pub const PROVENANCE: &str =
    "oracle: selah 0.30.0 (rust-old/, tag 0.30.0), image 0.25.10, png 0.18.1, zune-jpeg 0.5.15, image-webp 0.2.4";
pub const REGENERATE: &str = "regenerate: cargo run --release --locked --manifest-path scripts/oracle/Cargo.toml";

/// One golden row: `fn <TAB> class <TAB> inputs <TAB> outputs <TAB> note`.
pub struct Row {
    pub func: String,
    pub class: &'static str,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub note: String,
}

/// Escape a string token's payload so a token is one run of printable, non-space bytes: `\\`, `\t`, `\n`, `\r`,
/// `\s` (space), and `\xNN` for every other byte below 0x20 and for 0x7F. Bytes from 0x80 up (UTF-8) are written
/// as they are.
pub fn esc(s: &str) -> String {
    let mut o: Vec<u8> = Vec::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'\\' => o.extend_from_slice(b"\\\\"),
            b'\t' => o.extend_from_slice(b"\\t"),
            b'\n' => o.extend_from_slice(b"\\n"),
            b'\r' => o.extend_from_slice(b"\\r"),
            b' ' => o.extend_from_slice(b"\\s"),
            b if b < 0x20 || b == 0x7F => o.extend_from_slice(format!("\\x{b:02X}").as_bytes()),
            b => o.push(b),
        }
    }
    // Only ASCII bytes were rewritten (to ASCII), so the result is still the input's valid UTF-8.
    String::from_utf8(o).expect("escaping keeps UTF-8 valid")
}

/// Token constructors (format 1, scripts/oracle/README.md).
pub fn tg(x: f32) -> String { format!("g:{:08x}", x.to_bits()) }
pub fn tf(x: f64) -> String { format!("f:{:016x}", x.to_bits()) }
pub fn ti(x: i64) -> String { format!("i:{x}") }
pub fn tb(x: bool) -> String { format!("b:{}", u8::from(x)) }
pub fn ts(x: &str) -> String { format!("s:{}", esc(x)) }
pub fn th(x: &[u8]) -> String {
    let mut s = String::from("h:");
    for b in x { s.push_str(&format!("{b:02x}")); }
    s
}

/// Seeded xorshift64 for grids.
pub struct Xs(pub u64);
impl Xs {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Notes are free text for people; only tabs and newlines are escaped so a row stays one line.
fn note_text(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n").replace('\r', "\\r")
}

/// Render a module's `.tsv` (header + rows) into `out`.
pub fn emit_tsv(out: &mut Out, module: &str, columns_note: &str, rows: &[Row]) {
    let mut s = String::new();
    s.push_str(&format!("# format 1 · module {module}\n"));
    s.push_str(&format!("# {PROVENANCE}\n"));
    s.push_str(&format!("# {REGENERATE}\n"));
    s.push_str(&format!("# columns: fn<TAB>class<TAB>inputs<TAB>outputs<TAB>note · {columns_note}\n"));
    s.push_str(&format!("# rows: {}\n", rows.len()));
    for r in rows {
        let ins = if r.inputs.is_empty() { "-".to_string() } else { r.inputs.join(" ") };
        let outs = if r.outputs.is_empty() { "-".to_string() } else { r.outputs.join(" ") };
        s.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", r.func, r.class, ins, outs, note_text(&r.note)));
    }
    out.insert(format!("{module}.tsv"), s.into_bytes());
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("repo root")
}

/// Every file under `dir`, as paths relative to `dir`, sorted.
fn walk(dir: &Path, base: &Path, acc: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            walk(&p, base, acc);
        } else {
            acc.push(p.strip_prefix(base).expect("under base").to_string_lossy().into_owned());
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--bench") {
        b_codec::run(&repo_root());
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--reverse") {
        let dir = args.get(i + 1).map(PathBuf::from).unwrap_or_else(|| repo_root().join("build/reverse"));
        let bad = m_codec::reverse(&dir) + m_system::reverse_history(&dir);
        std::process::exit(i32::from(bad > 0));
    }
    let check = args.iter().any(|a| a == "--check");
    let golden = repo_root().join("tests/golden");

    let mut out = Out::new();
    let fixtures = fixtures::build();
    fixtures::emit(&mut out, &fixtures);
    let mut modules = Vec::new();
    m_decode::emit(&mut out, &fixtures);
    modules.push("decode");
    m_error::emit(&mut out);
    modules.push("error");
    m_fmtf::emit(&mut out);
    modules.push("fmtf");
    m_geometry::emit(&mut out);
    modules.push("geometry");
    m_core::emit(&mut out);
    modules.push("core");
    m_uuid::emit(&mut out);
    modules.push("uuid");
    m_rfc3339::emit(&mut out);
    modules.push("rfc3339");
    m_ai::emit(&mut out, &fixtures);
    modules.push("ai");
    m_codec::emit(&mut out, &fixtures);
    modules.push("codec");
    m_annotate::emit(&mut out, &fixtures);
    modules.push("annotate");
    m_system::emit(&mut out);
    modules.push("system");
    let mut src_out = SrcOut::new();
    t_ai::emit(&mut src_out);
    t_debug::emit(&mut src_out);
    out.insert("modules.txt".to_string(), format!("{}\n", modules.join("\n")).into_bytes());

    let mut existing = Vec::new();
    walk(&golden, &golden, &mut existing);
    let stale: Vec<&String> = existing.iter().filter(|p| !out.contains_key(*p)).collect();

    if check {
        let mut bad = 0usize;
        for (rel, bytes) in &out {
            match std::fs::read(golden.join(rel)) {
                Ok(have) if have == *bytes => {}
                Ok(_) => {
                    eprintln!("DIFFERS  tests/golden/{rel}");
                    bad += 1;
                }
                Err(_) => {
                    eprintln!("MISSING  tests/golden/{rel}");
                    bad += 1;
                }
            }
        }
        for rel in &stale {
            eprintln!("STALE    tests/golden/{rel} (no emitter writes it)");
            bad += 1;
        }
        for (rel, bytes) in &src_out {
            match std::fs::read(repo_root().join(rel)) {
                Ok(have) if have == *bytes => {}
                _ => {
                    eprintln!("DIFFERS  {rel} (generated source)");
                    bad += 1;
                }
            }
        }
        if bad > 0 {
            eprintln!("oracle --check: {bad} file(s) out of date; regenerate with the command in any .tsv header");
            std::process::exit(1);
        }
        println!("oracle --check: {} files up to date", out.len() + src_out.len());
    } else {
        for rel in &stale {
            std::fs::remove_file(golden.join(rel)).expect("remove stale");
        }
        for (rel, bytes) in &out {
            let p = golden.join(rel);
            std::fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
            std::fs::write(&p, bytes).expect("write");
        }
        for (rel, bytes) in &src_out {
            std::fs::write(repo_root().join(rel), bytes).expect("write generated source");
        }
        println!("oracle: wrote {} files ({} stale removed)", out.len() + src_out.len(), stale.len());
    }
}
