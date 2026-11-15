# selah port parity — measured baselines and bounds

> The measurements behind the port plan's parity classes (§4). A bound is set here **before** the comparison it
> gates, and never widened to make a row pass.

## 1. Decode baseline (M0 step 7, 2026-11-14)

**Setup.**
- The oracle is `tests/golden/decode.tsv`: 63 fixtures, with Rust selah 0.30.0's verdict and RGBA8 pixels through
  its own `annotate_image` path (`scripts/oracle/README.md`).
- The port side is the M0 stand-in in `tests/parity.tcyr`: selah's signature sniff (GIF, WebP and unknown refused),
  then chitra 1.3.1's `chitra_image_decode_budget` at `MAX_DECODE_ALLOC`.
- `decode` is **not gated** yet: differences are recorded as "measured".

**Result: `[decode] ok 47, FAILED 0, measured 14, accepted 2`.** It is **identical on x86_64 ELF, aarch64 (qemu)
and x86_64 PE (wine)**, byte for byte across the three outputs, because chitra is integer-only.

| Group | Fixtures | Result |
|---|---|---|
| PNG: RGBA8, RGB8, gray 1/2/4/8, gray+alpha, palette 1/2/4/8, tRNS (palette, RGB key, gray key), 16-bit (RGB, RGBA, gray, gray+alpha), Adam7 (RGBA8, palette 4-bit, gray 1-bit) | 24 | **all bit-identical** |
| BMP: `image`'s RGB8 / RGBA8 / L8, 24-bit bottom-up and top-down, palette 1/4/8, RLE8, RLE4, V4 32-bit with an alpha mask | 11 | **all bit-identical** |
| BMP 16-bit (5-5-5, and 5-6-5 bitfields) | 2 | max \|d\| **1** (finding F3) |
| BMP 32-bit `BI_RGB` (noise in the fourth byte) | 1 | max \|d\| **254**, in the alpha channel (finding F4) |
| JPEG without chroma subsampling: `image`'s encoder at q50 gray, q75 and q95 RGB, q75 noise; jpeg-encoder 4:4:4 and gray | 6 | max \|d\| **2** (finding F2) |
| JPEG with chroma subsampling: 4:2:0 (64×64, 33×17, restart interval), 4:2:2, 4:4:0 | 5 | max \|d\| **13–36** (finding F1) |
| JPEG 4:2:0 1×1 | 1 | bit-identical |
| CMYK JPEG | 1 | **chitra refuses** ("JPEG component count unsupported"); Rust decodes (finding F5) |
| Progressive JPEG; lossless WebP | 2 | accepted: refused, as ADR 0002 #2 and #1 say |
| Refusals: truncated PNG and JPEG, a corrupted IDAT CRC, IHDR 40000×1 (over `MAX_IMAGE_DIMENSION`), IHDR 20000×20000 (1.6 GB, over `MAX_DECODE_ALLOC`), a BMP with an unknown DIB header size, GIF, empty, noise, text | 11 | **every verdict agrees** |

**Class-B byte histogram** over every decoded JPEG byte: \|d\| = 0: 92,036; 1: 15,223; 2: 9,965; 3–8: 5,707;
over 8: 737; max 36.

**Since the baseline:** the CMYK row is `accepted` under ADR 0002 #16, a temporary refusal filed with chitra. The
run is now `ok 47, measured 13, accepted 3`. The 13 JPEG fixtures also carry their source pixels, so §2 measures
accuracy, not just the difference from Rust.

## 2. Findings, measured against the source, and filed

Each finding is a real difference between chitra 1.3.1 and Rust's decoders, not a fault in the harness. Under
[ADR 0003](../adr/0003-accuracy-is-the-bar.md) the question is not "which matches Rust" but **"which is right"**:
measured against the pixels each JPEG was encoded from, and against the BMP format's own definition. A chitra defect
is filed with chitra, and nobody has to choose an option.

### Accuracy against the source pixels (2026-11-14)

