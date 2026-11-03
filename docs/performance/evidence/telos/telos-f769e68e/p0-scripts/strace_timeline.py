"""Timeline of the processes an `ignite build` ran: python3 -I strace_timeline.py <file.strace>

Input: strace --seccomp-bpf -f -ttt -e trace=execve,exit_group -s 4000 -o <file>.
Output: one line per exec'd process: start offset, duration, program and the arguments that
identify the step (clang: -c/-o and the input; cargo/rustc: --crate-name).
"""
import re
import sys

EXEC = re.compile(r'^(\d+)\s+(\d+\.\d+)\s+(?:<\.\.\. execve resumed>|execve\()(.*)')
EXIT = re.compile(r'^(\d+)\s+(\d+\.\d+)\s+\+\+\+ exited with (\d+) \+\+\+')
ARGV = re.compile(r'\[(.*?)\], 0x|\[(.*?)\], \[')

procs = {}
order = []
first = None
for line in open(sys.argv[1]):
    m = EXEC.match(line)
    if m and "ENOENT" not in line:
        pid, ts, rest = m.group(1), float(m.group(2)), m.group(3)
        if first is None:
            first = ts
        if "execve resumed" in line and pid in procs and procs[pid].get("pending"):
            procs[pid]["pending"] = False
            continue
        a = re.search(r'\["(.*?)"\]', rest)
        argv = a.group(1).split('", "') if a else [rest[:80]]
        if pid not in procs:
            order.append(pid)
        procs[pid] = {"start": ts, "argv": argv, "end": None, "pending": "<unfinished" in rest}
        continue
    m = EXIT.match(line)
    if m:
        pid, ts = m.group(1), float(m.group(2))
        if pid in procs:
            procs[pid]["end"] = ts
            procs[pid]["code"] = m.group(3)

if first is None:
    print("no execve in", sys.argv[1])
    sys.exit(1)

def label(argv):
    prog = argv[0].rsplit("/", 1)[-1]
    if prog in ("clang", "clang-22"):
        out = next((argv[i + 1] for i, a in enumerate(argv) if a == "-o" and i + 1 < len(argv)), "?")
        if "-c" in argv:
            src = next((a for a in argv if a.endswith(".ll")), "?")
            opt = next((a for a in argv if a.startswith("-O")), "")
            return f"clang -c {opt} {src.rsplit('/', 1)[-1]}"
        if "-emit-llvm" in argv:
            return "clang target probe (-S -emit-llvm /dev/null)"
        return f"clang link -> {out.rsplit('/', 1)[-1]}"
    if "--crate-name" in argv:
        name = argv[argv.index("--crate-name") + 1]
        return f"{prog} rustc --crate-name {name}"
    if prog in ("cargo", "rustc", "sccache", "ld.lld", "lld", "ld", "cc", "collect2"):
        return f"{prog} {' '.join(argv[1:4])}"
    return f"{prog} {' '.join(argv[1:3])}"[:100]

last_end = max((p["end"] for p in procs.values() if p["end"]), default=first)
print(f"{'start':>7} {'dur':>7}  step")
for pid in order:
    p = procs[pid]
    dur = (p["end"] - p["start"]) if p["end"] else float("nan")
    print(f"{p['start'] - first:7.3f} {dur:7.3f}  {label(p['argv'])}")
print(f"total (first exec to last exit): {last_end - first:.3f} s")
