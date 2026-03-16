# Selah — AI-Native Screenshot & Annotation Tool

> Hebrew: סלה (pause/capture a moment)

[![License](https://img.shields.io/badge/license-GPLv3-blue)](LICENSE)
[![Status](https://img.shields.io/badge/status-development-yellow)]()

**Selah** is an AI-powered screenshot and annotation tool for [AGNOS](https://github.com/MacCracken/agnosticos). It wraps the daimon screen capture API and adds annotation, OCR, smart crop, and automatic redaction of sensitive content.

## Features

- **Screen capture** — Full screen or region capture via daimon's `/v1/screen/capture` API
- **Annotation engine** — Arrows, rectangles, circles, text, highlights, freeform drawing
- **Redaction** — Manual and AI-assisted redaction of sensitive content
- **OCR** — Text extraction from screenshots
- **Smart crop** — AI-suggested crop regions based on content analysis
- **PII detection** — Automatic detection of emails, phone numbers, credit cards, IP addresses
- **SVG overlay** — Non-destructive annotation rendering
- **MCP tools** — 5 native tools for agent-driven screenshot workflows

## Architecture

```
selah
├── selah-core      — Screenshot types, annotation primitives, geometry, formats
├── selah-capture   — Daimon screen capture API client, file saving
├── selah-annotate  — Annotation canvas, drawing tools, redaction, SVG rendering
└── selah-ai        — OCR, PII detection, smart crop suggestions
```

## Usage

```bash
# Capture full screen
selah capture

# Capture a region
selah capture --region 100,100,800,600

# Annotate an existing image
selah annotate screenshot.png

# Extract text from an image
selah ocr screenshot.png

# Auto-detect and redact sensitive content
selah redact screenshot.png
```

## AGNOS Integration

Selah integrates with AGNOS through:

- **daimon API** (port 8090) — screen capture via `/v1/screen/capture`
- **hoosh API** (port 8088) — LLM-assisted OCR and content description
- **MCP tools** — `selah_capture`, `selah_annotate`, `selah_ocr`, `selah_redact`, `selah_history`
- **agnoshi intents** — "take screenshot", "annotate image", "redact sensitive data"

## License

GPL-3.0
