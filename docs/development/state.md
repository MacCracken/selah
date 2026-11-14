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
| M4 Annotation | ✅ done 2026-11-14 |
| M5–M7 | not started |

## Source

- Rust oracle: `rust-old/` (7,160 lines in `src/`, 289 tests), frozen; green from its new location (2026-11-14).
- Cyrius: `src/` holds error, log, fmtf, json, uuid, rfc3339, geometry, core, ai, the generated ai_data, png_enc,
  bmp_enc, jpeg_enc, codec, fsutil, raster, annotate and the selah facade (about 5,680 lines), bundled as
  `dist/selah.cyr` (`[lib]`); `src/main.cyr` is the build-smoke stub.
- Surface parity: 56/56 (`scripts/surface_parity.py`): error, geometry, core, ai, annotate, lib (the facade) and
  fsutil whole. http, history, capture, daimon and mcp are pending their milestones.
- **Known limits, each filed upstream:**
  - every decode's memory stays with the process (chitra, L16), which matters for the MCP server, and so does every
    base64 decode (bayan, L19);
  - images over 16.7 MP are refused (chitra, L17);
  - decoding runs about 20× slower than Rust (chitra, L18), and every PNG compression pays about 110 µs up front
    (sankoch, L20).

## Tests

- `cyrius test`: `tests/parity.tcyr` (the golden corpus), the ported Rust tests (`error`, `geometry`, `core`, `ai`,
  `codec`, `annotate`, `fsutil` and `integration`: 170 Rust tests and 13 doctests, 443 assertions), and `selah.tcyr`
  (the bundle, as a consumer sees it).
- Golden corpus: 47,323 rows, every module gated, the harness output identical on x86_64 ELF, aarch64 (qemu) and
  x86_64 PE (wine).
  - M1's 34,863 and `ai`'s 8,457 are all class A and all pass.
  - `decode` (66): 55 ok, 3 accepted (ADR 0002 #1, #2, #16), 8 upstream (F1, F3 and the pixel cap, filed with chitra).
  - `codec` (480): 361 ok, 119 accepted (WebP).
  - `annotate` (3,457): 3,445 ok, 4 accepted (WebP), 8 upstream (the 16-bpp BMPs under F3).
  - `scripts/oracle -- --reverse`: Rust decodes all 165 Cyrius-encoded images and loads all 45 Cyrius-written layers.
  - See [`port-parity.md`](port-parity.md) §2, §5, §6 and §7.
- Benches: all 13 criterion benches ported, plus 6 codec benches with Rust numbers from `oracle --bench`
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
- bayan: base64 decoding with an allocator (`bayan/docs/development/issues/2026-11-14-base64-decode-has-no-allocator-form.md`,
  L19)
- sankoch: the per-call LZ77 table reset and the deflate rate
  (`sankoch/docs/development/issues/2026-11-14-every-compress-call-pays-110-us-to-reset-the-lz77-tables.md`, L20)

## Next

M5 System and clients: history (JSONL with `xflock`, on fsutil, which is already here), the clipboard spawn helper
(L3), the HTTP layer over sandhi, capture, and the daimon and hoosh clients retargeted (D2). Live capture waits on
daimon's routes (L9).
