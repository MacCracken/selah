# Selah Roadmap

## v0.30.0 — Toolchain, Dependencies & Hardening (final Rust release)

The Cyrius port (1.0.0) follows this release: [selah-port-plan.md](selah-port-plan.md). The open items below carry over
to Cyrius after 1.0.0.


- [x] MSRV 1.89 → 1.100 (edition 2024); dependencies to latest (ranga 1.0, bote 0.92, reqwest 0.13, hisab 1.4, base64 0.23, criterion 0.8)
- [x] `image` restricted to supported codecs (PNG/JPEG/BMP/WebP); dependency tree 288 → 229 crates; zero ignored advisories
- [x] Decode limits (decompression-bomb guard), bounded file reads and HTTP bodies
- [x] Leak-free redaction (normalized rects, outward rounding, NaN rejected); MCP redact fails closed for unlocatable pixel text
- [x] Rasterizer hardening (line clipping, bounded ellipse sampling) — no overflow/hang on extreme coordinates
- [x] History: OS file locks, atomic rewrites, private permissions, bounded-memory `list`
- [x] MCP: inputs must be images, outputs must be image files (no symlinks), traversal checks on both; schema-conformant annotations; `engine: "hoosh"` OCR wired up
- [x] PII detection: punctuation-tolerant, space-grouped cards, `(555) 123-4567`, no double reports
- [x] 289 tests (was 171), mock-HTTP coverage of capture/daimon/hoosh clients
- [ ] Map OCR bounding boxes to PII matches (pixel-level auto-redaction)
- [ ] Format-aware metadata scan for the stub OCR (skip compressed pixel data → no false positives)
- [ ] Optional MCP path confinement to an allowed root directory
- [ ] Per-call rendering work budget for MCP `selah_annotate`

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

- [x] Public API stabilization
- [x] All `#[non_exhaustive]` audited — applied to all public enums
- [x] Full documentation with doc-tests on all key public items
