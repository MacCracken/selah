# Rust-test ledger

> **Generated** by `python3 -I scripts/rust_test_ledger.py .`; `--check` gates it (port plan M7). Every
> test of the frozen Rust oracle (`rust-old/`, selah 0.30.0) is ported to a Cyrius test, or recorded here
> with its reason. The golden corpus (`tests/golden/`) checks every ported function on its grid besides;
> this ledger is about the oracle's own assertions.

**289 Rust tests:** 288 ported, 0 folded into golden rows, 1 not applicable, 0 deferred.

| Rust file | Cyrius suites | Rust tests | Ported | Golden | N/A | Deferred |
|---|---|---:|---:|---:|---:|---:|
| `src/error.rs` | `tests/error.tcyr` | 6 | 6 | 0 | 0 | 0 |
| `src/geometry.rs` | `tests/geometry.tcyr` | 19 | 19 | 0 | 0 | 0 |
| `src/core.rs` | `tests/core.tcyr` | 39 | 39 | 0 | 0 | 0 |
| `src/ai.rs` | `tests/ai.tcyr` | 39 | 39 | 0 | 0 | 0 |
| `src/annotate.rs` | `tests/annotate.tcyr`, `tests/codec.tcyr` | 44 | 44 | 0 | 0 | 0 |
| `src/fsutil.rs` | `tests/fsutil.tcyr` | 7 | 7 | 0 | 0 | 0 |
| `src/history.rs` | `tests/history.tcyr` | 18 | 18 | 0 | 0 | 0 |
| `src/http.rs` | `tests/http.tcyr` | 6 | 6 | 0 | 0 | 0 |
| `src/capture.rs` | `tests/capture.tcyr` | 19 | 19 | 0 | 0 | 0 |
| `src/daimon.rs` | `tests/daimon.tcyr` | 20 | 20 | 0 | 0 | 0 |
| `src/mcp.rs` | `tests/mcp/mcp.tcyr` | 41 | 40 | 0 | 1 | 0 |
| `tests/integration.rs` | `tests/integration.tcyr`, `tests/history.tcyr`, `tests/codec.tcyr` | 17 | 17 | 0 | 0 | 0 |
| doc tests | (below) | 14 | 14 | 0 | 0 | 0 |
| **Total** | | **289** | **288** | **0** | **1** | **0** |

## Doc tests

- `rust-old/src/ai.rs`, `ai::suggest_redactions`: `tests/ai.tcyr`, group "doctest ai::suggest_redactions".
- `rust-old/src/annotate.rs`, `annotate::AnnotationCanvas::new`: `tests/annotate.tcyr`, group "doctest AnnotationCanvas::new".
- `rust-old/src/annotate.rs`, `annotate::convert_format`: `tests/codec.tcyr`, group "doctest convert_format".
- `rust-old/src/core.rs`, `core::Annotation::new`: `tests/core.tcyr`, group "doctests core".
- `rust-old/src/core.rs`, `core::ImageFormat::extension`: `tests/core.tcyr`, group "doctests core".
- `rust-old/src/core.rs`, `core::ImageFormat::from_path`: `tests/core.tcyr`, group "doctests core".
- `rust-old/src/core.rs`, `core::derive_output_path`: `tests/core.tcyr`, group "doctests core".
- `rust-old/src/core.rs`, `core::xml_escape`: `tests/core.tcyr`, group "doctests core".
- `rust-old/src/geometry.rs`, `geometry::Rect::new`: `tests/geometry.tcyr`, group "doctest geometry::Rect".
- `rust-old/src/geometry.rs`, `geometry::Rect::normalized`: `tests/geometry.tcyr`, group "doctest geometry::Rect".
- `rust-old/src/history.rs`, `history::HistoryStore::open`: `tests/history.tcyr`, group "doctest: HistoryStore::open".
- `rust-old/src/lib.rs`, `annotate_image`: `tests/integration.tcyr`, group "doctests lib".
- `rust-old/src/lib.rs`, `decode_image_data`: `tests/integration.tcyr`, group "doctests lib".
- `rust-old/src/lib.rs`, `redact_image`: `tests/integration.tcyr`, group "doctests lib".

## Tests without a port

| Rust file | Test | Status |
|---|---|---|
| `src/mcp.rs` | `test_run_async_inside_current_thread_runtime` | n/a: it checks that a sync handler can block on a future inside a current-thread tokio runtime; the port's clients are blocking (ADR 0002 #8) and there is no runtime to nest |
