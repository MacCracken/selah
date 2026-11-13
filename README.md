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

Selah is a library crate (the GUI/CLI lives in [taswir](https://github.com/MacCracken/taswir)):

```
selah/src/
├── core.rs       Annotation, AnnotationKind, ImageFormat, Color, Screenshot, Monitor
├── geometry.rs   Rect (backed by hisab)
├── capture.rs    CaptureClient — daimon screen capture API, clipboard
├── annotate.rs   AnnotationCanvas — pixel rendering (ranga), SVG overlay, format conversion
├── ai.rs         PII detection, OCR stub, smart crop suggestions
├── daimon.rs     DaimonClient, HooshClient (LLM vision OCR)
├── history.rs    HistoryStore — JSONL screenshot history
└── mcp.rs        MCP server via bote (feature "mcp")
```

## Usage

```toml
[dependencies]
selah = "0.29"
# selah = { version = "0.29", features = ["mcp"] }  # MCP server
```

```rust
use selah::{Annotation, AnnotationKind, Color, ImageFormat, Rect, SelahError};

fn annotate(png: &[u8]) -> Result<Vec<u8>, SelahError> {
    // Detect PII in text (e.g. from OCR)
    for s in selah::suggest_redactions("mail admin@example.com, card 4111 1111 1111 1111") {
        println!("{}: {}", s.target_type, s.matched_text);
    }

    // Draw annotations onto an encoded image
    selah::annotate_image(
        png,
        &[
            Annotation::new(AnnotationKind::Rectangle, Rect::new(10.0, 10.0, 200.0, 80.0), Color::RED),
            Annotation::new(AnnotationKind::Redaction, Rect::new(40.0, 120.0, 160.0, 24.0), Color::BLACK),
        ],
        ImageFormat::Jpeg,
    )
}
```

Requires Rust 1.100+. Supported image codecs: PNG, JPEG, BMP, WebP.

## AGNOS Integration

Selah integrates with AGNOS through:

- **daimon API** (port 8090) — screen capture via `/v1/screen/capture`
- **hoosh API** (port 8088) — LLM-assisted OCR and content description
- **MCP tools** — `selah_capture`, `selah_annotate`, `selah_ocr`, `selah_redact`, `selah_history`
- **agnoshi intents** — "take screenshot", "annotate image", "redact sensitive data"

## License

GPL-3.0
