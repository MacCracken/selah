# Architecture

## Crate Structure

```
selah/
├── selah-core       Core types: Screenshot, Annotation, geometry, formats
├── selah-capture    Daimon screen capture API client, file I/O
├── selah-annotate   Annotation canvas, drawing tools, SVG rendering
├── selah-ai         OCR, PII detection, smart crop suggestions
└── src/main.rs      CLI binary (clap derive)
```

## Data Flow

```
User command
    │
    ▼
┌──────────┐     POST /v1/screen/capture     ┌─────────┐
│  selah    │ ──────────────────────────────► │  daimon  │
│  capture  │ ◄────────────────────────────── │  (8090)  │
└──────────┘     image data (base64)          └─────────┘
    │
    ▼
┌──────────┐
│  selah    │  Screenshot struct (core types)
│  core     │
└──────────┘
    │
    ├──► selah-annotate  →  SVG overlay / annotated image
    │
    └──► selah-ai        →  OCR text, redaction suggestions, crop hints
```

## Dependencies Between Crates

- `selah-core` has no internal dependencies (leaf crate)
- `selah-capture` depends on `selah-core`
- `selah-annotate` depends on `selah-core`
- `selah-ai` depends on `selah-core`
- The binary depends on all four crates

## External Integration

- **daimon** (port 8090): Screen capture via `/v1/screen/capture`, permissions via `/v1/screen/permissions`
- **hoosh** (port 8088): Future OCR via LLM vision models
- **AGNOS marketplace**: Distributed as `.agnos-agent` package
