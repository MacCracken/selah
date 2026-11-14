# selah Rust → Cyrius port plan

> **Plan of record** for porting selah to Cyrius, drafted 2026-11-14 against the installed Cyrius 6.7.18 toolchain.
> The Rust tree is cut as the **final Rust release, 0.30.0** (0.29.4 plus the 2026-11 audit work), and frozen in
> `rust-old/` as the **reference oracle**. The port is judged by accuracy; Rust is the floor it must meet, not
> behaviour to imitate ([ADR 0003](../adr/0003-accuracy-is-the-bar.md)). The Cyrius port ships as **1.0.0**.
>
> The shape follows tanmatra's plan (`../tanmatra/docs/development/tanmatra-port-plan.md`), as kimiya, pravash and
> ushma did. selah differs from those math libraries in where the risk sits. Its numerics are a few f32 rectangle
> operations. The risk is in image codecs, JSON that has to match `serde_json` byte for byte, two HTTP services whose
> Cyrius versions serve different routes, and an MCP server.

| Thing | Value | Source |
|---|---|---|
| Rust source (oracle) | selah **0.30.0**, the final Rust release (0.29.4 plus the 2026-11 audit work, cut 2026-11-14). 12 files and 7,160 lines in `src/`, 3,853 of them before the test modules. **289 tests**: 258 unit, 17 integration, 14 doc. 13 criterion benches. Green on rustc 1.100.0, re-run 2026-11-14 (M0 step 0) | `src/`, `CHANGELOG.md`, `docs/development/roadmap.md` |
| Cyrius toolchain | **6.7.18** | `~/.cyrius/current` |
| Image decode | **chitra 1.3.1** (`dist/chitra.cyr`): PNG (every depth and colour type, Adam7), baseline JPEG, BMP and GIF, all to RGBA8. **Decode-only by policy** | `../chitra/README.md` |
| Pixels | **ranga 2.4.0** (`dist/ranga.cyr`, the core profile): pixel buffers, `blend_row` | `../ranga/src/pixel.cyr`, `blend.cyr` |
| MCP | **bote 3.8.15** (`dist/bote.cyr`, the full bundle, because the stdio transport isn't in `bote-core`) | `../bote/dist/` |
| Stdlib folds used | bayan 1.11.1 (JSON, base64), sandhi 1.13.7 (HTTP client), sakshi 2.5.10 (tracing). sankoch 2.8.9 (zlib, CRC-32) comes in as chitra's `dist/sankoch-zlib.cyr` profile, not the full fold (M0 probe 4c) | `../cyrius/docs/ecosystem.md` |
| Not used | **hisab 3.4.1**. Its only 2D surface is `HVec2` (f64, heap-allocated) and a quadtree; it has no 2D rect, and its bundle is 3.6 MB | `../hisab/src/geo.cyr` |
| Services | **daimon 2.5.6** has no `/v1/screen/*` routes (filed, below). **hoosh 2.12.3** serves `/v1/chat/completions`, but image parts reach only OpenAI-shaped providers, and request bodies are capped at 1 MiB (filed) | `../daimon/src/router.cyr`, `../hoosh/src/lib/provider.cyr` |
| Consumers | **None in Cyrius.** taswir has no local repo. aethersafta is Rust 0.50.0 and names selah only in its docs. aethersafha (Cyrius) does its own capture. Rust consumers stay on the final Rust release | sibling repos |
| Target | selah **1.0.0**, Cyrius | this document |

---

## 0. Decisions

### 0.1 The maintainer's (2026-11-14)

- **D1. Formats: PNG, JPEG and BMP, in and out. WebP comes later.**
  - **Decoding** is chitra's. selah sniffs the signature itself and calls chitra only for PNG, JPEG and BMP. **GIF stays
    refused** even though chitra decodes it: the Rust audit deliberately kept every decoder selah doesn't support off
    the untrusted-input path.
  - **Encoding** is selah's, because chitra refuses it permanently:
    - PNG, transcribed from kii 2.7.0's `src/png.cyr` (its ADR 0014);
    - BMP;
    - a new baseline JPEG encoder.
  - **WebP** in either direction returns `SL_ERR_UNSUPPORTED_FORMAT`. A WebP codec is post-1.0 work.
  - **Progressive JPEG input** is a divergence: chitra refuses SOF2 (its one remaining deferral), and Rust's `image`
    decodes it. Recorded in ADR 0002.
- **D2. The clients are retargeted to the live Cyrius services during the port.** Each change is recorded in ADR 0002.
  - **`DaimonClient.register_agent`** posts to `POST /v1/agents` with `{"name", "type": "Service"}` and returns the
    integer `id`. Rust posted to `/v1/agents/register`, which the Cyrius daimon doesn't serve.
  - **`DaimonClient.heartbeat` is dropped.** *(This is a default taken while filing, not one of the questions asked;
    overrule it if wanted.)* Nothing in selah calls it, and daimon keeps no liveness record for an API agent. A
    `"heartbeat"` message on `/v1/agents/{id}/messages` would only fill the agent's own queue. It comes back if daimon
    gains agent liveness.
  - **`HooshClient.ocr`** moves to `POST /v1/chat/completions`.
    - **Request:** one user message whose content is an array: a text part with the OCR instruction, and an
      `image_url` part holding a `data:` URL. The model comes from a new `HooshConfig.model`.
    - **Expected answer:** the first choice's content as JSON, `{"text","confidence","regions":[…]}`.
    - **Validation is unchanged from Rust:** confidence clamped to 0..1, and `parse_region`'s filtering and
      normalizing.
    - **Body cap:** the image is downscaled and JPEG-encoded until the request fits hoosh's 1 MiB cap. Regions are
      scaled back to source coordinates.
  - **`CaptureClient`** keeps Rust's `/v1/screen/capture` and `/v1/screen/monitors` contract.
    - Its `agent_id` becomes a config field. The default is Rust's constant `"selah"`, and the registered id is used
      when one is given.
    - It **waits on daimon** adding the routes.
  - **Filed 2026-11-14,** limited to what selah truly needs:
    - `../daimon/docs/development/issues/2026-11-14-selah-screen-capture-routes.md`: the two screen routes, and
      nothing else; registration needs no new route and heartbeat is dropped;
    - `../hoosh/docs/development/issues/2026-11-14-selah-vision-image-parts-reach-only-openai-shaped-providers.md`: no
      new route; image parts on the Ollama, Anthropic and Gemini paths, and the 1 MiB cap.
- **D3. Version 1.0.0.** This is a major bump without resetting, as every AGNOS port did (tanmatra and kimiya 2.0.0,
  ranga 2.0.0). The `[Unreleased]` Rust work is cut first as the final Rust release; its version and tag are the
  maintainer's (M0 step 0).

### 0.2 Defaults taken by this plan (the maintainer may overrule any of them)

- **D4. Geometry is local.**
  - `SlRect` keeps hisab's Rust layout, `{min, max}` in **f32**. So `width = max.x − min.x` and `new(x, y, w, h)`
    stores `x + w` rounded as Rust rounds it. That is what makes rect rows bit-exact.
  - `SlVec2` is local too.
  - There is no hisab dependency, because hisab has no 2D rect. Revisit if hisab grows one.
- **D5. f32 semantics are kept**, so layer files and SVG stay byte-compatible with Rust.
  - **Reading JSON:** parse the number as f64, then narrow it to f32, which is exactly Rust's `RectRepr { x: f64 } … as
    f32`. That double rounding is Rust's own, so no f32 decimal parser is needed.
  - **Writing JSON:** the shortest f32 digits in serde_json's form (`10.0`).
  - **Writing SVG:** Rust's `Display` form (`10`).
  - **Printer:** transcribed from pravash's `src/fmtf.cyr`, which is impetus's Burger & Dybvig printer with the f32
    case added (ledger L1).
- **D6. The API is blocking.** Every Rust `async fn` becomes a blocking call with a deadline. Capture keeps Rust's 30 s
  default. There is no tokio: bote's stdio loop is synchronous.
- **D7. Errors.**
  - **Codes:** `enum SlError`, with append-only values, returned in a value-form `Result`.
  - **Detail:** tanmatra's errors carried no detail, but selah's do: an HTTP status and body, a tool's stderr, a path.
    The MCP tools hand that text to the agent.
  - **Decided by M0 probe 4f (2026-11-14):** every fallible function takes a trailing optional
    `detail_out = 0` (6.7.7 default arguments). When it is non-zero, the failure stores its text there. The value
    passes through `?` unchanged, and a caller that doesn't want the text simply omits the argument.
  - A per-thread slot (`lib/thread_local.cyr`) was rejected. It needs every consumer's main thread to call
    `thread_local_init` first, which is a hidden contract for a library, and it is order-sensitive around
    `fdlopen`.
- **D8. JSON is hand-written on bayan.**
  - `#derive(Serialize)` has open issues (cyrius `2026-11-07-derive-serialize-*`, `2026-11-11-derive-*`,
    `2026-11-12-derive-serialize-in-a-second-private-file-fails-to-compile`).
  - The output must match serde_json byte for byte. `../ranga/src/serde.cyr` is the model.
