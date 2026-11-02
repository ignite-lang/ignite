"""Write the `--- actual` blocks of a fixture test output to the .expected
files of the listed fixture stems: n1_bless.py OUTPUT STEM [STEM ...]."""
import re
import sys

out = open(sys.argv[1]).read()
stems = set(sys.argv[2:])
pattern = re.compile(r"^(/\S+\.telos)\n--- expected\n.*?--- actual\n(.*?)\n\n", re.S | re.M)
done = set()
for m in pattern.finditer(out):
    path = m.group(1)
    stem = path.rsplit("/", 1)[1][: -len(".telos")]
    if stem not in stems:
        continue
    body = m.group(2)
    lines = [l for l in body.split("\n") if l.startswith(stem + ".telos:")]
    open(path[: -len(".telos")] + ".expected", "w").write("".join(l + "\n" for l in lines))
    done.add(stem)
    print("blessed", stem, len(lines))
missing = stems - done
if missing:
    print("not in output:", sorted(missing))
