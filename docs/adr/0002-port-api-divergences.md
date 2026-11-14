# 0002 — Accept the planned API changes in the Cyrius 1.0 API

**Status**: Accepted
**Date**: 2026-11-14

## Context

Under ADR 0001 a divergence from the Rust 0.30.0 oracle is a defect unless an ADR accepts it. The port plan
(`docs/development/selah-port-plan.md` §0) records the decisions behind the changes below. D1–D3 are the maintainer's
(2026-11-14); D4–D13 are the plan's defaults, which the maintainer may overrule. There will be no Rust 1.0: Rust
consumers stay on 0.30.x, and the changes are made in the Cyrius API only.

This ADR accepts each change, and fixes how the parity harness still compares it, so that no function leaves parity
entirely. Rows are appended as later milestones realise or add divergences, and are never renumbered.

Under ADR 0003 this ADR also records the other kind of divergence: a **Rust defect** — Rust disagreeing with ground
truth or the spec — fixed in the port rather than imitated. Such rows say so, and name where the oracle emits the
corrected expectation. A temporary refusal caused by a dependency gap names the upstream issue it waits on.

## Decision

The Cyrius 1.0.0 API differs from Rust 0.30.0 in exactly these places:

| # | Rust 0.30.0 | Cyrius 1.0.0 | Golden comparison |
|---|---|---|---|
| 1 (D1) | `ImageFormat::WebP` decodes and encodes (lossy and lossless decode, lossless encode) | the enum value stays, and `extension`, `mime_type`, `from_path` and parsing keep parity. Any decode or encode of WebP returns `SL_ERR_UNSUPPORTED_FORMAT`, and so does a WebP capture request *(M5)*: the port could not decode the answer | WebP rows of the format functions class A; WebP decode and encode rows map the oracle's success to the port's `UnsupportedFormat`, noted and counted |
| 2 (D1) | progressive JPEG (SOF2) input decodes | refused with `SL_ERR_UNSUPPORTED_FORMAT`: chitra 1.3.1 refuses SOF2 (its one deferral). **Temporary:** filed as chitra `2026-11-14-progressive-jpeg-has-a-consumer`; the refusal leaves when chitra decodes SOF2 (ledger L15) | progressive fixtures: oracle success ↔ the port's `UnsupportedFormat`, noted and counted |
| 3 (D1) | GIF is refused (the `image` crate is built without it) | GIF stays refused, although chitra decodes it: selah sniffs the signature and never hands GIF to chitra | class A on the verdict (both refuse) |
| 4 (D2) | `DaimonClient::register_agent` posts to `/v1/agents/register` and reads `agent_id` | posts `{"name", "type": "Service"}` to `POST /v1/agents`, the Cyrius daimon's route, and returns its integer `id` | class W: the request and the parsing are checked against fixtures from daimon 2.5.6's handler; no Rust row |
| 5 (D2) | `DaimonClient::heartbeat` posts to `/v1/agents/{id}/heartbeat` | removed: nothing in selah calls it, daimon keeps no liveness record for an API agent, and a `"heartbeat"` message would only fill the agent's own queue | surface parity lists it as removed by this row |
| 6 (D2) | `HooshClient::ocr` posts `{"image", "task": "ocr"}` to `/v1/vision/ocr` | posts an OpenAI content-part chat request to `/v1/chat/completions` (`HooshConfig.model` names the model). The image is downscaled and JPEG-encoded to fit hoosh's 1 MiB body cap, and regions are scaled back. `HooshConfig.model` has no default (hoosh refuses a chat request without one), so `ocr` refuses before sending until it is set. The reply is the first choice's content; a Markdown code fence around it is removed, and its JSON is read as Rust read `/v1/vision/ocr`'s *(M5: 28 `ocr_answer` rows and 2 `wire_ocr` rows)* | the request is class W; the reply parsing (`confidence` clamp, `parse_region`) is class A against the oracle on the same JSON |
| 7 (D2) | `CaptureClient` sends `agent_id: "selah"` | the agent id is a config field, defaulting to `"selah"`; the routes and bodies are Rust's | class A on the request body for the default; the waiting daimon routes are filed (`daimon/docs/development/issues/2026-11-14-selah-screen-capture-routes.md`) |
| 8 (D6) | every client method is `async` | blocking, with deadlines (`total_ms`, `read_ms` and `write_ms`; capture's default stays 30 s) | the results are compared; timing is not |
| 9 (D6) | `CaptureClient::with_client(reqwest::Client)` | removed; the test seam is the base URL plus the config. `SlCaptureClient_with_timeout(base_url, ms)` takes what a configured client would have set *(M5)* | surface parity lists it as removed by this row |
| 10 (D7) | `SelahError` variants carry a `String` | `SL_ERR_*` codes in a value-form `Result`; the text goes through an optional trailing `detail_out` argument | the code is class A against the variant; the text is compared where the oracle's message is deterministic, and masked (temp paths, OS error text) where it is not |
| 11 (D9) | free functions in modules (`selah::annotate_image`, `core::xml_escape`) | `selah_<rust name>`; types `Sl<Name>` with `impl` methods; constants and enum members `SL_*` | surface parity strips the prefix before diffing |
| 12 (D4) | `pub use hisab::Vec2` | `SlVec2`, local (hisab 3.4.1 has no 2D f32 rect or vector) | class A |
| 13 (D13) | the reserved no-op feature `ai` | not carried; there are no Cyrius consumers to keep building | none |
| 14 *(M1, 2026-11-14)* | a refused JSON document is a `serde_json::Error` whose message names the field and the line and column | the readers (`SlRect_from_json`, `SlColor_from_json`, `SlAnnotation_from_json`, the enums' `from_json`) refuse exactly what serde refuses, as `SL_ERR_SERDE`, with the port's own message | `E:serde` rows compare the verdict only; every accept, and every value read, is class A |
| 15 *(M1, 2026-11-14)* | `Rect::x`, `y`, `width`, `height` and `area` return `f32` | they return an `f64` holding the f32's exact value: Cyrius has no f32 return type. `f32_from` of it gives the f32 back unchanged, and the arithmetic stays f32 in Rust's order | class A on the f32 bits (`f32_from` of the result) |
| 16 *(2026-11-14)* | CMYK and YCCK JPEGs (Adobe APP14) decode, converted to RGB | refused with `SL_ERR_UNSUPPORTED_FORMAT`: chitra 1.3.1 refuses 4-component JPEG (its ADR 0009). **Temporary:** filed as chitra `2026-11-14-cmyk-ycck-jpeg-refused`, which asks for the CMYK samples without a guess; selah then converts, documented as approximate unless the file carries an ICC profile (ledger L14) | the CMYK fixture: oracle success ↔ the port's `UnsupportedFormat`, noted and counted |
| 17 *(M3, 2026-11-14)* — **Rust defect** | `convert_format` and every JPEG output go through `image` 0.25.10's JpegEncoder, whose arithmetic is approximate: its FDCT's second pass rounds the odd outputs with 2^10 where 2^14 is right (a downward bias), it truncates each coefficient to an integer before rounding the quotient, and it rounds Y, Cb and Cr to 8 bits before the transform. 0.56 % of its quantised coefficients differ from the exact values on the corpus | the same container (baseline, 4:4:4, quality 75, Annex K tables), with the coefficients computed to the mathematics: 20-bit FDCT constants, 6 fraction bits of YCbCr, round to nearest. 0.008 % of coefficients differ from the exact values, by one | class R: the coefficients against an exact f64 evaluation (none off by more than 1, at most 1 % per image); Rust's decoded accuracy is measured, not compared byte for byte (port-parity.md §5) |
| 18 *(M4, 2026-11-14; M5)* | `AnnotationCanvas::save_to_file`, and *(M5)* `CaptureClient::save_to_file`, write with `std::fs::write`: truncate in place (a crash leaves a partial file) at the umask's mode | written atomically (a sibling temp created 0600, synced, renamed over the target), as Rust's own `fsutil::write_atomic` writes the history; the plan of record (M4). Annotation text and screenshots can be sensitive | the file's bytes are class A (layer_json); Rust loads every Cyrius-written layer (`oracle --reverse`) |
| 19 *(M4, 2026-11-14)* — **Rust defect** | a canvas with an annotation whose x, y, width or height is not finite saves "successfully": serde_json writes `null`, and `load_from_file` then refuses the file (`invalid type: null, expected f64`). The layer is lost without an error | `SlAnnotationCanvas_save_to_file` refuses such a canvas with AnnotationError and writes nothing. Loading is unchanged: a file with `null` is refused, as Rust refuses it | layer_json rows for those canvases expect `E:AnnotationError`, the corrected expectation `scripts/oracle/src/m_annotate.rs` emits; layer_load rows on Rust's own such files are refused on both sides |
| 20 *(M4, 2026-11-14)* | `add_annotation` and `apply_redaction` return a `Uuid`; `remove_annotation` takes one; `get_annotations` returns `&[Annotation]`; `render_to_image`, `annotate_image`, `redact_image`, `convert_format` and `decode_image_data` return `Vec<u8>` | the id is the address of the annotation's 16 id bytes (valid while the canvas holds it), and `remove_annotation` takes such 16 bytes; the canvas owns what it is given; annotations go in and out as a vec of `*SlAnnotation`; bytes come back as a freelist buffer with its length through `len_out` (free with `selah_bytes_free`); `redact_image`'s suggestions through `suggestions_out` | the same behaviour, compared through these shapes in the harness and the ported tests |
| 21 *(M5, 2026-11-14)* | serde_json accepts a document whose defect sits only in a member it ignores: an unknown key given twice, a lone surrogate escape (`"\ud800"`) or invalid UTF-8 inside an ignored string; serde_json's `Value` keeps the last of two equal keys | refused: bayan refuses a duplicate key anywhere (its 1.11.1 policy: a validator and an actor must not disagree about a key) and a lone surrogate, and selah refuses a document that is not UTF-8. Well-formed JSON reads as serde reads it: `-0` is the float −0.0, an integer past i64 the nearest double, and an out-of-range float literal (`1e400`) in a field selah reads is refused (`src/json.cyr`) | the rows that hold such a member are tagged and pass when the port refuses (`history_line`, `history_id`, `ocr_answer`); every other JSON row is class A |
| 22 *(M5, 2026-11-14)* | reqwest's texts for a failed request (`error sending request for url (…)`), and `response too large (N bytes, limit M)` from a declared Content-Length before the body is read; `endpoint_url` re-serializes the endpoint through the url crate (lowercasing the host, dropping a default port) and accepts any scheme | the HTTP layer's own texts, naming sandhi's error kind (`timed out`, `the connection failed`); a body over the limit gets Rust's text, but one past sandhi's buffer (the limit plus 64 KiB) is sandhi's PROTOCOL error, which also covers malformed and cut-short answers (filed: sandhi `2026-11-14-buffered-client-reserves-the-whole-response-cap`); the endpoint's text is kept as given, an http(s) URL with a host is required, and the segments are encoded as the url crate encodes them. The status error (`<context> <code> <reason>: <body>…`, lossy UTF-8, the 512-byte cut) is Rust's exactly | the status texts are class A (`capture_http`, `monitors_http`); the encoded paths are class A (`agent_path`); transport texts are not compared |
| 23 *(M5, 2026-11-14)* | `copy_to_clipboard` feeds the tool's stdin from a second thread and joins it: a tool that exits without reading all of its input while a background process of its holds the pipe leaves the call waiting for ever | one poll loop writes stdin and drains stderr until the tool exits; a tool that exits successfully before taking all of its input is a write failure (`failed to write to clipboard tool stdin: …`). SIGPIPE is blocked while writing and consumed through a signalfd (Rust's runtime ignores SIGPIPE process-wide; a library cannot) | the texts (`clipboard tool exited with <ExitStatus>: …`, `failed to run <tool>: …`) and the MIME choice are class A (`exit_status`, `mime`); the spawn is tested on x86_64 and aarch64 (`tests/capture.tcyr`) |

## Consequences

- **Positive**
  - Every row says how it is still compared. Only #5, #9 and #13 leave parity, and each one removes something.
  - Rows 4, 6 and 7 make the clients work against the live Cyrius services where the routes exist. Rust 0.30.0's
    clients do not work against them.
- **Negative**
  - Rust 0.30.x callers of the removed or changed items rewrite against the Cyrius API. That is expected of a port,
    because the Cyrius API is not source-compatible anyway.
  - Capture against a live daimon waits on the filed daimon routes. OCR through non-OpenAI hoosh providers waits on
    the filed hoosh issue.

## Alternatives considered

- **Keep Rust's client wire contract unchanged:** offered to the maintainer on 2026-11-14 and declined in favour of
  retargeting.
- **Write WebP and a progressive JPEG decoder now:** declined for 1.0.0 (D1). Both are tracked in the port plan §7.
