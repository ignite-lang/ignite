"""Fill .expected files from the `--- actual` blocks of a fixture test log.

usage: s10p1_expected.py <log> <fixture dir> <name>...
"""
import os
import sys

lines = open(sys.argv[1]).read().split("\n")
fixture_dir = sys.argv[2]
wanted = set(sys.argv[3:])
done = set()
i = 0
while i < len(lines):
    line = lines[i].strip()
    if line.endswith(".telos") and i + 1 < len(lines) and lines[i + 1].startswith("--- expected"):
        name = os.path.basename(line)[: -len(".telos")]
        j = i + 2
        while j < len(lines) and not lines[j].startswith("--- actual"):
            j += 1
        j += 1
        actual = []
        while j < len(lines) and lines[j] != "":
            actual.append(lines[j])
            j += 1
        if name in wanted:
            text = "\n".join(actual) + "\n" if actual else ""
            with open(f"{fixture_dir}/{name}.expected", "w") as f:
                f.write(text)
            done.add(name)
            print(f"{name}: {len(actual)} line(s)")
        i = j
    else:
        i += 1
missing = wanted - done
if missing:
    print("MISSING:", sorted(missing))
    sys.exit(1)
