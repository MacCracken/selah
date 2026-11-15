# Selah Roadmap

> Volatile state lives in [`state.md`](state.md); finished work is in [`CHANGELOG.md`](../../CHANGELOG.md). The
> Cyrius port (M0–M7, 1.0.0) is complete; its plan of record, with the conversion ledger the `L` numbers below refer
> to, is [`selah-port-plan.md`](selah-port-plan.md). The Rust crate's roadmap through 0.30.0 is in git history and
> `rust-old/`; its open items are carried into the versions below.

## 1.0.x — upstream fixes, picked up as they land

Patch releases: each item retires a workaround or a limit when the named issue lands upstream, with no API change.
Every issue is filed in its own repository, with selah's measurement.

- [ ] bote: a setter for the stdio line limit (L23,
  `bote/docs/development/issues/2026-11-14-stdio-line-limit-has-no-setter.md`). selah sets 4 MiB, and ADR 0002 #25's
  limit of about 1,950 annotations a call goes
- [ ] sandhi: the response cap grown as data arrives, and an over-cap answer named (L22,
  `sandhi/docs/development/issues/2026-11-14-buffered-client-reserves-the-whole-response-cap.md`). The body limit
  stops carrying 64 KiB of head room, and "too large" gets its own message
- [ ] sandhi: `total_ms` bounding a blocked read (L10,
  `sandhi/docs/development/issues/2026-11-11-total-ms-does-not-bound-a-blocked-first-read.md`)
- [ ] bayan: base64 with an allocator (L19, `bayan/docs/development/issues/2026-11-14-base64-decode-has-no-allocator-form.md`).
  `selah_decode_image_data` stops copying, and the MCP server stops growing with each base64 decode
- [ ] bayan: JSON parse throughput (L21, `bayan/docs/development/issues/2026-11-14-json-value-parse-is-15-to-25-ns-per-byte.md`),
  two thirds of reading a history line
- [ ] sankoch: PNG decode speed, now mostly its inflate (L18,
  `sankoch/docs/development/issues/2026-11-14-inflate-is-most-of-a-png-decode.md`)
- [ ] sankoch: the per-call LZ77 table reset (L20,
  `sankoch/docs/development/issues/2026-11-14-every-compress-call-pays-110-us-to-reset-the-lz77-tables.md`), about
  half of a small PNG encode
- [ ] cyrius: freelist reuse of blocks over 4 KiB (L24,
  `cyrius/docs/development/issues/2026-11-11-freelist-maps-every-block-over-4k.md`). The scratch arenas' spare chunk
  goes
- [ ] cyrius: a spawn that feeds stdin (L3, `cyrius/docs/development/issues/2026-11-14-process-has-no-spawn-that-feeds-stdin.md`),
  which replaces `src/spawn.cyr`
- [ ] cyrius: RFC 3339 with fractions and offsets in `lib/chrono.cyr` (L5,
  `cyrius/docs/development/issues/2026-11-14-chrono-rfc3339-has-no-fraction-or-offset.md`), which replaces most of
  `src/rfc3339.cyr`
- [ ] cyrius: a stable sort (L4), a shortest float printer (L1), and `#derive(Serialize)` fixed and byte-identical to
  serde_json (L2), which retire the local versions in `history`, `fmtf` and `json`

## 1.1.0 — the selah-side items, unblocked now

- [ ] **CMYK and YCCK JPEG.** chitra 1.4.0's `chitra_jpeg_decode_cmyk` returns the samples and the Adobe transform;
  selah converts them to RGB, approximate unless the file carries an ICC profile, measured against the CMYK
  fixture's source and Rust. Closes ADR 0002 #16 (L14)
- [ ] **A format-aware metadata scan for the stub OCR**: skip compressed pixel data (PNG `IDAT`, JPEG scans), so the
  scan stops finding "text" in it (Rust roadmap)
- [ ] **MCP: optional path confinement** to an allowed root directory (Rust roadmap)
- [ ] **MCP: a per-call rendering work budget** for `selah_annotate` (Rust roadmap)

## 1.2.0 — the live services

- [ ] **Capture against a live daimon** once its `/v1/screen/*` routes ship (L9,
  `daimon/docs/development/issues/2026-11-14-selah-screen-capture-routes.md`). If daimon's contract differs from
  Rust's, retarget `CaptureClient` under an ADR 0002 row
- [ ] **OCR on more providers** once hoosh translates image parts beyond OpenAI-compatible models (L8,
  `hoosh/docs/development/issues/2026-11-14-selah-vision-image-parts-reach-only-openai-shaped-providers.md`); relax
  the downscale if the 1 MiB cap moves
- [ ] **Redaction that sees the pixels**: map OCR bounding boxes to PII matches, so `selah_redact` redacts text
  rendered in the image instead of failing closed (Rust roadmap)

## 1.3.0 — WebP, and retiring the oracle

- [ ] **WebP decode and encode** (ADR 0002 #1), in selah or a sibling codec package; the Rust oracle covers it
  (`image` decodes lossy and lossless, encodes lossless)
- [ ] **Remove `rust-old/`** once WebP is done or explicitly dropped and the corpus and its generator are archived
  (port plan §7)
- [ ] **A shared encoder package** (L6): selah's PNG, BMP and JPEG writers, which aethersafta and kiran would
  otherwise each carry. Where it lives is the maintainer's call

## Later (1.x)

Carried from the Rust roadmap (0.25–0.26); each needs a service API that does not exist yet.

- [ ] Describe a screenshot's content through hoosh
- [ ] Smart crop by content saliency through hoosh (the rule-of-thirds crop stays the fallback)
- [ ] Annotation suggestions
- [ ] Agent lifecycle with daimon: registration is there (ADR 0002 #4); a heartbeat returns once daimon keeps
  liveness for an API agent (ADR 0002 #5)
- [ ] Index screenshots for semantic search through daimon, and query their content in natural language (RAG)
- [ ] agnoshi intents (voice-driven capture)
- [ ] t-ron as MCP security middleware

Dropped from the Rust roadmap: "adopt ranga for format I/O" (chitra decodes and selah encodes; the encoders' home is
the 1.3.0 item above).
