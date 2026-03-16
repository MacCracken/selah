# Roadmap

## Phase 1 — Core Capture & Annotate (current)

- [x] Screenshot types and annotation primitives (selah-core)
- [x] Daimon screen capture API client (selah-capture)
- [x] Annotation canvas with SVG rendering (selah-annotate)
- [x] Regex-based PII detection (selah-ai)
- [x] Rule-of-thirds smart crop suggestions (selah-ai)
- [x] CLI with capture, annotate, ocr, redact subcommands
- [x] CI/CD pipeline (check, test, clippy, fmt, release)

## Phase 2 — Full GUI & Integration

- [ ] Interactive annotation GUI (egui or Flutter)
- [ ] Clipboard integration (wl-copy on Wayland, xclip on X11)
- [ ] Multi-monitor support
- [ ] Keyboard shortcuts and global hotkey capture
- [ ] Annotation persistence (save/load annotation layers)
- [ ] Image format conversion
- [ ] History viewer for past captures

## Phase 3 — AI Features

- [ ] LLM-powered OCR via hoosh vision models
- [ ] Content description (describe what's in the screenshot)
- [ ] Intelligent auto-redaction with pixel-level masking
- [ ] Smart crop using content saliency detection
- [ ] Automatic annotation suggestions
- [ ] MCP tool registration with daimon
- [ ] agnoshi intent integration
