# scripts/oracle: what the port derives from the Rust oracle

`oracle` runs the frozen Rust selah 0.30.0 in [`rust-old/`](../../rust-old/) and writes the golden corpus under
`tests/golden/` (port plan §2 step 5). The Cyrius parity harness (`tests/parity.tcyr`, step 6) replays every row
against the port and checks it within its parity class (port plan §4).

```sh
cargo run --release --locked --manifest-path scripts/oracle/Cargo.toml             # write
cargo run --release --locked --manifest-path scripts/oracle/Cargo.toml -- --check  # verify; exit 1 on any difference
```

## Rules

- **`rust-old/` is never built in place or edited.** This crate depends on it by path, with `features = ["mcp"]`.
  `Cargo.lock` was seeded from `rust-old/Cargo.lock`, so every shared dependency is the version the oracle was
  released with; the codec crates are pinned exactly (`image` 0.25.10, `png` 0.18.1, `flate2` 1.1.11).
  `jpeg-encoder` 0.6.1 is the only crate the oracle does not have; it only produces fixtures.
- **The output is deterministic.** Two runs write byte-identical files, and `--check` passes on any machine.
  Patterns come from seeded xorshift; no timestamps, random UUIDs or hash-map order reach a file.
- **The corpus is regenerated only from Rust 0.30.0.** A wrong oracle value is fixed in Rust, released as 0.30.x,
  and regenerated. It is never patched in a `.tsv`.
- **Modules arrive with their milestone.** Each milestone adds its emitter (`src/m_<module>.rs`, with the grid from
  the port plan §3) and registers it in `src/main.rs`.
- **The oracle owns `tests/golden/`.** `--check` reports any file there that no emitter writes as stale, and a
  plain run deletes it.
- **Generated sources.** `src/t_<module>.rs` writes a Cyrius data module into `src/` (today `src/ai_data.cyr`, Rust's
  `char::is_alphanumeric` as a range table). It is never edited by hand; `--check` compares it like the corpus, so it
  must be written in `cyrius fmt`'s canonical form.
- **`tests/golden/modules.txt`** lists every module. `tests/parity.tcyr` replays exactly that list, so a module is
  replayed as soon as its emitter exists (as pending, until its functions get dispatcher entries).

