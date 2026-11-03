import re
import subprocess
import sys
import os

root = sys.argv[1]
env = dict(os.environ)
env.update({
    "CARGO_TARGET_DIR": "/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-main",
    "CARGO_INCREMENTAL": "0",
    "CARGO_PROFILE_DEV_DEBUG": "line-tables-only",
})

ARMS = {
    "Decl": "{p}Decl::Alias(_) => unimplemented!(\"slice 6: type alias\"),",
    "StmtKind": "{p}StmtKind::LetPattern {{ .. }} => unimplemented!(\"slice 6: let pattern\"),",
    "ExprKind": "{p}ExprKind::Tuple(_) | {p}ExprKind::Return(_) => unimplemented!(\"slice 6: tuple, return expression\"),",
    "PatternKind": "{p}PatternKind::Bool(_) | {p}PatternKind::Or(_) | {p}PatternKind::Tuple(_) | {p}PatternKind::List(_) => unimplemented!(\"slice 6: patterns\"),",
}

for round in range(12):
    out = subprocess.run(
        ["cargo", "build", "--workspace", "--all-targets", "--message-format=short"],
        cwd=root, env=env, capture_output=True, text=True,
    ).stderr
    sites = {}
    for line in out.splitlines():
        m = re.match(r"^([^:]+\.rs):(\d+):(\d+): error\[E0004\]: non-exhaustive patterns: (.*)$", line)
        if not m:
            continue
        msg = m.group(4)
        for enum in ARMS:
            if re.search(r"\b" + enum + r"::", msg) or (enum == "StmtKind" and "LetPattern" in msg) or (enum == "Decl" and "Decl::Alias" in msg):
                if enum == "Decl" and "Decl::Alias" not in msg:
                    continue
                sites[(m.group(1), int(m.group(2)))] = enum
                break
    others = [l for l in out.splitlines() if ": error" in l and "E0004" not in l and "could not compile" not in l]
    if not sites:
        print("\n".join(others) if others else "clean")
        break
    for (file, lineno), enum in sorted(sites.items(), key=lambda s: (s[0][0], -s[0][1])):
        path = os.path.join(root, file)
        lines = open(path, encoding="utf-8").read().split("\n")
        i = lineno - 1
        while not lines[i].rstrip().endswith("{"):
            i += 1
        indent = re.match(r"\s*", lines[i]).group(0) + "    "
        prefix = "crate::ast::" if file.startswith("crates/telos-syntax/src") else "telos_syntax::ast::"
        lines.insert(i + 1, indent + ARMS[enum].format(p=prefix))
        open(path, "w", encoding="utf-8").write("\n".join(lines))
        print(f"round {round}: {file}:{lineno} {enum}")
