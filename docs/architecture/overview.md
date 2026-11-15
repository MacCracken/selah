# Architecture overview

selah is one flat Cyrius library. Its modules share one namespace with each other and with the consumer, and are
bundled in dependency order, since Cyrius is single-pass: a module comes after everything it calls.

## Modules

| Module | What it holds |
|---|---|
| `error` | the `SL_ERR_*` codes and their Display, Rust's `SelahError` |
| `version_str` | `SL_VERSION`, which must equal `VERSION` (the User-Agent and the MCP `serverInfo`) |
| `log` | tracing through sakshi; the library never configures sakshi |
| `scratch` | a freelist arena handed to sandhi and bayan for one call, then freed |
| `fmtf` | the shortest f32 / f64 digits, in Rust's Display and serde_json's forms |
| `json` | JSON writers byte-identical to serde_json, and readers that accept what serde accepts |
| `uuid`, `rfc3339` | UUID v4, and RFC 3339 timestamps with nanoseconds (chrono's writer and readers) |
| `geometry` | `SlRect` (f32 `{min, max}`, Rust's layout) and `SlVec2` |
| `core` | `SlAnnotation`, the kinds, `SlColor`, image formats, `SlScreenshot`, `SlMonitor`, `xml_escape` |
| `debug_data`, `ai_data` | **generated** from Rust: the characters `{:?}` escapes, and `char::is_alphanumeric` |
| `ai` | PII detection, the stub OCR, smart crop |
| `png_enc`, `bmp_enc`, `jpeg_enc`, `codec` | signature sniffing, decoding through chitra with Rust's limits, selah's encoders, `convert_format` |
| `fsutil` | bounded reads, atomic private writes, private directories |
| `raster`, `annotate` | the rasterizer (ranga's `blend_row`), the canvas, layer files, SVG, `render_to_image` |
| `selah` | the facade: `selah_annotate_image`, `selah_redact_image`, `selah_decode_image_data` |
| `history` | the JSONL screenshot history |
| `http`, `spawn` | sandhi with deadlines and Rust's error texts; a spawn that feeds stdin (the clipboard) |
| `capture`, `daimon` | `SlCaptureClient`, `SlDaimonClient`, `SlHooshClient` |
| `mcp` | the MCP server on bote; only in the `[lib.mcp]` profile |

`[lib]` bundles everything but `mcp` as `dist/selah.cyr`; `[lib.mcp]` adds it, as `dist/selah-mcp.cyr`, whose consumer
declares bote.

## Data flow

```
                 POST /v1/screen/capture, GET /v1/screen/monitors
  SlCaptureClient ─────────────────────────────────────────────► daimon (8090)
        │        ◄───────────────────────────────────────────── base64 image in JSON
        ▼
  image bytes ──► codec (sniff, chitra decode, limits) ──► SlImage (RGBA)
        │                                                     │
        │                         annotate / raster ◄─────────┘  annotations, redactions
        │                                │
        │                                └──► codec encode (PNG, JPEG, BMP) ──► bytes, files (atomic, 0600)
        │
        ├──► ai: stub OCR (embedded text) ──► suggest_redactions ──► redaction regions
        │
        └──► SlHooshClient: POST /v1/chat/completions (downscaled JPEG) ──► hoosh (8088) ──► text, regions

  history: one JSON line per capture, $XDG_DATA_HOME/selah/history.jsonl, under flock
  mcp: the five tools over bote's stdio, each a call into the above
```

## Limits

| Limit | Value | Where |
|---|---|---|
| Decoded image side | 32,768 px | `SL_MAX_IMAGE_DIMENSION` (`annotate`) |
| Decoded buffer (native and RGBA copy, from the header) | 512 MiB | `SL_MAX_DECODE_ALLOC` (`annotate`) |
| Annotation layer file | 64 MiB | `SL_MAX_LAYER_FILE_BYTES` (`annotate`) |
| MCP input file | 256 MiB | `SL_MAX_INPUT_FILE_BYTES` (`mcp`) |
| MCP annotations per call | 10,000 | `SL_MAX_ANNOTATIONS` (`mcp`) |
| MCP request line over stdio | 128 KiB | bote's stdio buffer (ledger L23) |
| Capture response body | 256 MiB | `http` |
| Other API response bodies | 16 MiB | `http` |
| OCR request to hoosh | 1 MiB | `daimon`: the image is downscaled until it fits |
| HTTP deadlines | 10 s connect; 30 s daimon and capture, 60 s hoosh; read and write each bounded too | `http`, `capture`, `daimon` |
| History lock wait | 5 s | `history` |

## Memory

- Decoded images and pixel buffers live on the freelist and go back to it (`SlImage_free`, `selah_bytes_free`).
- Each HTTP request and each JSON parse runs in a scratch arena, freed when the call ends. The arena never frees a
  block early, because sandhi and bayan may read a block they have freed; one spare 64 KiB chunk outlives the
  arenas (ledger L24).
- chitra decodes through the freelist (`_sl_chitra_allocator` in `codec`, chitra 1.4.0's allocator form): a
  decode's working buffers go back before it returns, and `SlImage_free` returns the image through
  `chitra_image_free`.
- bayan's base64 allocates from the bump heap, which never frees, so a long-running consumer such as the MCP server
  grows with each base64 decode (ledger L19; filed).

## Concurrency

The library keeps no locks of its own. The history file is shared between processes through `flock` on
`<history>.lock`. The MCP server's API URL, history store and hoosh configuration are module state, set by
`selah_mcp_dispatcher`, because bote's handlers take no context: one server per process. The scratch arena's spare
chunk is the only state threads could race on, and it is taken and given back with `atomic_cas`. chitra's allocator
vtable is rewritten on every decode with the same five values, so two threads writing it at once agree.

## External services

- **daimon** (port 8090): `/v1/screen/capture` and `/v1/screen/monitors` (waiting on daimon, ledger L9), and
  `POST /v1/agents` for registration (ADR 0002 #4).
- **hoosh** (port 8088): `POST /v1/chat/completions` with an image part, for OCR (ADR 0002 #6); a model must be
  configured.
- **bote** 3.8.15 for MCP, **sandhi** for HTTP, **bayan** for JSON and base64, **chitra** for decoding, **ranga** for
  blending, **sakshi** for tracing.
