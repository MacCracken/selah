# Changelog

All notable changes to Selah will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

**selah is becoming a Cyrius library.** 1.0.0 ports the Rust crate 0.30.0, which is frozen in `rust-old/` as the
reference oracle ([port plan](docs/development/selah-port-plan.md), [ADR 0001](docs/adr/0001-cyrius-port-with-rust-oracle.md)).
Accuracy is the bar, and Rust is the floor, not behaviour to imitate ([ADR 0003](docs/adr/0003-accuracy-is-the-bar.md)).
The Rust crate stays at 0.30.x on crates.io.

### Port — M0 (initialize)

- **Oracle frozen**: 0.30.0 (tag `0.30.0`, `1a494b4`) moved to `rust-old/` by `cyrius port` on Cyrius 6.7.18. It is
  green from there: 289 tests. `rust-old/Cargo.lock` is tracked (`.gitignore` ignores only `/Cargo.lock`), so the
  oracle builds against its released dependency versions. The Rust CI, scripts, development guide and architecture
  overview moved to `rust-old/`.
- **Scaffold repaired** (cyrius issue `2026-11-08-cyrius-port-clobbers-project-docs-and-omits-bench`):
  - `CLAUDE.md` merged from the Rust rules and the scaffold's Cyrius rules;
  - the roadmap restored, with the port milestones on top;
  - `state.md` written for the real version;
  - `cyrius.cyml` takes `version = "${file:VERSION}"`, and `[test] stdlib` has `bench`;
  - the six stub functions are documented, so `cyrius audit` passes.
- **Manifest**:
  - chitra 1.3.1 and ranga 2.4.0;
  - bote 3.8.15, optional behind `[features] mcp`;
  - sankoch through chitra's zlib profile, not the full fold.
  - `lib/` and `cyrius.lock` are vendored by `cyrius deps --features mcp`.
- **CI**: the scaffold's Cyrius workflow, extended with `test --bounds`, both `deps --locked` profiles, `audit`, and
  a no-`@unsafe` check on `src/`.
- **ADRs**: [0001](docs/adr/0001-cyrius-port-with-rust-oracle.md) (the port and its oracle) and
  [0002](docs/adr/0002-port-api-divergences.md) (the API changes, each with its parity comparison).
- **Toolchain probes**: f32 rect arithmetic is bit-identical to Rust; small values through `Result`; chitra,
  sankoch and bayan in one unit; bote's dispatcher; sandhi against a mock server; the error-detail design. Results
  are in the port plan §2. Upstream:
  - daimon and hoosh issues filed for the routes selah needs;
  - a data point appended to sandhi's `total_ms` issue.
