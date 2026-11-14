//! `oracle` — drives the frozen Rust selah 0.30.0 in `rust-old/` and writes the Cyrius port's golden corpus
//! under `tests/golden/` (port plan §2 step 5). Format 1 is specified in `scripts/oracle/README.md`.
//!
//! ```sh
//! cargo run --release --locked --manifest-path scripts/oracle/Cargo.toml             # write
//! cargo run --release --locked --manifest-path scripts/oracle/Cargo.toml -- --check  # verify; exit 1 on any difference
//! ```

mod fixtures;
mod m_decode;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every file the oracle owns, keyed by its path under `tests/golden/`.
pub type Out = BTreeMap<String, Vec<u8>>;

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

/// Escape a string token's payload: backslash, tab, newline, carriage return, and space (tokens are
/// space-separated), so a token never contains raw whitespace.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\t' => o.push_str("\\t"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            ' ' => o.push_str("\\s"),
            c => o.push(c),
        }
    }
    o
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
    let check = std::env::args().any(|a| a == "--check");
    let golden = repo_root().join("tests/golden");

    let mut out = Out::new();
    let fixtures = fixtures::build();
    fixtures::emit(&mut out, &fixtures);
    let mut modules = Vec::new();
    m_decode::emit(&mut out, &fixtures);
    modules.push("decode");
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
        if bad > 0 {
            eprintln!("oracle --check: {bad} file(s) out of date; regenerate with the command in any .tsv header");
            std::process::exit(1);
        }
        println!("oracle --check: {} files up to date", out.len());
    } else {
        for rel in &stale {
            std::fs::remove_file(golden.join(rel)).expect("remove stale");
        }
        for (rel, bytes) in &out {
            let p = golden.join(rel);
            std::fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
            std::fs::write(&p, bytes).expect("write");
        }
        println!("oracle: wrote {} files ({} stale removed)", out.len(), stale.len());
    }
}
