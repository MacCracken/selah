# Selah — Claude Code Instructions

## Project Identity

**Selah** (Hebrew: pause/capture a moment) — AI-native screenshot capture, annotation, and redaction library for AGNOS

- **Type**: Flat library crate
- **License**: GPL-3.0
- **MSRV**: 1.89
- **Version**: SemVer 0.D.M pre-1.0

## Consumers

taswir (GUI/CLI app), aethersafta (compositing), kiran (game engine, planned)

## Architecture

```
selah/src/
├── lib.rs        Crate root — re-exports, facade API (annotate_image, redact_image, decode_image_data)
├── core.rs       Types — Annotation, AnnotationKind, ImageFormat, Color, Screenshot, Monitor
├── geometry.rs   Rect wrapper over hisab::Rect, backward-compatible serde
├── error.rs      SelahError (#[non_exhaustive])
├── capture.rs    CaptureClient — async reqwest to daimon API, clipboard, session detection
├── history.rs    HistoryStore — JSONL persistence with file locking
├── annotate.rs   AnnotationCanvas — pixel rendering (ranga), SVG output, format conversion
├── ai.rs         PII detection, OCR stub, smart crop suggestions
├── daimon.rs     DaimonClient, HooshClient — agent lifecycle, LLM vision OCR
└── mcp.rs        MCP server via bote (feature-gated: "mcp")
```

## Ecosystem Dependencies

| Need | Use | NOT |
|------|-----|-----|
| Vectors, geometry | `hisab` (wraps glam) | `glam` directly |
| Image pixel ops | `ranga` (pixel buffers, blend) | Manual pixel math |
| MCP protocol | `bote` (JSON-RPC, tool registry) | Custom JSON-RPC |
| Image format I/O | `image` (codecs) | — (ranga lacks codecs) |

## Development Process

### P(-1): Scaffold Hardening (before any new features)

1. Test + benchmark sweep of existing code
2. Cleanliness check: `cargo fmt --check`, `cargo clippy --all-features --all-targets -- -D warnings`, `cargo audit`, `cargo deny check`
3. Get baseline benchmarks (`./scripts/bench-history.sh`)
4. Initial refactor + audit (performance, memory, security, edge cases)
5. Cleanliness check — must be clean after audit
6. Additional tests/benchmarks from observations
7. Post-audit benchmarks — prove the wins
8. Repeat audit if heavy

### Development Loop (continuous)

1. Work phase — new features, roadmap items, bug fixes
2. Cleanliness check: `cargo fmt --check`, `cargo clippy --all-features --all-targets -- -D warnings`, `cargo audit`, `cargo deny check`
3. Test + benchmark additions for new code
4. Run benchmarks (`./scripts/bench-history.sh`)
5. Audit phase — review performance, memory, security, throughput, correctness
6. Cleanliness check — must be clean after audit
7. Deeper tests/benchmarks from audit observations
8. Run benchmarks again — prove the wins
9. If audit heavy → return to step 5
10. Documentation — update CHANGELOG, roadmap, docs
11. Return to step 1

### Key Principles

- **Never skip benchmarks.** Numbers don't lie. The CSV history is the proof.
- **Tests + benchmarks are the way.** Minimum 80%+ coverage target.
- **Own the stack.** If an AGNOS crate wraps an external lib, depend on the AGNOS crate.
- **No magic.** Every operation is measurable, auditable, traceable.
- **`#[non_exhaustive]`** on all public enums.
- **`#[must_use]`** on all pure functions.
- **`#[inline]`** on hot-path functions.
- **`write!` over `format!`** — avoid temporary allocations.
- **Feature-gate optional deps** — consumers pull only what they need.
- **tracing on all operations** — structured logging for audit trail.

## DO NOT
- **Do not commit or push** — the user handles all git operations (commit, push, tag)
- **NEVER use `gh` CLI** — use `curl` to GitHub API only
- Do not add unnecessary dependencies — keep it lean
- Do not `unwrap()` or `panic!()` in library code
- Do not skip benchmarks before claiming performance improvements
- Do not commit `target/` or `Cargo.lock` (library crates only)
