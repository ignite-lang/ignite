"""Reachable std fns of an LLVM-target build: python3 -I ir_reach.py <out>/llvm

Counts `define` lines per .ll, the symbols each fn body references, and walks the
call graph from program.ll and main_*.ll (bodies and global initializers) into std.ll.
"""
import re
import sys
from pathlib import Path

ir_dir = Path(sys.argv[1])
SYM = re.compile(r'@("[^"]+"|[-A-Za-z$._][-A-Za-z$._0-9]*)')
DEFINE = re.compile(r'^define\b.*?@("[^"]+"|[-A-Za-z$._][-A-Za-z$._0-9]*)\(')
DECLARE = re.compile(r'^declare\b.*?@("[^"]+"|[-A-Za-z$._][-A-Za-z$._0-9]*)\(')


def parse(path):
    lines = path.read_text().splitlines()
    defines = {}
    declares = set()
    toplevel_refs = set()
    current = None
    for i, line in enumerate(lines):
        m = DEFINE.match(line)
        if m:
            current = m.group(1)
            defines[current] = {"start": i, "end": None, "refs": set()}
            continue
        d = DECLARE.match(line)
        if d:
            declares.add(d.group(1))
            continue
        if current is not None:
            if line.startswith("}"):
                defines[current]["end"] = i
                current = None
            else:
                for s in SYM.findall(line):
                    defines[current]["refs"].add(s)
        else:
            for s in SYM.findall(line):
                toplevel_refs.add(s)
    return lines, defines, declares, toplevel_refs


units = {}
for ll in sorted(ir_dir.glob("*.ll")):
    units[ll.stem] = parse(ll)
    lines, defines, declares, top = units[ll.stem]
    body_lines = sum(d["end"] - d["start"] + 1 for d in defines.values() if d["end"] is not None)
    print(f"{ll.name}: lines={len(lines)} define={len(defines)} declare={len(declares)} fn_body_lines={body_lines}")

std_lines, std_defs, std_decls, std_top = units["std"]
all_defs = {}
for name, (lines, defines, declares, top) in units.items():
    for fn, info in defines.items():
        all_defs[fn] = (name, info)

roots = set()
for name, (lines, defines, declares, top) in units.items():
    if name == "std":
        continue
    roots |= top
    for fn, info in defines.items():
        roots.add(fn)
        roots |= info["refs"]

reach = set()
stack = [r for r in roots if r in all_defs]
while stack:
    fn = stack.pop()
    if fn in reach:
        continue
    reach.add(fn)
    unit, info = all_defs[fn]
    for s in info["refs"]:
        if s in all_defs and s not in reach:
            stack.append(s)

std_reach = [fn for fn in reach if all_defs[fn][0] == "std"]
std_reach_lines = sum(std_defs[fn]["end"] - std_defs[fn]["start"] + 1 for fn in std_reach)
std_body_lines = sum(d["end"] - d["start"] + 1 for d in std_defs.values())
print(f"std.ll: defined fns={len(std_defs)} reachable from program+mains={len(std_reach)} "
      f"({100 * len(std_reach) / max(1, len(std_defs)):.0f}%)")
print(f"std.ll: fn body lines={std_body_lines} reachable body lines={std_reach_lines} "
      f"({100 * std_reach_lines / max(1, std_body_lines):.0f}%) of {len(std_lines)} total lines")

GLUE = ("copy.", "closure.", "fnval.", "spawn.", "eq.")
by_module = {}
for fn, info in std_defs.items():
    name = fn.strip('"')
    glue = next((g for g in GLUE if name.startswith(g)), None)
    if glue:
        name = name[len(glue):]
    parts = name.split("<")[0].split(".")
    if parts[0] == "std":
        key = ".".join(parts[:3]) if len(parts) > 2 and parts[1] == "protocol" else ".".join(parts[:2])
    else:
        key = "(glue) " + parts[0]
    if glue:
        key = key + " (glue)"
    size = info["end"] - info["start"] + 1
    by_module.setdefault(key, [0, 0, 0, 0])
    by_module[key][0] += 1
    by_module[key][2] += size
    if fn in reach:
        by_module[key][1] += 1
        by_module[key][3] += size
print("std module: defined fns / reachable fns, body lines / reachable body lines")
for key, (n, r, s, rs) in sorted(by_module.items(), key=lambda kv: -kv[1][2]):
    print(f"  {key}: {n}/{r}, {s}/{rs}")
unreached = sorted(fn for fn in std_defs if fn not in reach)
print(f"unreached std fns ({len(unreached)}):")
for fn in unreached:
    print(f"  {fn}")
