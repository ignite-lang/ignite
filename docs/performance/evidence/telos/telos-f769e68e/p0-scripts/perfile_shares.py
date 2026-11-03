"""Per-file share of the warm `cargo test` step.
python3 -I perfile_shares.py <gate-warm-*.log.summary ...> -- <perfile-*.log.summary ...>
Each STEP line carries its own loads; nothing is discarded, loads are printed."""
import re
import statistics
import sys

STEP = re.compile(r'^STEP (\S+) secs=([\d.]+) load_before=([\d.]+) load_after=([\d.]+) exit=(\d+)')
args = sys.argv[1:]
split = args.index("--")
gate_files, perfile_files = args[:split], args[split + 1:]


def read(paths):
    steps = {}
    for p in paths:
        for line in open(p):
            m = STEP.match(line)
            if m:
                steps.setdefault(m.group(1), []).append((float(m.group(2)), float(m.group(3)), float(m.group(4))))
    return steps


def fmt(samples):
    secs = [s[0] for s in samples]
    loads = [s[1] for s in samples] + [s[2] for s in samples]
    return statistics.median(secs), f"{statistics.median(secs):6.2f} s (range {min(secs):.2f}-{max(secs):.2f}, n={len(secs)}, load {min(loads):.0f}-{max(loads):.0f})"


gate = read(gate_files)
per = read(perfile_files)
test_med, test_txt = fmt(gate["test"])
print("warm gate steps:")
for label in ("fmt", "clippy", "test", "test-ignite"):
    if label in gate:
        print(f"  {label:12} {fmt(gate[label])[1]}")
print(f"\nper-file runs of crates/ignite/tests (cargo test -p ignite --test main <file>::), after the touch:")
if "no-run" in per:
    print(f"  {'compile (--no-run)':28} {fmt(per['no-run'])[1]}")
rows = []
for label, samples in per.items():
    if label.startswith("file:"):
        med, txt = fmt(samples)
        rows.append((med, label[5:], txt))
rows.sort(reverse=True)
total_files = sum(r[0] for r in rows)
for med, name, txt in rows:
    print(f"  {name:28} {txt}   {100 * med / test_med:5.1f}% of test step, {100 * med / total_files:5.1f}% of file sum")
print(f"  {'sum of files':28} {total_files:6.2f} s   {100 * total_files / test_med:5.1f}% of test step")
if "whole-ignite-main" in per:
    med, txt = fmt(per["whole-ignite-main"])
    print(f"  {'whole ignite main binary':28} {txt}   {100 * med / test_med:5.1f}% of test step")
print(f"  {'cargo test step (warm gate)':28} {test_txt}")