The oracle writes each JPEG fixture's source pixels as `fixtures/<file>.source.rgba` (RGBA8). `tests/parity.tcyr`
decodes each fixture and prints both decoders' error against the source: mean and maximum absolute error over the
RGB channels, on a 0–255 scale.

| Fixture | Subsampling | chitra 1.3.1: mean / max | Rust (`image` 0.25.10, zune-jpeg): mean / max |
|---|---|---|---|
| `jpeg-image-q75-rgb-33x17-grad` | none | 2.674 / 32 | 2.676 / 32 |
| `jpeg-image-q95-rgb-64x64-grad` | none | 0.684 / 9 | 0.687 / 9 |
| `jpeg-image-q50-gray-33x17-grad` | gray | 0.545 / 1 | 0.545 / 1 |
| `jpeg-image-q75-rgb-33x17-noise` | none | 13.944 / 76 | 13.937 / 76 |
| `jpeg-enc-444-64x64` | 4:4:4 | 1.123 / 24 | 1.121 / 24 |
| `jpeg-enc-gray-64x64` | gray | **0.281** / 1 | 0.296 / 1 |
| `jpeg-enc-420-64x64` | 4:2:0 | **2.345 / 78** | 1.808 / 54 |
| `jpeg-enc-restart-64x64` | 4:2:0, DRI 2 | **2.345 / 78** | 1.808 / 54 |
| `jpeg-enc-420-33x17` | 4:2:0, partial MCUs | **5.402 / 58** | 4.062 / 47 |
| `jpeg-enc-422-64x64` | 4:2:2 | **1.873** / 51 | 1.578 / 51 |
| `jpeg-enc-440-64x64` | 4:4:0 | **1.991 / 47** | 1.865 / 46 |
| `jpeg-enc-420-1x1` | 4:2:0 | 0 / 0 | 0 / 0 |
| `jpeg-enc-progressive-64x64` | 4:2:0, SOF2 | refused | 1.808 / 54 |

Without subsampling the two decoders are equally accurate: the largest gap in the mean is 0.007 (the noise
fixture), and chitra is the closer of the two on grayscale. With subsampling chitra is 0.13–1.34 worse on the mean
and up to 24 worse at the worst pixel. All of that gap is the upsampler.

### F1: JPEG chroma upsampling, box (chitra) against interpolated (Rust): chitra defect, filed

- **Measured:** the table above. On the 4:2:0 fixtures chitra's mean error is about 30 % higher, and its worst
  pixel is 78 levels off against Rust's 54. Against Rust directly the difference is at most 36 per channel (§1).
- **Cause:** chitra replicates each chroma sample over its block, as `djpeg -nosmooth` does. JFIF places chroma at
  the block's centre, and libjpeg's default interpolating filter (which zune-jpeg also uses) reconstructs that.
- **Filed:** `chitra/docs/development/issues/2026-11-14-box-chroma-upsampling-loses-accuracy.md` asks for
  interpolation as the default, verified against `djpeg` without `-nosmooth` and against these source pixels.
- **Meanwhile:** selah can't upsample for itself, because chitra returns only RGBA8 (ledger L12). The subsampled
  rows go on the known-defect list at M3.

### F2: IDCT rounding: not a defect

- **Measured:** without subsampling, the two decoders differ from each other by at most 2 per channel, and they are
  equally accurate against the source (table above).
- **Cause:** chitra's `_chitra_jpeg_descale` rounds half away from zero; zune-jpeg's IDCT rounds differently. Both
  are inside IEEE 1180.
- **Bound:** **K_jpeg = 2** is class B's conformance bound between two correct decoders, not a "match Rust"
  allowance. Class B's accuracy gate is the one that catches F1.

### F3: BMP channels narrower than 8 bits: chitra defect, filed

- **Measured:**
  - the 5-5-5 and 5-6-5 fixtures differ by 1 on 204 and 235 bytes;
  - the oracle's `round(v · 255 / (2ⁿ − 1))` reproduces every pixel;
  - chitra's bit replication `(v << 3) | (v >> 2)` does not.
