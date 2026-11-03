"""Replace the block between two marker lines of a file with the contents of another file.

usage: python3 -I splice.py FILE START_MARKER END_MARKER REPLACEMENT_FILE
The start marker line is replaced too; the end marker line is kept.
"""
import sys

path, start, end, replacement = sys.argv[1:5]
lines = open(path, encoding="utf-8").read().split("\n")
starts = [i for i, line in enumerate(lines) if line.strip() == start.strip()]
ends = [i for i, line in enumerate(lines) if line.strip() == end.strip()]
if len(starts) != 1 or len(ends) != 1 or ends[0] <= starts[0]:
    sys.exit(f"markers: start {starts}, end {ends}")
new = open(replacement, encoding="utf-8").read().rstrip("\n").split("\n")
out = lines[: starts[0]] + new + lines[ends[0] :]
open(path, "w", encoding="utf-8").write("\n".join(out))
print(f"replaced lines {starts[0] + 1}..{ends[0]} with {len(new)} lines")
