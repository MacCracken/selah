# MCP Tools

Selah exposes 5 MCP tools for agent-driven screenshot workflows. The server is `src/mcp.cyr`, in the `[lib.mcp]`
profile (`dist/selah-mcp.cyr`), on bote 3.8.15; its consumer declares bote itself (`[deps.bote]`, as selah does behind
`[features] mcp`).

## Running the server

- `selah_run_server(api_url)` serves MCP over stdio until stdin closes: one JSON-RPC request per line, one answer per
  line. Captures go to daimon at `api_url` and are recorded in the default history
  (`$XDG_DATA_HOME/selah/history.jsonl`); when it cannot be opened, history is off and `selah_history` says so.
- `selah_mcp_dispatcher(api_url, history)` returns the bote dispatcher, for another of bote's transports or for a
  test; `history` is a `*SlHistoryStore`, or 0 for none.
- `selah_mcp_set_hoosh_config(cfg)` sets the hoosh endpoint and model that `selah_ocr`'s `hoosh` engine uses. hoosh
  needs a model, and the default configuration has none, so the engine refuses until one is set.
- `serverInfo` is `{"name":"selah","version":<selah's version>}`.
- A request line over 128 KiB is refused with `-32700 parse error: line too long`, about 1,950 bare annotations in
  one `selah_annotate` call (bote's limit, filed; ADR 0002 #25).

## Path and input safety

All tools apply the same rules to client-supplied paths:

- Paths containing `..` components, empty paths and paths with NUL bytes are rejected — for **outputs as well as inputs**.
- Inputs must be regular files no larger than 256 MiB (`SL_MAX_INPUT_FILE_BYTES`) whose content is a PNG, JPEG or
  BMP; directories, FIFOs and devices such as `/dev/zero` are refused before opening, and non-image files (keys,
  configs) are refused before any processing. A WebP input is recognised and refused (ADR 0002 #1).
- Outputs must have an image extension (`png`, `jpg`, `jpeg`, `bmp`; `webp` is refused) and may not be an existing
  symlink, directory or device. The output format follows the extension.
- Images are decoded with limits: at most 32,768 px a side, and at most 16,777,216 pixels in all (chitra's cap, filed).
- Default outputs are written **next to the input file** (`<stem>_<suffix>.<ext>`, `.png` if the input has no image
  extension).
- Outputs are written atomically and private (0600): a temp file beside the target, renamed over it.
- Absolute paths are allowed: the server runs with the permissions of its user and does not confine clients to a
  directory.

Errors are returned as MCP error content (`isError: true`). That includes arguments that fail a tool's input schema
(a wrong type, a value outside an enum, a limit out of range), which bote reports with its own text.

## selah_capture

Take a screenshot via the daimon screen capture API.

**Parameters:**
- `region` (optional): `{ x, y, width, height }` — omit for full screen. Negative sizes are normalized, off-screen
  parts are clipped; empty or non-finite regions are rejected.
- `format` (optional): `png` | `jpg` | `jpeg` | `bmp` (default: the `output` extension, else `png`), lowercase.
  `webp` is refused. Unknown formats, or a format that contradicts the `output` extension, are an error.
- `output` (optional): file path to save (default: `screenshot.<format>`). Validated before capturing.

**Returns:** `{ path, width, height, format, timestamp }` — `path` is absolute. The capture is recorded in the
screenshot history.

## selah_annotate

Add annotations to an image.

**Parameters:**
- `image_path`: path to the source image
- `annotations`: array (max 10 000) of `{ kind, position: { x, y, width, height }, color?, text? }`
  - `kind`, lowercase: `arrow` | `rectangle` | `circle` | `text` | `highlight` | `redaction` | `freeform`. The schema
    refuses any other spelling.
  - `color`: `{ r, g, b, a }` (0–255, default opaque red)
- `output` (optional): output file path (default: `<image>_annotated.<ext>` next to the input)

**Returns:** `{ path, format, annotation_count }`

## selah_ocr

Extract text from an image. Read-only.

**Parameters:**
- `image_path`: path to the image
- `engine` (optional): `local` | `hoosh` (default: `local`)
  - `local` is a **stub**: it only finds ASCII text embedded in the file's bytes (metadata), not text rendered in the
    image, and returns no regions.
  - `hoosh` asks hoosh's chat completions (`/v1/chat/completions`) with the image, downscaled and re-encoded as JPEG
    until the request fits hoosh's 1 MiB cap; the regions are scaled back to the image. It needs a model
    (`selah_mcp_set_hoosh_config`).

**Returns:** `{ text, confidence, regions: [{ x, y, width, height }], engine, is_stub }`

## selah_redact

Detect and redact sensitive content in an image.

**Parameters:**
- `image_path`: path to the image
- `targets` (optional): array of `email` | `phone` | `credit_card` | `ip_address` (default: all). Unknown targets are
  an error.
- `output` (optional): output file path (default: `<image>_redacted.<ext>` next to the input)

Detection currently uses the local stub OCR, which scans the file's embedded bytes (metadata), **not the rendered
pixels**: text visible in the image is not detected or redacted. Matches it does find live in embedded metadata,
which the re-encoded output does not carry over; they are reported in `metadata_matches`.

**Fails closed:** if a real (pixel) OCR engine reports sensitive text it cannot locate, the tool returns an error and
writes nothing, rather than producing an unredacted copy named `*_redacted`.

**Returns:** `{ path, format, redactions: [{ target_type, confidence, matched_text }], metadata_matches?: [{
target_type, matched_text }], warning? }` — `matched_text` is masked (e.g. `us************om`); `metadata_matches`
and `warning` are present when the stub OCR was used.

## selah_history

List recent screenshots and their metadata. Read-only.

**Parameters:**
- `limit` (optional): max results, 0–1000 (default: 20)
- `since` (optional): an RFC 3339 timestamp, read as chrono's `parse_from_rfc3339` reads it; an invalid one is an
  error that names chrono's reason (`premature end of input`, `input contains invalid characters`, `input is out of
  range`, `trailing input`)

**Returns:** `{ screenshots: [{ id, path, timestamp, source, format, dimensions }] }`, newest first

## Differences from Rust 0.30

ADR 0002 #24 and #25 list them: `tools/list` carries the read-only annotations, a schema violation is an `isError`
result rather than JSON-RPC −32602, `serverInfo` is selah's, the hoosh engine needs a model, and stdio's request line
is limited to 128 KiB. WebP is refused everywhere (#1).
