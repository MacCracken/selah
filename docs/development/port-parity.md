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

Each bound is set before the first gated comparison (M3), and none is ever widened.

| Bound | Value | Set | Gates from |
|---|---|---|---|
| Class B accuracy, mean | port mean abs error vs source ≤ reference's + **0.05** | 2026-11-14, from §2: the largest IDCT-only excess is 0.007, and the upsampling defect it must catch is 0.13–1.34 | M3 |
| Class B accuracy, max | port max abs error vs source ≤ reference's + **2** | 2026-11-14 (K_jpeg) | M3 |
| K_jpeg (conformance, same upsampling) | **2** per channel | 2026-11-14, from §1 | M3 |
| 16-bit BMP | class A against the correctly rounded value (F3) | 2026-11-14 | M3, on the known-defect list until chitra rounds |
