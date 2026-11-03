"""Per-test durations from a pertest log (ts.py-stamped `--test-threads=1` output).
python3 -I pertest_table.py <pertest-*.log>...: the delta on a `test X ... ok` line is the time
since the previous line, i.e. that test's own wall time when the harness runs one at a time."""
import re
import sys

LINE = re.compile(r'^\s*([\d.]+)\s+\+\s*([\d.]+)\s+test (\S+) \.\.\. (ok|FAILED|ignored)')
rows = []
for path in sys.argv[1:]:
    start_load = end_load = "?"
    for line in open(path):
        if line.startswith("START"):
            m = re.search(r'load=([\d.]+)', line)
            start_load = m.group(1) if m else "?"
        elif line.startswith("END"):
            m = re.search(r'load=([\d.]+)', line)
            end_load = m.group(1) if m else "?"
        m = LINE.match(line)
        if m:
            own = path.rsplit("/", 1)[-1].removeprefix("pertest-1-").removesuffix(".log")
            if m.group(3).split("::")[0] != own:
                continue
            rows.append((float(m.group(2)), m.group(3), m.group(4), path.rsplit("/", 1)[-1]))
    print(f"{path.rsplit('/', 1)[-1]}: load {start_load} -> {end_load}")
rows.sort(reverse=True)
total = sum(r[0] for r in rows)
print(f"\n{len(rows)} tests serialised, {total:.2f} s in total; the 25 slowest:")
for secs, name, status, src in rows[:25]:
    print(f"  {secs:7.2f} s  {100 * secs / total:5.1f}%  {name} {'' if status == 'ok' else status}")
by_file = {}
for secs, name, status, src in rows:
    by_file.setdefault(name.split("::")[0], [0.0, 0])
    by_file[name.split("::")[0]][0] += secs
    by_file[name.split("::")[0]][1] += 1
print("\nper file (serialised sum, tests):")
for f, (s, n) in sorted(by_file.items(), key=lambda kv: -kv[1][0]):
    print(f"  {f:14} {s:7.2f} s  {100 * s / total:5.1f}%  {n} tests")
