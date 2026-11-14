# Architecture

## Crate Structure

Selah is a single flat library crate:

```
selah/src/
├── lib.rs        Crate root — re-exports, facade API (annotate_image, redact_image, decode_image_data)
├── core.rs       Types — Annotation, AnnotationKind, ImageFormat, Color, Screenshot, Monitor
├── geometry.rs   Rect wrapper over hisab::Rect, backward-compatible serde
├── error.rs      SelahError (#[non_exhaustive])
├── capture.rs    CaptureClient — async reqwest to daimon API, clipboard, session detection
├── history.rs    HistoryStore — JSONL persistence with OS file locking, atomic rewrites
├── annotate.rs   AnnotationCanvas — pixel rendering (ranga), SVG output, format conversion
├── ai.rs         PII detection, OCR stub, smart crop suggestions
├── daimon.rs     DaimonClient, HooshClient — agent lifecycle, LLM vision OCR
├── mcp.rs        MCP server via bote (feature-gated: "mcp")
├── http.rs       (private) shared HTTP client, status checks, bounded response bodies
└── fsutil.rs     (private) bounded file reads, atomic writes, private permissions
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

## Safety Limits

| Limit | Value | Where |
|-------|-------|-------|
| Max decoded image side | 32 768 px | `annotate::MAX_IMAGE_DIMENSION` |
| Max decoded buffer (native and RGBA copy, from header) | 512 MiB | `annotate::MAX_DECODE_ALLOC` |
| Max annotation layer file | 64 MiB | `annotate::MAX_LAYER_FILE_BYTES` |
| Max MCP input file | 256 MiB | `mcp::MAX_INPUT_FILE_BYTES` |
| Max MCP annotations per call | 10 000 | `mcp::MAX_ANNOTATIONS` |
| Max capture response body | 256 MiB | `http` (private) |
| Max other API response body | 16 MiB | `http` (private) |
| HTTP timeouts | 10 s connect; 30 s (daimon) / 60 s (hoosh) total | `http::build_client` |
| History lock wait | 5 s | `history` |

## External Integration

- **daimon** (port 8090): Screen capture via `/v1/screen/capture`, permissions via `/v1/screen/permissions`
- **hoosh** (port 8088): OCR via LLM vision models (`HooshClient::ocr`, MCP `selah_ocr` with `engine: "hoosh"`)
- **AGNOS marketplace**: Distributed as `.agnos-agent` package
