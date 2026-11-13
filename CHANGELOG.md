# Changelog

All notable changes to Selah will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Changed

- **Toolchain**: MSRV 1.89 → **1.100** (edition 2024 remains the latest edition); CI MSRV job updated
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
