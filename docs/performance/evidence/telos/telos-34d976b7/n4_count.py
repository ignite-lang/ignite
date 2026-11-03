#!/usr/bin/env python3
"""Line counts of Telos files: total, test lines (test items and the comment
lines directly above them), the rest. Blank lines count where they fall
(blank lines between items count as non-test)."""
import sys


def count(path):
    lines = open(path, encoding="utf-8").read().split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    test = [False] * len(lines)
    i = 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("test "):
            start = i
            j = i - 1
            while j >= 0 and lines[j].startswith("//"):
                j -= 1
            for k in range(j + 1, start):
                test[k] = True
            if line.rstrip().endswith("{"):
                k = i
                while k < len(lines) and lines[k] != "}":
                    k += 1
                end = k
            else:
                k = i
                while k + 1 < len(lines) and lines[k + 1].startswith("  "):
                    k += 1
                end = k
            for k in range(start, end + 1):
                test[k] = True
            i = end + 1
        else:
            i += 1
    t = sum(test)
    return len(lines), t, len(lines) - t


total = [0, 0, 0]
print(f"{'file':45} {'total':>6} {'tests':>6} {'code':>6}")
for p in sys.argv[1:]:
    a, t, c = count(p)
    total = [total[0] + a, total[1] + t, total[2] + c]
    print(f"{p:45} {a:6} {t:6} {c:6}")
print(f"{'total':45} {total[0]:6} {total[1]:6} {total[2]:6}")