- **D9. Names.**
  - Public free functions are `selah_<rust name>`, e.g. `selah_annotate_image` and `selah_suggest_redactions`.
  - Types are `Sl<Name>` with `impl` methods.
  - Constants and enum members are `SL_*`. Private helpers are `_sl_*`.
  - The reason is one flat namespace and cyrius issue `2026-11-11-duplicate-fn-across-bundles-last-wins`: a consumer
    that also defines `xml_escape` would silently replace selah's.
  - Surface parity strips the prefix before diffing.
- **D10. Memory.**
  - Decoded images and pixel buffers go on the freelist (`fl_alloc` / `fl_free`). Blocks over 4 KiB are mapped, so a
    freed image goes back to the OS.
  - chitra decodes with its budget variant at `MAX_DECODE_ALLOC`.
  - MCP per-call scratch lives in an arena that is reset at the start of every call.
  - All of Rust's limits carry over as constants: `MAX_IMAGE_DIMENSION`, `MAX_DECODE_ALLOC`,
    `MAX_LAYER_FILE_BYTES`, `MAX_INPUT_FILE_BYTES`, `MAX_ANNOTATIONS`, and the HTTP body caps.
- **D11. Tracing.** Logging goes through sakshi via `src/log.cyr`, in bote's `src/log.cyr` pattern: the library never
  configures sakshi.
- **D12. Clipboard.** `wl-copy` and `xclip` are fed through a local spawn helper (pipe, fork, dup2, execve), with a
  poll loop that writes stdin while draining stderr. `lib/process.cyr` has no spawn that takes stdin (ledger L3).
- **D13. Features.**
  - `mcp` becomes `[features] mcp`, an `optional = true` bote dependency and a `[lib.mcp]` profile.
  - The reserved no-op feature `ai` is not carried over, because it only existed so that Rust consumers kept
    building.

---

## Porting models and references

- **tanmatra 2.0.x**, `../tanmatra`: the template for this plan. Copy:
  - M0 with `cyrius port` and the repair afterwards;
  - `scripts/oracle/` and the golden TSVs;
  - `tests/parity.tcyr`;
  - `scripts/surface_parity.py` and `scripts/rust_test_ledger.py`;
  - the CI shape.
- **kimiya 2.0.0**, `../kimiya`: `docs/adr/0002-port-api-divergences.md` is the model for selah's ADR 0002, a
  numbered table of deliberate changes, each saying how parity still compares it.
- **ranga 2.4.0**, `../ranga`:
  - `src/serde.cyr`, for JSON identical to serde_json;
  - `scripts/bench-history.sh`, rewritten for `cyrius bench`;
  - `docs/development/port-mechanics.md`: extract doctests mechanically and build surface parity early.
- **pravash**, `../pravash`:
  - `src/fmtf.cyr`, the shortest f32/f64 printer;
  - the conversion-ledger format (§6);
  - `json.cyr` for hand-written codecs.
- **itihas**, `../itihas`: `src/mcp.cyr` lines 277–355 are the bote 3.x MCP template, and
  `docs/development/mcp-port-reference.md` goes with them. Its manifest pins bote as `optional = true` behind
  `[features] mcp`, which is the shape selah needs.
- **kii 2.7.0**, `../kii/src/png.cyr`: the PNG writer (adaptive filters, `zlib_compress` level 6, CRC-32). Its
  manifest shows chitra 1.3.1 and the stdlib `sankoch` building together.
- **aethersafha**, `../aethersafha/src`:
  - `screen_capture.cyr` `scap_encode_bmp`, for the BMP layout;
  - `daimon_client.cyr`, a Cyrius daimon client.
- **hoosh clients:** `../itihas/src/hoosh.cyr`, `../agnosai/src/llm/hoosh.cyr`, `../thoth/src/hoosh.cyr`.
- **Language:**
  - `../cyrius/docs/guides/cyrius-guide.md`, `docs/stdlib-reference.md` and `docs/api-surface.snapshot`;
  - `docs/development/ecosystem-migration.md`: read the entries at every pin bump;
  - `cyrius help manifest`.

---

## 1. Scope

### In scope for 1.0.0

| Rust module | Code lines | Public fns | Tests | Cyrius module(s) |
|---|---|---|---|---|
| `error.rs` | 31 | — | 6 | `src/error.cyr` |
| `geometry.rs` | 173 | 12 | 19 | `src/geometry.cyr` |
| `core.rs` | 520 | 10 | 39 | `src/core.cyr`, `src/fmtf.cyr`, `src/uuid.cyr`, `src/rfc3339.cyr`, `src/json.cyr` |
| `fsutil.rs` | 106 | — | 7 | `src/fsutil.cyr` |
| `http.rs` | 132 | — | 6 | `src/http.cyr` |
| `ai.rs` | 396 | 3 | 39 | `src/ai.cyr` |
| `annotate.rs` | 658 | 12 | 44 | `src/codec.cyr`, `src/png_enc.cyr`, `src/bmp_enc.cyr`, `src/jpeg_enc.cyr`, `src/raster.cyr`, `src/annotate.cyr` |
| `history.rs` | 328 | 7 | 18 | `src/history.cyr` |
| `capture.rs` | 348 | 11 | 19 | `src/spawn.cyr`, `src/capture.cyr` |
| `daimon.rs` | 275 | 9 | 20 | `src/daimon.cyr` |
| `lib.rs` | 141 | 3 | 14 doc | `src/selah.cyr` (the facade) |
| `mcp.rs` (feature `mcp`) | 745 | 1 | 41 | `src/mcp.cyr`, in `[lib.mcp]` only |
| `tests/integration.rs` | — | — | 17 | `tests/integration.tcyr` |

"Code lines" counts the lines before `#[cfg(test)]`. M0 step 0 re-counts everything on the frozen tree.

### Not carried over

- **Rust-only mechanics:**
  - `async` and tokio (D6);
  - `CaptureClient::with_client(reqwest::Client)`: the test seam becomes the base URL plus the config;
  - `#![forbid(unsafe_code)]`: the Cyrius rule instead is no `@unsafe` blocks in `src/`, enforced by a grep in CI;
  - `Debug` impls.
  - `Display` and `FromStr` **are** carried wherever their text reaches output: SVG, MCP, error messages. They
    become `_display` and `_parse` functions.
