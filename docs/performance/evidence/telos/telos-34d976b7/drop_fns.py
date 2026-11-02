"""Delete top-level Rust fns (with their attributes and doc comments) from a file.

usage: python3 -I drop_fns.py FILE NAME...
A fn is the lines from `fn NAME(` (column 0) back over preceding `#[...]` and
`///` lines, forward to the first line that is exactly `}`, plus one following
blank line.
"""
import sys

path = sys.argv[1]
names = sys.argv[2:]
lines = open(path, encoding="utf-8").read().split("\n")
removed = []
for name in names:
    starts = [i for i, line in enumerate(lines) if line.startswith(f"fn {name}(")]
    if len(starts) != 1:
        sys.exit(f"{name}: found {len(starts)} definitions")
    start = starts[0]
    while start > 0 and (lines[start - 1].startswith("#[") or lines[start - 1].startswith("///")):
        start -= 1
    end = start
    while lines[end] != "}":
        end += 1
    end += 1
    if end < len(lines) and lines[end] == "":
        end += 1
    removed.append((name, start + 1, end))
    del lines[start:end]
open(path, "w", encoding="utf-8").write("\n".join(lines))
for name, a, b in removed:
    print(f"dropped {name}: lines {a}..{b}")
