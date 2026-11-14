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
| 1 (D1) | `ImageFormat::WebP` decodes and encodes (lossy and lossless decode, lossless encode) | the enum value stays, and `extension`, `mime_type`, `from_path` and parsing keep parity. Any decode or encode of WebP returns `SL_ERR_UNSUPPORTED_FORMAT` | WebP rows of the format functions class A; WebP decode and encode rows map the oracle's success to the port's `UnsupportedFormat`, noted and counted |
| 2 (D1) | progressive JPEG (SOF2) input decodes | refused with `SL_ERR_UNSUPPORTED_FORMAT`: chitra 1.3.1 refuses SOF2 (its one deferral). **Temporary:** filed as chitra `2026-11-14-progressive-jpeg-has-a-consumer`; the refusal leaves when chitra decodes SOF2 (ledger L15) | progressive fixtures: oracle success ↔ the port's `UnsupportedFormat`, noted and counted |
| 3 (D1) | GIF is refused (the `image` crate is built without it) | GIF stays refused, although chitra decodes it: selah sniffs the signature and never hands GIF to chitra | class A on the verdict (both refuse) |
| 4 (D2) | `DaimonClient::register_agent` posts to `/v1/agents/register` and reads `agent_id` | posts `{"name", "type": "Service"}` to `POST /v1/agents`, the Cyrius daimon's route, and returns its integer `id` | class W: the request and the parsing are checked against fixtures from daimon 2.5.6's handler; no Rust row |
| 5 (D2) | `DaimonClient::heartbeat` posts to `/v1/agents/{id}/heartbeat` | removed: nothing in selah calls it, daimon keeps no liveness record for an API agent, and a `"heartbeat"` message would only fill the agent's own queue | surface parity lists it as removed by this row |
| 6 (D2) | `HooshClient::ocr` posts `{"image", "task": "ocr"}` to `/v1/vision/ocr` | posts an OpenAI content-part chat request to `/v1/chat/completions` (`HooshConfig.model` names the model). The image is downscaled and JPEG-encoded to fit hoosh's 1 MiB body cap, and regions are scaled back. The reply's JSON is validated as Rust validates `/v1/vision/ocr`'s | the request is class W; the reply parsing (`confidence` clamp, `parse_region`) is class A against the oracle on the same JSON |
| 7 (D2) | `CaptureClient` sends `agent_id: "selah"` | the agent id is a config field, defaulting to `"selah"`; the routes and bodies are Rust's | class A on the request body for the default; the waiting daimon routes are filed (`daimon/docs/development/issues/2026-11-14-selah-screen-capture-routes.md`) |
| 8 (D6) | every client method is `async` | blocking, with deadlines (`total_ms`, `read_ms` and `write_ms`; capture's default stays 30 s) | the results are compared; timing is not |
| 9 (D6) | `CaptureClient::with_client(reqwest::Client)` | removed; the test seam is the base URL plus the config | surface parity lists it as removed by this row |
| 10 (D7) | `SelahError` variants carry a `String` | `SL_ERR_*` codes in a value-form `Result`; the text goes through an optional trailing `detail_out` argument | the code is class A against the variant; the text is compared where the oracle's message is deterministic, and masked (temp paths, OS error text) where it is not |
| 11 (D9) | free functions in modules (`selah::annotate_image`, `core::xml_escape`) | `selah_<rust name>`; types `Sl<Name>` with `impl` methods; constants and enum members `SL_*` | surface parity strips the prefix before diffing |
| 12 (D4) | `pub use hisab::Vec2` | `SlVec2`, local (hisab 3.4.1 has no 2D f32 rect or vector) | class A |
| 13 (D13) | the reserved no-op feature `ai` | not carried; there are no Cyrius consumers to keep building | none |
| 14 *(M1, 2026-11-14)* | a refused JSON document is a `serde_json::Error` whose message names the field and the line and column | the readers (`SlRect_from_json`, `SlColor_from_json`, `SlAnnotation_from_json`, the enums' `from_json`) refuse exactly what serde refuses, as `SL_ERR_SERDE`, with the port's own message | `E:serde` rows compare the verdict only; every accept, and every value read, is class A |
| 15 *(M1, 2026-11-14)* | `Rect::x`, `y`, `width`, `height` and `area` return `f32` | they return an `f64` holding the f32's exact value: Cyrius has no f32 return type. `f32_from` of it gives the f32 back unchanged, and the arithmetic stays f32 in Rust's order | class A on the f32 bits (`f32_from` of the result) |
| 16 *(2026-11-14)* | CMYK and YCCK JPEGs (Adobe APP14) decode, converted to RGB | refused with `SL_ERR_UNSUPPORTED_FORMAT`: chitra 1.3.1 refuses 4-component JPEG (its ADR 0009). **Temporary:** filed as chitra `2026-11-14-cmyk-ycck-jpeg-refused`, which asks for the CMYK samples without a guess; selah then converts, documented as approximate unless the file carries an ICC profile (ledger L14) | the CMYK fixture: oracle success ↔ the port's `UnsupportedFormat`, noted and counted |

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