- **serde on every type.** Codecs are written for what selah itself reads or writes: layer files, history lines, MCP
  arguments and results, HTTP bodies, `OcrResult` and the suggestions. Serde derives that nothing exercises, such as
  on `Screenshot`, wait for a Cyrius consumer.
- **`hisab::Vec2` re-export** (D4), **the `ai` feature** (D13), **WebP** (D1), **`heartbeat`** (D2).

### Deferred until after 1.0.0

| Item | Why | Unblocked when |
|---|---|---|
| WebP decode and encode | no Cyrius WebP codec exists (D1) | a WebP codec is written: in selah, or as a sibling encoder package (the maintainer's call) |
| Progressive JPEG input | chitra's one deferral | chitra ships SOF2 |
| Capture against a live daimon | daimon has no `/v1/screen/*` routes | the daimon issue (filed 2026-11-14) lands, in its roadmap's 2.9.x slot |
| OCR through Ollama, Anthropic or Gemini models | hoosh forwards image parts only to OpenAI-shaped providers | the hoosh issue (filed 2026-11-14) lands |
| The open Rust roadmap items: map OCR bounding boxes to PII matches, a format-aware metadata scan, MCP path confinement, a per-call render budget | feature work, not port work | continue in Cyrius after 1.0.0 |

---

## 2. M0: freeze the oracle, then initialize with `cyrius port`

`cyrius port` is the initializer: don't hand-build the project structure. Where it gets something wrong, repair it here.
Its known defects are filed in `../cyrius/docs/development/issues/2026-11-08-cyrius-port-clobbers-project-docs-and-omits-bench.md`,
and tanmatra, pravash, ushma and impetus have each hit them. **The maintainer commits.** Agents don't commit or push in
this repo (CLAUDE.md).

0. **Freeze a green oracle (Rust, before the move).**
   - Run the full P(-1) cleanliness check on the current tree:
     - `cargo fmt --check`;
     - `cargo clippy --all-features --all-targets -- -D warnings`;
     - `cargo test --all-features` and `cargo test --doc`;
     - `cargo audit` and `cargo deny check`.
   - Record the test count (289 expected) and a bench baseline (`./scripts/bench-history.sh`). The Rust bench numbers
     are what M7 compares against.
   - Cut `[Unreleased]` as the **final Rust release**: the CHANGELOG section, `VERSION` and `Cargo.toml`. The version,
     tag and publish are the maintainer's.
   - Run `cargo clean`. `target/` is **7.4 GB**, and `cyrius port` moves it into `rust-old/` without saying so.
   - **Make the oracle's lock trackable.** `.gitignore` has a bare `Cargo.lock` line, as a library crate should. After
     the move, `rust-old/Cargo.lock` (which pins the oracle's exact dependency versions) and `scripts/oracle/Cargo.lock`
     would both stay untracked; pravash and ushma both hit this. Change the line to `/Cargo.lock`.
1. **Scaffold.**
   ```sh
   cd /home/macro/Repos/selah
   cyrius port --dry-run .
   cyrius port .
   ```
   - **Moved into `rust-old/`:** expect `Cargo.toml`, `Cargo.lock`, `src/`, `tests/`, `benches/`, `examples/`,
     `Makefile`, `codecov.yml` and `deny.toml`. If the scaffold leaves `rust-toolchain.toml` behind, move it by hand.
   - **Written:** `src/main.cyr`, `cyrius.cyml`, `.gitattributes` (`lib/** -text`), `tests/selah.{tcyr,bcyr,fcyr}`
     and the docs skeleton.
2. **Repair what the scaffold overwrites.**
   - **`CLAUDE.md`.** Merge `git show HEAD:CLAUDE.md` with the scaffold's Cyrius rules. Keep the identity, the
     consumers, the process, and **Do not commit or push** and **NEVER use `gh`**. Translate the key principles:

     | Rust principle | Cyrius equivalent |
     |---|---|
     | `#[non_exhaustive]` on public enums | enum values are append-only and never renumbered; `match` keeps an `_ =>` arm |
     | `#[must_use]` on pure functions | `#must_use`, plus `#pure` where it applies |
     | `#[inline]` on hot paths | dropped: cyrius does not inline (cyrius issue `2026-11-11-inline-refuses-small-numeric-helpers`) |
     | `write!` over `format!` | `str_builder` appends; no intermediate `Str` |
     | feature-gate optional deps | `[features]`, `optional = true` and `[lib.<profile>]` |
     | tracing on all operations | sakshi through `src/log.cyr` (D11) |
     | no `unwrap()` / `panic!()` | no `+?`, `-?`, `*?`, `assert_fatal` or `result_unwrap` in `src/` |
     | the cleanliness check | `cyrius fmt --check`, `cyrius lint`, `cyrius vet`, a warning-free `cyrius test`, `test --bounds`, `cyrius bench`, `distlib --all` and `--check`, `deps --locked [--features mcp]`, `cyrius audit`, surface parity |
     | (new) | cross-check against `rust-old/`, and diverge only through an ADR; never edit `rust-old/` or `lib/`; never widen a parity class |

   - **`docs/development/roadmap.md`** is restored from git. **`state.md`** gets the real version, not `0.1.0`.
   - **Workflows.** Keep the scaffold's Cyrius CI and extend it with `test --bounds`, `deps --locked --features mcp`,
     `audit`, `distlib --check`, surface parity and the Rust-test ledger. The Rust workflows move to
     `rust-old/.github/workflows/`, where GitHub doesn't run them. CI has no Rust job: the golden corpus is regenerated
     locally.
   - **`cyrius.cyml`:** set `version = "${file:VERSION}"` and add `bench` to `[test] stdlib`. Give the six stub
     functions doc comments, so `cyrius audit` passes.
   - **Rust-only material** moves to `rust-old/`: `scripts/bench-history.sh` and `scripts/version-bump.sh`, and
     `docs/development.md` and `docs/architecture/overview.md`. They are rewritten for Cyrius at M7.
   - **ADRs** go in `docs/adr/`, numbered with four digits:
     - `0001-cyrius-port-with-rust-oracle.md`: why a port, and not FFI;
     - `0002-port-api-divergences.md`: D1, D2, D4–D13, and every later divergence.
   - If selah hits a scaffold defect beyond the five listed in the cyrius issue, add a *Seen again: selah* section
     there.
3. **The manifest.** A draft follows. The stdlib list is confirmed by `cyrius vet` once code exists.
   ```toml
   [package]
   name = "selah"
   version = "${file:VERSION}"
   description = "AI-native screenshot capture, annotation, and redaction library for AGNOS"
   license = "GPL-3.0-only"
   language = "cyrius"
   repository = "https://github.com/MacCracken/selah"
   cyrius = "6.7.18"

   [build]
   entry = "src/main.cyr"
   output = "build/selah"

   [deps]
   # No "sankoch": chitra's own [deps.sankoch] vendors dist/sankoch-zlib.cyr (zlib, deflate, CRC-32),
   # which is all selah uses. The full fold adds Brotli/zstd/xz tables to static data (M0 probe 4c).
   stdlib = ["syscalls", "string", "alloc", "str", "fmt", "io", "fs", "vec", "hashmap", "math",
             "tagged", "result", "freelist", "chrono", "random", "process", "thread",
             "bayan", "sandhi", "sakshi"]

   [deps.chitra]
   git = "https://github.com/MacCracken/chitra.git"
   tag = "1.3.1"
   modules = ["dist/chitra.cyr"]

   [deps.ranga]
   git = "https://github.com/MacCracken/ranga.git"
   tag = "2.4.0"
   modules = ["dist/ranga.cyr"]

   [deps.bote]
   git = "https://github.com/MacCracken/bote.git"
   tag = "3.8.15"
   modules = ["dist/bote.cyr"]
   optional = true

   [features]
   default = []
   mcp = ["bote"]

   [test]
   stdlib = ["assert", "bench"]

   [lib]
   modules = ["src/error.cyr", "src/log.cyr", "src/fmtf.cyr", "src/json.cyr", "src/uuid.cyr", "src/rfc3339.cyr",
              "src/geometry.cyr", "src/core.cyr", "src/ai_data.cyr", "src/ai.cyr", "src/codec.cyr", "src/png_enc.cyr",
              "src/bmp_enc.cyr", "src/jpeg_enc.cyr", "src/raster.cyr", "src/annotate.cyr", "src/fsutil.cyr",
              "src/history.cyr", "src/http.cyr", "src/spawn.cyr", "src/capture.cyr", "src/daimon.cyr",
              "src/selah.cyr"]

   [lib.mcp]
   modules = [ <the whole [lib] list>, "src/mcp.cyr" ]
   ```
   - `[lib]` order is load-bearing, because no `src/` file includes another. `[lib] modules = []` is invalid, so
     `[lib]` grows from `src/error.cyr` at M1.
   - Pin by git plus `tag` only, with no `path =` override: a path override bypasses the commit pin in `cyrius.lock`.
     `lib/` and `cyrius.lock` are committed.
4. **Toolchain probes.** Each is recorded in §5 as *probed* once it has been compiled and run on 6.7.18.
   - **a. f32.** `SlRect` with four f32 fields: `x + w` rounds to f32 as Rust's does; `abs`, `floor` and `ceil`
     behave; the saturating f32 → u32 cast (`clamp_to_u32`: NaN and negatives go to 0) matches.
   - **b. Small structs in a `Result`.** A 16-byte `SlRect` or a 4-byte `SlColor` inside a `Result` runs into cyrius
     issue `2026-11-08-small-struct-result-payload-read-as-address`. Expected outcome: rects travel by out-pointer, and
     `SlColor` is a packed `u32` word (`r | g<<8 | b<<16 | a<<24`).
   - **c. chitra and sankoch in one unit.** chitra's own `[deps.sankoch]` (`dist/sankoch-zlib.cyr`) beside the stdlib
     fold `sankoch`, which selah's PNG encoder needs. kii builds that way; confirm that selah's unit does too.
   - **d. bote 3.8.15.** The full bundle inside an optional profile: it compiles beside sandhi and bayan, the handler
     ABI is `fn h(args_cstr, claims) → result_cstr`, and `transport_stdio_run` works. Confirm whether bote's own deps
     (libro, majra) resolve on their own or need lines in this manifest.
   - **e. sandhi.** The client with deadlines and capped bodies, run against a mock server on a thread. This is the
     M5 test seam that replaces Rust's `serve_once`.
   - **f. The error design (D7).** A per-thread detail slot (`thread_local`) next to a value-form `Result`, through
     `?`.

   **Results (2026-11-14, x86_64, all compiled and run; programs in the session scratchpad, not the repo):**
   - **a. f32: bit-identical to Rust** (rustc 1.100.0 reference program).
     - `Rect::new(0.1, 0.2, 0.3, −0.7)` gives `max_x` 0x3ecccccd, width 0x3e99999a and area 0x3e570a3e on both sides.
     - `new(16777216, …, 1, …)` gives `max_x` 0x4b800000 and width 0, on both sides.
     - The saturating cast gives `−1.5 → 0`, `NaN → 0`, `2³² → 4294967295` and `3.7 → 3`, as Rust's `as u32` does.
     - The f32 rules: an f32 operator's right operand must already be f32, so convert with `f32_from` / `f32_to`;
       `f64_abs` / `f64_trunc` work on the exact f64 widening. No type warnings.
   - **b. `Result`.**
     - A packed colour word rides `Ok` and `?`, with the payload intact on both the `Ok` and the `Err` path.
     - An `SlRect` written through an out-pointer next to a `Result` tag works. Rects never travel inside a `Result`
       (cyrius issue `2026-11-08-small-struct-result-payload-read-as-address`).
   - **c. chitra + sankoch + bayan.**
     - `bayan_base64_decode` returns a **`{data, len}` pair**, not the bytes.
     - `chitra_image_decode_budget` decodes a 1×1 PNG to RGBA `255,0,0,255`, and refuses it under a 3-byte budget
       with code 34.
     - `zlib_compress` / `zlib_decompress` round-trip.
     - **`crc32` needs `crc32_init_table()` first** (its documented precondition; skipping it faults).
     - The resolver vendors chitra's `dist/sankoch-zlib.cyr` only when `sankoch` is **not** a stdlib leaf, hence the
       manifest above.
   - **d. bote 3.8.15.**
     - Its transitive deps (libro 2.11.3, majra 2.13.0, patra 1.16.6, sigil 3.13.15) resolve and lock on their own,
       so the manifest needs no lines for them.
     - The full bundle compiles beside sandhi and bayan.
     - `dispatcher_register_tool` plus `codec_process_message` answer `tools/list` and `tools/call`.
     - `ann_read_only()` emits all four hints (`readOnlyHint` true, `destructiveHint` false, `idempotentHint` true,
       `openWorldHint` false). M6 compares this with Rust bote 0.92's output.
   - **e. sandhi against a loopback mock server on a thread.**
     - `user_headers` is a `sandhi_headers_new()` object, and errors come from `sandhi_http_err_kind`.
     - The request goes out right: method, path, `Content-Type`, `Authorization` and body.
     - **`total_ms` alone does not bound a silent server.** It waited 3,000 ms, then reported `err_kind` 6 (protocol).
       With `read_ms` also set it reports `err_kind` 4 (timeout) at 301 ms.
     - This is filed upstream as sandhi `2026-11-11-total-ms-does-not-bound-a-blocked-first-read` (by ushma);
       selah's data point is appended there. **selah always sets `read_ms` and `write_ms` with `total_ms`** (L10).
   - **f. Error detail.** A trailing `detail_out = 0` default argument carries the text through `?`, and a call that
     omits it is unaffected. This decided D7.
   - **The static-data advisory.** Any unit with sandhi in it prints `warning: large static data (~136–187 KB)`,
     even an empty `main`, and DCE does not remove it.
     - sandhi plus the full `sankoch` fold crosses the threshold, and so does sandhi plus chitra's zlib profile
       (143 KB).
     - itihas (bote + sandhi) prints it too (136 KB).
     - It is an aggregate of the deps' tables, not selah's. The warning-free gate exempts **exactly this advisory**,
       by its text; every other warning still fails.
5. **The golden-value generator, `scripts/oracle/`** (tanmatra's design).
   - **The crate** depends on `../../rust-old` by path, with `features = ["mcp"]`. It never builds inside `rust-old/`.
     Its `Cargo.lock` is seeded from `rust-old/Cargo.lock`.
   - **Output** is `tests/golden/<module>.tsv`, one row per call: `fn<TAB>class<TAB>inputs<TAB>outputs<TAB>note`.
     - f32 values are written as bit patterns (`g:41200000`) and f64 values the same way (`f:…`).
     - Strings are escaped.
     - Byte blobs and pixel buffers go to files under `tests/golden/blob/`, named by their sha256.
   - **Deterministic.** Inputs come from seeded xorshift. Ids and timestamps are fixed through public fields, never
     `Uuid::new_v4()`. `--check` fails on any stale file.
   - **The image fixtures** come from the oracle's own encoders, so they double as decode fixtures:
     - the PNG colour types and bit depths `image` accepts;
     - BMP variants;
     - baseline JPEG at each subsampling;
     - malformed, truncated and decompression-bomb inputs.
   - **Coverage grows per milestone.** Each milestone adds its module's grid (§3) when it starts.
6. **The parity harness**, `tests/parity.tcyr` plus `tests/parity_core.cyr`.
   - It replays every module in `tests/golden/modules.txt`, a list the generator writes.
   - Each row gets one outcome: ok, **FAILED**, measured or pending.
   - The exit code is the number of failing modules.
7. **Baselines**, recorded in `docs/development/port-parity.md` (done 2026-11-14: §1 there is the result, §2 the findings F1–F5, measured against source pixels and filed with chitra):
   - JPEG decode, chitra against `image`, on every JPEG fixture: a histogram of the maximum per-channel difference,
     which sets **K_jpeg** (class B) before the first gated comparison, and both decoders' error against the
     fixture's source pixels (class B's accuracy gate);
   - PNG and BMP decode, expected exact;
   - f32 rect arithmetic, spot-checked;
   - all of it on x86_64 ELF, x86_64 PE (wine) and aarch64 (qemu), as tanmatra did. macOS is recorded as unmeasured
     unless a host is used.

**Gate:**
- `cyrius build` and `cyrius bench` pass (`bench` proves the manifest repair).
- `cyrius test` passes with no warnings (the harness with 0 modules).
- `cyrius lint` and `cyrius fmt --check` are clean.
- The generator and the ADRs are ready for the maintainer to commit.

---

## 3. Milestones

Every milestone ends with the same gates:
- `cyrius build`;
- `cyrius test` **with no warnings**, and `cyrius test --bounds`;
- `cyrius bench`, with the module's benches recorded against the Rust numbers;
- `cyrius vet`, `cyrius lint` and `cyrius fmt --check` on the module's files, and `cyrius audit`;
- `cyrius distlib --all`, then `distlib --check`; from M6 also `--features mcp`;
- `cyrius deps --locked`;
- **surface parity:** every in-scope Rust `pub fn` in the module has a Cyrius counterpart, or a reason written in ADR
  0002;
- CHANGELOG and `state.md` updated.

| Milestone | Modules | Key work | Golden grid (minimum) |
|---|---|---|---|
| **M1 Foundation** | error, log, fmtf, json, geometry, core, uuid, rfc3339 | `SlError` and the error detail (D7). `SlRect` / `SlVec2` in the `{min, max}` f32 layout (D4). Enums with `_display` / `_parse`: `AnnotationKind` parses case-insensitively; `ImageFormat` keeps `extension`, `mime_type` and `from_path`, and **`WebP` stays an enum value** that the codecs refuse (D1), so the surface and parsing keep parity. `SlColor` with `to_css`. `xml_escape` and `derive_output_path`. The shortest f32/f64 printer in two forms (L1). UUID v4 from `random_bytes`. RFC 3339 with chrono's `AutoSi` fraction digits, over `clock_epoch_ns` (L5) | every enum variant, both directions; paths for `from_path`; a rect grid with ±0, NaN, ±∞, subnormals, f32 MAX and negative sizes, with `contains_point` / `intersects` pairs; `xml_escape` over bytes 0–255 and UTF-8 samples; f32 and f64 printing over every binade, compared with Rust `Display` and serde_json |
| **M2 PII and smart crop** | ai | The `suggest_redactions` tokenizer. The email, card (Luhn, grouped and split cards), IPv4 and phone (separators required) classifiers. Results in text order, each token at most once. The `extract_text_regions` stub and `suggest_smart_crop` | the inputs of the Rust tests, plus a seeded generator of PII-shaped strings: valid and invalid Luhn, every grouping, every phone separator, edge octets, Unicode punctuation and whitespace; byte blobs for the stub OCR; crop dimensions from 0 to 2³² − 1 |
| **M3 Codecs** | codec, png_enc, bmp_enc, jpeg_enc | Signature sniffing for PNG, JPEG and BMP; GIF and WebP refused (D1). Limits through chitra's budget variant. PNG encoding from kii. BMP encoding. A **baseline JPEG encoder** (forward DCT, the Annex K tables), with quality and chroma subsampling matched to the defaults of Rust `image`'s `JpegEncoder`, measured when M3 starts. `convert_format` | every decode fixture: PNG and BMP are class A, JPEG class B (accuracy against the source pixels). Malformed, truncated and bomb inputs give **the same accept or refuse verdict as Rust** (A). Encoders are class R: Cyrius encode then Cyrius decode is exact for PNG and BMP, and in the **reverse oracle** Rust decodes every Cyrius-encoded file to the same pixels (PNG, BMP) or within the PSNR bound (JPEG) |
| **M4 Annotation** | raster, annotate, selah | The rasterizer as Rust has it: `fill_rect`, `blend_rect` (ranga `blend_row`, `BM_NORMAL`), `stroke_rect`, bounded ellipse sampling, line clipping and drawing, `stamp`, outward rounding of redactions, rejection of NaN redactions. The canvas: add, remove, clear, count, `apply_redaction`, layer save and load (`LAYER_VERSION`, `MAX_LAYER_FILE_BYTES`, private atomic writes). `to_svg`, `render_to_image`. The facade: `selah_annotate_image`, `selah_redact_image`, `selah_decode_image_data` | every `AnnotationKind` × rect class (normal, negative size, fractional, partly or wholly off-canvas, NaN, ±∞, huge) × colours with alpha 0, 128 and 255, on seeded images from 1×1 to 64×64 plus one 512×512. Rendered through a PNG round trip on both sides, so the RGBA is class A. The leak regressions from the Rust audit. SVG text (A). Layer JSON both ways: Rust-written layers load in Cyrius, and Cyrius-written layers load in Rust |
| **M5 System and clients** | fsutil, http, history, spawn, capture, daimon | Private permissions, bounded reads, atomic writes (`file_replace_atomic`). History as JSONL: `xflock` non-blocking with a 5 s retry deadline, atomic rewrites, `list` in bounded memory (stable order, L4), get and delete, a torn last line skipped. Session detection, and the clipboard over the spawn helper (D12, L3). The HTTP layer over sandhi: status checks, capped bodies, `Authorization: Bearer`, percent-encoded path segments. **The clients retargeted (D2)**, including the OCR downscale loop | history files written by Rust are read by Cyrius, and the reverse; `RegionSpec::from_rect` over the rect grid (A); the capture request body for the unchanged contract (A, captured by the mock server); `parse_region` and the confidence clamp (A); the retargeted daimon and hoosh requests as class W fixtures, with mock answers for success, non-2xx, oversized bodies, malformed JSON and timeouts |
| **M6 MCP profile** | mcp | The five tools on bote 3.8.15 (itihas template): identical names, schemas, descriptions and read-only annotations (`selah_ocr`, `selah_history`). Path and input safety: an input must be an image, an output must have an image extension, no symlinks, traversal refused. `selah_ocr` `engine: "hoosh"` goes through the retargeted client. `transport_stdio_run` | `tools/list`, `initialize`, an unknown method, argument validation for each tool, and every tool's answer on temp-dir fixtures (A after masking temp paths); the refusals the Rust audit pinned (symlinks, traversal, non-image inputs and outputs) |
| **M7 Release 1.0.0** | all | **The Rust-test ledger:** each of the 289 tests ported, folded into golden rows, or marked n/a or deferred (`scripts/rust_test_ledger.py --check`). **Full surface parity.** All 13 benches recorded against Rust in `docs/benchmarks-rust-v-cyrius.md`, and a Cyrius `scripts/bench-history.sh` (ranga's). `dist/selah.cyr` and `dist/selah-mcp.cyr`. README, `docs/mcp-tools.md`, `docs/development.md` and the architecture doc rewritten for Cyrius. The CHANGELOG `1.0.0` entry: *selah is a Cyrius library*, Breaking (pointing at ADR 0002), Verified. `VERSION` 1.0.0. The tag is the maintainer's | the whole corpus green on x86_64 ELF, x86_64 PE and aarch64 |

**Order:** M1 → M2 → M3 → M4 → M5 → M6 → M7.
- After M1, M2 and M3 are independent of each other.
- M4 needs M3.
- M5 needs M3, because the OCR downscale encodes JPEG.
- M6 needs M4 and M5.

---

## 4. Parity policy

**Accuracy is the bar; Rust is the reference and the floor** ([ADR 0003](../adr/0003-accuracy-is-the-bar.md)).
- **Ground truth decides where it exists:** the source pixels a fixture was encoded from, the format
  specification, the mathematics. The port must be at least as accurate as Rust, measured the same way.
- **Where Rust's behaviour is the contract, the port matches it exactly.** This covers JSON, layer and history
  files, SVG, enum names and verdicts.
- **Defects are fixed where they live:**
  - a Rust defect is fixed in the port, not imitated;
  - a dependency defect is filed upstream with the measurement.

  Neither is a decision for the maintainer.

| Class | Tolerance | Applies to |
|---|---|---|
| **A: exact** | bit-identical f32/f64; equal integers, strings and bytes. Expected is Rust's value, or the corrected value where ADR 0002 records a Rust defect | rect operations; enums; `xml_escape`; output paths; PII matches; smart crop; SVG; JSON text (layers, history, MCP, `OcrResult`); RGBA after rendering; PNG and BMP decode (lossless: the spec defines one answer, and the oracle's is checked against it, e.g. F3's rounding and F4's opacity); the accept or refuse verdict on every input |
| **B: JPEG decode** | **accuracy:** against each fixture's source pixels, the port's mean and max absolute RGB error ≤ the reference decoder's + 0.05 (mean) and + 2 (max). **Conformance:** where both upsample chroma the same way, per-channel \|Δ\| ≤ K_jpeg = 2. Both set before the first gated comparison (port-parity.md §5) | JPEG decode (chitra; the reference is `image` 0.25.10 / zune-jpeg), and anything rendered over a JPEG source |
| **R: round trip** | lossless: decode(encode(px)) = px exactly, under chitra and under Rust's decoder (the reverse oracle, `scripts/oracle -- --reverse`). JPEG: the quantised coefficients agree with the exact mathematics (JFIF colour, T.81 DCT, round to nearest): none off by more than 1, at most 1 % off by one, per image (port-parity.md §5, which also records the per-image Rust bound this replaced). Accuracy against Rust's own encoding is measured, not gated | the PNG, BMP and JPEG encoders. **Encoded bytes are never compared:** deflate and the JPEG encoder's choices differ from Rust's crates, and both outputs are valid |
| **W: wire, Cyrius only** | equal to fixtures checked in under `tests/golden/wire/`; there is no Rust row | the retargeted daimon and hoosh requests and their parsing (D2) |

Rules:
- **Never widen a class to make a row pass.** A failure beyond its class is a defect in one of three places:
  - **in the port:** fix it;
  - **in a Cyrius dependency** (chitra, ranga, bote or the stdlib):
    - file it upstream, with the measurement;
    - put its rows on the harness's known-defect list under the issue's file name. They report `upstream`, and a
      listed row that starts passing fails the run;
    - where selah can be accurate without duplicating the dependency, add a ledger row (§6);
  - **in the Rust oracle** (Rust disagrees with ground truth or the spec):
    - fix it in the port;
    - add an ADR 0002 row marked *Rust defect*;
    - have `scripts/oracle` emit the corrected expectation.

    A 0.30.x patch for Rust consumers is optional.
- **An API change accepted in ADR 0002** is the other exception. The harness lists such rows by name and still
  reports them.
- **The audit's regression tests travel with the code,** as Cyrius assertions as well as golden rows:
  - negative-size and fractional redactions;
  - NaN rejection;
  - the decode-bomb limits;
  - the MCP refusals for symlinks, traversal and non-image inputs;
  - the history torn-line skip.

  Parity proves "as accurate as Rust, and compatible where Rust is the contract"; these prove "safe".
- **Branch points are tested on both sides:**
  - line clipping at the canvas edge;
  - the ellipse sampling bound;
  - the dimension limit at exactly `MAX_IMAGE_DIMENSION`;
  - the layer file cap;
  - the history lock deadline;
  - the hoosh 1 MiB cap, just under it and just over it.

---

## 5. Rust → Cyrius mapping (selah specifics)

### Dependencies

| Rust | Cyrius |
|---|---|
| `hisab::Rect`, `Vec2` (f32) | `SlRect` (`{min, max}`, f32) and `SlVec2`, local (D4) |
| `ranga::PixelBuffer`, `blend_row` (Rust ranga 1.0) | ranga 2.4.0: `pixel_view_new` / `pixel_buffer_from_view` over the decoded RGBA8, and `blend_row(src, dst, n, BM_NORMAL, opacity)`. ranga's own oracle holds it to Rust ranga, so highlight pixels stay class A |
| `image` decode, `guess_format`, `Limits` | a local sniff, then `chitra_image_decode_budget` with `MAX_DECODE_ALLOC`; the dimension check comes before any allocation |
| `image` encode | `src/png_enc.cyr` (kii), `src/bmp_enc.cyr`, `src/jpeg_enc.cyr`; sankoch `zlib_compress` and `crc32`, after one `crc32_init_table()` |
| `reqwest` (async, rustls) | sandhi `sandhi_http_get_opts` / `post_opts`, blocking: headers in a `sandhi_headers_new()` object; `total_ms` **with `read_ms` and `write_ms`** (L10) and `max_response_bytes`; failures from `sandhi_http_err_kind`. Native TLS is the stdlib default |
| `serde`, `serde_json` | bayan `bayan_json_parse` for reading; `str_builder` and `str_builder_add_json_str` for writing (D8) |
| `base64` | `bayan_base64_encode` / `bayan_base64_decode`; decode returns a `{data, len}` pair |
| `uuid` v4 | `random_bytes`, then RFC 4122 bits and formatting (`src/uuid.cyr`) |
| `chrono::DateTime<Utc>` | `clock_epoch_ns` and `src/rfc3339.cyr`. The stdlib `iso8601` is whole seconds only (L5) |
| `thiserror` `SelahError` | `enum SlError` and a trailing `detail_out = 0` argument (D7) |
| `tracing` | sakshi, through `src/log.cyr` (D11) |
| `tokio` (`mcp`) | none: bote's stdio loop is blocking |
| `bote` 0.92 (Rust) | bote 3.8.15: `schema_new` / `schema_prop_new`, `tool_def_new`, `tool_def_with_annotations(ann_read_only())`, `dispatcher_register_tool`, `transport_stdio_run`, `content_text_response` |
| `std::process` (`wl-copy`, `xclip`) | `src/spawn.cyr` (D12, L3) |
| `File::try_lock` | `xflock(fd, LOCK_EX \| LOCK_NB)` in a retry loop with a 5 s deadline |
| `fs::rename` for atomic rewrites | `file_replace_atomic`; `file_write_atomic` |
| `BinaryHeap` top-N in `history::list` | a bounded heap keyed on (timestamp, line index), or a local stable merge sort (L4) |
| `std::env::var` | `getenv` |

### Types

| Rust | Cyrius |
|---|---|
| `Result<T, SelahError>` | a value-form `Result` with `SL_ERR_*` codes; the detail text through a trailing `detail_out = 0` argument (D7). Never return a `Result` through `callptr`, because its payload is lost |
| `Rect` (Copy, 16 bytes) | `struct SlRect { min_x: f32; min_y: f32; max_x: f32; max_y: f32; }`, passed by value as an argument and returned through an out-pointer (probe 4b) |
| `Color { r, g, b, a: u8 }` | a packed `u32` word with `sl_color_r/g/b/a`, and constants `SL_COLOR_RED` … `SL_COLOR_YELLOW` |
| `#[non_exhaustive] enum` (`AnnotationKind`, `ImageFormat`, `RedactionTarget`, `CaptureSource`, `SessionType`) | `enum` with explicit, append-only values; `match` keeps an `_ =>` arm |
| `CaptureSource::Region(Rect)` / `Window(String)`, `CaptureRegion` | a kind enum plus a separate payload (an `SlRect` or a `Str`). A `: stack` enum can't carry a struct or nest inside a `Result` |
| `Annotation { id, kind, position, color, text: Option<String> }` | `struct SlAnnotation`: `id` is 16 bytes of UUID, `text` a `Str` or 0 |
| `Vec<Annotation>`, `Vec<RedactionSuggestion>`, … | `lib/vec.cyr` of pointers. A hot path takes a caller buffer |
| `Vec<u8>` (image bytes) | a freelist buffer and a length (D10) |
| `String` | `Str`, built with `str_builder` |
| `Option<T>` returns | a value-form `Option`, or a handle that is 0 for none |
| `(Vec<u8>, Vec<RedactionSuggestion>)` from `redact_image` | a multi-value `return (bytes, suggestions)`, with the length through an out-pointer |
| `u32` / `i32` fields (`Monitor`, `RegionSpec`) | i64 values, range-checked at every boundary. `u32` exists as a field and a parameter but not as a return type, and there is no unsigned compare |

### Cyrius rules that shape selah's code

These are carried from tanmatra and kimiya, and re-checked at every pin bump.

- **Floats.**
  - Bind every call result to a typed `f32` / `f64` local before using it in an operator.
  - Negate with `-x`, never `(0 - x)`.
  - There are no exponent literals, and no hex patterns in a `const`.
  - `f64_round` rounds ties to even.
  - Rust's `as u32` saturates (NaN goes to 0); write it out.
- **Shifts.** `>>` is logical and `>>>` arithmetic. Choose by the Rust operand's signedness. This matters in the JPEG
  encoder, the Luhn arithmetic and the blending.
- **Arrays.** `var buf[N]` is N **bytes**; use `var a: i64[N]` for slots. Never return an array local. Big image
  scratch goes on the freelist, never on the stack.
- **Struct values** changed a lot across 6.7.x. Keep the shapes simple, pass `*T` when the callee mutates, and
  re-probe at every pin bump.
- **SIMD.** Don't use the `f64v_*` builtins until cyrius issue `2026-11-06-f64v-builtins-no-scalar-tail` is fixed;
  ranga's `blend_row` is the vectorized path.
- **Audit.** Lines over 120 characters fail `cyrius audit`. `cyrius bench` stops a file after 300 s, so split
  large suites.
- **Python** helpers in `scripts/` run with `-I -B`.

---

## 6. Upstream filings and the conversion ledger

Every local stand-in for a missing upstream piece is a ledger row. Each row has a `# UPSTREAM(Ln)` comment at its site
and leaves when the upstream piece ships. The format is pravash's.

| Id | Local stand-in | Why | Upstream | Leaves when |
|---|---|---|---|---|
| L1 | `src/fmtf.cyr`: shortest f32/f64 digits, in Rust `Display` and serde_json forms (from pravash / impetus) | the stdlib has no shortest round-trip printer | cyrius `2026-11-13-derive-debug-and-shortest-float-digits` | the stdlib ships a printer |
| L2 | hand JSON codecs (D8) | `#derive(Serialize)` defects | cyrius `2026-11-07-derive-serialize-key-scan-reads-past-nul`, `…-reader-memsets-a-refused-allocation`, `…-vec-struct-element-loop-hangs-on-stray-brace`, `2026-11-11-derive-writer-faults-on-a-zero-str-field`, `2026-11-12-derive-serialize-in-a-second-private-file-fails-to-compile` | the derive is fixed and byte-matches serde_json |
| L3 | `src/spawn.cyr`: a spawn that writes stdin while draining stderr | `lib/process.cyr` has `run`, `spawn` and `exec_capture`, but none feeds stdin | **to file at M5** as a cyrius stdlib request | the stdlib grows it |
| L4 | a stable top-N or merge sort for `history::list` | `vec_sort_by` is introsort and not stable | cyrius `2026-11-11-no-stable-sort` | the stdlib has a stable sort |
| L5 | `src/rfc3339.cyr`: RFC 3339 with nanoseconds, chrono's AutoSi writer and its relaxed parser | `iso8601` / `iso8601_parse` are whole seconds only, with no offset but `Z` | **filed** `../cyrius/docs/development/issues/2026-11-14-chrono-rfc3339-has-no-fraction-or-offset.md` | `lib/chrono.cyr` covers it |
| L6 | `src/png_enc.cyr`, `bmp_enc.cyr`, `jpeg_enc.cyr` | chitra is decode-only by policy, and no Cyrius encoder library exists | none. A sibling encoder package is the maintainer's call (§7) | an encoder package exists |
| L7 | `src/geometry.cyr` | hisab has no 2D f32 rect | none: selah needs six operations | hisab grows a 2D rect |
| L8 | the OCR downscale loop, and only OpenAI-compatible OCR models | hoosh's 1 MiB cap, and its image parts on other providers | **filed** `../hoosh/docs/development/issues/2026-11-14-selah-vision-image-parts-reach-only-openai-shaped-providers.md` | hoosh lands it |
| L9 | `CaptureClient` tested only against the mock server | daimon has no `/v1/screen/*` routes | **filed** `../daimon/docs/development/issues/2026-11-14-selah-screen-capture-routes.md` | daimon ships the routes |
| L10 | every HTTP call sets `read_ms` and `write_ms` alongside `total_ms` | `total_ms` alone doesn't bound a blocked `recv` (M0 probe 4e) | sandhi `2026-11-11-total-ms-does-not-bound-a-blocked-first-read` (ushma's; selah's data point appended) | sandhi clamps `SO_RCVTIMEO` to the deadline |
| L11 | `src/codec.cyr`: a 32-bpp `BI_RGB` BMP is decoded opaque (alpha forced to 255; in since M3) | chitra reads the unused fourth byte as alpha whenever any of it is non-zero (F4) | **filed** `../chitra/docs/development/issues/2026-11-14-bmp-bi-rgb-32bpp-unused-byte-read-as-alpha.md` | chitra reads `BI_RGB` 32 bpp as opaque |
| L12 | none possible: chitra exposes no chroma planes. Subsampled-JPEG rows are on the harness's known-defect list | box chroma upsampling, measurably less accurate than interpolated (F1) | **filed** `../chitra/docs/development/issues/2026-11-14-box-chroma-upsampling-loses-accuracy.md` | chitra upsamples by interpolation |
| L13 | none: 16-bpp BMP rows are on the known-defect list (correcting them in selah would re-parse chitra's bitfield masks) | sub-8-bit BMP channels widened by bit replication, not to the nearest value (F3) | **filed** `../chitra/docs/development/issues/2026-11-14-bmp-sub-8-bit-replication-is-not-the-nearest-value.md` | chitra rounds |
| L14 | CMYK / YCCK JPEG refused (ADR 0002 #16) | chitra refuses 4-component JPEG (F5) | **filed** `../chitra/docs/development/issues/2026-11-14-cmyk-ycck-jpeg-refused.md` | chitra returns the CMYK samples |
| L15 | progressive JPEG refused (ADR 0002 #2) | chitra refuses SOF2 | **filed** `../chitra/docs/development/issues/2026-11-14-progressive-jpeg-has-a-consumer.md` | chitra decodes SOF2 |
| L16 | none: `SlImage` records that chitra owns its pixels and calls `chitra_image_free`, a no-op in 1.3.1 | chitra allocates every decode from the bump allocator and never frees it, so a long-running consumer (the MCP server) grows with every decode | **filed** `../chitra/docs/development/issues/2026-11-14-every-decode-leaks-in-a-long-running-consumer.md` | chitra takes an allocator and frees |
| L17 | none: images over 16,777,216 pixels are refused (the 16.8 MP fixture is on the known-defect list) | chitra's fixed `CHITRA_MAX_PIXELS` fires before the caller's budget | **filed** `../chitra/docs/development/issues/2026-11-14-fixed-16-mp-cap-refuses-screenshots.md` | the budget governs |
| L18 | none (measured in the M3 benches) | chitra decodes about 20× slower than Rust's `image` (a 4K JPEG about 1.9 s) | **filed** `../chitra/docs/development/issues/2026-11-14-decode-throughput-is-about-20x-rust-image.md` | chitra's decode is within the range of the other ported paths |

Toolchain defects found during the port are filed in `../cyrius/docs/development/issues/` with a repro, not worked
around silently. That includes the probe results in M0 step 4 when they find one.

---

## 7. After 1.0.0

1. **WebP**, as a codec in selah or a sibling encoder package. Its parity follows ADR 0002's WebP row, and the Rust
   oracle covers it (`image` decodes lossy and lossless WebP and encodes lossless).
2. **Capture against a live daimon** once its routes ship (L9). If daimon's route differs from Rust's contract,
   retarget `CaptureClient` and record it in ADR 0002.
3. **OCR on more providers** once hoosh translates image parts (L8). Relax the downscale if the cap moves.
4. **Extracting the encoders** into a package other repos can share; aethersafha and kii each carry their own PNG
   writer today.
5. **The Rust roadmap's open items**, continued in Cyrius: map OCR bounding boxes to PII matches, a format-aware
   metadata scan, MCP path confinement, a per-call render budget.
6. **Removing `rust-old/`.** This happens only after three things: WebP is done or explicitly dropped, the corpus and
   generator are archived, and the Rust-test ledger is closed. prakash and mela kept theirs recoverable from git.

---

## 8. Tracking

| Milestone | Status | Golden rows | Ported tests | Benches | Notes |
|---|---|---|---|---|---|
| M0 Initialize | ✅ done 2026-11-14 | `decode` 63 (the baseline: 47 ok, 14 measured, 2 accepted; identical on x86_64 ELF, aarch64 and x86_64 PE) | — | Rust baseline at `c0da3e1` (all 13 in `rust-old/bench-history.csv`) | Step 0: the oracle green on rustc 1.100.0 (289 tests), released and tagged as **0.30.0** (`1a494b4`). Steps 1–3: `cyrius port`, the scaffold repaired (CLAUDE.md, roadmap, state, workflows, version, bench, stub docs), the Rust material moved to `rust-old/`, ADRs 0001 and 0002, the manifest with chitra, ranga and an optional bote, `lib/` vendored. Step 4: probes a–f (§2). Steps 5–7: `scripts/oracle/` (deterministic, `--check`; 63 fixtures), `tests/parity.tcyr`, and the decode baseline with five findings, F1–F5, measured against source pixels and filed with chitra ([port-parity.md](port-parity.md) §2, ADR 0003). The M0 gate is green: build, test, `test --bounds`, bench, lint, fmt, docs, audit; the only warning is the tolerated static-data advisory |
| M1 Foundation | ✅ done 2026-11-14 | error 30 + fmtf 24,060 + geometry 8,129 + core 1,331 + uuid 144 + rfc3339 1,169 = **34,863, all class A, all equal on x86_64 ELF, aarch64 and x86_64 PE** | error.tcyr (6 Rust tests + 1), geometry.tcyr (19 + 1 doctest), core.tcyr (39 + 5 doctests + 2): 186 assertions; selah.tcyr smoke-tests `dist/selah.cyr` | 6 (`rect_new` 5.7×, `rect_contains_point` 17×, `rect_intersects` 32×, `xml_escape_plain` 5.2×, `_special` 6.8×, `_long` 13× Rust: [benchmarks-rust-v-cyrius.md](../benchmarks-rust-v-cyrius.md)) | `error`, `log`, `fmtf`, `json`, `uuid`, `rfc3339`, `geometry`, `core`; `[lib]` and `dist/selah.cyr`; surface parity 34/34 (`scripts/surface_parity.py`). Learned: zmij's JSON positional range is −6..=12 for f32 (−5..=15 for f64); serde also accepts `{"Variant": null}` for a unit variant and a struct as an array; Rust's `f32::min`/`max` on ±0 give the second operand. ADR 0002 rows #14 (serde error text) and #15 (f64 accessors). Filed: cyrius chrono RFC 3339 (L5) |
| M2 PII and smart crop | ✅ done 2026-11-14 | ai **8,451, all class A, all equal on x86_64 ELF, aarch64 and x86_64 PE** (suggest_redactions 3,051 over Rust's test inputs and a seeded PII generator; extract_text_regions 191 including every image fixture; suggest_smart_crop 216 up to u32::MAX; char::is_alphanumeric 4,005; the three JSON shapes 988) | ai.tcyr: 39 Rust tests + 1 doctest + 1 Cyrius-only (70 assertions) | 4 (`suggest_redactions_clean` 6.3×, `_pii` 5.6×, `_long` 6.3×, `extract_text_regions_64k` 7.2× Rust) | `src/ai.cyr`, and `src/ai_data.cyr` **generated** by the oracle (`t_ai.rs`): Rust's own `char::is_alphanumeric`, which is **Unicode 18.0.0** in rustc 1.100 while the stdlib's `lib/unicode` is 17.0.0, so the table, not a category test, is what matches. List-returning functions take the allocator first (`_a` forms, D10). Learned: serde's array form may leave off a trailing `#[serde(default)]` field. Surface parity 3/3 (37/37 in all) |
| M3 Codecs | ✅ done 2026-11-14 | decode **53 ok, 3 accepted, 8 upstream** (gated: class A, class B accuracy + conformance, the known-defect list); codec **349 ok, 115 accepted** (convert_format 252 verdicts and dimensions; encode 212: PNG and BMP lossless, JPEG against the exact mathematics, 18 of 223,872 coefficients off by one); `oracle --reverse` 159/159; identical on x86_64 ELF, aarch64 and x86_64 PE | codec.tcyr: 8 Rust tests + 1 doctest + 5 Cyrius-only (57 assertions); `test_render_to_jpeg_and_webp` waits for M4 | 6 Cyrius-only, Rust's numbers from `oracle --bench`: encode PNG 5.6×, BMP 1.1×, JPEG 2.5×; decode PNG 19×, JPEG 23× (chitra, L18); convert 4.1× | `src/codec.cyr` (sniff, the header probe that enforces Rust's limits before chitra allocates, chitra's errors mapped, the F4 opacity guard L11, `SlImage`), `src/png_enc.cyr` (kii's), `src/bmp_enc.cyr` (V4, BI_BITFIELDS with alpha, as `image` writes RGBA), `src/jpeg_enc.cyr` (baseline 4:4:4 q75; 20-bit LL&M FDCT, 6 fraction bits of YCbCr, rounded quantisation). The harness gained `upstream` rows, exact `adr=0002#N` parsing with the error variant checked, `c:` CRC tokens for pixels over 4 MiB, and the exact-mathematics check. **Learned:** `image` 0.25.10's JPEG encoder biases its FDCT (ADR 0002 #17); chitra leaks every decode (L16), caps images at 16.7 MP (L17) and decodes about 20× slower than Rust (L18), all filed. The JPEG encoder's per-image Rust bound was replaced after its first comparison (port-parity.md §5) |
| M4 Annotation | ⏳ | | | `render_to_image_64x64`, `render_to_image_512_bmp`, `to_svg_100` | |
| M5 System and clients | ⏳ | | | | live capture waits on daimon (L9) |
| M6 MCP profile | ⏳ | | | | |
| M7 Release 1.0.0 | ⏳ | | ledger: 289 Rust tests | all 13 | the tag is the maintainer's |
