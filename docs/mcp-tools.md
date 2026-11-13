# MCP Tools

Selah exposes 5 MCP tools for agent-driven screenshot workflows (feature `mcp`).

## Path and input safety

All tools apply the same rules to client-supplied paths:

- Paths containing `..` components, empty paths and paths with NUL bytes are rejected — for **outputs as well as inputs**.
- Inputs must be regular files no larger than 256 MiB (`MAX_INPUT_FILE_BYTES`) whose content is a supported image (PNG, JPEG, BMP, WebP); directories, FIFOs and devices such as `/dev/zero` are refused before opening, and non-image files (e.g. keys, configs) are refused before any processing.
- Outputs must have an image extension (`png`, `jpg`, `jpeg`, `bmp`, `webp`) and may not be an existing symlink, directory or device. The output format follows the extension.
- Images are decoded with limits (≤ 32768 px per side; native buffer and RGBA working copy each ≤ 512 MiB, checked from the header).
- Default outputs are written **next to the input file** (`<stem>_<suffix>.<ext>`, `.png` if the input has no image extension).
- Absolute paths are allowed: the server runs with the permissions of its user and does not confine clients to a directory.

Errors are returned as MCP error content (`isError: true`).

## selah_capture

Take a screenshot via the daimon screen capture API.

**Parameters:**
- `region` (optional): `{ x, y, width, height }` — omit for full screen. Negative sizes are normalized, off-screen parts are clipped; empty or non-finite regions are rejected.
- `format` (optional): `png` | `jpg` | `jpeg` | `bmp` | `webp` (default: the `output` extension, else `png`). Unknown formats, or a format that contradicts the `output` extension, are an error.
- `output` (optional): file path to save (default: `screenshot.<format>`). Validated before capturing.

**Returns:** `{ path, width, height, format, timestamp }` — `path` is absolute. The capture is recorded in the screenshot history.

## selah_annotate

Add annotations to an image.

**Parameters:**
- `image_path`: path to the source image
- `annotations`: array (max 10 000) of `{ kind, position: { x, y, width, height }, color?, text? }`
  - `kind` (case-insensitive): `arrow` | `rectangle` | `circle` | `text` | `highlight` | `redaction` | `freeform`
  - `color`: `{ r, g, b, a }` (0–255, default opaque red)
- `output` (optional): output file path (default: `<image>_annotated.<ext>` next to the input)

**Returns:** `{ path, format, annotation_count }`

## selah_ocr

Extract text from an image. Read-only.

**Parameters:**
- `image_path`: path to the image
- `engine` (optional): `local` | `hoosh` (default: `local`)
  - `local` is a **stub**: it only finds ASCII text embedded in the file's bytes (metadata), not text rendered in the image, and returns no regions.
  - `hoosh` sends the image to hoosh (`http://localhost:8088/v1/vision/ocr`) for real OCR.

**Returns:** `{ text, confidence, regions: [{ x, y, width, height }], engine, is_stub }`

## selah_redact

Detect and redact sensitive content in an image.

**Parameters:**
- `image_path`: path to the image
- `targets` (optional): array of `email` | `phone` | `credit_card` | `ip_address` (default: all). Unknown targets are an error.
- `output` (optional): output file path (default: `<image>_redacted.<ext>` next to the input)

Detection currently uses the local stub OCR, which scans the file's embedded bytes (metadata), **not the rendered pixels**: text visible in the image is not detected or redacted. Matches it does find live in embedded metadata, which the re-encoded output does not carry over; they are reported in `metadata_matches`.

**Fails closed:** if a real (pixel) OCR engine reports sensitive text it cannot locate, the tool returns an error and writes nothing, rather than producing an unredacted copy named `*_redacted`.

**Returns:** `{ path, format, redactions: [{ target_type, confidence, matched_text }], metadata_matches?: [{ target_type, matched_text }], warning? }` — `matched_text` is masked (e.g. `us************om`); `metadata_matches` and `warning` are present when the stub OCR was used.

## selah_history

List recent screenshots and their metadata. Read-only.

**Parameters:**
- `limit` (optional): max results, 0–1000 (default: 20)
- `since` (optional): RFC 3339 timestamp; invalid timestamps are an error

**Returns:** `{ screenshots: [{ id, path, timestamp, source, format, dimensions }] }`
