"""The Rust-test ledger: every one of the oracle's tests is ported, or recorded with a reason.

Port plan M7 (tanmatra's script, after mela). Usage, from the repo root:
    python3 -I scripts/rust_test_ledger.py .           # write docs/development/rust-test-ledger.md
    python3 -I scripts/rust_test_ledger.py . --check   # exit 1 if the ledger is stale or a test is unaccounted

It lists each `#[test]` / `#[tokio::test]` in rust-old/src/*.rs and rust-old/tests/*.rs, and each doc test, and
looks for its port: a `test_group("...")` in one of the Cyrius suites that Rust file's tests go to (FILES). A group
title names its Rust test: `test_x`, or `core::test_x` when two Rust modules share the name, or several at once as
`test_x_a / _b / _c` (each `_b` completes a prefix of the first name to another Rust test), with an optional
` (comment)` after. A Rust test with no port must have an entry in REASONS, and a REASONS entry for a test that is
ported, or that does not exist, is an error. So is a group titled like a Rust test that does not exist.
"""
import re, sys
from pathlib import Path

root = Path(sys.argv[1]).resolve()
check = "--check" in sys.argv[2:]
out_path = root / "docs/development/rust-test-ledger.md"

# Rust file -> the Cyrius suites its tests are ported into.
FILES = {
    "src/error.rs": ["tests/error.tcyr"],
    "src/geometry.rs": ["tests/geometry.tcyr"],
    "src/core.rs": ["tests/core.tcyr"],
    "src/ai.rs": ["tests/ai.tcyr"],
    "src/annotate.rs": ["tests/annotate.tcyr", "tests/codec.tcyr"],
    "src/fsutil.rs": ["tests/fsutil.tcyr"],
    "src/history.rs": ["tests/history.tcyr"],
    "src/http.rs": ["tests/http.tcyr"],
    "src/capture.rs": ["tests/capture.tcyr"],
    "src/daimon.rs": ["tests/daimon.tcyr"],
    "src/mcp.rs": ["tests/mcp/mcp.tcyr"],
    "tests/integration.rs": ["tests/integration.tcyr", "tests/history.tcyr", "tests/codec.tcyr"],
}

# The doc tests: (Rust file, item, as cargo lists it) -> (Cyrius suite, group title).
DOC_TESTS = {
    ("src/ai.rs", "ai::suggest_redactions"): ("tests/ai.tcyr", "doctest ai::suggest_redactions"),
    ("src/annotate.rs", "annotate::AnnotationCanvas::new"): ("tests/annotate.tcyr", "doctest AnnotationCanvas::new"),
    ("src/annotate.rs", "annotate::convert_format"): ("tests/codec.tcyr", "doctest convert_format"),
    ("src/core.rs", "core::Annotation::new"): ("tests/core.tcyr", "doctests core"),
    ("src/core.rs", "core::ImageFormat::extension"): ("tests/core.tcyr", "doctests core"),
    ("src/core.rs", "core::ImageFormat::from_path"): ("tests/core.tcyr", "doctests core"),
    ("src/core.rs", "core::derive_output_path"): ("tests/core.tcyr", "doctests core"),
    ("src/core.rs", "core::xml_escape"): ("tests/core.tcyr", "doctests core"),
    ("src/geometry.rs", "geometry::Rect::new"): ("tests/geometry.tcyr", "doctest geometry::Rect"),
    ("src/geometry.rs", "geometry::Rect::normalized"): ("tests/geometry.tcyr", "doctest geometry::Rect"),
    ("src/history.rs", "history::HistoryStore::open"): ("tests/history.tcyr", "doctest: HistoryStore::open"),
    ("src/lib.rs", "annotate_image"): ("tests/integration.tcyr", "doctests lib"),
    ("src/lib.rs", "decode_image_data"): ("tests/integration.tcyr", "doctests lib"),
    ("src/lib.rs", "redact_image"): ("tests/integration.tcyr", "doctests lib"),
}

# (Rust file, test) -> why it has no port.
REASONS = {
    ("src/mcp.rs", "test_run_async_inside_current_thread_runtime"): (
        "n/a: it checks that a sync handler can block on a future inside a current-thread tokio runtime; the port's "
        "clients are blocking (ADR 0002 #8) and there is no runtime to nest"),
}

TEST_ATTR = re.compile(r"^\s*#\[(?:tokio::)?test(?:\(.*\))?\]\s*$")


def rust_tests(rel):
    lines = (root / "rust-old" / rel).read_text().split("\n")
    names = []
    for i, line in enumerate(lines):
        if TEST_ATTR.match(line):
            for nxt in lines[i + 1 : i + 6]:
                m = re.match(r"\s*(?:pub )?(?:async )?fn (\w+)", nxt)
                if m:
                    names.append(m.group(1))
                    break
    return names


def rust_doc_tests():
    """(file, item) of each doc test, found as cargo finds them: a ``` fence in a /// or //! block (not `text`,
    `ignore`, `no_run` is still compiled and counted)."""
    found = []
    for path in sorted((root / "rust-old/src").glob("*.rs")):
        rel = f"src/{path.name}"
        in_code = False
        for line in path.read_text().split("\n"):
            s = line.strip()
            if not (s.startswith("///") or s.startswith("//!")):
                in_code = False
                continue
            body = s[3:].strip()
            if body.startswith("```"):
                if not in_code:
                    lang = body[3:].strip()
                    in_code = True
                    if lang in ("", "rust", "no_run", "rust,no_run", "should_panic", "compile_fail"):
                        found.append(rel)
                else:
                    in_code = False
    return found


def groups(rel):
    return re.findall(r'test_group\("((?:[^"\\]|\\.)*)"\)', (root / rel).read_text())


