"""Insert slice-10 placeholder arms right after `match ... {` lines.

usage: s10p1_placeholders.py FILE LINE:KIND [LINE:KIND ...]
KIND: expr, stmt, ty, builtin, decl
"""
import re
import sys

ARMS = {
    "expr": [
        "ExprKind::Quantity { .. } | ExprKind::NamedArg { .. } | ExprKind::Brace { .. } => {",
        '    unimplemented!("slice 10: quantity literals, named arguments and brace literals")',
        "}",
    ],
    "stmt": [
        "StmtKind::ForPattern { .. } => {",
        '    unimplemented!("slice 10: `for` with a tuple pattern")',
        "}",
    ],
    "ty": [
        "Ty::Map(..) | Ty::Quantity(_) | Ty::Ref(_) | Ty::Signal(_) | Ty::SystemRef | Ty::Protocol => {",
        '    unimplemented!("slice 10: Map, quantity, reference, signal, system and protocol types")',
        "}",
    ],
    "builtin": [
        "Builtin::MapEmpty",
        "| Builtin::MapGet",
        "| Builtin::MapPut",
        "| Builtin::MapRemove",
        "| Builtin::MapKeys",
        "| Builtin::MapLen",
        "| Builtin::MapContains",
        "| Builtin::DurationToMs",
        "| Builtin::DurationToNs",
        "| Builtin::ByteSizeToInt",
        "| Builtin::RatioPpm",
        "| Builtin::IoErrorWithExitCode",
        "| Builtin::ModuleRecord => {",
        '    unimplemented!("slice 10: Map, quantity, IoError.with_exit_code and module(..) builtins")',
        "}",
    ],
    "decl": [
        "Decl::Assembly(_) => {}",
    ],
}

path = sys.argv[1]
lines = open(path).read().split("\n")
specs = []
for spec in sys.argv[2:]:
    n, kind = spec.split(":")
    specs.append((int(n), kind))
for n, kind in sorted(specs, reverse=True):
    line = lines[n - 1]
    if not re.search(r"match .*\{\s*$", line):
        sys.exit(f"{path}:{n} is not a match line: {line!r}")
    indent = re.match(r"\s*", line).group(0) + "    "
    arm = [indent + a for a in ARMS[kind]]
    lines[n:n] = arm
open(path, "w").write("\n".join(lines))
