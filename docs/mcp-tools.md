# MCP Tools

Selah exposes 5 MCP tools for agent-driven screenshot workflows.

## selah_capture

Take a screenshot via the daimon screen capture API.

**Parameters:**
- `region` (optional): `{ x, y, width, height }` — omit for full screen
- `format` (optional): `png` | `jpg` | `bmp` | `webp` (default: `png`)
- `output` (optional): file path to save (default: auto-generated)

**Returns:** `{ path, width, height, format, timestamp }`

## selah_annotate

Add annotations to an image.

**Parameters:**
- `image_path`: path to the source image
- `annotations`: array of `{ kind, position: { x, y, width, height }, color, text }`
- `output` (optional): output file path

**Returns:** `{ path, annotation_count }`

## selah_ocr

Extract text from an image.

**Parameters:**
- `image_path`: path to the image
- `engine` (optional): `local` | `hoosh` (default: `local`)

**Returns:** `{ text, confidence, regions: [{ text, bounding_box }] }`

## selah_redact

Detect and redact sensitive content in an image.

**Parameters:**
- `image_path`: path to the image
- `targets` (optional): array of `email` | `phone` | `credit_card` | `ip_address` (default: all)
- `output` (optional): output file path

**Returns:** `{ path, redactions: [{ target_type, confidence, matched_text }] }`

## selah_history

List recent screenshots and their metadata.

**Parameters:**
- `limit` (optional): max results (default: 20)
- `since` (optional): ISO 8601 timestamp

**Returns:** `{ screenshots: [{ id, path, timestamp, source, dimensions }] }`
