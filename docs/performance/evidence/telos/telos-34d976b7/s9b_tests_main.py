import pathlib
import re

ROOT = pathlib.Path("/home/timo/projects/telos/crates")

for crate in sorted(ROOT.iterdir()):
    tests = crate / "tests"
    files = sorted(p for p in tests.glob("*.rs") if p.name != "main.rs") if tests.is_dir() else []
    if not files:
        continue
    has_common = (tests / "common" / "mod.rs").is_file()
    lines = [
        "//! Every integration test of this crate in one test executable: one",
        "//! compile and one link instead of one per file (`docs/vision.md`,",
        "//! invariant 9). Each file is a module; harness names are",
        "//! `<file>::<test>`.",
        "",
    ]
    if has_common:
        lines.append("mod common;")
    lines += [f"mod {p.stem};" for p in files]
    (tests / "main.rs").write_text("\n".join(lines) + "\n")
    for p in files:
        text = p.read_text()
        new = re.sub(r"^mod common;$", "use crate::common;", text, flags=re.M)
        if new != text:
            p.write_text(new)
    toml = crate / "Cargo.toml"
    text = toml.read_text()
    if "autotests" not in text:
        text = re.sub(r"^(description = .*)$", r"\1\nautotests = false", text, count=1, flags=re.M)
        text = text.rstrip("\n") + '\n\n[[test]]\nname = "main"\npath = "tests/main.rs"\n'
        toml.write_text(text)
    print(crate.name, len(files), "common" if has_common else "")
