# Selah Roadmap

## v0.24.3 — Library Restructure (current release)

- [x] Core capture and annotation types
- [x] Daimon screen capture API client
- [x] Annotation canvas with pixel rendering and SVG output
- [x] Regex-based PII detection (email, phone, credit card, IP)
- [x] Rule-of-thirds smart crop suggestions
- [x] Clipboard integration (Wayland/X11)
- [x] Multi-monitor support
- [x] Annotation persistence (save/load JSON layers)
- [x] Image format conversion (PNG, JPEG, BMP, WebP)
- [x] Screenshot history (JSONL with file locking)
- [x] Restructured as flat library crate (no binary, no GUI)
- [x] Adopted hisab for geometry (Rect backed by hisab::Rect, Vec2)
- [x] Adopted bote for MCP server (replaced custom JSON-RPC)
- [x] Adopted ranga for annotation rendering (SIMD pixel blending)
- [x] Daimon/Hoosh client integration module
- [x] Facade API: `annotate_image()`, `redact_image()`, `decode_image_data()`
- [x] 160 tests, criterion benchmarks, integration tests

## v0.25.x — AI Features

- [ ] Real OCR via hoosh vision models (replace byte-scanning stub)
- [ ] Content description (describe what's in a screenshot via LLM)
- [ ] Intelligent auto-redaction with pixel-level bounding boxes from OCR
- [ ] Smart crop using content saliency detection via hoosh
- [ ] Automatic annotation suggestions
- [ ] Agent registration with daimon (Tier 1 lifecycle)

## v0.26.x — Ecosystem Integration

- [ ] Adopt ranga fully (replace `image` crate for format I/O when ranga adds codecs)
- [ ] Vector search integration via daimon (index screenshots for semantic search)
- [ ] RAG integration (query screenshot content via natural language)
- [ ] agnoshi intent integration (voice-driven screenshot commands)
- [ ] t-ron MCP security middleware

## v1.0 — Stable API

- [ ] Public API stabilization
- [ ] All `#[non_exhaustive]` audited
- [ ] Full documentation with doc-tests
- [ ] Published to crates.io with stable guarantees
