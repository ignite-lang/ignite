#!/usr/bin/env python3
"""Call counts per callee (and per caller->callee) from a callgrind output file.

usage: cgcalls.py FILE REPLIES [NAME_SUBSTRING ...]
"""
import re
import sys
from collections import Counter

path, replies = sys.argv[1], int(sys.argv[2])
wanted = sys.argv[3:]
names = {}
callee_calls = Counter()
pair_calls = Counter()
current_fn = None
pending = None


def resolve(spec):
    m = re.match(r"\((\d+)\)(?: (.*))?", spec)
    if not m:
        return spec
    ident, name = m.group(1), m.group(2)
    if name:
        names[ident] = name
    return names.get(ident, ident)


with open(path, errors="replace") as handle:
    for line in handle:
        line = line.rstrip("\n")
        if line.startswith("fn="):
            current_fn = resolve(line[3:])
        elif line.startswith("cfn="):
            pending = resolve(line[4:])
        elif line.startswith("calls=") and pending is not None:
            count = int(line[6:].split()[0])
            callee_calls[pending] += count
            pair_calls[(current_fn, pending)] += count
            pending = None

print(f"top callees by calls per reply ({replies} replies)")
for name, count in callee_calls.most_common(40):
    print(f"{count / replies:10.2f}  {name}")
for w in wanted:
    print(f"\ncallers of *{w}* (calls per reply)")
    rows = [(c, a, b) for (a, b), c in pair_calls.items() if w in b]
    for c, a, b in sorted(rows, reverse=True)[:25]:
        print(f"{c / replies:10.2f}  {a} -> {b}")
