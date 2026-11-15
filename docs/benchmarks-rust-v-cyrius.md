# selah benchmarks — Rust 0.30.0 against the Cyrius port

The Cyrius benches (`tests/selah.bcyr`, `cyrius bench`) carry the Rust criterion suite's names and inputs
(`rust-old/benches/benchmarks.rs`), so each row compares the same work. Rows join as their milestone ports the code.

- **Rust:** criterion medians from `rust-old/bench-history.csv` at `c0da3e1` (2026-11-14, rustc 1.100.0, release
  profile), the 0.30.0 baseline taken at M0 step 0.
- **Cyrius:** `cyrius bench` on the same machine (Cyrius 6.7.18, x86_64), per-op mean.
- **History:** `scripts/bench-history.sh` appends every Cyrius bench to `bench-history.csv`. The series starts at
  1.0.0 (2026-11-14, load average 3.5–5.2 on 16 cores); 1.0.1's rows follow (chitra 1.4.1: `decode_jpeg_512`
  55.8 → 14.5 ms, `mcp_annotate_8x8` 281 → 248 µs). `rust-old/bench-history.csv` is Rust's, frozen.
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
| `render_to_image_64x64` | 16.2 µs | 995 µs | 61× | M4 | decode + draw + PNG encode of 64 × 64: sankoch's ~110 µs per compression (L20), chitra's ~200 µs per small decode (L18), the PNG filter (its loop was 22 ns/byte, now ~15) |
| `render_to_image_512_bmp` | 1.29 ms | 10.6 ms | 8.2× | M4 | BMP in and out, so the drawing dominates; the ellipse's 1,600 sin / cos pairs go through fdlibm |
| `to_svg_100` | 40.0 µs | 201 µs | 5.0× | M4 | an integer fast path in the f32 printer (was 380 µs: 684 ns a coordinate through the general shortest-digits path) |

### M3 codecs (Cyrius-only benches; Rust's numbers from `oracle --bench`)