- **Which is right:** the nearest 8-bit value to `v · 255 / (2ⁿ − 1)`.
  - Rounding is never more than 0.484 away from it at 5 bits, nor 0.476 at 6 bits.
  - Replication is up to 0.677 and 0.714 away, and differs from rounding on 4 of 32 and 10 of 64 levels.
  - The PNG specification (§ 13.13) calls the rounded form the most accurate.
- **Filed:** `chitra/docs/development/issues/2026-11-14-bmp-sub-8-bit-replication-is-not-the-nearest-value.md`.
- **Meanwhile:** these are class A rows against the correctly rounded value, on the known-defect list (L13).

### F4: 32-bpp `BI_RGB` BMP, the fourth byte read as alpha: chitra defect, filed

- **Measured:** max \|d\| 254, all in alpha. 559 of 561 pixels decode non-opaque.
- **Which is right:** opaque. In Microsoft's `BITMAPINFOHEADER` definition, the high byte of each `BI_RGB` DWORD
  "is not used". Alpha is declared through a mask, and chitra reads declared alpha correctly: the V4 fixture is
  bit-identical. chitra instead guesses from the data, which its own ADR 0009 forbids for CMYK.
- **Filed:** `chitra/docs/development/issues/2026-11-14-bmp-bi-rgb-32bpp-unused-byte-read-as-alpha.md`.
- **Meanwhile:** selah decodes these files opaque itself, in `src/codec.cyr` at M3 (ledger L11), so the row is
  class A and `ok`. The guard leaves when chitra lands the fix.

### F5: CMYK and YCCK JPEG: chitra gap, filed

- **What happens:** Rust decodes them and converts to RGB. chitra refuses them (its ADR 0009 declines to guess a
  colour conversion).
- **Filed:** `chitra/docs/development/issues/2026-11-14-cmyk-ycck-jpeg-refused.md`. It asks for:
  - the CMYK samples, which the file determines with no guess;
  - the Adobe inversion flag;
  - ICC as the accurate path to RGB.
- **Meanwhile:** selah refuses these files, as ADR 0002 #16 records (ledger L14).

### Progressive JPEG: chitra deferral, now with a consumer, filed

- chitra's roadmap defers SOF2 "until a consumer demonstrably needs it", and selah is that consumer.
- **Filed:** `chitra/docs/development/issues/2026-11-14-progressive-jpeg-has-a-consumer.md`.
- **Meanwhile:** refused, as ADR 0002 #2 records (ledger L15).

## 3. M1 (2026-11-14): everything class A, nothing measured

The six M1 modules have no tolerance class but A: 34,863 rows (`error` 30, `fmtf` 24,060, `geometry` 8,129, `core`
1,331, `uuid` 144, `rfc3339` 1,169), **every one equal, and the harness output byte-identical on x86_64 ELF, aarch64
(qemu) and x86_64 PE (wine)**. f32 arithmetic in Rust's order needed no bound: every rect operation is bit-identical,
NaN rows matching as NaN. Three facts the corpus taught (each now pinned):

- serde_json 1.0.154 (zmij) writes floats positionally for a decimal exponent in −5..=15 (f64) but −6..=12 (f32);
- serde accepts a unit variant as `{"Variant": null}` and a struct as a JSON array of its fields;
- Rust's `f32::min` / `f32::max` (glam 0.29.3's `Vec2::min` / `max`) give the second operand on equal operands,
  ±0 included, and the non-NaN one when one is NaN.

## 4. M2 (2026-11-14): everything class A

`ai`'s 8,451 rows are all class A and all equal, identical on x86_64 ELF, aarch64 and x86_64 PE. Two facts the corpus
taught:

- rustc 1.100's `char` methods are **Unicode 18.0.0**; the stdlib's `lib/unicode` is 17.0.0. The email trim
  (`!c.is_alphanumeric()`) therefore reads a table generated from Rust itself (`src/ai_data.cyr`), not general
  categories.
- serde's array form of a struct may leave off a trailing field that has `#[serde(default)]` (`OcrResult`'s
  `is_stub`).

## 5. Bounds

Each bound is set before the first gated comparison, and none is ever widened. The one exception, the JPEG encoder's
bound, is recorded below with its history and the maintainer's approval.

