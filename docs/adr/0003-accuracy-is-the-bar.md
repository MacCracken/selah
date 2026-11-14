# 0003 — Accuracy is the bar; the Rust oracle is a reference, not the definition of correct

**Status**: Accepted
**Date**: 2026-11-14
**Amends**: [0001](0001-cyrius-port-with-rust-oracle.md) (its parity classes and its divergence rule)

## Context

ADR 0001 froze Rust 0.30.0 as the parity oracle and called any difference beyond a row's class a defect. That rule
takes Rust's output to be correct. The M0 decode baseline (`docs/development/port-parity.md` §1) showed the
assumption doesn't hold. Measured against **ground truth** (the pixels each JPEG fixture was encoded from, and the
BMP format's own definition):

- **Rust is more accurate in some places.** It uses interpolated chroma upsampling where chitra uses box (F1), it
  rounds 5- and 6-bit BMP channels where chitra replicates bits (F3), and it reads 32-bpp `BI_RGB` as opaque where
  chitra guesses alpha (F4).
- **The two are equally accurate in another place.** In F2 the IDCTs differ, but both are as close to the source as
  each other.
- **Nothing makes Rust right by construction.** When a Rust result is wrong against the spec, copying it is a
  defect too.

"Match Rust" would have made the port imitate Rust's defects. It would also have hidden real defects in Cyrius
dependencies behind questions for the maintainer.

## Decision

**The port is judged by accuracy. Rust 0.30.0 is the reference implementation and the floor, not the target.**

1. **Where ground truth exists, the port is measured against it.** Ground truth means:
   - the source pixels a fixture was encoded from (`fixtures/<file>.source.rgba`);
   - the format specification;
   - the mathematical definition of the operation.

   The port must be **at least as accurate as Rust**, measured the same way.
2. **Where Rust's behaviour is the contract, the port matches it exactly (class A).** That covers the formats
   consumers read and write (JSON, layer and history files, SVG), enum names and parsing, and accept-or-refuse
   verdicts. Here compatibility is what correct means. The same holds for lossless decode, where the spec defines
   one right answer and the oracle supplies it, once its value has been checked against the spec rather than
   assumed.
3. **A Rust defect is fixed in the port, not imitated.** "Defect" means Rust disagrees with ground truth or the
   spec. Each fix:
   - gets a row in ADR 0002 marked *Rust defect*;
   - has `scripts/oracle` emit the corrected expectation, named where it is applied, so the row stays gated.

   A 0.30.x patch release for Rust consumers is optional and separate.
4. **A defect in a Cyrius dependency is filed upstream, with the measurement.** It is not a decision for the
   maintainer. Each affected row goes on a named known-defect list in the harness, keyed by the issue's file name.
   - Listed rows report `upstream`. They do not fail the run, and they are always printed.
   - **A listed row that starts passing fails the run.** That forces the list to shrink when the fix lands.
   - Where selah can make the result accurate without duplicating the dependency, it does, through a ledger row
     (port plan §6) that leaves when the upstream fix lands.
5. **Bounds are still set before the first gated comparison, and never widened.**

**The classes, restated:**

- **A: exact.** Unchanged in tolerance. "Expected" is Rust's value, or the corrected value from rule 3.
- **B: lossy decode (JPEG).** Two gates:
  - **Accuracy.** On each fixture with known source pixels, the port's mean and maximum absolute RGB error against
    the source are no worse than the reference decoder's, plus 0.05 on the mean and 2 on the maximum.
  - **Conformance.** Where the port and the reference upsample chroma the same way, the per-channel difference is
    at most **K_jpeg = 2**. That is the IDCT rounding difference measured at M0.
- **R: round trip.** Unchanged. The lossy bound is PSNR against the source, which was always the accuracy measure.
- **W: wire.** Unchanged.

## Consequences

- **Positive**
  - The port can be more accurate than Rust, and the harness shows where it is.
  - Upstream defects are filed with data, by the port, as they are found. That happened for chitra on 2026-11-14:
    F1, F3, F4, CMYK, and progressive JPEG.
  - The known-defect list cannot rot, because a fix that lands makes a listed row fail until the row is removed
    from the list.
- **Negative**
  - The oracle now carries ground truth beside Rust's outputs (the `.source.rgba` fixtures), and in time some
    corrected expectations. Those are more generator code to keep deterministic under `--check`.
  - Until chitra's fixes land, the subsampled-JPEG and 16-bpp BMP rows report `upstream`, not `ok`.
- **Neutral**
  - Most rows are unaffected. Every M1 and M2 row is class A, and so far Rust's value is the correct one in each
    (IEEE f32 arithmetic, serde's JSON, the PII rules).

## Alternatives considered

- **Bug-for-bug parity with Rust:** rejected. It makes the port inherit Rust's mistakes, and it hides a less
  accurate dependency whenever the dependency happens to agree with Rust's error.
- **Accept each upstream difference in ADR 0002 with a widened bound:** rejected. That is widening a class to make
  a row pass, under another name. The bound would stay after the fix landed.
