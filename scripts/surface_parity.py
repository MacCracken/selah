"""Surface parity: every in-scope public item of the Rust oracle has a Cyrius counterpart.

Port plan §3 (a per-milestone gate) and M7 (the release gate: no module left pending). Usage, from the repo root:
    python3 -I scripts/surface_parity.py .

For each ported module it lists rust-old/src/<module>.rs's public items before `#[cfg(test)]` and checks that src/
defines the Cyrius name the porting rules give (port plan D9):
- free `pub fn f`                     -> `fn selah_f(`
- `impl Enum { pub fn m }`            -> `fn selah_<snake(Enum)>_m(`          (ImageFormat::extension)
- `impl Struct { pub fn m }`          -> `fn m(` inside `impl Sl<Struct> {`, or a free `fn Sl<Struct>_m(`
- `impl Struct { pub const X: T }`    -> `var SL_<SNAKE(Struct)>_<X>` (Color::RED -> SL_COLOR_RED)
- `pub const X` at module level       -> `SL_<X>` (a var or const)
- `SelahError::Variant`               -> its `SL_ERR_*` member in enum SlError
Anything else needs an entry in EXCEPTIONS with its reason (usually an ADR 0002 row). Every file under rust-old/src
must be in MODULES, PENDING_MODULES or NOT_APPLICABLE_MODULES. Exits 1 on any item missing or any file unaccounted.
"""
import re
import sys
from pathlib import Path

root = Path(sys.argv[1]).resolve()

# The ported modules, with the Rust types whose methods become free functions ("enums") or impl methods
# ("structs").
MODULES = {
    "error": {"enums": set(), "structs": set()},
    "geometry": {"enums": set(), "structs": {"Rect"}},
    "core": {"enums": {"ImageFormat"}, "structs": {"Annotation", "Color", "RedactionTarget"}},
}

# Rust files whose milestone has not come yet (port plan §3): counted, not checked. M7 empties this.
PENDING_MODULES = {
    "ai": "M2",
    "annotate": "M3/M4",
    "lib": "M4 (the facade: annotate_image, redact_image, decode_image_data)",
    "fsutil": "M5",
    "http": "M5",
    "history": "M5",
    "capture": "M5",
    "daimon": "M5",
    "mcp": "M6",
}
NOT_APPLICABLE_MODULES = {}

# Rust item -> why it has no Cyrius name of the rule's shape.
EXCEPTIONS = {
    "Rect::as_hisab": "the layout is SlRect's own public fields (min_x, min_y, max_x, max_y); ADR 0002 #12",
    "SelahError::AnnotationError": "SL_ERR_ANNOTATION",
}

ERROR_NAMES = {
    "CaptureFailed": "SL_ERR_CAPTURE_FAILED",
    "InvalidRegion": "SL_ERR_INVALID_REGION",
    "UnsupportedFormat": "SL_ERR_UNSUPPORTED_FORMAT",
    "Io": "SL_ERR_IO",
    "Api": "SL_ERR_API",
}


def snake(camel):
    return re.sub(r"(?<!^)([A-Z])", r"_\1", camel).lower()


def rust_items(module):
    src = (root / f"rust-old/src/{module}.rs").read_text()
    cut = src.find("#[cfg(test)]")
    src = src if cut < 0 else src[:cut]
    items = []
    impl = None
    depth = 0
    for line in src.splitlines():
        s = line.strip()
        m = re.match(r"impl(?:<[^>]*>)?\s+(?:(\w+(?:::\w+)*)\s+for\s+)?(\w+)(?:<[^>]*>)?\s*\{", s)
        if m and depth == 0:
            impl = (m.group(2), m.group(1))
        fm = re.match(r"pub (?:const )?fn (\w+)", s)
        if fm:
            if impl and depth >= 1:
                items.append((f"{impl[0]}::{fm.group(1)}", impl[0], fm.group(1)))
            elif depth == 0:
                items.append((fm.group(1), None, fm.group(1)))
        cm = re.match(r"pub const ([A-Z][A-Z0-9_]*)\s*:", s)
        if cm:
            if impl and depth >= 1:
                items.append((f"{impl[0]}::{cm.group(1)}", impl[0], f"const {cm.group(1)}"))
            elif depth == 0:
                items.append((f"const {cm.group(1)}", None, f"const {cm.group(1)}"))
        depth += line.count("{") - line.count("}")
        if depth == 0:
            impl = None
    if module == "error":
        body = src[src.index("pub enum SelahError"):]
        body = body[: body.index("\n}")]
        for v in re.findall(r"^\s{4}([A-Z]\w+)", body, re.M):
            items.append((f"SelahError::{v}", "SelahError", v))
    return items


cyr = "\n".join(p.read_text() for p in sorted((root / "src").glob("*.cyr")))


def impl_methods(struct):
    m = re.search(rf"^impl Sl{struct} \{{(.*?)^\}}", cyr, re.S | re.M)
    return set(re.findall(r"^\s+fn (\w+)\(", m.group(1), re.M)) if m else set()


def has_counterpart(label, ty, name, kinds):
    if ty == "SelahError":
        return re.search(rf"\b{ERROR_NAMES.get(name, '?')} =", cyr) is not None
    if name.startswith("const "):
        const = name[6:]
        full = f"SL_{snake(ty).upper()}_{const}" if ty else f"SL_{const}"
        return re.search(rf"^(?:var|const) {full}\b", cyr, re.M) is not None
    if ty in kinds["structs"]:
        if name in impl_methods(ty):
            return True
        return re.search(rf"^fn Sl{ty}_{name}\(", cyr, re.M) is not None
    if ty in kinds["enums"]:
        return re.search(rf"^fn selah_{snake(ty)}_{name}\(", cyr, re.M) is not None
    return re.search(rf"^fn selah_{name}\(", cyr, re.M) is not None


missing = 0
for module, kinds in MODULES.items():
    found = 0
    items = rust_items(module)
    for label, ty, name in items:
        if label in EXCEPTIONS:
            found += 1
            continue
        if has_counterpart(label, ty, name, kinds):
            found += 1
        else:
            missing += 1
            print(f"MISSING  {module}: {label}")
    print(f"{module:<10} {found}/{len(items)} Rust public items have a Cyrius counterpart")
known = set(MODULES) | set(PENDING_MODULES) | set(NOT_APPLICABLE_MODULES)
for path in sorted((root / "rust-old/src").rglob("*.rs")):
    mod = str(path.relative_to(root / "rust-old/src"))[:-3]
    if mod not in known:
        missing += 1
        print(f"UNKNOWN  rust-old/src/{mod}.rs is not ported, pending or marked not applicable")
for mod, when in PENDING_MODULES.items():
    print(f"{mod:<10} {len(rust_items(mod))} Rust public items, pending: {when}")
for mod, why in NOT_APPLICABLE_MODULES.items():
    print(f"{mod:<10} not applicable: {why}")
print(f"surface parity: {missing} missing")
sys.exit(1 if missing else 0)
