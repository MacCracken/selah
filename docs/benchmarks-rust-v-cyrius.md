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
| `suggest_redactions_clean` | 392 ns | 2.46 µs | 6.3× | M2 | each call's results in an arena reset after it (Rust frees its Vec); the tokenizer skips printable ASCII before the Unicode whitespace test (was 3.13 µs) |
| `suggest_redactions_pii` | 461 ns | 2.60 µs | 5.6× | M2 | as above (was 3.52 µs) |
| `suggest_redactions_long` | 36.7 µs | 232 µs | 6.3× | M2 | as above (was 290 µs) |
| `extract_text_regions_64k` | 61.9 µs | 446 µs | 7.2× | M2 | the result in an arena reset after it; plain runs are copied whole |
| `render_to_image_64x64` | 16.2 µs | — | | M4 | |
| `render_to_image_512_bmp` | 1.29 ms | — | | M4 | |
| `to_svg_100` | 40.0 µs | — | | M4 | |

### M3 codecs (Cyrius-only benches; Rust's numbers from `oracle --bench`)

Rust's criterion suite has no codec benches, so these have no `bench-history.csv` row. `scripts/oracle -- --bench`
times Rust selah 0.30.0's own paths on the same 512 × 512 RGBA image `tests/selah.bcyr` builds (`(x ^ y, 3x + y,
5y ^ x, 255) mod 256`), decoding the very files the Cyrius bench wrote (`build/bench/`). Each figure is a mean over
the iterations shown in the bench, measured 2026-11-14 on the same machine.

| Bench | Rust | Cyrius | Ratio | Notes |
|---|---|---|---|---|
| `encode_png_512` | 10.4 ms | 58.9 ms | 5.6× | kii's filter heuristic and sankoch's level-6 deflate (Rust: png 0.18.1's defaults) |
| `encode_bmp_512` | 2.14 ms | 2.45 ms | 1.1× | |
| `encode_jpeg_512` | 12.9 ms | 32.5 ms | 2.5× | 4:4:4 q75; the Cyrius encoder computes to the exact mathematics (ADR 0002 #17) |
| `decode_png_512` | 1.31 ms | 24.9 ms | 19× | chitra 1.3.1; filed (L18) |
| `decode_jpeg_512` | 2.54 ms | 59.5 ms | 23× | chitra 1.3.1: bit-serial Huffman decoding; filed (L18) |
| `convert_format_png_to_jpeg_512` | 14.5 ms | 59.8 ms | 4.1× | decode + encode |