| Bound | Value | Set | Gates from |
|---|---|---|---|
| Class B accuracy, mean | port mean abs error vs source ≤ reference's + **0.05** | 2026-11-14, from §2: the largest IDCT-only excess is 0.007, and the upsampling defect it must catch is 0.13–1.34 | M3 |
| Class B accuracy, max | port max abs error vs source ≤ reference's + **2** | 2026-11-14 (K_jpeg) | M3 |
| K_jpeg (conformance, same upsampling) | **2** per channel | 2026-11-14, from §1 | M3 |
| 16-bit BMP | class A against the correctly rounded value (F3) | 2026-11-14 | M3, on the known-defect list until chitra rounds |
| Class R, PNG and BMP | decode(encode(px)) = px exactly, under chitra and under Rust's `image` | port plan §4 (M0) | M3 |
| Class R, JPEG encoder | the quantised coefficients against the exact mathematics: **none off by more than 1, at most 1 % off by one**, per image | 2026-11-14, M3 (see below) | M3 |

### The JPEG encoder's bound: what was set first, and why it changed

This change was made **after a first comparison**, which CLAUDE.md's rule forbids for a widening. So it is written
down in full here. It was put to the maintainer when M3 closed, and **approved on 2026-11-14**.

1. **Set first (M3 start):** per image, the port's JPEG decoded to the input no worse than Rust's own JPEG of the
   same pixels: mean error + 0.050 and max + 2, the class B margins.
2. **First comparison:** 7 of 53 images failed, under either decoder. All 7 were synthetic noise (palette and RGB
   noise fixtures), plus a q75 noise JPEG that Rust's encoder had itself produced.
3. **What it measured:**
   - With `image` 0.25.10's own arithmetic switched in (8-bit YCbCr, its FDCT, truncate then round), no image came
     out worse than Rust's (worst margin +0.000 in mean error, Rust decoding both). So the encoder's structure was
     right: tables, Huffman coding, block order and edge replication. The switch was an experiment and is not in
     the code.
   - With exact arithmetic, the per-image differences from Rust run from −1.28 to +0.55 dB PSNR, both ways and on
     gradients too. Over the corpus the port is ahead: mean error 8.611 against 8.648, PSNR 28.542 against
     28.531 dB.
   - The worst image is Rust re-encoding a JPEG its own encoder wrote, which reproduces the earlier coefficients
     almost exactly.
   - So a per-image "no worse than Rust" bound measures rounding noise between two correct encoders. The only way to
     pass it is to copy `image`'s arithmetic, which ADR 0003 rules out.
4. **What replaced it:** the encoder's own definition. For given quantisation tables the right coefficients are
   round(DCT(YCbCr) / Q), with JFIF's colour transform, T.81's DCT and the edge replicated. The harness evaluates
   this directly in f64 and compares every coefficient the encoder writes.
   - The port: **18 of 223,872 coefficients off by one (0.008 %), none by more.**
   - `image` 0.25.10's arithmetic, transcribed into a scratch script: 1,263 of the same 223,872 off (0.56 %).
     Its FDCT's second pass rounds its odd outputs with 2^10 where 2^14 is right, it truncates each coefficient to
     an integer before rounding it, and it rounds YCbCr to 8 bits first.
   - The per-image comparison with Rust stays in both the harness and `oracle --reverse`, as a measurement.

## 6. M3 (2026-11-14): codecs gated

