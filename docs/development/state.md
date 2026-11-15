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
| M5 System and clients | ✅ done 2026-11-14 |
| M6 MCP profile | ✅ done 2026-11-14 |
| M7 Release 1.0.0 | not started |

## Source

- Rust oracle: `rust-old/` (7,160 lines in `src/`, 289 tests), frozen; green from its new location (2026-11-14).
- Cyrius: `src/` holds error, version_str, log, scratch, fmtf, json, uuid, rfc3339, geometry, the generated
  debug_data, core, the generated ai_data, ai, png_enc, bmp_enc, jpeg_enc, codec, fsutil, raster, annotate, the selah
  facade, history, http, spawn, capture and daimon, bundled as `dist/selah.cyr` (`[lib]`), and mcp, which the
  `[lib.mcp]` profile adds as `dist/selah-mcp.cyr` (about 11,000 lines in all, 470 of them generated tables);
  `src/main.cyr` is the build-smoke stub.
- Surface parity: 86/86 (`scripts/surface_parity.py .`): every module whole (`heartbeat` and `with_client` removed
  by ADR 0002 #5 and #9).
- **Known limits, each filed upstream:**
  - every decode's memory stays with the process (chitra, L16), which matters for the MCP server, and so does every
    base64 decode (bayan, L19);
  - images over 16.7 MP are refused (chitra, L17);
  - decoding runs about 20× slower than Rust (chitra, L18), and every PNG compression pays about 110 µs up front
    (sankoch, L20);
  - reading JSON (the history, a capture answer) is bound by bayan's parser, 14–25 ns a byte (bayan, L21);
  - sandhi reserves a request's whole response cap, so selah hands it a freelist arena (sandhi, L22);
  - the MCP server's stdio refuses a request line over 128 KiB, about 1,950 annotations in one call (bote, L23);
  - the freelist maps every block over 4 KiB afresh, so the arenas keep one spare chunk (cyrius, L24).

## Tests

- `cyrius test` (16 suites, 794 assertions): `tests/parity.tcyr` (the golden corpus), the ported Rust tests (`error`,
  `geometry`, `core`, `ai`, `codec`, `annotate`, `fsutil`, `integration`, `history`, `http`, `capture`, `daimon` and
  `mcp`: 274 Rust tests and 14 doctests), and `selah.tcyr` (the bundle, as a consumer sees it). The HTTP tests run
  against a loopback mock server on a thread (`tests/mock_http.cyr`).
- `tests/mcp/` (the MCP suites, the MCP golden harness, the `dist/selah-mcp.cyr` smoke test and the MCP benches) runs
  under a plain `cyrius test`: its `test.cyml` prepends bote. Under `--features mcp` every bote symbol would be
  defined twice.
- Golden corpus: 48,095 rows, every module gated, the harness output identical on x86_64 ELF, aarch64 (qemu) and
  x86_64 PE (wine); the MCP harness on x86_64 and aarch64.
  - M1's 34,863 and `ai`'s 8,457 are all class A and all pass.
  - `decode` (66): 55 ok, 3 accepted (ADR 0002 #1, #2, #16), 8 upstream (F1, F3 and the pixel cap, filed with chitra).
  - `codec` (480): 361 ok, 119 accepted (WebP).
  - `annotate` (3,457): 3,445 ok, 4 accepted (WebP), 8 upstream (the 16-bpp BMPs under F3).
  - `system` (662): 654 ok, 8 accepted (WebP captures, and the ADR 0002 #21 JSON rows).
  - `mcp` (110): 106 ok, 4 accepted (WebP under ADR 0002 #1, the hoosh engine under #24), through Rust's real stdio
    server.
  - `scripts/oracle -- --reverse`: Rust decodes all 165 Cyrius-encoded images, loads all 45 Cyrius-written layers and
    lists all 3 Cyrius-written history files.
  - See [`port-parity.md`](port-parity.md) §2 and §5–§9.
- Benches: all 13 criterion benches ported, plus 6 codec, 5 M5 and 4 MCP benches with Rust numbers from
  `oracle --bench` ([`../benchmarks-rust-v-cyrius.md`](../benchmarks-rust-v-cyrius.md)).

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
- bayan: base64 decoding and encoding with an allocator
  (`bayan/docs/development/issues/2026-11-14-base64-decode-has-no-allocator-form.md`, L19), and the JSON parser's
  throughput (`bayan/docs/development/issues/2026-11-14-json-value-parse-is-15-to-25-ns-per-byte.md`, L21)
- sandhi: the response cap reserved up front, over-cap as PROTOCOL
  (`sandhi/docs/development/issues/2026-11-14-buffered-client-reserves-the-whole-response-cap.md`, L22)
- cyrius: a spawn that feeds stdin (`cyrius/docs/development/issues/2026-11-14-process-has-no-spawn-that-feeds-stdin.md`, L3)
- sankoch: the per-call LZ77 table reset and the deflate rate
  (`sankoch/docs/development/issues/2026-11-14-every-compress-call-pays-110-us-to-reset-the-lz77-tables.md`, L20)
- bote: a setter for the stdio line limit (`bote/docs/development/issues/2026-11-14-stdio-line-limit-has-no-setter.md`,
  L23)
- cyrius: large-block reuse in the freelist
  (`cyrius/docs/development/issues/2026-11-11-freelist-maps-every-block-over-4k.md`, pravash's; selah's data point
  appended, L24)

## Next

M7 Release 1.0.0: the Rust-test ledger closed (289 tests: ported, folded into golden rows, or n/a), full surface
parity, README, `docs/development.md` and the architecture doc rewritten for Cyrius, the CHANGELOG `1.0.0` entry and
`VERSION` 1.0.0. The tag is the maintainer's. Live capture still waits on daimon's routes (L9).
