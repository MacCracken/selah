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
| M3–M7 | not started |

## Source

- Rust oracle: `rust-old/` (7,160 lines in `src/`, 289 tests), frozen; green from its new location (2026-11-14).
- Cyrius: `src/` holds error, log, fmtf, json, uuid, rfc3339, geometry, core, ai and the generated ai_data (about
  3,100 lines), bundled as
  `dist/selah.cyr` (`[lib]`); `src/main.cyr` is the build-smoke stub.
- Surface parity: 37/37 for the ported modules (`scripts/surface_parity.py`); annotate, lib, fsutil, http,
  history, capture, daimon and mcp are pending their milestones.

## Tests

- `cyrius test`: `tests/parity.tcyr` (the golden corpus), the ported Rust tests (`error.tcyr`, `geometry.tcyr`,
  `core.tcyr`, `ai.tcyr`: 103 Rust tests and 8 doctests, 256 assertions), and `selah.tcyr` (the bundle, as a
  consumer sees it).
- Golden corpus: 43,377 rows. M1's 34,863 and M2's 8,451 are all class A and all pass, identical on x86_64 ELF,
  aarch64 (qemu) and x86_64 PE (wine). `decode` (63, not gated until M3): 47 ok, 13 measured, 3 accepted. Findings F1–F5 are measured
  against the source pixels of the 13 JPEG fixtures and filed with chitra ([`port-parity.md`](port-parity.md) §2).
- Benches: 10 of 13 ported, recorded against Rust in [`../benchmarks-rust-v-cyrius.md`](../benchmarks-rust-v-cyrius.md).

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
  - progressive JPEG (`progressive-jpeg-has-a-consumer`)

## Next

M3 Codecs, with nothing waiting on a decision. M3:
- gates `decode` under ADR 0003: class B's accuracy and conformance bounds (port-parity.md §5);
- puts the F1 and F3 rows on the known-defect list until chitra lands its fixes;
- adds the F4 opacity guard (L11);
- refuses CMYK and progressive JPEG (ADR 0002 #16, #2);
- writes the PNG, BMP and JPEG encoders.
