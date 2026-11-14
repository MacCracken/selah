# selah benchmarks — Rust 0.30.0 against the Cyrius port

The Cyrius benches (`tests/selah.bcyr`, `cyrius bench`) carry the Rust criterion suite's names and inputs
(`rust-old/benches/benchmarks.rs`), so each row compares the same work. Rows join as their milestone ports the code.

- **Rust:** criterion medians from `rust-old/bench-history.csv` at `c0da3e1` (2026-11-14, rustc 1.100.0, release
  profile), the 0.30.0 baseline taken at M0 step 0.
- **Cyrius:** `cyrius bench` on the same machine (Cyrius 6.7.18, x86_64), per-op mean.
- Cyrius does not inline, and every call is a real call, so tiny operations show the largest ratios. The sibling ports
  measure the same shape: kimiya's median is 7.7×, tanmatra's 2–20×.

| Bench | Rust | Cyrius | Ratio | Milestone | Notes |
|---|---|---|---|---|---|
| `rect_new` | 1.93 ns | 10.9 ns | 5.7× | M1 | |
| `rect_contains_point` | 1.20 ns | 20.9 ns | 17× | M1 | the normalized bounds computed inline in f64 (was 48 ns through a normalized struct) |
| `rect_intersects` | 0.99 ns | 31.6 ns | 32× | M1 | as contains_point (was 84 ns) |
| `xml_escape_plain` | 21.2 ns | 110 ns | 5.2× | M1 | Cyrius escapes into one reused builder; Rust allocates a `String` per call (a bump heap would grow by the whole run's output) |
| `xml_escape_special` | 129 ns | 874 ns | 6.8× | M1 | as above |
| `xml_escape_long` | 604 ns | 8.04 µs | 13× | M1 | as above; plain runs are scanned through a 256-byte table and copied whole (was 42.9 µs) |
| `suggest_redactions_clean` | 392 ns | — | | M2 | |
| `suggest_redactions_pii` | 461 ns | — | | M2 | |
| `suggest_redactions_long` | 36.7 µs | — | | M2 | |
| `extract_text_regions_64k` | 61.9 µs | — | | M2 | |
| `render_to_image_64x64` | 16.2 µs | — | | M4 | |
| `render_to_image_512_bmp` | 1.29 ms | — | | M4 | |
| `to_svg_100` | 40.0 µs | — | | M4 | |
