# Changelog

All notable changes to Selah will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [2026.3.16] - 2026-03-16

### Added — Initial Release

- **selah-core**: Screenshot types, annotation primitives (shapes, text, arrows), crop regions, image formats, color and geometry types
- **selah-capture**: Daimon screen capture API client (`/v1/screen/capture`), file saving, clipboard placeholder
- **selah-annotate**: Annotation canvas with drawing tools, redaction, SVG overlay rendering
- **selah-ai**: Regex-based PII detection (email, phone, credit card, IP), rule-of-thirds smart crop suggestions, text region extraction
- **CLI**: `capture`, `annotate`, `ocr`, `redact` subcommands
- **CI/CD**: GitHub Actions for check, test, clippy, fmt, release (amd64 + arm64)
- **30+ tests** across all crates, 0 warnings
