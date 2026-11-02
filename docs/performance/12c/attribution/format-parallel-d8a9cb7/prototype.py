#!/usr/bin/env python3
"""External diagnostic only: partition CargoFmt package selection, never its files."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import subprocess
import time


def capture(repo, command):
    start = time.monotonic()
    result = subprocess.run(command, cwd=repo, capture_output=True, text=True)
    return dict(command=command, seconds=time.monotonic() - start, exit=result.returncode,
                stdout=result.stdout, stderr=result.stderr)


def plan(repo, jobs):
    command = ["cargo", "metadata", "--no-deps", "--format-version", "1"]
    captured = capture(repo, command)
    if captured["exit"]:
        raise RuntimeError(captured)
    metadata = json.loads(captured["stdout"])
    members = set(metadata["workspace_members"])
    packages = [p for p in metadata["packages"] if p["id"] in members]
    if len(packages) != len(members):
        raise RuntimeError("Workspace metadata omitted a member")
    package_dirs = {str(Path(p["manifest_path"]).parent.resolve()) for p in packages}
    external = [d["path"] for p in packages for d in p["dependencies"]
                if d.get("path") and str(Path(d["path"]).resolve()) not in package_dirs]
    roots = sorted((dict(package=p["name"], root=t["src_path"], edition=t["edition"], kind=t["kind"])
                    for p in packages for t in p["targets"]), key=lambda r: (r["package"], r["root"]))
    if external:
        # --all also follows local path dependencies; never narrow that scope.
        commands = [["cargo", "fmt", "--all", "-v", "--", "--check"]]
        return commands, roots, captured, external
    weighted = []
    for package in packages:
        directory = Path(package["manifest_path"]).parent
        weight = sum(path.stat().st_size for child in ("src", "tests", "examples", "benches")
                     if (directory / child).exists() for path in (directory / child).rglob("*.rs"))
        weighted.append((weight, package["name"]))
    groups = [dict(weight=0, packages=[]) for _ in range(min(jobs, len(packages)))]
    for weight, name in sorted(weighted, key=lambda item: (-item[0], item[1])):
        group = min(groups, key=lambda group: group["weight"])
        group["packages"].append(name)
        group["weight"] += weight
    commands = [["cargo", "fmt", "-v"] + [part for name in sorted(group["packages"]) for part in ("-p", name)]
                + ["--", "--check"] for group in groups]
    return commands, roots, captured, []


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--jobs", type=int, default=4, choices=range(1, 5))
    parser.add_argument("--plan-only", action="store_true")
    parser.add_argument("--serial", action="store_true")
    args = parser.parse_args()
    args.repo = args.repo.resolve()
    if args.report.exists():
        raise ValueError("Refusing to overwrite a diagnostic report")
    start = time.monotonic()
    if args.serial:
        commands, roots, metadata, external = [["cargo", "fmt", "--all", "-v", "--", "--check"]], [], None, []
    else:
        commands, roots, metadata, external = plan(args.repo, args.jobs)
    results = []
    if not args.plan_only:
        with ThreadPoolExecutor(max_workers=args.jobs) as pool:
            results = list(pool.map(lambda command: capture(args.repo, command), commands))
    report = dict(repo=str(args.repo), prototype_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  jobs=args.jobs, seconds=time.monotonic() - start, commands=commands, targets=roots,
                  metadata=metadata, external_path_dependency_fallback=external, results=results,
                  exit=max((row["exit"] for row in results), default=0))
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    for result in results:
        print(result["stdout"], end="")
        print(result["stderr"], end="")
    return report["exit"]


if __name__ == "__main__":
    raise SystemExit(main())