| Module | Milestone | Rows | Emitter | Notes |
|---|---|---|---|---|
| `decode` | M0 (baseline), gated at M3 | 64 | `src/m_decode.rs` | the fixtures in `tests/golden/fixtures/`, built by `src/fixtures.rs` |
| `error` | M1 | 30 | `src/m_error.rs` | `SelahError`'s Display |
| `fmtf` | M1 | 24,060 | `src/m_fmtf.rs` | f32 / f64 through Rust's Display and serde_json (zmij): every binade, powers of ten, a seeded sweep |
| `geometry` | M1 | 8,129 | `src/m_geometry.rs` | `Rect` (rects given as `Rect::new`'s arguments, returned as the layout), its JSON both ways |
| `core` | M1 | 1,331 | `src/m_core.rs` | the enums, `xml_escape`, `derive_output_path`, `Color`, `RedactionTarget`, `CaptureSource`, `Annotation` JSON |
| `uuid` | M1 | 144 | `src/m_uuid.rs` | ids written (hyphenated, simple, JSON) and read (`Uuid::parse_str`'s forms) |
| `rfc3339` | M1 | 1,169 | `src/m_rfc3339.rs` | chrono's `DateTime<Utc>` through serde, both ways |
| `ai` | M2 | 8,453 | `src/m_ai.rs` | PII detection over Rust's test inputs and a seeded generator; the stub OCR over every fixture; smart crop; the JSON shapes |
| `codec` | M3 | 464 | `src/m_codec.rs` | `convert_format`'s verdict and dimensions for every fixture of at most 1 MP against every target; `encode` rows for the encoders' round trips (below) |

## Fixtures

`tests/golden/fixtures/` holds the image inputs, built deterministically by `src/fixtures.rs` (so `--check` covers
them too) and listed with their provenance in `fixtures/INDEX.tsv`. The producers:

- `png` 0.18.1 for every colour type and bit depth it writes (palette, sub-byte, 16-bit, tRNS);
- hand-written writers for Adam7 interlace (png 0.18 writes none), malformed PNGs, and the BMP variants `image`
  cannot write (palette depths, RLE8 and RLE4, bitfields, top-down, a V4 header);
- `image` 0.25.10's own BMP, JPEG and WebP encoders, which are what selah 0.30.0 itself writes;
- `jpeg-encoder` 0.6.1 for chroma subsampling, progressive, restart intervals and CMYK.

**Ground truth.** Each JPEG fixture encoded from known pixels also gets `fixtures/<file>.source.rgba`, which holds
those pixels as RGBA8, row-major, at the fixture's size (gray sources are widened to RGBA). Lossy decode is judged by
accuracy against these pixels, with Rust's decoder as the floor (ADR 0003, port plan §4 class B). The harness prints
both decoders' mean and maximum error per fixture.

## Format 1

### Header

Each `.tsv` starts with five `#` lines: the format and module, the oracle provenance, the regenerate command, the
column list, and the row count. Lines starting with `#` are comments.

### Rows

```text
fn <TAB> class <TAB> inputs <TAB> outputs <TAB> note
```

- **`fn`** names what was called, as the port plan's surface names it (`decode`).
- **`class`** is `A`, `B`, `R` or `W` (port plan §4).
- **`inputs` and `outputs`** are space-separated tokens, or `-` when empty.
- **`note`** is free text for people. Only tab, newline, carriage return and backslash are escaped. The harness
  reads exactly one thing from it: an `adr=0002#N` tag, which marks a row the port is expected to answer
  differently under ADR 0002 row N.

### Tokens

| Token | Meaning |
|---|---|
| `i:<decimal>` | an integer |
| `g:<8 hex>` | an f32, as its IEEE-754 bit pattern |
| `f:<16 hex>` | an f64, as its IEEE-754 bit pattern |
| `s:<text>` | a string; `\\`, `\t`, `\n`, `\r` and `\s` (space) are escaped |
| `b:0` / `b:1` | a bool |
| `p:<path>` | a file under `tests/golden/` (a fixture input, or a byte blob such as decoded RGBA) |
| `c:<8 hex>` | the CRC-32 of an output too big to keep as a blob (decoded RGBA over 4 MiB) |
| `e:<Enum>.<Variant>` | an enum value |
| `h:<hex>` | raw bytes (a UUID's 16) |
| `N` | `None` |
| `E:<Variant>` | the call returned `Err` with this `SelahError` variant. In `decode` the message is in the note; from M1 an `s:` token after it carries the message (the port's `detail_out`). `E:serde` is a refused JSON document: the verdict is compared, not serde_json's message (ADR 0002 #14) |

### `decode`

`decode <class> p:fixtures/<file>` → `i:<w> i:<h> p:decode/<file>.rgba` (RGBA8, row-major, no padding) or
`E:<Variant>`. The oracle's pixels are taken through selah's own public path: `selah::annotate_image(bytes, &[],
ImageFormat::Png)` decodes exactly as `render_to_image` does (format guessing, the dimension and allocation limits,
the RGBA8 conversion) and re-encodes losslessly, which `oracle` decodes back. Class A for PNG, BMP and every
refusal; class B for a decoded JPEG.

### `codec`

- `convert_format A p:fixtures/<file> e:ImageFormat.<T>` → `i:<w> i:<h>` (the converted bytes read back with `image`)
  or `E:<Variant>`. A WebP target carries `adr=0002#1`; a source refused under ADR 0002 keeps its tag.
- `encode R p:decode/<file>.rgba i:<w> i:<h> e:ImageFormat.<T>`: the oracle's decoded pixels of a fixture, as the
  encoders' input on both sides.
  - PNG, BMP → `b:1`: the port's decode of its own encoding must give the pixels back exactly.
  - JPEG → `i:<mean x 1000> i:<max>`: Rust selah's own q75 JPEG of the pixels, decoded by `image`, against them. The
    harness gates the port's JPEG on the exact mathematics, and only reports its accuracy against these figures
    (port-parity.md §5).
  - WebP → `b:1` with `adr=0002#1`.

### Modes besides writing and `--check`

- `--reverse [dir]` (default `build/reverse`): class R's other half. `cyrius test` writes every file the port
  encodes there, beside its input pixels (`.rgba`) and size (`.dims`). Rust must decode each at the right size, and
  every PNG and BMP to exactly the pixels. JPEG accuracy against Rust's own encoding is printed per file (with
  `SELAH_REVERSE_VERBOSE=1`) and over the corpus. Exit 1 on a failure.
- `--bench`: Rust selah's codec paths timed on the image `tests/selah.bcyr` builds, decoding the files it wrote to
  `build/bench/` (`docs/benchmarks-rust-v-cyrius.md`).

## Comparison

- **A**: the verdict (ok or refused) must agree; on ok, every output token must be equal, and in `decode` the
  dimensions and every byte of the RGBA. A `g:` / `f:` float token matches bit for bit, except that any NaN matches
  any NaN (NaN payloads are not meaningful, and differ between targets).
- **B**: the verdict and dimensions must agree, and two gates hold (ADR 0003; bounds in
  `docs/development/port-parity.md` §5). **Accuracy:** where the fixture has `.source.rgba`, the port's mean and
  max error against the source are within +0.050 and +2 of Rust's. **Conformance:** every channel is within K_jpeg
  (2) of the oracle's.
- **R**: see `codec` above. Lossless means exact under both decoders; the JPEG encoder is held to the exact
  mathematics.
- **`adr=0002#N`**: the port refuses with `UnsupportedFormat`, as ADR 0002 row N says (the N is parsed exactly).
  Reported by name as accepted.
- **Known defects:** a row on the harness's `_par_known` list (a filed upstream issue) reports `upstream` while it
  still shows the defect, and fails once it passes, so the list is cut when the fix lands.
