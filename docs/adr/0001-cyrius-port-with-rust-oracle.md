# 0001 — Port to Cyrius, with Rust 0.30.0 frozen as the parity oracle

**Status**: Accepted — its parity classes and divergence rule are amended by
[0003](0003-accuracy-is-the-bar.md): accuracy is the bar, and Rust is the reference and the floor
**Date**: 2026-11-14

## Context

AGNOS libraries are moving from Rust to Cyrius. Every library selah builds on is Cyrius already: chitra (image
decode), ranga 2.4.0 (pixels), bote 3.8.15 (MCP), and the stdlib folds bayan, sandhi, sankoch and sakshi. The services
it talks to, daimon 2.5.6 and hoosh 2.12.3, are Cyrius too. tanmatra, kimiya, pravash and ushma have completed the
same kind of port by the method this ADR follows.

selah 0.30.0 is the result of the 2026-11 audit and repair sweep: leak-free redaction, decode limits, MCP path
hardening and PII detection fixes. On 2026-11-14 it was green on rustc 1.100.0: fmt, clippy and rustdoc with
`-D warnings`, `cargo audit`, `cargo deny`, and 289 tests. A translation needs an oracle it can check row by row, and
only a repaired crate can be one.

## Decision

selah 1.0.0 is a Cyrius library. The Rust 0.30.0 crate (tag `0.30.0`, commit `1a494b4`) is frozen in `rust-old/` as
the parity oracle, and nothing in it is edited.

- **Initialization.** The port was started with `cyrius port` on Cyrius 6.7.18.
- **Scope, milestones, parity policy.** As `docs/development/selah-port-plan.md`.
- **Parity classes:**
  - A: exact;
  - B: JPEG decode, within a per-channel bound measured before the first comparison;
  - R: encoder round trip;
  - W: Cyrius-only wire fixtures for the retargeted clients.
- **Divergences.** A divergence from the oracle beyond its class is a defect, unless an ADR accepts it. ADR 0002
  accepts the planned API changes.
- **Oracle mechanics.** The generators live in `scripts/oracle/` and depend on `rust-old/` by path.
  `rust-old/Cargo.lock` is committed, so the oracle always builds against the dependency versions it was released
  with.

## Consequences

- **Positive**
  - Correctness is measured, not asserted. Every in-scope public function is diffed against the repaired Rust crate.
  - The audit's regression tests travel with the code: negative-size and fractional redactions, NaN rejection,
    decode-bomb limits, MCP path refusals.
- **Negative**
  - Two toolchains are needed until the port completes: cargo for the oracle and its generators, cyrius for the
    library.
  - Encoded image bytes cannot be compared: the Rust crates' deflate and JPEG encoders make different, equally valid
    choices. Encoders are judged by round trips and a reverse oracle (Rust decodes Cyrius-encoded files) instead.
  - `rust-old/` stays in the tree until WebP is ported or dropped, the corpus is archived, and the Rust-test ledger
    is closed.
- **Neutral**
  - The 0.30.0 Rust CI, scripts, development guide and architecture overview are kept under `rust-old/`, where
    GitHub does not run the workflows.

## Alternatives considered

- **Port without an oracle** (translate and re-test by hand): rejected. Redaction and PII detection are exactly
  where a silent difference leaks data, and only a row-by-row comparison against the repaired crate shows there is
  none.
- **Keep Rust as the primary implementation and expose it to Cyrius via FFI**: rejected. The migration's goal is a
  sovereign Cyrius stack, and every dependency selah needs is already Cyrius.
