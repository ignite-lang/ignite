"""Interleaved pairs over three trees: python3 -I pairs3.py <summary>...

For every label, the trees before (60aef38), s11 (slice-11 close) and after (slice 11b) are
paired by pair id. Prints per label and tree the median and range of the times and the loads,
and the median ratio after/before and after/s11 over the pairs where both runs exited 0 and
their loads differ by at most 5.
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
            f"load {min(loads):.1f}-{max(loads):.1f}, n={len(secs)}, exits {sorted(set(v[3] for v in values))}")


def ratio(pairs, base, other):
    kept, dropped = [], []
    for pair, trees in sorted(pairs.items()):
        b, a = trees.get(base), trees.get(other)
        if not (b and a):
            continue
        if b[3] != "0" or a[3] != "0":
            dropped.append((pair, "exit"))
        elif abs(b[1] - a[1]) > 5:
            dropped.append((pair, "load"))
        else:
            kept.append(a[0] / b[0])
    if not kept:
        return f"{other}/{base}: no pairs kept, discarded {dropped}"
    return (f"{other}/{base}: median {statistics.median(kept):.3f} (range {min(kept):.3f}-{max(kept):.3f}), "
            f"pairs kept {len(kept)}, discarded {len(dropped)} {dropped if dropped else ''}")


for label in order:
    pairs = runs[label]
    print(f"## {label}")
    for tree in ("before", "s11", "after"):
        values = [trees[tree] for trees in pairs.values() if tree in trees]
        if values:
            print(f"  {tree:7}: {fmt_abs(values)}")
    print(f"  {ratio(pairs, 'before', 'after')}")
    if any('s11' in trees for trees in pairs.values()):
        print(f"  {ratio(pairs, 's11', 'after')}")
