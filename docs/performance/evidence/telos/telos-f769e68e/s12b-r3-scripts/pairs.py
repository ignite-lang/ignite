"""Interleaved pairs of two named trees: python3 -I pairs.py <before-tree> <after-tree> <summary>...

As s12-p4-scripts/pairs.py, with the two tree names given: for every label, a pair is the two
trees' STEP lines with one pair id; a pair whose load_before values differ by more than 5, or
with a non-zero exit, is discarded; per label the median of the kept after/before ratios, the
counts, the absolute times (median, range) and loads per tree. Other trees' lines are listed as
absolute times only.
"""
import re
import statistics
import sys
from collections import defaultdict

STEP = re.compile(
    r'^STEP tree=(\S+) label=(\S+) pair=(\S+) secs=([\d.]+) load_before=([\d.]+) load_after=([\d.]+) exit=(\d+)'
)
before_name, after_name = sys.argv[1], sys.argv[2]
runs = defaultdict(lambda: defaultdict(dict))
order = []
for path in sys.argv[3:]:
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


print(f"# pairs {before_name} -> {after_name}")
for label in order:
    pairs = runs[label]
    kept, dropped = [], []
    others = defaultdict(list)
    for pair, trees in sorted(pairs.items()):
        b, a = trees.get(before_name), trees.get(after_name)
        if b and a:
            if b[3] != "0" or a[3] != "0":
                dropped.append((pair, b, a, "exit"))
            elif abs(b[1] - a[1]) > 5:
                dropped.append((pair, b, a, "load"))
            else:
                kept.append((pair, b, a))
        for tree, v in trees.items():
            if tree not in (before_name, after_name) or not (b and a):
                others[tree].append(v)
    print(f"## {label}")
    if kept:
        ratios = [a[0] / b[0] if b[0] > 0 else float("nan") for _, b, a in kept]
        print(f"  ratio {after_name}/{before_name}: median {statistics.median(ratios):.3f} "
              f"(range {min(ratios):.3f}-{max(ratios):.3f}), pairs kept {len(kept)}, discarded {len(dropped)}")
        print(f"  {before_name}: {fmt_abs([b for _, b, _ in kept])}")
        print(f"  {after_name}: {fmt_abs([a for _, _, a in kept])}")
    for pair, b, a, why in dropped:
        print(f"  discarded pair {pair} ({why}): {before_name} {b[0]:.2f}s@{b[1]} exit={b[3]}, "
              f"{after_name} {a[0]:.2f}s@{a[1]} exit={a[3]}")
    for tree, values in sorted(others.items()):
        print(f"  {tree} only: {fmt_abs(values)}")
