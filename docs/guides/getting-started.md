# Getting started with selah

## Depend on it

In your project's `cyrius.cyml`:

```toml
[deps.selah]
git = "https://github.com/MacCracken/selah.git"
tag = "1.0.0"
modules = ["dist/selah.cyr"]
```

`cyrius deps` vendors the bundle into your `lib/`, with what it needs (chitra, ranga and the stdlib modules its
manifest names). Include nothing by hand: the manifest's modules are prepended to every build.

## A first program

```cyrius
fn main(): i64 {
    alloc_init();
    var n = 0;
    var png = file_read_whole_max("shot.png", 268435456, &n);
    if (png == 0) { return 1; }
    var anns = vec_new();
    var r = SlRect_new(f32_from(10.0), f32_from(10.0), f32_from(120.0), f32_from(40.0));
    vec_push(anns, SlAnnotation_new(SL_KIND_REDACTION, r, SL_COLOR_BLACK));
    var out_n = 0;
    var detail = 0;
    var t, out = selah_annotate_image(png, n, anns, SL_FMT_PNG, &out_n, &detail);
    if (is_err_result(t) == 1) {
        str_println(selah_error_display(out, detail));
        return 1;
    }
    file_write_all("shot_redacted.png", out, out_n);
    selah_bytes_free(out);
    return 0;
}

var r = main();
sys_exit_group(r);
```

Every fallible function returns a `Result` with an `SL_ERR_*` code, and its message through the optional last
argument; `selah_error_display` prints both. The README's example (tested in `tests/selah.tcyr`) adds PII detection.

## The MCP server

Take `dist/selah-mcp.cyr` instead of `dist/selah.cyr`, declare bote 3.8.15 beside it, and call
`selah_run_server("http://localhost:8090")`: the five tools over stdio. [`../mcp-tools.md`](../mcp-tools.md)
describes them.

## Working on selah itself

[`../development.md`](../development.md): building, the test suites, the golden corpus against the Rust oracle, and
the checks before a change lands.
