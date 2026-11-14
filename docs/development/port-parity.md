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

## 2. Findings to settle before M3 gates `decode`

Each finding is a real difference between chitra and Rust's decoders, not a fault in the harness. M3 can't gate
`decode` until each one is either fixed upstream, handled in selah, or accepted in ADR 0002. The options are listed,
with a recommendation; the choice is the maintainer's.

### F1 — JPEG chroma upsampling: smooth (Rust) versus box (chitra)

- **Where:** every subsampled JPEG.
- **Size of the difference:**
  - 4:2:0: max 36, with 7,390 of 16,384 bytes differing at 64×64;
  - 4:2:2: max 16;
  - 4:4:0: max 13;
  - an odd-size 4:2:0: max 28.
- **Cause:**
  - zune-jpeg 0.5.15 (Rust's decoder) upsamples chroma with libjpeg's default "fancy" triangle filter;
  - chitra does box upsampling and is verified against `djpeg -nosmooth` (its README);
  - the 4:4:4 and gray rows (max 1–2) show the IDCTs agree, so the remainder is upsampling.
- **Why it matters:** most real-world JPEGs are 4:2:0. A redaction or annotation drawn on a JPEG source would differ
  from Rust's by up to 36 per channel near colour edges.
- **Options:**
  - **(a) Ask chitra for fancy upsampling** (libjpeg's `h2v1` / `h2v2` filters), either as default or opt-in. Then
    K_jpeg ≈ 2 for every JPEG. **Recommended.**
  - **(b) Accept box upsampling in ADR 0002** with a per-subsampling bound (K ≈ 36). Class B then says little near
    edges.
  - **(c) Upsample in selah.** Not possible: chitra exposes only the final RGBA8, not the chroma planes.

### F2 — IDCT rounding

- **Where:** JPEG without subsampling.
- **Size:** max \|d\| 2. The `image`-encoded fixtures and jpeg-encoder's 4:4:4 give 2; grayscale gives 1.
- **Cause:** chitra's README documents `_chitra_jpeg_descale` rounding half away from zero, within IEEE 1180's
  tolerance; zune-jpeg's IDCT rounds differently.
- **Proposal:** **K_jpeg = 2** for JPEG without subsampling. This is the bound class B gates on at M3 (and for every
  JPEG if F1 is fixed upstream).

### F3 — BMP 16-bit channel expansion

- **Where:** 5-5-5 and 5-6-5 BMPs.
- **Size:** max \|d\| 1.
- **Cause:** the oracle expands a 5-bit channel as `round(v · 255 / 31)` (and 6-bit as `round(v · 255 / 63)`), which
  reproduces all 561 pixels of the 5-5-5 fixture. Bit replication `(v << 3) | (v >> 2)` reproduces only 386, so it
  is evidently chitra's.
- **Options:**
  - (a) ask chitra for the exact rounding;
  - (b) **accept in ADR 0002** with a ±1 bound on 16-bit BMP rows. **Recommended:** 16-bit BMP screenshots are rare,
    and ±1 is invisible.
  - (c) decode 16-bit BMPs in selah. That duplicates chitra.

### F4 — BMP 32-bit `BI_RGB`: is the fourth byte alpha?

- **Where:** `BI_RGB` 32-bit BMPs whose fourth byte is not 255.
- **Size:** max \|d\| 254, all in alpha.
- **Cause:**
  - the oracle follows the BMP spec: in `BI_RGB` the fourth byte is reserved, so every pixel is opaque;
  - chitra uses the byte as alpha;
  - a V4 header with an explicit alpha mask is bit-identical on both sides.
- **Why it matters:** such a BMP decodes **partly transparent** in the port, and every redaction, highlight or
  re-encode then carries that alpha.
- **Options:**
  - **(a) Guard in selah**: when the BMP header says `BI_RGB` and 32 bpp, force alpha to 255 after decoding. That is
    a few lines in `src/codec.cyr`, with no upstream dependency. **Recommended**, together with (b).
  - (b) report it to chitra as a spec divergence.
  - (c) accept it. Not recommended.

### F5 — CMYK JPEG

- **Where:** CMYK and YCCK JPEGs (Adobe APP14).
- **What happens:** Rust decodes and converts them to RGB; chitra refuses them, permanently by its README.
- **Options:**
  - **(a) accept in ADR 0002** as a refusal, beside progressive JPEG (#2). Screenshots are never CMYK.
    **Recommended.**
  - (b) convert CMYK in selah. Not possible on chitra's RGBA8-only output.

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

| Bound | Value | Set | Gates from |
|---|---|---|---|
| K_jpeg (no chroma subsampling) | 2 | 2026-11-14, from §1 | M3 |
| K_jpeg (subsampled) | open: 2 if F1 (a), about 36 if F1 (b) | — | M3 |
| 16-bit BMP | 1, if F3 (b) | — | M3 |
