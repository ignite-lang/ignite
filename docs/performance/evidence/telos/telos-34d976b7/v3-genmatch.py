import sys

n = int(sys.argv[1])
out = sys.argv[2]
lines = ["package demo", ""]
ty = ", ".join(["Bool"] * n)
lines.append("fn g(t: (" + ty + ")) -> Int {")
lines.append("  return match t {")
for i in range(n):
    for lit in ("true", "false"):
        cols = ["_"] * n
        cols[i] = lit
        lines.append("    (" + ", ".join(cols) + ") => " + str(i))
lines.append("  }")
lines.append("}")
with open(out, "w") as f:
    f.write("\n".join(lines) + "\n")
