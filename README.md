# Selah — AI-Native Screenshot & Annotation Library

> Hebrew: סלה (pause/capture a moment)

[![License](https://img.shields.io/badge/license-GPLv3-blue)](LICENSE)
[![Language](https://img.shields.io/badge/language-Cyrius-informational)]()

**Selah** owns screenshot work for [AGNOS](https://github.com/MacCracken/agnosticos) agents: capture through daimon,
annotation and redaction drawn onto the pixels, PII detection, OCR through hoosh, a screenshot history, and the MCP
tools that expose all of it. It is a Cyrius library; the GUI and CLI live in
[taswir](https://github.com/MacCracken/taswir).

selah 1.x is a port of the Rust crate 0.30.0, which stays on crates.io for Rust consumers and is frozen in
`rust-old/` as the reference oracle. The port is held to the Rust crate's behaviour where Rust's behaviour is the
contract (JSON, layer and history files, SVG, the MCP answers), and to ground truth where there is one (pixels, the
formats' specifications): accuracy is the bar, and Rust is the floor ([ADR 0003](docs/adr/0003-accuracy-is-the-bar.md)).

## Features

- **Annotation** — arrows, rectangles, circles, text, highlights, freeform and redactions, drawn onto the pixels
  (blending through ranga) or written as an SVG overlay; layers saved and loaded as JSON
- **Redaction** — manual regions, and PII-driven: emails, phone numbers, credit cards (Luhn-checked) and IPv4
  addresses, with outward rounding so nothing leaks at the edges
- **Codecs** — PNG, JPEG and BMP in (chitra) and out (selah's own encoders); format conversion
- **Capture** — full screen, region and monitor capture through daimon, and the clipboard (`wl-copy` / `xclip`)
- **OCR** — through hoosh's chat completions, the image downscaled to fit hoosh's body cap
- **Smart crop** — suggested crop regions
- **History** — a JSONL screenshot history with OS file locking and atomic rewrites
- **MCP** — five tools (`selah_capture`, `selah_annotate`, `selah_ocr`, `selah_redact`, `selah_history`) on bote, in
  their own bundle ([docs/mcp-tools.md](docs/mcp-tools.md))

## Using selah

In your `cyrius.cyml`:

```toml
[deps.selah]
git = "https://github.com/MacCracken/selah.git"
tag = "1.0.1"
modules = ["dist/selah.cyr"]
```

For the MCP server, take the `mcp` profile's bundle instead, and declare bote yourself:

```toml
[deps.selah]
git = "https://github.com/MacCracken/selah.git"
tag = "1.0.1"
modules = ["dist/selah-mcp.cyr"]

[deps.bote]
git = "https://github.com/MacCracken/bote.git"
tag = "3.8.15"
modules = ["dist/bote.cyr"]
```

Then `selah_run_server("http://localhost:8090")` serves the tools over stdio; `selah_mcp_dispatcher` gives the bote
dispatcher for another transport.

## Example

PII in a string, then a box and a redaction drawn onto an image and returned as JPEG. This is the code
`tests/selah.tcyr` compiles against the bundle and runs:

<!-- example: tests/selah.tcyr -->

```cyrius
# vec_get returns an untyped word; a typed pointer comes back through a function's return type.
fn as_suggestion(p): *SlRedactionSuggestion {
    return p;
}

# Prints the PII found in `text`, then draws a red box and a black redaction onto the image png[0..n) (PNG, JPEG or
# BMP) and returns it as JPEG: the bytes (free them with selah_bytes_free) and their length at `out_len`, or 0.
fn readme_example(text, png, n, out_len): i64 {
    var found = selah_suggest_redactions(text, strlen(text));
    var i = 0;
    while (i < vec_len(found)) {
        var s: *SlRedactionSuggestion = as_suggestion(vec_get(found, i));
        var sb = str_builder_new();
        str_builder_add_cstr(sb, SlRedactionTarget_key(s.target));
        str_builder_add_cstr(sb, ": ");
        str_builder_add(sb, s.matched_text);
        str_println(str_builder_build(sb));
        i = i + 1;
    }
    var anns = vec_new();
    var box = SlRect_new(f32_from(10.0), f32_from(10.0), f32_from(200.0), f32_from(80.0));
    vec_push(anns, SlAnnotation_new(SL_KIND_RECTANGLE, box, SL_COLOR_RED));
    var bar = SlRect_new(f32_from(40.0), f32_from(120.0), f32_from(160.0), f32_from(24.0));
    vec_push(anns, SlAnnotation_new(SL_KIND_REDACTION, bar, SL_COLOR_BLACK));
    var detail = 0;
    var t, out = selah_annotate_image(png, n, anns, SL_FMT_JPEG, out_len, &detail);
    if (is_err_result(t) == 1) {
        str_println(selah_error_display(out, detail));
        return 0;
    }
    return out;
}
```

## Conventions

- **Names.** Rust's free functions are `selah_<name>` (`selah::annotate_image` is `selah_annotate_image`), types are
  `Sl<Name>` with their methods as `Sl<Name>_<method>`, and constants and enum members are `SL_*`. Every public enum
  keeps explicit, append-only values.
- **Errors.** A fallible function returns a `Result` whose error is an `SL_ERR_*` code; its text comes back through an
  optional trailing `detail_out`, and `selah_error_display(code, detail)` prints them as Rust's `SelahError` did.
  Library code never aborts.
- **Memory.** Bytes a function returns (an encoded image) are freed with `selah_bytes_free`. Functions that build
  lists also come in `_a` forms that take an allocator.
- **Where the API differs from Rust 0.30**, [ADR 0002](docs/adr/0002-port-api-divergences.md) lists every change and
  how parity still compares it: the async clients are blocking with deadlines, the daimon and hoosh clients speak to
  the live Cyrius services, WebP is recognised but refused, and so on.

## Accuracy and parity

- A golden corpus of 48,095 rows, generated from the Rust crate itself (`scripts/oracle/`), is replayed on every
  `cyrius test`, with the same results on x86_64, aarch64 and x86_64 PE (the MCP server's rows on x86_64 and aarch64)
  ([port-parity.md](docs/development/port-parity.md)).
- All 289 of the Rust crate's tests are accounted for: 288 ported, one not applicable
  ([rust-test-ledger.md](docs/development/rust-test-ledger.md)).
- Every public item of the Rust crate has a counterpart, or an ADR 0002 row (`scripts/surface_parity.py`).
- Benchmarks against Rust's: [docs/benchmarks-rust-v-cyrius.md](docs/benchmarks-rust-v-cyrius.md).

## Known limits

Each is filed with the library that owns it, and tracked in [state.md](docs/development/state.md):

- Capture against a live daimon waits on daimon's `/v1/screen/*` routes; the client is tested against a mock server.
- WebP and CMYK JPEG are refused (CMYK: chitra returns the samples since 1.4.0, and the conversion to RGB is on the
  roadmap).
- OCR through hoosh reaches OpenAI-compatible vision models only.
- One MCP request line over stdio is at most 128 KiB, about 1,950 annotations (bote's limit).
- The MCP bundle does not build for Windows (PE): bote's dependency majra does not target it. The library itself
  does.

## AGNOS integration

- **daimon** (port 8090) — screen capture, monitors, agent registration
- **hoosh** (port 8088) — OCR through vision models
- **MCP** — the five tools, for agents and agnoshi intents ("take screenshot", "annotate image", "redact sensitive
  data")

## Development

[docs/development.md](docs/development.md) covers building, the tests, the oracle and the release checks;
[docs/architecture/overview.md](docs/architecture/overview.md) the modules and data flow.

## License

GPL-3.0-only