def keys_of(title, rust_names):
    """The (module or None, test name) keys a group title names."""
    t = re.sub(r"\s+\(.*\)$", "", title)
    parts = [p.strip() for p in t.split(" / ")]
    first = parts[0]
    mod = None
    if "::" in first:
        mod, first = first.split("::", 1)
    if not re.fullmatch(r"test_\w+", first):
        return []
    keys = [(mod, first)]
    for suf in parts[1:]:
        if not suf.startswith("_"):
            return keys
        cands = [first[:k] + suf for k in range(len(first)) if first[k] == "_"]
        hit = [c for c in cands if c in rust_names]
        keys.append((mod, hit[-1] if hit else first + suf))
    return keys


all_rust = {rel: rust_tests(rel) for rel in FILES}
all_names = {n for names in all_rust.values() for n in names}
errors = []
rows = []        # (rust file, cyrius suites, tests, counts)
notes = []       # (rust file, test, reason)
claimed = set()  # (cyrius suite, module, name) keys that matched a Rust test
for rel, suites in FILES.items():
    tests = all_rust[rel]
    stem = Path(rel).stem
    keyset = {}
    for cy in suites:
        for title in groups(cy):
            for k in keys_of(title, all_names):
                keyset.setdefault(k, cy)
    counts = {"ported": 0, "n/a": 0, "deferred": 0, "golden": 0}
    for t in tests:
        hit = keyset.get((stem, t)) or keyset.get((None, t))
        if hit:
            counts["ported"] += 1
            claimed.add((hit, t))
            if (rel, t) in REASONS:
                errors.append(f"{rel}: {t} is ported but has a REASONS entry")
            continue
        reason = REASONS.get((rel, t))
        if reason is None:
            errors.append(f"{rel}: {t} is neither ported nor recorded")
            continue
        counts[reason.split(":")[0]] += 1
        notes.append((rel, t, reason))
    for (r, t) in REASONS:
        if r == rel and t not in tests:
            errors.append(f"{rel}: REASONS names {t}, which is not a test")
    rows.append((rel, suites, len(tests), counts))

# A group titled like a Rust test must name one.
for cy in sorted({c for s in FILES.values() for c in s}):
    for title in groups(cy):
        for mod, name in keys_of(title, all_names):
            if name not in all_names:
                errors.append(f"{cy}: group {title!r} names {name}, which is not a Rust test")

doc_found = rust_doc_tests()
for rel in sorted(set(doc_found)):
    want = doc_found.count(rel)
    have = sum(1 for (r, _) in DOC_TESTS if r == rel)
    if want != have:
        errors.append(f"doc tests in rust-old/{rel}: {want} in the source, {have} in DOC_TESTS")
for (rel, item), (cy, title) in DOC_TESTS.items():
    if title not in groups(cy):
        errors.append(f"doc test {rel} ({item}): {cy} has no group {title!r}")

n_unit = sum(r[2] for r in rows)
total = n_unit + len(DOC_TESTS)
sums = {k: sum(r[3][k] for r in rows) for k in ("ported", "n/a", "deferred", "golden")}
sums["ported"] += len(DOC_TESTS)

md = []
md.append("# Rust-test ledger")
md.append("")
md.append("> **Generated** by `python3 -I scripts/rust_test_ledger.py .`; `--check` gates it (port plan M7). Every")
md.append("> test of the frozen Rust oracle (`rust-old/`, selah 0.30.0) is ported to a Cyrius test, or recorded here")
md.append("> with its reason. The golden corpus (`tests/golden/`) checks every ported function on its grid besides;")
md.append("> this ledger is about the oracle's own assertions.")
md.append("")
md.append(f"**{total} Rust tests:** {sums['ported']} ported, {sums['golden']} folded into golden rows, "
          f"{sums['n/a']} not applicable, {sums['deferred']} deferred.")
md.append("")
md.append("| Rust file | Cyrius suites | Rust tests | Ported | Golden | N/A | Deferred |")
md.append("|---|---|---:|---:|---:|---:|---:|")
for rel, suites, n, c in rows:
    cys = ", ".join(f"`{s}`" for s in suites)
    md.append(f"| `{rel}` | {cys} | {n} | {c['ported']} | {c['golden']} | {c['n/a']} | {c['deferred']} |")
md.append(f"| doc tests | (below) | {len(DOC_TESTS)} | {len(DOC_TESTS)} | 0 | 0 | 0 |")
md.append(f"| **Total** | | **{total}** | **{sums['ported']}** | **{sums['golden']}** | **{sums['n/a']}** | "
          f"**{sums['deferred']}** |")
md.append("")
md.append("## Doc tests")
md.append("")
for (rel, item), (cy, title) in DOC_TESTS.items():
    md.append(f"- `rust-old/{rel}`, `{item}`: `{cy}`, group \"{title}\".")
md.append("")
md.append("## Tests without a port")
md.append("")
md.append("| Rust file | Test | Status |")
md.append("|---|---|---|")
for rel, t, reason in notes:
    md.append(f"| `{rel}` | `{t}` | {reason} |")
md.append("")
text = "\n".join(md)

for e in errors:
    print("LEDGER", e)
if check:
    old = out_path.read_text() if out_path.exists() else ""
    if old != text:
        print(f"STALE    {out_path.relative_to(root)} (regenerate: python3 -I scripts/rust_test_ledger.py .)")
        errors.append("stale")
else:
    out_path.write_text(text)
print(f"rust-test ledger: {total} tests ({n_unit} #[test], {len(DOC_TESTS)} doc tests): {sums['ported']} ported, "
      f"{sums['golden']} golden, {sums['n/a']} n/a, {sums['deferred']} deferred; {len(errors)} problem(s)")
sys.exit(1 if errors else 0)
