"""Interleaved before/after pairs: python3 -I pairs.py <summary>...

For every label, a pair is the before and after STEP lines with one pair id. A pair whose two
load_before values differ by more than 5 is discarded. Prints per label the median of the
kept pairs' after/before ratios, the kept and discarded counts, and the absolute times
(median and range) and loads per tree. Labels with one tree only print their absolute times.
"""
import re
import statistics
import sys
from collections import defaultdict

STEP = re.compile(
    r'^STEP tree=(\S+) label=(\S+) pair=(\S+) secs=([\d.]+) load_before=([\d.]+) load_after=([\d.]+) exit=(\d+)'
)
runs = defaultdict(lambda: defaultdict(dict))
order = []
for path in sys.argv[1:]:
    for line in open(path):
        m = STEP.match(line)
        if not m:
            continue
        tree, label, pair, secs, lb, la, code = m.groups()
        if label not in order:
            order.append(label)
        runs[label][pair][tree] = (float(secs), float(lb), float(la), code)


def fmt_abs(values):
    secs = [v[0] for v in values]
    loads = [v[1] for v in values]
    return (f"median {statistics.median(secs):.2f} s, range {min(secs):.2f}-{max(secs):.2f} s, "
            f"load {min(loads):.1f}-{max(loads):.1f}, n={len(secs)}")


for label in order:
    pairs = runs[label]
    kept, dropped, before_only, after_only = [], [], [], []
    for pair, trees in sorted(pairs.items()):
        b, a = trees.get("before"), trees.get("after")
        if b and a:
            if b[3] != "0" or a[3] != "0":
                dropped.append((pair, b, a, "exit"))
            elif abs(b[1] - a[1]) > 5:
                dropped.append((pair, b, a, "load"))
            else:
                kept.append((pair, b, a))
        elif b:
            before_only.append(b)
        elif a:
            after_only.append(a)
    print(f"## {label}")
    if kept:
        ratios = [a[0] / b[0] if b[0] > 0 else float("nan") for _, b, a in kept]
        print(f"  ratio after/before: median {statistics.median(ratios):.3f} "
              f"(range {min(ratios):.3f}-{max(ratios):.3f}), pairs kept {len(kept)}, discarded {len(dropped)}")
        print(f"  before: {fmt_abs([b for _, b, _ in kept])}")
        print(f"  after:  {fmt_abs([a for _, _, a in kept])}")
    for pair, b, a, why in dropped:
        print(f"  discarded pair {pair} ({why}): before {b[0]:.2f}s@{b[1]} exit={b[3]}, after {a[0]:.2f}s@{a[1]} exit={a[3]}")
    if before_only:
        print(f"  before only: {fmt_abs(before_only)}")
    if after_only:
        print(f"  after only:  {fmt_abs(after_only)}")
