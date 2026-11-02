#!/usr/bin/env python3
"""Check the complete workspace with at most four concurrent CargoFmt groups.

CargoFmt owns target discovery, recursive modules, editions and configuration.
Only whole workspace packages are partitioned; no Rust file list is maintained.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile

SERIAL = ["cargo", "fmt", "--all", "--", "--check"]


def format_commands(metadata, jobs):
    members = set(metadata["workspace_members"])
    packages = [package for package in metadata["packages"] if package["id"] in members]
    if len(packages) != len(members):
        raise ValueError("Cargo metadata omitted a workspace member")
    directories = {Path(package["manifest_path"]).parent.resolve() for package in packages}
    # --all also follows local path dependencies outside the workspace. Keep
    # CargoFmt's original discovery for that wider graph, including cycles.
    if not packages or any(Path(dependency["path"]).resolve() not in directories
                           for package in packages for dependency in package["dependencies"]
                           if dependency.get("path")):
        return [SERIAL.copy()]
    weighted = []
    for package in packages:
        directory = Path(package["manifest_path"]).parent
        try:
            weight = sum(path.stat().st_size for name in ("src", "tests", "examples", "benches")
                         for path in (directory / name).rglob("*.rs"))
        except OSError:
            # Weights only schedule work. CargoFmt still decides whether any
            # unreadable or dangling source is actually in the format scope.
            weight = 1
        weighted.append((weight, package["name"]))
    groups = [dict(weight=0, packages=[]) for _ in range(min(jobs, len(packages)))]
    for weight, name in sorted(weighted, key=lambda item: (-item[0], item[1])):
        group = min(groups, key=lambda group: group["weight"])
        group["packages"].append(name)
        group["weight"] += weight
    return [["cargo", "fmt"] + [part for name in sorted(group["packages"]) for part in ("-p", name)]
            + ["--", "--check"] for group in groups if group["packages"]]


def run(repo, command):
    try:
        # Inherit every environment variable and the workspace working directory.
        return subprocess.run(command, cwd=repo, capture_output=True, text=True, errors="replace")
    except OSError as error:
        return subprocess.CompletedProcess(command, 127, "", str(error) + "\n")


def status(results):
    return max((result.returncode if result.returncode >= 0 else 128 - result.returncode
                for result in results), default=0)


def diagnostics(results):
    for result in results:
        print("$ " + shlex.join(result.args), file=sys.stderr)
        sys.stderr.write(result.stdout)
        sys.stderr.write(result.stderr)


def self_check():
    with tempfile.TemporaryDirectory(prefix="ignite-format-check-") as temp:
        packages = [dict(id=name, name=name, manifest_path=str(Path(temp) / name / "Cargo.toml"),
                         dependencies=[]) for name in ("one", "two", "three", "four", "five")]
        for index, package in enumerate(packages):
            source = Path(package["manifest_path"]).parent / "src/lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text(" " * (index + 1))
        metadata = dict(workspace_members=[package["id"] for package in packages], packages=packages)
        commands = format_commands(metadata, 4)
        selected = [command[index + 1] for command in commands for index, part in enumerate(command) if part == "-p"]
        assert len(commands) == 4 and sorted(selected) == sorted(package["name"] for package in packages)
        assert all(command[-2:] == ["--", "--check"] for command in commands)
        packages[0]["dependencies"] = [dict(path=str(Path(temp) / "outside"))]
        assert format_commands(metadata, 4) == [SERIAL], "External path dependencies retain --all discovery"
        packages[0]["dependencies"] = [dict(path=str(Path(packages[1]["manifest_path"]).parent))]
        assert format_commands(metadata, 4) != [SERIAL], "Workspace dependencies remain covered by the partition"
        assert status([subprocess.CompletedProcess([], 0), subprocess.CompletedProcess([], -9)]) == 137
    print("Self-check passed: complete package coverage, worker limit, external-dependency fallback and failure status")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jobs", type=int, choices=range(1, 5), default=4)
    parser.add_argument("--self-check", action="store_true")
    args = parser.parse_args()
    if args.self_check:
        self_check()
        return 0
    repo = Path(__file__).resolve().parents[1]
    metadata = run(repo, ["cargo", "metadata", "--no-deps", "--format-version", "1"])
    if metadata.returncode:
        diagnostics([metadata])
        return status([metadata])
    sys.stderr.write(metadata.stderr)
    try:
        commands = format_commands(json.loads(metadata.stdout), args.jobs)
    except (ValueError, KeyError, OSError) as error:
        diagnostics([metadata])
        print("Cannot establish complete format scope: " + str(error), file=sys.stderr)
        return 1
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        # Preserve every result and stable group order even after a failure.
        results = list(pool.map(lambda command: run(repo, command), commands))
    code = status(results)
    if code:
        diagnostics(results)
    else:
        for result in results:
            sys.stdout.write(result.stdout)
            sys.stderr.write(result.stderr)
    return code


if __name__ == "__main__":
    raise SystemExit(main())