- **Golden generator** (`scripts/oracle/`):
  - a Rust crate on `rust-old/` by path, with its lock seeded from `rust-old/Cargo.lock` (every shared crate at the
    oracle's version);
  - deterministic output, with `--check`;
  - format 1 is specified in its README;
  - the first module, `decode`: 63 fixtures (PNG in every colour type and depth, Adam7, tRNS; BMP in palette, RLE,
    bitfield, top-down and V4 forms; JPEG baseline at every subsampling, with restart intervals, progressive and
    CMYK; WebP; GIF; malformed and oversized inputs), with Rust 0.30.0's verdict and RGBA through `annotate_image`.
- **Parity harness** (`tests/parity.tcyr`, `tests/parity_core.cyr`): replays `tests/golden/modules.txt`. Each row
  ends ok, FAILED, measured, accepted (ADR 0002) or pending; the exit code is the number of failing modules.
- **Decode baseline** ([port-parity.md](docs/development/port-parity.md)), chitra 1.3.1 against Rust's decoders:
  - every PNG and most BMPs bit-identical, and every refusal agrees;
  - JPEG within 2 without chroma subsampling;
  - five findings for M3 to settle: chroma upsampling (up to 36), 16-bit BMP expansion (±1), 32-bit `BI_RGB` alpha,
    CMYK refused, and K_jpeg = 2.
  - Identical on x86_64 ELF, aarch64 (qemu) and x86_64 PE (wine).

### Port — M1 (foundation)

- **Modules** (`src/`), bundled as `dist/selah.cyr` through `[lib]`:
  - `error`: `SlError` codes; fallible functions take a trailing `detail_out = 0` for the message (port plan D7);
  - `log`: sakshi, bote's contract;
  - `fmtf`: the shortest round-trip digits of f32 and f64 in Rust's Display and serde_json's forms (impetus's
    Burger & Dybvig printer, generalised as pravash's);
  - `json`: serde_json's string escaping, and readers on bayan that accept and refuse what serde does (both of
    serde's enum forms, and the array form of a struct);
  - `uuid`: v4 ids, written hyphenated or simple, read in all four of `Uuid::parse_str`'s forms;
  - `rfc3339`: chrono's AutoSi writer and its relaxed parser, item by item;
  - `geometry`: `SlRect` in hisab's `{min, max}` f32 layout, so every operation rounds as Rust's does;
  - `core`: the enums, `xml_escape`, `derive_output_path`, `SlColor`, `SlRedactionTarget`, `SlCaptureSource`,
    `SlAnnotation`, each with its Display and serde JSON.
- **Golden corpus**: six new oracle modules (`error`, `fmtf`, `geometry`, `core`, `uuid`, `rfc3339`), 34,863 rows,
  every one class A and every one equal, on x86_64 ELF, aarch64 and x86_64 PE.
- **Ported Rust tests**: `tests/error.tcyr`, `tests/geometry.tcyr`, `tests/core.tcyr` (64 tests and 7 doctests, plus
  three Cyrius-only); `tests/selah.tcyr` includes `dist/selah.cyr` the way a consumer does.
- **Benches**: the six M1 benches against Rust in `docs/benchmarks-rust-v-cyrius.md` (5.2× to 32×).
- **Surface parity**: `scripts/surface_parity.py`, 34/34 for the ported modules; in CI with `distlib --check`.
- **ADR 0002** rows #14 (serde_json's error text is not reproduced; the verdict is) and #15 (rect accessors return the
  f32's exact f64, since Cyrius has no f32 return type).
- **Filed upstream**: cyrius `2026-11-14-chrono-rfc3339-has-no-fraction-or-offset` (ledger L5).

### Port — M2 (PII detection and smart crop)

- **`src/ai.cyr`**:
  - `selah_suggest_redactions`: emails, Luhn-checked cards (plain, dashed or space-grouped), IPv4 addresses and
    phones, each token reported once, in text order;
  - `selah_extract_text_regions`: the stub OCR;
  - `selah_suggest_smart_crop`: in f32, as Rust computes it;
  - the three result types with serde's JSON.

  Each list-returning function has an `_a` form that takes the allocator first, so a caller can keep a call's
  results in an arena (port plan D10).
- **`src/ai_data.cyr`, generated by the oracle**: Rust's `char::is_alphanumeric` as 858 code-point ranges. rustc
  1.100 carries Unicode 18.0.0 and the stdlib's `lib/unicode` 17.0.0, so only a table from Rust matches the email
  trim exactly.
- **Golden corpus**: `ai`, 8,451 rows (Rust's test inputs, a seeded PII generator, every image fixture through the
  OCR stub, crops up to `u32::MAX`), every one class A and equal on x86_64 ELF, aarch64 and x86_64 PE.
- **Ported Rust tests**: `tests/ai.tcyr` (39 tests and the doctest, plus one Cyrius-only test of the `_a` forms).
- **Benches**: the four M2 benches, at 5.6× to 7.2× Rust.
- **Surface parity**: 37/37.

### Port — M5 (system and clients)

- **`src/history.cyr`:** `SlHistoryEntry` and `SlHistoryStore` (open, open_default, record, list, get, delete).
  - One JSON line per entry, byte for byte as serde_json writes it, read as serde reads it; a line that doesn't read
    is skipped.
  - Writers take `flock` on `<history>.lock`, retried every 10 ms for 5 s; a torn last line is terminated before an
    append; `delete` rewrites the file atomically and keeps every line it cannot read.
  - `list` keeps the newest entries in a bounded heap (newest first, ties in file order), each line parsed in a
    scratch arena reset per line.
- **`src/http.cyr`:** requests over sandhi with `read_ms`, `write_ms` and `total_ms` (L10), Rust's status text
  (reason phrase, lossy UTF-8, the 512-byte cut), Rust's "too large" text, and endpoint URLs with the url crate's
  segment encoding.
- **`src/spawn.cyr`** (L3): a tool run with bytes on its stdin and its stderr kept, in one poll loop; SIGPIPE blocked
  and consumed through a signalfd.
- **`src/capture.cyr`:** `SlCaptureClient` (capture, capture_full, capture_region, capture_monitor, list_monitors,
  save_to_file, copy_to_clipboard), `RegionSpec`, session detection, the clipboard's MIME sniff from image 0.25.10's
  table. Core gained `SlMonitor` and `SlCaptureRegion`, and Rect's Debug form.
- **`src/daimon.cyr`:** the configs (with Rust's JSON and a Debug that never shows the key) and the clients,
  retargeted (D2): `register_agent` posts to daimon's `POST /v1/agents`; `ocr` asks hoosh's chat completions,
  downscaling and re-encoding the image as JPEG until the request fits hoosh's 1 MiB cap (L8), and scales the regions
  back. `heartbeat` is gone.
- **`src/scratch.cyr`:** a freelist arena handed to sandhi and bayan, so requests and parses leave nothing behind (a
  request with a 256 MiB cap leaves 736 bytes on the bump heap; a parse, none).
- **`src/json.cyr`**, for every reader: `-0` reads as −0.0 and an integer past i64 as the nearest double, as serde
  reads them (bayan read 0 and refused); a float literal past f64 is refused in a field selah reads; strings must be
  UTF-8. One pass, a word at a time through plain ASCII.
- **Rust's `{:?}` beyond ASCII:** `src/debug_data.cyr`, generated from Rust (914 ranges), makes every Debug text
  exact.
- **Divergences (ADR 0002):**
  - #21: a defect in a member serde ignores (a duplicate key, a lone surrogate, invalid UTF-8) is refused.
  - #22: the HTTP layer's own transport texts; sandhi's PROTOCOL error past its buffer; the endpoint kept as given.
  - #23: the clipboard ends when the tool exits, and a tool that leaves input unread is an error.
  - #1, #6, #9 and #18 extended: WebP capture refused; OCR needs a model, and a fenced answer is unwrapped;
    `with_timeout`; `save_to_file` atomic and 0600.
- **Golden:** a new `system` module, 662 rows (654 ok, 8 accepted), through Rust's own HistoryStore and clients
  against a loopback copy of `serve_once`. Rust lists every Cyrius-written history file (`oracle --reverse`).
  Identical on x86_64 ELF, aarch64 (qemu) and x86_64 PE (wine). Two mutation probes showed the gates bite.
- **Tests:**
  - `history.tcyr`: 18 Rust tests, the `HistoryStore::open` doctest and `test_history_store_integration`.
  - `http.tcyr`: 6 Rust tests, against a loopback mock server on a thread (`tests/mock_http.cyr`).
  - `capture.tcyr`: 19 Rust tests, and the spawn helper's cases; also run under qemu-aarch64.
  - `daimon.tcyr`: 20 Rust tests (the two heartbeat tests as one), and the OCR downscale end to end.
- **Benches** (new, with Rust's numbers from `oracle --bench`): `history_record` 0.84×, `history_list_1000` 11×,
  `history_get_1000` 19×, `history_delete_miss_1000` 16×, `capture_parse_1mb` 96×.
  - The scratch allocator went from a freelist call per allocation to an arena on the freelist (3.5 µs a history
    line to nothing), and the JSON pre-pass was fused into one word-at-a-time pass (2.2 to 0.75 µs a line; 29 to
    19 ms for the 1 MiB answer).
- **Filed:**
  - cyrius: no spawn that feeds stdin (L3);
  - bayan: the JSON parser's throughput (L21), and base64 encoding added to the allocator issue (L19);
  - sandhi: the whole response cap reserved per request, and over-cap reported as PROTOCOL (L22).

### Port — M4 (annotation)

- **`src/raster.cyr`:** Rust's rasterizer in Rust's arithmetic.
  - Rect geometry in f32, pixel bounds rounded outward, saturating float → int casts.
  - Fill, ranga `blend_row` for highlights, stroke, and stamp.
  - The ellipse, sampled at a bound of 2^20 points; lines clipped with Liang–Barsky, then Bresenham.
- **`src/annotate.cyr`:**
  - `SlAnnotationCanvas`: add, remove, clear, count, `apply_redaction`.
  - The layer file is serde_json's pretty print, byte for byte, read back as serde reads it.
  - `to_svg`, and `render_to_image`, which refuses NaN redactions first.
- **`src/selah.cyr`:** `selah_decode_image_data`, `selah_annotate_image`, `selah_redact_image`.
- **`src/fsutil.cyr`** (brought forward from M5): bounded reads that refuse non-regular files, atomic 0600 writes, and
  private directories.
- **Divergences (ADR 0002):**
  - #18: the layer file is written atomically and 0600.
  - #19, a Rust defect: Rust wrote a layer it could not load back when a region was not finite. The port refuses
    that save.
  - #20: the API shapes of ids, vecs and returned bytes.
- **Golden:** a new `annotate` module, 3,457 rows: render by CRC of the RGBA over every kind × 29 rect classes × 3
  colours on 1×1 to 512×512, SVG, layers both ways, base64 verdicts, and `redact_image`. All match.
  - Rust loads every Cyrius-written layer (`oracle --reverse`).
  - Two mutation probes showed the gates bite.
  - New fixtures: a 512×512 gradient, and a PNG carrying PII in tEXt.
- **Tests:**
  - `annotate.tcyr`: 38 Rust tests and the canvas doctest.
  - `integration.tcyr`: 14 integration tests and the three `lib.rs` doctests.
  - `fsutil.tcyr`: 7 Rust tests.
- **Benches:** `render_to_image_64x64` 61×, `render_to_image_512_bmp` 8.2×, `to_svg_100` 5.0× Rust.
  - M1's f32 printer gained an integer fast path, which took `to_svg` from 380 to 201 µs.
  - The PNG filter loop got tighter, which took `encode_png_512` from 59 to 49 ms.
- **Filed:**
  - bayan: base64 has no allocator form (L19);
  - sankoch: about 110 µs of LZ77 table reset per compression, and a level-blind rate (L20).

### Port — M3 (codecs)

- **`src/codec.cyr`:**
  - `SlImage` (RGBA8, pixels owned by selah or by chitra), `selah_convert_format`, `selah_bytes_free`, and
    `SL_MAX_IMAGE_DIMENSION` / `SL_MAX_DECODE_ALLOC`.
  - **Decoding:** the signature is sniffed (GIF stays refused). A header probe enforces Rust's limits, the larger of
    the native and RGBA buffers included, before chitra allocates. chitra's errors map to `AnnotationError`, and
    progressive and CMYK JPEG to `UnsupportedFormat`.
  - A 32-bpp `BI_RGB` BMP decodes opaque, as the format defines it (L11).
- **Encoders:**
  - `src/png_enc.cyr`: kii's PNG writer on the freelist.
  - `src/bmp_enc.cyr`: a V4 header, BI_BITFIELDS with alpha, the layout `image` writes for RGBA.
  - `src/jpeg_enc.cyr`: baseline, 4:4:4, quality 75 like Rust. Its coefficients are computed to the mathematics:
    20-bit FDCT constants, 6 fraction bits of YCbCr, round to nearest. 0.008 % of them differ from an exact f64
    evaluation, against 0.56 % for `image` 0.25.10, whose FDCT biases its odd outputs (ADR 0002 #17).
- **Gates:**
  - `decode` is gated under ADR 0003: class A, plus class B accuracy and conformance.
  - New in the harness: the `upstream` outcome and its known-defect list, exact `adr=0002#N` parsing with the error
    variant checked, and `c:` CRC tokens.
  - New `codec` module: 252 `convert_format` and 212 `encode` rows. PNG and BMP are lossless under both decoders.
    `scripts/oracle -- --reverse` has Rust decode every Cyrius-encoded file, and `-- --bench` gives Rust's codec
    numbers.
- **The JPEG encoder's bound changed after its first comparison.** It started per image, "no worse than Rust's own
  JPEG + 0.050 mean / + 2 max", and 7 synthetic-noise images failed it. Two correct encoders differ there by
  rounding noise: −1.28 to +0.55 dB per image, while the port is ahead over the corpus, 28.542 against 28.531 dB.
  It now gates on the exact mathematics instead, which the maintainer approved on 2026-11-14. The history is in
  port-parity.md §5.
- **Tests:** `tests/codec.tcyr`: 8 Rust tests and the `convert_format` doctest, plus round trips, the F4 guard and
  the refusal codes (57 assertions). The bundle smoke test converts PNG to BMP.
- **Benches:** six codec benches. Encoders run at 1.1–5.6× Rust; chitra's decoding at 19–23× (L18).
- **Filed with chitra:**
  - every decode's memory is kept for the life of the process (L16);
  - a fixed 16.7 MP cap below selah's limits (L17; a 16.8 MP fixture added);
  - decode throughput (L18).

### Port — accuracy policy (before M3)

- **[ADR 0003](docs/adr/0003-accuracy-is-the-bar.md): accuracy is the bar.**
  - Ground truth decides where it exists: source pixels, the format spec, the math. The port must be at least as
    accurate as Rust.
  - Rust's behaviour is matched exactly only where it is the contract: JSON, layers, history, SVG, enum names,
    verdicts.
  - A Rust defect is fixed in the port. A dependency defect is filed upstream with the measurement, and its rows go
    on a named known-defect list.
- **Ground-truth fixtures:** the oracle writes `fixtures/<file>.source.rgba` for the 13 JPEG fixtures. The harness
  prints chitra's and Rust's mean and max error against them.
- **What the measurement found:**
  - without chroma subsampling the decoders are equally accurate (F2: not a defect);
  - with subsampling chitra's box upsampling is 0.13–1.34 worse on the mean and up to 24 worse at the worst pixel
    (F1).
- **Class B redefined** (port plan §4, port-parity.md §5):
  - accuracy: mean and max error against the source, no worse than the reference decoder's + 0.05 / + 2;
  - conformance: K_jpeg = 2 where the decoders upsample alike.
- **Filed with chitra** (`docs/development/issues/2026-11-14-*`):
  - interpolated chroma upsampling (F1);
  - rounded sub-8-bit BMP channels (F3);
  - an opaque 32-bpp `BI_RGB` (F4);
  - the CMYK samples (F5);
  - progressive JPEG, of which selah is now the consumer.
- **ADR 0002 #16:** CMYK refused, temporarily. Ledger rows L11–L15 track the chitra filings, including the F4
  opacity guard selah adds at M3.

## [0.30.0] - 2026-11-14

**Final Rust release.** selah is moving to Cyrius as **1.0.0**. This tree is frozen in `rust-old/` as the parity
oracle for that port ([`docs/development/selah-port-plan.md`](docs/development/selah-port-plan.md)). Rust consumers
stay on 0.30.x; there is no Rust 1.0.

### Changed

- **Toolchain**: MSRV 1.89 → **1.100** (edition 2024 remains the latest edition). The CI MSRV job now sets
  `RUSTUP_TOOLCHAIN=1.100`: `rust-toolchain.toml`'s `stable` channel outranked the toolchain the job installed, so
  the job ran the runner's preinstalled 1.99.1 and failed
- **ranga** 0.29 → 1.0 — `PixelBuffer` accessors; highlight blending now uses row-wise SIMD `blend_row`
- **bote** 0.50 → 0.92 — `selah_ocr` and `selah_history` carry MCP read-only tool annotations
- **reqwest** 0.12 → 0.13 (`rustls` feature, aws-lc-rs provider; `ring` no longer in the tree)
- **hisab** 1.3 → 1.4 with `default-features = false` (`geo`, `num`, `transforms` only — `num` is required because hisab 1.4's `transforms` uses it without enabling it)
- **base64** 0.22 → 0.23, **criterion** 0.5 → 0.8 (`std::hint::black_box`)
- **image** restricted to the codecs selah supports (`png`, `jpeg`, `bmp`, `webp`): exotic decoders (TIFF, EXR, GIF, AVIF, …) are no longer reachable from untrusted input; dependency tree 288 → 229 crates; the `paste` (RUSTSEC-2024-0436) ignore is gone — zero ignored advisories
- **tokio** is now optional and only pulled in by the `mcp` feature (feature set reduced to `rt`)
- **hoosh** dependency removed — it was never used; the `ai` feature is kept as a reserved no-op so existing `features = ["ai"]` keep building
- Redundant dev-dependencies removed; stale `deny.toml` license allowances and `ring` clarification removed
- `CaptureClient::capture(CaptureRegion::Window(_))` now returns `InvalidRegion` instead of silently capturing the **full screen** (daimon's API has no window selector)
- `HistoryStore::open_default()` returns an error instead of writing under `./` when neither `XDG_DATA_HOME` nor `HOME` is an absolute path; empty/relative `XDG_DATA_HOME` is ignored per the XDG spec
- `suggest_redactions()` returns matches in text order (was grouped by type) and reports each token at most once
- MCP default outputs (`selah_annotate`, `selah_redact`) are written next to the input file instead of the server's working directory; output format follows the output extension
- MCP tools only accept inputs whose content is a supported image, and only write outputs with an image extension (`selah_capture` infers the format from it; a contradicting `format` is an error)
- MCP `selah_capture` reports and records the absolute output path

### Added

- `Rect::normalized()`, `Rect::is_finite()`; `ImageFormat::from_path()`; `RedactionTarget::key()`; `FromStr` for `AnnotationKind` (case-insensitive)
- `annotate::{MAX_IMAGE_DIMENSION, MAX_DECODE_ALLOC, MAX_LAYER_FILE_BYTES, LAYER_VERSION}` and `mcp::{MAX_INPUT_FILE_BYTES, MAX_ANNOTATIONS}` limits
- `HistoryStore::path()`
- MCP `selah_ocr` `engine: "hoosh"` now actually calls hoosh (was silently ignored); response includes `engine` and `is_stub`
- `#![forbid(unsafe_code)]`
- Benchmarks: `render_to_image_512_bmp`, `to_svg_100`, `extract_text_regions_64k`

### Security

- **Redaction leaks fixed**: redaction rects with negative width/height (drag from bottom-right) drew nothing; fractional edges left partially-covered pixels unredacted. Rects are now normalized and rounded outward; a NaN redaction region is an error instead of a silent no-op. SVG output normalizes rects too (SVG drops negative sizes)
- **MCP `selah_redact` is explicit about what it did**: stub-OCR matches (which come from embedded metadata, not pixels) are reported as `metadata_matches` — the re-encoded output does not carry that metadata — with a warning that rendered text was not analyzed; if a real OCR engine reports text it cannot locate, the tool fails closed instead of writing an unredacted `*_redacted` copy. `redact_image()` logs a warning and its docs no longer suggest a pixel-redaction route that doesn't exist
- **MCP file access**: `selah_annotate`/`selah_redact` validated only the input path — `output` is now validated too (also empty/NUL paths). Inputs must be supported images (so e.g. `selah_ocr` can no longer echo a private key's contents); outputs must have an image extension and may not be an existing symlink/directory/device (no clobbering `~/.bashrc` or writing through links)
- **Resource exhaustion**: image decoding enforces dimension/allocation limits, budgeting both the native buffer and the RGBA working copy from the header (decompression bombs, incl. small-file grayscale images that expand 4×); MCP inputs and annotation layers are size-capped and must be regular files (`/dev/zero`, FIFOs rejected before opening); HTTP error bodies are truncated and response bodies size-capped; HTTP connect timeout added
- **Rasterizer**: arrows are clipped to the canvas before Bresenham (previously `i32` overflow → panic in debug builds and up to ~2³² iterations for far-off endpoints); ellipse sampling is capped, skipped when off-canvas, and clamped before integer conversion (huge partly-visible circles overflowed `i64`)
- **No panicking HTTP fallback**: if the TLS backend cannot initialise, requests return `SelahError::Api` instead of `reqwest::Client::new()` panicking
- **daimon**: `DaimonConfig::api_key` was never sent — now sent as `Authorization: Bearer`; `Debug` redacts it; agent IDs are percent-encoded path segments (no path injection) and validated
- **hoosh OCR**: confidence clamped to `0..=1`, non-finite/empty regions dropped
- **History**: file permissions `0600` (directory `0700`); `Rect` deserialization rejects non-finite/out-of-range values
- `mask_pii` panicked on multibyte UTF-8 (byte slicing) — now char-based
- `xml_escape` drops characters that are illegal in XML 1.0 so SVG output is always well-formed

### Fixed

- **JPEG output was broken**: `convert_format`/`annotate_image`/`render_to_image` to JPEG always failed (`Rgba8` unsupported by the encoder); alpha is now dropped for JPEG
- **History locking**: replaced the create/delete lockfile (stale forever after a crash) with OS file locks (`File::try_lock`); `record` now takes the lock too, so appends can no longer be lost to a concurrent `delete`; `delete` rewrites atomically (temp + fsync + rename); `list` streams with bounded memory (top-N heap) and skips invalid-UTF-8 lines instead of failing; a torn final line (crash mid-append) is terminated before the next append so that entry isn't lost
- **MCP `selah_annotate`** rejected schema-conformant input (required `id`, PascalCase `kind`, `color`); now accepts the advertised schema (lowercase/any-case kind, optional color, extra fields ignored)
- **MCP runtime**: async handlers ran via `Handle::block_on`, which deadlocks on a current-thread runtime; now uses a private runtime on a scoped thread
- **MCP argument validation**: invalid `format`, `region`, `since`, `limit`, `engine` and `targets` are errors instead of being silently defaulted/ignored; output is validated before capturing
- **PII detection**: emails/IPs/cards followed by punctuation (`user@example.com.`, `10.0.0.1.`) were missed; space-grouped cards (`4111 1111 1111 1111`, Amex `3782 822463 10005`) and `(555) 123-4567` are detected; IPs are no longer also reported as phone numbers; removed an `unwrap()`
- **SVG arrows** referenced an undefined `#arrow` marker; each arrow now gets its own marker in its color
- Clipboard: MIME type sniffed from data (was always `image/png`); stdin fed from a separate thread and only the exit status awaited (avoids pipe deadlock with forking clipboard tools); empty `WAYLAND_DISPLAY` no longer counts as Wayland
- `CaptureClient` regions: partly off-screen regions are clipped instead of shifted; empty/non-finite regions rejected; `capture_monitor("")` rejected
- `DaimonClient`/`HooshClient` trim trailing slashes from endpoints
- Annotation layers with a newer `version` are rejected instead of misread

### Performance

Criterion medians, `./scripts/bench-history.sh` (baseline = pre-upgrade `424dc7e`; history in `bench-history.csv`):

| Benchmark | Baseline | Deps only | Final | vs baseline |
|-----------|---------:|----------:|------:|------------:|
| `render_to_image_64x64` | 24.7 µs | 17.5 µs | 17.6 µs | **−29%** |
| `render_to_image_512_bmp` (new) | — | 2.48 ms | 1.39 ms | **−44%** vs deps |
| `to_svg_100` (new) | — | 49.0 µs | 43.2 µs | **−12%** vs deps |
| `extract_text_regions_64k` (new) | — | 88.3 µs | 66.8 µs | **−24%** vs deps |
| `suggest_redactions_clean` | 830 ns | 710 ns | 412 ns | **−50%** |
| `suggest_redactions_pii` | 1.27 µs | 1.17 µs | 513 ns | **−60%** |
| `suggest_redactions_long` | 73.5 µs | 69.6 µs | 41.1 µs | **−44%** |
| `xml_escape_plain` | 36.0 ns | 45.5 ns | 23.3 ns | **−35%** |
| `xml_escape_long` | 2.92 µs | 2.72 µs | 679 ns | **−77%** |
| `xml_escape_special` | 114 ns | 103 ns | 139 ns | +22% |
| `rect_intersects` | 1.33 ns | 1.16 ns | 1.06 ns | −20% |
| `rect_contains_point` | 0.99 ns | 1.25 ns | 1.31 ns | +33% |

- Rendering: row-slice fills and SIMD `blend_row` instead of per-pixel `set_rgba`/`blend_pixel`; no RGBA copy when the decoded image is already RGBA (`into_rgba8`)
- PII detection: single pass, no per-token allocations
- `xml_escape`: table-driven scan with bulk copies and a no-escape fast path. The dense-special-character input is slower because it now also filters XML-illegal characters (the realistic plain/long inputs are 35–77% faster)
- `rect_contains_point` normalizes negative-size rects (correctness); still ~1.3 ns

### Known limitations (unchanged by this release)

- The local OCR is a stub: it scans encoded file bytes, so it can report spurious matches from compressed pixel data and never sees rendered text
- The MCP server does not confine paths to a root directory (absolute paths are allowed by design)
- No aggregate rendering budget per `selah_annotate` call beyond `MAX_ANNOTATIONS` and per-shape caps
- `read_limited` checks the file type before opening; a path swapped for a FIFO in that window can still block
- `CaptureClient::save_to_file` writes with the process umask (typically `0644`)

## [0.29.4] - 2026-03-29

### Changed

- **bote** 0.22 → 0.50 — migrated MCP tool registration to `ToolDef::new()`/`ToolSchema::new()` constructor API
- **hisab** 0.24 → 1.3
- **ranga** 0.24 → 0.29.4
- **hoosh** 1.0 added as optional dependency behind `ai` feature flag
- Updated all transitive dependencies to latest compatible versions

## [0.24.3] - 2026-03-24

### Changed

- **Major restructure**: converted from workspace with 5 sub-crates + binary into a flat `[lib]` crate
- Adopted `hisab` for geometry — `Rect` is now backed by `hisab::Rect` (f32 min/max Vec2), with custom serde for backward compatibility
- All sub-crates (`selah-core`, `selah-capture`, `selah-annotate`, `selah-ai`, `selah-mcp`) are now modules under `src/`
- Removed binary target (`[[bin]]`) — selah is now a pure library crate
- Re-export `hisab::Vec2` as `selah::Vec2` for convenience
- Added `decode_image_data()`, `annotate_image()`, and `redact_image()` convenience functions at crate root
- Added `geometry::Rect` wrapper with x/y/width/height accessors over hisab's min/max representation
- Added `#[non_exhaustive]` to `SelahError`
- Version scheme changed from CalVer to SemVer (0.24.3)
- Adopted `bote` for MCP server — replaced custom JSON-RPC implementation with bote's ToolRegistry, Dispatcher, and stdio transport
- Adopted `ranga` for annotation rendering — pixel operations use `PixelBuffer`, alpha blending uses `ranga::blend::blend_pixel()` with SIMD acceleration
- Added `daimon` module with `DaimonClient`, `DaimonConfig`, `HooshClient`, `HooshConfig` for agent lifecycle and LLM vision OCR integration
- MCP feature-gated behind `mcp` feature flag (enabled by default)
- GUI/CLI extracted to separate `taswir` application

## [2026.3.24] - 2026-03-24

### Security

- Fix SVG XSS injection: text content in `to_svg()` is now XML-escaped to prevent script injection when SVG output is rendered in a browser
- Add path traversal protection to all MCP tool file operations — paths containing `..` components are rejected
- Mask detected PII in MCP `selah_redact` tool responses — matched text is now partially redacted (e.g. `us************om`) instead of returned verbatim

### Fixed

- **Redaction pipeline**: zero-size redaction regions (from stub OCR) are now filtered out instead of silently producing unchanged images; CLI warns when OCR bounding boxes are unavailable
- **Phone detection**: require separator characters (dashes, dots, parens) to avoid false positives on timestamps, serial numbers, and other digit sequences
- **MCP server**: replaced blocking `stdin.lock().lines()` with async `tokio::io::BufReader` to avoid blocking the tokio runtime
- **GUI**: `ensure_texture()` no longer panics on image decode failure — displays error in status bar instead
- **Capture client**: `f64` region coordinates are now safely clamped to `u32` range instead of using unchecked `as` casts (negative values become 0)
- **History**: `delete` operation now uses file locking to prevent corruption from concurrent processes
- **HTTP timeout**: `CaptureClient` now sets a 30-second request timeout instead of hanging indefinitely if daimon is unresponsive
- **OCR stub honesty**: `OcrResult` now includes `is_stub` flag and confidence capped at 0.1; CLI prints explicit warning about stub limitations

### Changed

- `ImageFormat` now implements `FromStr`, eliminating duplicated format-parsing match blocks across CLI, MCP, and convert commands
- Added `derive_output_path()` helper in `selah-core` to replace duplicated `{stem}_{suffix}.{ext}` logic
- Extracted `check_response()` HTTP error helper in `selah-capture` to deduplicate response checking across 3 API methods
- Removed unused `format` parameter from `CaptureClient::save_to_file()`
- Added `xml_escape()` utility to `selah-core` for safe SVG/XML string embedding
- **78 tests** across all crates (up from 55), 0 warnings

## [2026.3.17] - 2026-03-17

### Added — Initial Release

- **selah-core**: Screenshot types, annotation primitives (shapes, text, arrows), crop regions, image formats, color and geometry types
- **selah-capture**: Daimon screen capture API client (`/v1/screen/capture`), file saving, clipboard placeholder
- **selah-annotate**: Annotation canvas with drawing tools, redaction, SVG overlay rendering
- **selah-ai**: Regex-based PII detection (email, phone, credit card, IP), rule-of-thirds smart crop suggestions, text region extraction
- **CLI**: `capture`, `annotate`, `ocr`, `redact` subcommands
- **CI/CD**: GitHub Actions for check, test, clippy, fmt, release (amd64 + arm64)

### Added — Phase 2

- **`selah convert`**: Image format conversion between PNG, JPEG, BMP, and WebP
- **Annotation persistence**: Save/load annotation layers as JSON files (`--save`/`--load` flags on `selah annotate`)
- **Enhanced history viewer**: `--json` output, `--info <uuid>` detail view, `--delete <uuid>` entry removal
- **Clipboard integration**: Marked complete (Wayland/X11 detection via `wl-copy`/`xclip`)
- **`selah gui`**: Interactive annotation GUI built on egui + eframe with tool palette, drag-to-annotate, zoom/pan, save/export
- **Multi-monitor support**: `--list-monitors` and `--monitor <id>` flags on `selah capture` via daimon API
- **Keyboard shortcuts**: In-app shortcuts for annotation tools (R/C/A/H/T/D/V), zoom (+/-/0), undo (Ctrl+Z), save (Ctrl+S)
- **72 tests** across all crates, 0 warnings
