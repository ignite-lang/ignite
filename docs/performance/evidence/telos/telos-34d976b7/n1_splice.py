import sys

path, start, end, repl = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
lines = open(path).read().split("\n")
new = open(repl).read().rstrip("\n").split("\n")
lines[start - 1:end] = new
open(path, "w").write("\n".join(lines))
