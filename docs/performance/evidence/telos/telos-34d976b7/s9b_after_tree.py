import filecmp
import pathlib
import shutil

S = pathlib.Path("/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a")
B = S / "s9b-before-tree"
A = S / "s9b-after-tree"
L = pathlib.Path("/home/timo/projects/telos")
SKIP = {
    "crates/telos/src/main.rs",
    "crates/telos/src/lib.rs",
    "crates/telos/src/llvm_build.rs",
    "crates/telos/src/test_build.rs",
}

if A.exists():
    shutil.rmtree(A)
shutil.copytree(B, A)

for path in L.rglob("*"):
    rel = path.relative_to(L).as_posix()
    if rel.startswith(("target/", ".git/")) or rel in ("target", ".git") or not path.is_file():
        continue
    if rel in SKIP or rel.startswith("crates/telos-llvm/src/"):
        continue
    dest = A / rel
    if dest.exists() and filecmp.cmp(path, dest, shallow=False):
        continue
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(path, dest)
    print("copied", rel)

text = (L / "crates/telos/src/test_build.rs").read_text()
old_start = "            let ir_dir = out.join(\"llvm\");\n"
start = text.index(old_start)
end = text.index("            };\n", start)
block = text[start:end]
new_block = (
    "            let options = telos_llvm::Options {\n"
    "                test_modules,\n"
    "                sources: sources.iter().map(|(p, s)| (p.clone(), s)).collect(),\n"
)
text = text[:start] + new_block + text[end:]
(A / "crates/telos/src/test_build.rs").write_text(text)
print("adapted crates/telos/src/test_build.rs")
