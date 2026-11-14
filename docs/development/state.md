# selah — Current State

> Refreshed every release and at each port milestone. CLAUDE.md is preferences/process/procedures (durable); this
> file is **state** (volatile).

## Version

**0.30.0** in `VERSION`: the final Rust release (tag `0.30.0`, commit `1a494b4`), frozen in `rust-old/` as the parity
oracle. The Cyrius port is in progress and ships as **1.0.0** at M7. Plan of record:
[`selah-port-plan.md`](selah-port-plan.md).

## Toolchain

- **Cyrius pin**: `6.7.18` (`cyrius.cyml [package].cyrius`)
- **Oracle**: rustc 1.100.0 for `rust-old/` and `scripts/oracle/`

## Port progress

| Milestone | State |
|---|---|
| M0 Initialize | ✅ done 2026-11-14 |
| M1–M7 | not started |

## Source

- Rust oracle: `rust-old/` (7,160 lines in `src/`, 289 tests), frozen; green from its new location (2026-11-14).
- Cyrius: `src/main.cyr`, the scaffold stub. `[lib]` arrives at M1.

## Tests

- `cyrius test`: the scaffold smoke test, and `tests/parity.tcyr` replaying the golden corpus.
- Golden corpus: `decode`, 63 fixtures. Baseline (not gated until M3): 47 ok, 14 measured, 2 accepted, 0 failed,
  identical on x86_64 ELF, aarch64 (qemu) and x86_64 PE (wine). Findings F1–F5 in [`port-parity.md`](port-parity.md).

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

## Next

M1 Foundation (error, log, fmtf, json, geometry, core, uuid, rfc3339). M3 starts by settling port-parity.md §2
F1–F5.
