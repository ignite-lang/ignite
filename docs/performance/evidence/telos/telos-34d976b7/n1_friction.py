import re
import sys

path = sys.argv[1]
lines = open(path).read().split("\n")
where = {
    "3": "section 13.1 row 3 (local unification)",
    "7": "section 13.5 (`==`, `!=` on every comparable type); `unwrap_or` stays open",
    "9": "section 13.3 (closures) and 13.1 row 9 (`R { f = e, ..base }`)",
    "13": "section 13.3",
    "14": "section 13.1 row 14 (frontend; backends iterative too)",
    "15": "section 13.4 (boxing)",
    "16": "section 13.1 row 16 (LLVM target)",
    "17": "section 13.8 (associated fns)",
}
out = []
for line in lines:
    m = re.match(r"^\| (\d+) \|", line)
    if m:
        n = m.group(1)
        cells = line.split(" | ")
        target = where.get(n, f"section 13.1 row {n}")
        cells[-1] = f"slice 9: `docs/slice.md` {target} |"
        line = " | ".join(cells)
    out.append(line)
open(path, "w").write("\n".join(out))
print("ok")
