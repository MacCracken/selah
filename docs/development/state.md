# selah — Current State

> Refreshed every release and at each port milestone. CLAUDE.md is preferences/process/procedures (durable); this
> file is **state** (volatile).

## Version

**0.30.0** in `VERSION`: the final Rust release (tag `0.30.0`, commit `1a494b4`), frozen in `rust-old/` as the
reference oracle. Accuracy is the bar, and Rust is the floor ([ADR 0003](../adr/0003-accuracy-is-the-bar.md)). The Cyrius port is in progress and ships as **1.0.0** at M7. Plan of record:
[`selah-port-plan.md`](selah-port-plan.md).

## Toolchain

- **Cyrius pin**: `6.7.18` (`cyrius.cyml [package].cyrius`)
- **Oracle**: rustc 1.100.0 for `rust-old/` and `scripts/oracle/`

## Port progress

| Milestone | State |
|---|---|
| M0 Initialize | ✅ done 2026-11-14 |
| M1 Foundation | ✅ done 2026-11-14 |
| M2 PII and smart crop | ✅ done 2026-11-14 |
| M3 Codecs | ✅ done 2026-11-14 |
| M4–M7 | not started |

## Source

- Rust oracle: `rust-old/` (7,160 lines in `src/`, 289 tests), frozen; green from its new location (2026-11-14).
- Cyrius: `src/` holds error, log, fmtf, json, uuid, rfc3339, geometry, core, ai, the generated ai_data, png_enc,
  bmp_enc, jpeg_enc and codec (about 4,360 lines), bundled as `dist/selah.cyr` (`[lib]`); `src/main.cyr` is the
  build-smoke stub.
- Surface parity: 40/40 (`scripts/surface_parity.py`): error, geometry, core and ai whole, and annotate.rs's codec
  half (`convert_format`, `MAX_IMAGE_DIMENSION`, `MAX_DECODE_ALLOC`). The rest of annotate, and lib, fsutil, http,
  history, capture, daimon and mcp, are pending their milestones.
- **Known limits, each filed with chitra:**
  - every decode's memory stays with the process (L16), which matters for the MCP server;
  - images over 16.7 MP are refused (L17);
  - decoding runs about 20× slower than Rust (L18).

## Tests

- `cyrius test`: `tests/parity.tcyr` (the golden corpus), the ported Rust tests (`error.tcyr`, `geometry.tcyr`,
  `core.tcyr`, `ai.tcyr`, `codec.tcyr`: 111 Rust tests and 9 doctests, 313 assertions), and `selah.tcyr` (the
  bundle, as a consumer sees it).
- Golden corpus: 43,844 rows, every module gated, the harness output identical on x86_64 ELF, aarch64 (qemu) and
  x86_64 PE (wine).
  - M1's 34,863 and `ai`'s 8,453 are all class A and all pass.
  - `decode` (64): 53 ok, 3 accepted (ADR 0002 #1, #2, #16), 8 upstream: the known-defect list, under chitra issues
    for F1, F3 and the pixel cap.
  - `codec` (464): 349 ok, 115 accepted (WebP).
  - `scripts/oracle -- --reverse`: Rust decodes all 159 Cyrius-encoded files.
  - See [`port-parity.md`](port-parity.md) §2, §5 and §6.
- Benches: 10 of 13 criterion benches ported, plus 6 codec benches with Rust numbers from `oracle --bench`
  ([`../benchmarks-rust-v-cyrius.md`](../benchmarks-rust-v-cyrius.md)).

## Dependencies

Declared in `cyrius.cyml`, vendored in `lib/` and pinned in `cyrius.lock`:

- stdlib: syscalls, string, alloc, str, fmt, io, fs, vec, hashmap, math, tagged, result, freelist, chrono, random,
  process, thread, bayan, sandhi, sakshi
- chitra 1.3.1 (brings sankoch 2.8.9's zlib profile), ranga 2.4.0
- optional, feature `mcp`: bote 3.8.15 (brings libro 2.11.3, majra 2.13.0, patra 1.16.6, sigil 3.13.15)
- test scope: assert, bench

Every build prints the tolerated `warning: large static data` advisory (the deps' aggregate tables; CLAUDE.md).

## Consumers

taswir, aethersafta, kiran (planned): none is in Cyrius yet. Rust consumers stay on 0.30.x.

## Waiting on other repos

- daimon: `/v1/screen/*` routes (`daimon/docs/development/issues/2026-11-14-selah-screen-capture-routes.md`)
- hoosh: image parts for non-OpenAI providers, the 1 MiB cap
  (`hoosh/docs/development/issues/2026-11-14-selah-vision-image-parts-reach-only-openai-shaped-providers.md`)
- sandhi: `total_ms` and a blocked read (`sandhi/docs/development/issues/2026-11-11-total-ms-does-not-bound-a-blocked-first-read.md`)
- cyrius: RFC 3339 with fractions and offsets in `lib/chrono.cyr` (`cyrius/docs/development/issues/2026-11-14-chrono-rfc3339-has-no-fraction-or-offset.md`)
- chitra (all `chitra/docs/development/issues/2026-11-14-*`):
  - interpolated chroma upsampling (`box-chroma-upsampling-loses-accuracy`, F1);
  - rounded sub-8-bit BMP channels (`bmp-sub-8-bit-replication-is-not-the-nearest-value`, F3);
  - opaque 32-bpp `BI_RGB` (`bmp-bi-rgb-32bpp-unused-byte-read-as-alpha`, F4);
  - CMYK samples (`cmyk-ycck-jpeg-refused`, F5);
  - progressive JPEG (`progressive-jpeg-has-a-consumer`);
  - memory (`every-decode-leaks-in-a-long-running-consumer`, L16);
  - the 16.7 MP cap (`fixed-16-mp-cap-refuses-screenshots`, L17);
  - decode speed (`decode-throughput-is-about-20x-rust-image`, L18)

## Next

M4 Annotation (raster, annotate, the selah facade). It draws onto `SlImage` pixels from `src/codec.cyr`, so
`render_to_image` inherits the codec gates. No decision is open: the maintainer approved M3's JPEG encoder bound,
which gates on the exact mathematics ([`port-parity.md`](port-parity.md) §5).
