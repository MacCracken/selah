"""The README's example is the one tests/selah.tcyr compiles and runs.

Usage, from the repo root:
    python3 -I scripts/check_readme_example.py .

The code between `# README example begins` and `# README example ends` in tests/selah.tcyr must appear in README.md,
in the ```cyrius block that follows the line `<!-- example: tests/selah.tcyr -->`. Exit 1 when they differ.
"""
import re, sys
from pathlib import Path

root = Path(sys.argv[1]).resolve()
test = (root / "tests/selah.tcyr").read_text()
m = re.search(r"^# README example begins\n(.*?)^# README example ends\n", test, re.S | re.M)
if not m:
    print("README example: tests/selah.tcyr has no example markers")
    sys.exit(1)
want = m.group(1).rstrip("\n")
readme = (root / "README.md").read_text()
r = re.search(r"^<!-- example: tests/selah\.tcyr -->\n+```cyrius\n(.*?)^```", readme, re.S | re.M)
if not r:
    print("README example: README.md has no `<!-- example: tests/selah.tcyr -->` block")
    sys.exit(1)
have = r.group(1).rstrip("\n")
if have != want:
    print("README example: README.md differs from tests/selah.tcyr; copy the code between the markers")
    sys.exit(1)
print("README example: the same as tests/selah.tcyr")
