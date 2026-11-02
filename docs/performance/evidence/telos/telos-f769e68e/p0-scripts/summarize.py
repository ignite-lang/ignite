"""Median and range per STEP label over several .summary files; a step with load >= 5 at
either end is listed but not counted. python3 -I summarize.py <file.summary>..."""
import re
import statistics
import sys

STEP = re.compile(r'^STEP (\S+) secs=([\d.]+) load_before=([\d.]+) load_after=([\d.]+) exit=(\d+)')
TOTAL = re.compile(r'^TOTAL secs=([\d.]+) load_end=([\d.]+)')
accepted = {}
rejected = {}
order = []
for path in sys.argv[1:]:
    run = path.rsplit("/", 1)[-1]
    start_load = None
    for line in open(path):
        m = STEP.match(line)
        if m:
            label, secs, lb, la, code = m.group(1), float(m.group(2)), float(m.group(3)), float(m.group(4)), m.group(5)
            if label not in order:
                order.append(label)
            ok = lb < 5 and la < 5 and code == "0"
            (accepted if ok else rejected).setdefault(label, []).append((secs, lb, la, run, code))
            continue
        m = TOTAL.match(line)
        if m:
            secs, la = float(m.group(1)), float(m.group(2))
            if "TOTAL" not in order:
                order.append("TOTAL")
            ok = la < 5 and start_load is not None and start_load < 5
            (accepted if ok else rejected).setdefault("TOTAL", []).append((secs, start_load, la, run, "0"))
        if line.startswith("START"):
            m2 = re.search(r'load=([\d.]+)', line)
            start_load = float(m2.group(1)) if m2 else None

for label in order:
    acc = accepted.get(label, [])
    rej = rejected.get(label, [])
    if acc:
        secs = [a[0] for a in acc]
        loads = [a[1] for a in acc] + [a[2] for a in acc]
        print(f"{label}: median {statistics.median(secs):.2f} s, range {min(secs):.2f}-{max(secs):.2f} s, "
              f"n={len(secs)} accepted, load {min(loads):.1f}-{max(loads):.1f}"
              + (f", discarded {len(rej)}: " + ", ".join(f"{r[0]:.2f}s@{r[1]}/{r[2]}{'' if r[4] == '0' else ' exit=' + r[4]}" for r in rej) if rej else ""))
    else:
        print(f"{label}: no accepted run; discarded {len(rej)}: "
              + ", ".join(f"{r[0]:.2f}s@{r[1]}/{r[2]}{'' if r[4] == '0' else ' exit=' + r[4]}" for r in rej))
