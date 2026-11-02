"""Apply exact, asserted replacements to a file: n1_edit.py FILE EDITS.py

EDITS.py defines EDITS = [(old, new), ...]; each old must occur exactly once
(or (old, new, count) to require count occurrences)."""
import sys

path, edits_path = sys.argv[1], sys.argv[2]
scope = {}
exec(open(edits_path).read(), scope)
text = open(path).read()
for edit in scope["EDITS"]:
    old, new = edit[0], edit[1]
    count = edit[2] if len(edit) > 2 else 1
    found = text.count(old)
    if found != count:
        sys.exit(f"expected {count} occurrence(s), found {found}:\n{old[:300]}")
    text = text.replace(old, new)
open(path, "w").write(text)
print("ok", len(scope["EDITS"]))
