# Changelog

All notable changes to Selah will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

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
