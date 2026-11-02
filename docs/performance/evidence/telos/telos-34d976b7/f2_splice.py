import sys

path, start, end, new = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
lines = open(path).read().split("\n")
replacement = open(new).read().rstrip("\n").split("\n")
lines[start - 1:end] = replacement
open(path, "w").write("\n".join(lines))
