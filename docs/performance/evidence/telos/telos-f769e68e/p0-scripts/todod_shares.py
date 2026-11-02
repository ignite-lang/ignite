"""Per-step shares of the todod builds from the strace timelines of several runs.
python3 -I todod_shares.py <logs-dir> <run-id>...
Reads todod-<run>-{llvm,rust}-{dev,release}.strace and todod-<run>.log.summary."""
import re
import statistics
import sys

EXEC = re.compile(r'^(\d+)\s+(\d+\.\d+)\s+(?:<\.\.\. execve resumed>|execve\()(.*)')
EXIT = re.compile(r'^(\d+)\s+(\d+\.\d+)\s+\+\+\+ exited with (\d+) \+\+\+')
STEP = re.compile(r'^STEP (\S+) secs=([\d.]+) load_before=([\d.]+) load_after=([\d.]+) exit=(\d+)')
logs = sys.argv[1]
runs = sys.argv[2:]


def procs_of(path):
    procs, order, first = {}, [], None
    for line in open(path):
        m = EXEC.match(line)
        if m and "ENOENT" not in line:
            pid, ts, rest = m.group(1), float(m.group(2)), m.group(3)
            first = ts if first is None else first
            if "execve resumed" in line and pid in procs and procs[pid].get("pending"):
                procs[pid]["pending"] = False
                continue
            a = re.search(r'\["(.*?)"\]', rest)
            argv = a.group(1).split('", "') if a else []
            if pid not in procs:
                order.append(pid)
            procs[pid] = {"start": ts, "argv": argv, "end": None, "pending": "<unfinished" in rest}
            continue
        m = EXIT.match(line)
        if m and m.group(1) in procs:
            procs[m.group(1)]["end"] = float(m.group(2))
    last = max(p["end"] for p in procs.values() if p["end"])
    return [procs[pid] for pid in order], first, last


def llvm_steps(path):
    procs, first, last = procs_of(path)
    steps = {}
    clangs = [p for p in procs if p["argv"] and p["argv"][0].rsplit("/", 1)[-1].startswith("clang")]
    first_cc = None
    for p in clangs:
        argv = p["argv"]
        dur = p["end"] - p["start"]
        if "-emit-llvm" in argv:
            steps["target probe"] = dur
        elif "-c" in argv:
            src = next(a for a in argv if a.endswith(".ll")).rsplit("/", 1)[-1]
            steps[f"clang -c {src}"] = dur
            first_cc = p["start"] if first_cc is None else min(first_cc, p["start"])
        else:
            steps["link (clang+ld)"] = dur
    steps["emit (ignite front end to first clang -c)"] = first_cc - first - steps.get("target probe", 0)
    total = last - first
    steps["other (process start/exit, stamps)"] = total - sum(steps.values())
    return steps, total


def rust_steps(path):
    procs, first, last = procs_of(path)
    steps = {}
    first_rustc = None
    for p in procs:
        argv = p["argv"]
        if not argv:
            continue
        prog = argv[0].rsplit("/", 1)[-1]
        if prog == "sccache" and "--crate-name" in argv:
            name = argv[argv.index("--crate-name") + 1]
            if name == "___":
                continue
            dur = p["end"] - p["start"]
            first_rustc = p["start"] if first_rustc is None else min(first_rustc, p["start"])
            if name in ("ignite_io", "ignite_std", "todod", "todo"):
                label = {"todo": "program crate: rustc todo (lib)", "todod": "program crate: rustc todod (bin)",
                         "ignite_std": "std crate: rustc ignite_std", "ignite_io": "substrate: rustc ignite_io"}[name]
                steps[label] = steps.get(label, 0) + dur
            elif name == "build_script_build":
                steps["deps: build scripts"] = steps.get("deps: build scripts", 0) + dur
            else:
                steps[f"deps: rustc {name}"] = steps.get(f"deps: rustc {name}", 0) + dur
        elif prog in ("cc", "build-script-build"):
            dur = p["end"] - p["start"]
            key = "link todod (cc)" if prog == "cc" and any("deps/" in a for a in argv) else "deps: build scripts"
            steps[key] = steps.get(key, 0) + dur
    total = last - first
    steps["cargo start (to first rustc, incl. ignite front end)"] = first_rustc - first
    steps["overlap (deps compiled in parallel; negative)"] = total - sum(steps.values())
    critical = ("cargo start (to first rustc, incl. ignite front end)", "deps: rustc libc", "deps: rustc nix",
                "substrate: rustc ignite_io", "std crate: rustc ignite_std", "program crate: rustc todo (lib)",
                "program crate: rustc todod (bin)", "link todod (cc)")
    steps["[critical path: cargo start + libc + nix + ignite_io + ignite_std + todo + todod + link]"] = sum(steps.get(k, 0) for k in critical)
    return steps, total


plain = {}
for run in runs:
    for line in open(f"{logs}/todod-{run}.log.summary"):
        m = STEP.match(line)
        if m:
            plain.setdefault(m.group(1), []).append((float(m.group(2)), float(m.group(3)), float(m.group(4))))

for target in ("llvm", "rust"):
    for profile in ("dev", "release"):
        key = f"{target}-{profile}"
        acc = {}
        totals = []
        for run in runs:
            steps, total = (llvm_steps if target == "llvm" else rust_steps)(f"{logs}/todod-{run}-{key}.strace")
            totals.append(total)
            for k, v in steps.items():
                acc.setdefault(k, []).append(v)
        p = plain[key]
        secs = [x[0] for x in p]
        loads = [x[1] for x in p] + [x[2] for x in p]
        ps = plain[key + "-strace"]
        print(f"\n== todod {target} {profile}: plain build median {statistics.median(secs):.2f} s "
              f"(range {min(secs):.2f}-{max(secs):.2f}, n={len(secs)}, load {min(loads):.0f}-{max(loads):.0f}); "
              f"strace'd build median {statistics.median([x[0] for x in ps]):.2f} s, timeline total median {statistics.median(totals):.2f} s")
        med_total = statistics.median(totals)
        rows = sorted(acc.items(), key=lambda kv: -statistics.median(kv[1]))
        for k, v in rows:
            m = statistics.median(v)
            print(f"  {k:48} {m:6.3f} s  {100 * m / med_total:5.1f}%   (range {min(v):.3f}-{max(v):.3f})")
