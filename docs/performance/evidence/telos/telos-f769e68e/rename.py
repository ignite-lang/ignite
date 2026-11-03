import os
import re
import subprocess
import sys

ROOT = "/home/timo/projects/telos"
PATTERN = re.compile(rb"[Tt][Ee][Ll][Oo][Ss]")
NEW = b"ignite"


def mapped(match: bytes) -> bytes:
    out = bytearray()
    for i, c in enumerate(NEW):
        source = match[min(i, len(match) - 1)]
        is_upper = 65 <= source <= 90
        out.append(c - 32 if is_upper else c)
    return bytes(out)


def main() -> None:
    os.chdir(ROOT)
    listing = subprocess.run(
        ["git", "ls-files", "-co", "--exclude-standard", "-z"],
        check=True,
        capture_output=True,
    ).stdout
    files = [p.decode() for p in listing.split(b"\0") if p]
    files = [p for p in files if os.path.isfile(p) and not os.path.islink(p)]
    changed = 0
    binary = []
    for path in files:
        with open(path, "rb") as f:
            data = f.read()
        if b"\0" in data:
            if PATTERN.search(data):
                binary.append(path)
            continue
        new = PATTERN.sub(lambda m: mapped(m.group(0)), data)
        if new != data:
            with open(path, "wb") as f:
                f.write(new)
            changed += 1
    print(f"CONTENT_CHANGED={changed}")
    for path in binary:
        print(f"BINARY_WITH_NAME={path}")

    paths = set()
    for path in files:
        parts = path.split("/")
        for depth in range(1, len(parts) + 1):
            paths.add("/".join(parts[:depth]))
    to_rename = [p for p in paths if PATTERN.search(os.path.basename(p).encode())]
    to_rename.sort(key=lambda p: p.count("/"), reverse=True)
    renamed_files = 0
    renamed_dirs = 0
    for path in to_rename:
        parent, base = os.path.split(path)
        new_base = PATTERN.sub(lambda m: mapped(m.group(0)), base.encode()).decode()
        target = os.path.join(parent, new_base) if parent else new_base
        if os.path.exists(target):
            print(f"EXISTS={target}", file=sys.stderr)
            sys.exit(1)
        was_dir = os.path.isdir(path)
        os.rename(path, target)
        if was_dir:
            renamed_dirs += 1
        else:
            renamed_files += 1
    print(f"RENAMED_FILES={renamed_files}")
    print(f"RENAMED_DIRS={renamed_dirs}")


if __name__ == "__main__":
    main()