| Module | Result |
|---|---|
| `decode` | **ok 53, accepted 3, upstream 8, FAILED 0**: every PNG and BMP row bit-identical but the two 16-bpp BMPs (F3); every JPEG within class B but the five subsampled ones (F1); progressive, CMYK and WebP refused as ADR 0002 #2, #16 and #1 say; the 16.8 MP PNG refused by chitra's pixel cap (L17). The F4 row is now `ok`: selah decodes 32-bpp `BI_RGB` opaque (L11) |
| `codec` / `convert_format` | 252 rows: every verdict and every converted image's dimensions as Rust's (WebP targets accepted under ADR 0002 #1) |
| `codec` / `encode` | 212 rows: PNG and BMP lossless under chitra (53 + 53); JPEG within the exact-mathematics bound (53); WebP accepted (53) |
| `oracle --reverse` | Rust decodes all 159 Cyrius-encoded files; every PNG and BMP to the exact input pixels |

The harness output is identical on x86_64 ELF, aarch64 (qemu) and x86_64 PE (wine).

**Rust defect found:** `image` 0.25.10's JPEG encoder arithmetic (above), recorded in ADR 0002 #17. The port does not
reproduce it.

**chitra, filed during M3 (beside F1–F5):**
- memory, `2026-11-14-every-decode-leaks-in-a-long-running-consumer` (L16);
- the pixel cap, `2026-11-14-fixed-16-mp-cap-refuses-screenshots` (L17);
- decode speed, `2026-11-14-decode-throughput-is-about-20x-rust-image` (L18; numbers in
  `docs/benchmarks-rust-v-cyrius.md`).

## 7. M4 (2026-11-14): annotation, all class A

| Rows | Result |
|---|---|
| `render` 2,663 | the RGBA of every rendering equal (by CRC-32) to Rust's: 7 kinds × 29 rect classes (normal, full, fractional, negative, partly and wholly off-canvas, NaN in each component, ±∞, 1e20 to 3e38, zero, tiny, subnormal, −0, arrows through and from off-canvas) × 3 colours (alpha 255, 128, 0) on 1×1, 7×3, 33×17 and 64×64 sources, one colour on 512×512, and overlapping sets; refused sources and NaN redactions refused alike |
| `to_svg` 415 | equal text |
| `layer_json` 61 | serde_json's pretty print byte for byte; 16 canvases with a non-finite region expect the corrected refusal (ADR 0002 #19) |
| `layer_load` 84 | Rust-written layers and hand-written variants (array form, unknown fields, version 0 / 2 / u32::MAX / 2^32 / 1.0, negative size, bad dates, missing fields, a duplicate key, an f32-overflowing rect, trailing text, whitespace): every verdict, size and annotation equal |
| `decode_image_data` 34 | every verdict equal: bayan's base64 is canonical-only, like Rust's STANDARD engine |
| `redact_image` 200 | equal, with the WebP rows accepted (ADR 0002 #1) and the 16-bpp BMP rows upstream (F3) |
| `oracle --reverse` | Rust loads all 45 Cyrius-written layers (and decodes the 165 encoded images) |

The harness output is identical on x86_64 ELF, aarch64 (qemu) and x86_64 PE (wine).

**Facts the grid taught:**
- ranga 2.4.0's `blend_row` and Rust ranga 1.0.0's (AVX2 on this host) agree to the bit on every highlight.
- The ellipse's `cos` / `sin` (fdlibm polyfills in Cyrius, glibc's libm in Rust) give the same pixels on the whole
  grid, so there is no class to set.
- Rust writes a layer file it cannot read back when an annotation's region is not finite (`null` in JSON). Fixed in
  the port: the save is refused (ADR 0002 #19).

Two mutation probes confirmed the gates bite: a highlight opacity of 77 for 76 failed 185 render rows, and one SVG
radius from the wrong side failed 37.


## 8. M5 (2026-11-14): system and clients, all class A (and W for the retargeted wire)

The rows drive Rust's own HistoryStore, CaptureClient, DaimonClient and HooshClient. The clients run on a
current-thread tokio runtime against a one-shot loopback server, a copy of Rust's `serve_once`, so every expectation
is what Rust's real client sent or concluded.

| Rows | Result |
|---|---|
| `history_line` 47, `history_id` 47 | serde_json's verdict on each line and the entry read, and the private EntryId's read: valid lines, whitespace and `\r`, unknown members, u32 edges (2^32, −1, `-0`, `1.0`, `1e2`, a string), every Uuid form, chrono's relaxed RFC 3339 (`t`, `z`, a space, offsets, 10 fraction digits, a leap second, February 30th, no zone, year 12026), the array form (7, 6 and 1 elements), torn and malformed lines, and invalid UTF-8 in a kept field. 3 lines × 2 rows are accepted under ADR 0002 #21 |
| `history_json` 11 | serde_json's line byte for byte: escapes and control characters, the fraction's 0 / 3 / 6 / 9 digits, years 0001 to 9999 and outside them, a leap second |
| `history_list` 160, `history_get` 56, `history_delete` 56 | over Rust-written files (with foreign lines, a torn tail, a missing final newline, duplicates, `\x0c` and `\r`, the array form, 200 entries): every limit (0, 1, 3, 10, unbounded) × `since` (none, before, on a nanosecond, after), every id; the order (newest first, ties in file order) and the rewritten file equal |
| `region` 39 | CaptureClient::capture's request body, or its InvalidRegion text with the rect's Debug form, over 33 rects (normalized, clipped, fractional edges, ±∞, NaN, 2^32 and past, f32 extremes); the 4 WebP rows are accepted (ADR 0002 #1) |
| `monitor_req` 12 | the body, and the blank-id refusal over Unicode whitespace (U+3000, U+0085, U+00A0) |
| `capture_http` 22, `monitors_http` 10 | the answer read (struct, array form, u32 / i32 edges, `-0`, a bool as 1, missing fields), and the status text: reason phrases, an unknown code (599), empty bodies, the 512-byte cut with and without `…`, a multi-byte character at the cut, and invalid UTF-8 decoded lossily as Rust does |
| `ocr_answer` 28 | the Rust-shape answer read into an OcrResult: the confidence clamp (`7.5`, `-1`, a string, null, `-0`, `-0.0`, subnormal, u64::MAX, `1e400` refusing the document), parse_region over f32 overflow, subnormals, non-objects and extra members, a non-object answer (defaults), invalid JSON; one duplicate-key row is accepted (ADR 0002 #21) |
| `agent_path` 26 | validate_agent_id's verdict and text (Rust's Debug of the id, C1 controls escaped), and the path the url crate encodes (`%2F`, `%3F`, `%23`, `%25`, UTF-8, the sub-delims it leaves) |
| `session` 25, `mime` 31, `exit_status` 39, `rect_debug` 33 | detect_session_type under each pair of variables; image 0.25.10's MIME table and its fallback; ExitStatus's Display for codes, the 31 Linux signals and core dumps; `{rect:?}` |
| `daimon_config` 9, `daimon_config_json` 3, `hoosh_config` 4 | serde's verdicts and JSON, the array forms included |
| `wire_register` 2, `wire_ocr` 2 (W) | the retargeted requests (ADR 0002 #4, #6), against fixtures from daimon's `api_register_agent` and the OpenAI content-part shape hoosh forwards |
| `oracle --reverse` | Rust lists all 3 Cyrius-written history files (one with a torn line, one rewritten by delete) and finds the port's order |

662 rows: 654 ok, 8 accepted, 0 failed. The harness output is identical on x86_64 ELF, aarch64 (qemu) and x86_64 PE
(wine).

**Facts the grid taught:**
- bayan reads a bare `-0` as the integer 0 and refuses an integer past i64, where serde_json reads the float −0.0 and
  the nearest double. A token pre-pass in `src/json.cyr` now gives bayan the float spelling, so every JSON reader in
  selah (layers and annotations included) reads them as serde does. serde_json also refuses a float literal past f64
  (`1e400`) in a field it reads; bayan reads ∞, which `_sl_json_as_f64` now refuses.
- serde accepts a defect in a member it ignores (a duplicate key, a lone surrogate, invalid UTF-8) where bayan
  refuses the document. That is recorded as ADR 0002 #21, not imitated.
- Rust's `{:?}` escapes every character core does not print (C1 controls, combining marks, unassigned code points).
  The set is generated from Rust (`src/debug_data.cyr`, 914 ranges, the same at every position in a string), so
  selah's Debug texts are exact beyond ASCII.

Two mutation probes confirmed the gates bite: a wrong reason phrase for 403 and printing U+0085 raw each failed their
row.

## 9. M6 (2026-11-14): the MCP server, class A but for the shape of a schema refusal

The rows drive Rust's real server. The oracle runs itself again as `--mcp-serve <url>`, which calls
`selah::mcp::run_server` with its working directory in a fresh session directory and `XDG_DATA_HOME` inside it, and
talks to it over pipes, a request per line. Capture sessions get a one-shot daimon (M5's `serve_once`). The harness
(`tests/mcp/parity_mcp.tcyr`) builds the same directory (the fixtures in `tests/golden/mcp/`, a directory named
`d.png`, a symlink `link.png`, the history file) and sends the same requests through `selah_mcp_dispatcher`.
Responses are compared byte for byte after re-serializing with sorted keys and masking the directory, times and ids;
an image a session writes is decoded and compared pixel for pixel.

| Rows | Result |
|---|---|
| `tool` 5 | each tool's description and input schema; rows sorted by name (Rust's order is a HashMap's) |
| `selah_annotate` 28 | every kind with colours and alpha, a negative size; PNG, JPEG, BMP and extensionless inputs; outputs by extension (`.png`, `.jpg`, `.bmp`), an existing file overwritten, a relative path; the refused outputs (`..`, a directory, a symlink, `.bashrc`, empty) and inputs (a key file, missing, a directory, empty, `..`, NUL); `invalid annotation N: …` for a coordinate past f32, a colour without alpha, a channel past 255; WebP in or out (ADR 0002 #1) |
| `selah_ocr` 11 | the local engine over a plain PNG, one with text in its metadata, one with PII, a JPEG; the hoosh engine (ADR 0002 #24); refused paths and arguments |
| `selah_redact` 15 | all targets and subsets of them, an empty list, refused targets; outputs as PNG, BMP and JPEG; the refusal when PII is found without a location |
| `selah_history` 35 | `limit` absent (20), 0, 2 and 1000, and 1001, −1 and 2.5 refused; `since` in 28 forms: 12 chrono accepts (fractions of 1, 9 and 10 digits, one nanosecond past an entry, a leap second, year 0, offsets, a space, a lowercase `t` or `z`, U+2212 as the offset's minus) and 16 it refuses, each with chrono's text |
| `selah_capture` 16 | full and region captures saved as PNG and BMP, a JPG name for PNG data, `JPG` and `tiff` refused, refused paths, a zero-width and an incomplete region, a region past f32, WebP (ADR 0002 #1), a 403, an answer that is not JSON, image data that is not base64; each capture followed by `selah_history` with limit 1, which shows the entry it recorded |

110 rows: 106 ok, 4 accepted (three WebP rows under ADR 0002 #1, the hoosh engine under #24), 0 failed. 19 rows compare
the verdict: Rust's bote answers a schema violation with JSON-RPC −32602 and bote 3.8.15 with an `isError` result
(ADR 0002 #24). The harness output is the same on x86_64 and aarch64 (qemu).

**Facts the grid taught:**
- Rust's bote 0.92 lists the tools in HashMap order, which changes between runs: the first M6 corpus failed
  `oracle --check` until the rows were sorted.
- bote 0.92 checks enums inside array items too, case-sensitively. The aliases selah's own parser accepts for a kind
  (`Rectangle`, `rect`, `ELLIPSE`, `free-form`, `redact`) never reach it over MCP; both servers refuse them.
- chrono's `parse_from_rfc3339`, which reads `since`, is strict where the history file's reader is relaxed.
  `_sl_rfc3339_strict` gives chrono's verdict and its four texts (`premature end of input`, `input contains invalid
  characters`, `input is out of range`, `trailing input`) on all 28 forms, U+2212 as a minus sign included.
- Rust's bench directory is the system temp directory, a tmpfs here; the first Cyrius bench wrote to `build/` on
  ext4, where a 200-byte write costs 58–71 µs against 16 µs. Both now run in the temp directory.

Two mutation probes confirmed the gates bite: accepting a `..` component failed 5 rows (and the capture probe wrote
its `../x.png` outside the session directory, which is what the check prevents), and shortening chrono's `premature
end of input` failed 5 rows.
