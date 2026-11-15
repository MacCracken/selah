# Development

selah is a Cyrius library, ported from the Rust crate 0.30.0 that `rust-old/` keeps as the reference oracle. The
process and its rules are in [`CLAUDE.md`](../CLAUDE.md); this guide is the how-to.

## Prerequisites

- **Cyrius**, at the version `cyrius.cyml` pins (`[package].cyrius`). CI installs exactly that version.
- **Rust** (rustc 1.100, `rust-old/rust-toolchain.toml`), only to regenerate the golden corpus or take Rust's bench
  numbers. CI has no Rust job.
- **qemu-aarch64** and **wine**, to run the test binaries for aarch64 and x86_64 PE.

## Layout

| Path | What |
|---|---|
| `src/` | the library, one module a file; `src/main.cyr` is the build-smoke stub |
| `dist/selah.cyr`, `dist/selah-mcp.cyr` | the bundles consumers include (`cyrius distlib`); never edited by hand |
| `tests/*.tcyr` | the ported Rust tests, a suite per module; `parity.tcyr` replays the golden corpus; `selah.tcyr` includes the bundle as a consumer does |
| `tests/mcp/` | the MCP suites, the MCP golden harness and benches; `test.cyml` prepends bote |
| `tests/*.bcyr`, `tests/mcp/*.bcyr` | benchmarks |
| `tests/golden/` | the golden corpus, **generated** by `scripts/oracle/` |
| `scripts/oracle/` | the Rust crate that writes the corpus from `rust-old/` |
| `rust-old/` | the Rust crate 0.30.0, frozen; never edited |
| `lib/` | vendored dependencies (`cyrius deps`); never edited |

## Build and test

```sh
cyrius deps --features mcp                 # vendor lib/, bote included (the MCP tests need it)
cyrius build src/main.cyr build/selah
cyrius test                                # every suite, tests/mcp/ included
cyrius test --bounds                       # the same, bounds-checked
cyrius bench                               # tests/*.bcyr and tests/mcp/*.bcyr
```

`cyrius test tests/<suite>.tcyr` runs one suite. Never pass `--features mcp` to `cyrius test`: `tests/mcp/test.cyml`
already prepends bote there, and the feature would define every bote symbol twice.

A build and a test run must be warning-free. The one tolerated warning is `large static data`, the aggregate of the
dependencies' tables.

## The checks before a change lands

```sh
cyrius fmt --check src/*.cyr tests/*.tcyr tests/*.bcyr tests/*.cyr tests/mcp/*.tcyr tests/mcp/*.bcyr
cyrius lint src/*.cyr tests/mcp/*.tcyr
cyrius vet src/main.cyr
cyrius distlib --all && cyrius distlib --check
cyrius deps --locked && cyrius deps --locked --features mcp
cyrius audit                               # fmt, lint, docs, test, bench
python3 -I scripts/surface_parity.py .     # every Rust pub item has a counterpart, or an ADR 0002 row
python3 -I scripts/rust_test_ledger.py . --check
python3 -I scripts/check_readme_example.py .
```

`cyrius audit` also requires a doc comment on every public function (no leading underscore) in `src/` and `tests/`,
a test's `main` included. CI runs all of these, and checks
that `src/` has no `@unsafe` and nothing that aborts (`+?`, `-?`, `*?`, `assert_fatal`, `result_unwrap`).

## The golden corpus

`scripts/oracle/` drives the Rust crate over each module's grid and writes `tests/golden/<module>.tsv`, with the
fixtures and blobs beside them; `tests/parity.tcyr` (and `tests/mcp/parity_mcp.tcyr` for the MCP server) replays every
row against the port. The format and the modules are in [`scripts/oracle/README.md`](../scripts/oracle/README.md);
the parity classes, bounds and findings in [`development/port-parity.md`](development/port-parity.md).

```sh
cargo run --release --locked --manifest-path scripts/oracle/Cargo.toml              # regenerate
cargo run --release --locked --manifest-path scripts/oracle/Cargo.toml -- --check   # the corpus is current
cargo run --release --locked --manifest-path scripts/oracle/Cargo.toml -- --reverse # after cyrius test
cargo run --release --locked --manifest-path scripts/oracle/Cargo.toml -- --bench   # after cyrius bench
```

- Generated files (`tests/golden/`, `src/ai_data.cyr`, `src/debug_data.cyr`) are never edited by hand.
- A parity class is never widened to make a row pass; a bound is set before a function's first comparison.
- Where the port is right and Rust is wrong, the port keeps the right answer, ADR 0002 records it, and the oracle
  emits the corrected expectation ([ADR 0003](adr/0003-accuracy-is-the-bar.md)).
- A defect in a dependency is filed in that repository with the measurement, and any local stand-in gets a ledger
  row and an `UPSTREAM(Ln)` comment at its site ([port plan §6](development/selah-port-plan.md)).

## Other targets

The harness output must be identical on x86_64 ELF, aarch64 and x86_64 PE. The MCP suites run on x86_64 and aarch64
only: the MCP profile does not build for PE, because bote's dependency majra does not target Windows.

```sh
cyrius build --aarch64 tests/parity.tcyr build/parity-a64 && qemu-aarch64 build/parity-a64
cyrius build --win tests/parity.tcyr build/parity.exe && wine build/parity.exe
```

A suite that relies on `test.cyml` or on the test stdlib needs those includes when built outside `cyrius test`: put
`include "lib/assert.cyr"` (and, for `tests/mcp/`, `lib/patra.cyr`, `lib/libro.cyr`, `lib/majra.cyr`,
`lib/bote.cyr`) in a wrapper under `build/` that then includes the suite.

## Benchmarks

```sh
cyrius bench                                  # print
./scripts/bench-history.sh                    # append to bench-history.csv
```

[`benchmarks-rust-v-cyrius.md`](benchmarks-rust-v-cyrius.md) sets every bench beside Rust's: the criterion medians
of `rust-old/bench-history.csv`, and `oracle --bench` for the benches Rust's suite lacks. Compare on a quiet machine:
the codec benches double under a neighbour's load. A performance claim needs a before and an after on the same bench.

## Releasing

1. `VERSION` holds the version, and `cyrius.cyml` reads it. `src/version_str.cyr`'s `SL_VERSION` (the User-Agent
   and the MCP `serverInfo`) must hold the same text; `tests/selah.tcyr` fails otherwise.
2. A `CHANGELOG.md` section `## [X.Y.Z] - YYYY-MM-DD`.
3. `cyrius distlib --all`, then every check above, on all three targets.
4. The maintainer tags `X.Y.Z` and pushes. `release.yml` runs CI, checks the tag against `VERSION`, and publishes
   the source tarball with its SHA256SUMS, which `cyrius deps` fetches downstream.
