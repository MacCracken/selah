# Changelog

All notable changes to Selah will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

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