Rust's criterion suite has no codec benches, so these have no `bench-history.csv` row. `scripts/oracle -- --bench`
times Rust selah 0.30.0's own paths on the same 512 × 512 RGBA image `tests/selah.bcyr` builds (`(x ^ y, 3x + y,
5y ^ x, 255) mod 256`), decoding the very files the Cyrius bench wrote (`build/bench/`).

**Re-measured for 1.0.1** (2026-11-14), both sides back to back on one machine at load average 2.4–3.8 on 16 cores.
Each figure is the fastest of three runs: three `oracle --bench` runs (stable to 0.3 % on decode), and three
`cyrius bench` runs interleaved with 1.0.0's. The M3 measurement had taken Rust's column under load, about 2× slow
(encode PNG 10.4 ms, decode JPEG 2.54 ms), so every M3 ratio looked better than it was.

| Bench | Rust | Cyrius 1.0.1 | Ratio | Cyrius 1.0.0 | Notes |
|---|---|---|---|---|---|
| `encode_png_512` | 4.91 ms | 47.6 ms | 9.7× | 47.4 ms | kii's filter heuristic and sankoch's level-6 deflate (Rust: png 0.18.1's defaults). M4 tightened the filter loop (59 → 49 ms); about 21 ms is sankoch's deflate, at about the same time on every level (L20) |
| `encode_bmp_512` | 1.21 ms | 2.19 ms | 1.8× | 2.22 ms | |
| `encode_jpeg_512` | 6.36 ms | 28.5 ms | 4.5× | 28.1 ms | 4:4:4 q75; the Cyrius encoder computes to the exact mathematics (ADR 0002 #17) |
| `decode_png_512` | 0.92 ms | 15.5 ms | 17× | 19.3 ms | chitra 1.4.1 (1.0.0: chitra 1.3.1). 5.7 ms of it is sankoch's inflate, 12× fdeflate's on the same stream (filed with sankoch, L18) |
| `decode_jpeg_512` | 1.29 ms | 14.8 ms | 11× | 48.9 ms | chitra 1.4.1: a Huffman lookahead table, an inlined IDCT, the DC-only block path (L18) |
| `convert_format_png_to_jpeg_512` | 7.14 ms | 44.4 ms | 6.2× | 47.0 ms | decode + encode |

- **The allocator's share:** about 1 ms of each decode is the price of the memory coming back
  ([port-parity §11](development/port-parity.md)). Through the bump heap the same decodes take 14.85 ms (PNG) and
  14.08 ms (JPEG).
- **A 1080p screenshot PNG** decodes in 47.5 ms, against Rust's 1.30 ms. Of that, 31.4 ms is sankoch's inflate,
  49× fdeflate's 0.64 ms (`sankoch/docs/development/issues/2026-11-14-inflate-is-most-of-a-png-decode.md`).

### M5 history and the HTTP layer (Cyrius-only benches; Rust's numbers from `oracle --bench`)

Rust's criterion suite has none of these either. Both sides build the same inputs: entry i has id i, path
`/home/u/Pictures/shot-<i>.png`, second 1763164800 + i, nanosecond (i × 7919) mod 10^9, "full screen", 1920 × 1080,
png; the history file holds entries 0..1000 as serde_json writes them; the capture answer carries 1 MiB of base64.
Each figure is the median of three runs, measured 2026-11-14 on a machine under load (load average 6–10 on 16
cores; single runs varied by up to 2×).

| Bench | Rust | Cyrius | Ratio | Notes |
|---|---|---|---|---|
| `history_record` | 19.0 µs | 15.9 µs | 0.84× | open the lock file, flock, append one line, close; mostly system calls |
| `history_list_1000` | 0.774 ms | 8.45 ms | 11× | 1,000 lines read, parsed and ranked, the 10 newest kept. Per line: bayan's parse ~5 µs (L21), selah's pre-pass 0.75 µs (was 2.2: one pass, a word at a time), the entry ~2 µs; the scratch allocator now costs what an arena does (it cost 3.5 µs a line making a freelist call per allocation) |
| `history_get_1000` | 0.361 ms | 6.82 ms | 19× | 1,000 lines read for the id only, the last one matching |
| `history_delete_miss_1000` | 0.473 ms | 7.51 ms | 16× | the file read whole and every line's id parsed under the lock; nothing written |
| `capture_parse_1mb` | 0.195 ms | 18.8 ms | 96× | bayan's string parse is ~15 ms of it (L21); selah's pre-pass 0.72 ms, the copy out the rest (was 29 ms before the pre-pass was fused) |

### M6 the MCP server (Cyrius-only benches; Rust's numbers from `oracle --bench`)

Rust selah's `build_dispatcher` is private, so Rust can only be timed through its stdio server. The oracle starts
it (`--mcp-serve`) and times one request line out and its answer back. `tests/mcp/mcp.bcyr` times the same requests
the same way, a server child running bote's stdio loop on two pipes, and also in process through
`codec_process_message`. Both work in the system temp directory (a tmpfs here) with the files from
`tests/golden/mcp/`: `shot.png` (8 × 8 RGBA) and `history.jsonl` (5 entries). Each figure is the median of three runs,
measured 2026-11-14 at a load average of 3–5 on 16 cores.

| Bench | Rust (stdio) | Cyrius (stdio) | Ratio | Cyrius in process | Notes |
|---|---|---|---|---|---|
| `mcp_stdio_floor` | 12.0 µs | 15.1 µs | 1.3× | | a method neither server has, answered −32601: the round trip with almost no work |
| `mcp_tools_list` | 45 µs | 45.9 µs | 1.0× | 31.1 µs | the five definitions (about 3 KB of descriptions and schemas), written by bote |
| `mcp_annotate_8x8` | 66 µs | 289.5 µs | 4.4× | 256.6 µs | the render is 157 µs: sankoch's ~110 µs a compression (L20) and chitra's small-image decode (L18). Then bote's handling ~40 µs, the arguments' parse 11 µs, the atomic write 16 µs, the input's read and checks 8 µs |
| `mcp_history_list` | 37 µs | 112.3 µs | 3.0× | 103.4 µs | the store's list of 5 lines is ~55 µs, mostly bayan's parse (L21) and the lock; the payload ~15 µs, wrapping it as a JSON string 9 µs, bote ~17 µs |

What M6 changed, each measured before and after on the same bench:

- The JSON string writer (`_sl_json_str`, behind every JSON selah writes) finds plain runs a word at a time and
  assembles its output in a 4 KiB chunk; it called the builder once per byte. The 782-byte history payload: 16.9 →
  9.15 µs. 64 KiB of plain text: 1.31 → 0.285 ms.
- The scratch arena keeps one spare 64 KiB chunk between calls. The freelist maps anything over 4 KiB afresh, so
  each call mapped and unmapped a chunk (6.2 µs) and faulted its pages in again (~2 µs a page); now 0.18 µs. The
  annotate arguments' parse: 17.6 → 11.0 µs. The MCP calls in process: annotate 280 → 257 µs, history 112 → 103 µs.
- The MCP outputs and `CaptureClient::save_to_file` stay atomic but drop the fsync (ADR 0002 #18): 28.7 → 0.76 ms
  a file.
- The M5 rows re-measured with these changes show no regression (`history_record` 13.6 µs, `history_list_1000`
  7.43 ms, `capture_parse_1mb` 16.6 ms); the load was lighter than at M5, so the drops are not claimed as wins.
