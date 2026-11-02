#!/usr/bin/env python3
"""Reproduce the frozen 11b performance comparison without editing a checkout.

All source exports, build caches and evidence have separate roots. Source exports
are git archives, not linked worktrees; keep evidence outside disposable targets.
"""
from __future__ import annotations

import argparse
import collections
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import sys
import tarfile
import tempfile
import time

SCHEMA = "ignite-performance/1"
GATE = [
    ("fmt", ["cargo", "fmt", "--all", "--", "--check"]),
    ("clippy", ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]),
    ("workspace", ["cargo", "test", "--workspace", "--no-fail-fast"]),
    ("inline", ["task", "test-ignite"]),
]


def run_git(repo, *args, data=None, env=None):
    return subprocess.check_output(["git", "-C", str(repo), *args], input=data, env=env)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def dump(path, value):
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def verify_manifest(root, manifest):
    changed = []
    for name, expected in manifest.items():
        path = Path(root) / name
        if not path.is_file() or digest(path) != expected:
            changed.append(name)
    if changed:
        raise ValueError("Source manifest mismatch: " + ", ".join(changed))


def source_manifest(root):
    return {str(p.relative_to(root)): digest(p) for p in sorted(Path(root).rglob("*")) if p.is_file()}


def recover(args):
    """Write only Git objects/ref through a private index, never the checkout index."""
    args.source, args.evidence = args.source.resolve(), args.evidence.resolve()
    manifest_path = args.manifest.resolve()
    manifest = json.loads(manifest_path.read_text())
    verify_manifest(args.source, manifest)
    if any(Path(name).is_absolute() or ".." in Path(name).parts for name in manifest):
        raise ValueError("Manifest paths must be relative source paths")
    args.evidence.mkdir(parents=True, exist_ok=True)
    parent = run_git(args.repo, "rev-parse", args.parent + "^{commit}").decode().strip()
    with tempfile.TemporaryDirectory(prefix="recover-index-", dir=args.evidence) as temp:
        env = dict(os.environ, GIT_INDEX_FILE=str(Path(temp) / "index"))
        run_git(args.repo, "read-tree", "--empty", env=env)
        entries = bytearray()
        for name in sorted(manifest):
            path = args.source / name
            if path.is_symlink():
                raise ValueError("Recovery needs an explicit symlink manifest: " + name)
            blob = run_git(args.repo, "hash-object", "-w", "--no-filters", str(path)).decode().strip()
            mode = "100755" if path.stat().st_mode & 0o111 else "100644"
            entries.extend(f"{mode} {blob}\t{name}\0".encode())
        run_git(args.repo, "update-index", "-z", "--index-info", data=bytes(entries), env=env)
        tree = run_git(args.repo, "write-tree", env=env).decode().strip()
    existing = subprocess.run(["git", "-C", str(args.repo), "rev-parse", "--verify", args.ref], capture_output=True)
    if existing.returncode == 0:
        commit = existing.stdout.decode().strip()
        if run_git(args.repo, "rev-parse", commit + "^{tree}").decode().strip() != tree:
            raise ValueError("Existing benchmark ref has a different tree; refusing replacement")
    else:
        message = (
            "Recover the exact 11b benchmark snapshot\n\n"
            "Recovered now from the retained pre-slice-12 source export; this is\n"
            "not a historical commit from the implementation session.\n"
            f"Recorded predecessor: {parent}\n"
            f"Verified source files: {len(manifest)}\n"
            f"Manifest SHA256: {digest(manifest_path)}\n"
            "The source tree is unchanged; provenance is retained in the main\n"
            "branch under docs/performance/baselines.\n"
        )
        commit = run_git(args.repo, "commit-tree", tree, "-p", parent, data=message.encode()).decode().strip()
        run_git(args.repo, "update-ref", args.ref, commit, "0" * len(commit))
    meta = dict(schema=SCHEMA, ref=args.ref, commit=commit, tree=tree, parent=parent,
                provenance="Recovered snapshot, committed at recovery time, not historical",
                source_files=len(manifest), source_manifest_sha256=digest(manifest_path))
    if manifest_path != args.evidence / "11b-source.json":
        shutil.copy2(manifest_path, args.evidence / "11b-source.json")
    dump(args.evidence / "11b.json", meta)
    if args.publish:
        subprocess.run(["git", "-C", str(args.repo), "push", "origin", args.ref + ":" + args.ref], check=True)
    print(json.dumps(meta, indent=2))


def compare_rows(rows, baseline="baseline", candidate="candidate", load_delta=5):
    paired = collections.defaultdict(dict)
    for row in rows:
        paired[(row["unit"], row["pair"])][row["label"]] = row
    result = []
    for unit in dict.fromkeys(r["unit"] for r in rows):
        good, excluded = [], []
        for (name, pair), values in paired.items():
            if name != unit or baseline not in values or candidate not in values:
                continue
            before, after = values[baseline], values[candidate]
            reason = None
            if before["exit"] or after["exit"]:
                reason = "command_failed"
            elif abs(before["load_before"] - after["load_before"]) > load_delta:
                reason = "load_mismatch"
            if reason:
                excluded.append(dict(pair=pair, reason=reason, exits=[before["exit"], after["exit"]],
                                     loads=[before["load_before"], after["load_before"]]))
            else:
                good.append((before, after))
        record = dict(unit=unit, baseline=baseline, valid_pairs=len(good), excluded=excluded)
        if good:
            record.update(before=statistics.median(b["seconds"] for b, _ in good),
                          candidate=statistics.median(a["seconds"] for _, a in good),
                          ratio=statistics.median(a["seconds"] / b["seconds"] for b, a in good),
                          loads=[min(r["load_before"] for pair in good for r in pair),
                                 max(r["load_before"] for pair in good for r in pair)])
        result.append(record)
    return result


def closure_eligible(controls, frozen_baseline_matches, comparison_present):
    """Loose controls are useful diagnostics, never performance acceptance."""
    return (frozen_baseline_matches and comparison_present
            and 0 < controls["load_delta"] <= 5
            and 0 < controls["absolute_max_load"] <= 9
            and 0 <= controls["extra_warm_passes"] <= 2
            and 0 <= controls["cold_retries"] <= 1
            and controls["warm_pairs"] >= 3
            and controls["check_pairs"] >= 15)


def safe_export(repo, commit, destination):
    data = run_git(repo, "archive", "--format=tar", commit)
    destination.mkdir(parents=True)
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        for member in archive.getmembers():
            name = Path(member.name)
            if name.is_absolute() or ".." in name.parts or member.issym() or member.islnk():
                raise ValueError("Unsupported source archive member: " + member.name)
        archive.extractall(destination, filter="data")


class Harness:
    def __init__(self, args):
        self.args = args
        self.repo = args.repo.resolve()
        self.sources, self.builds, self.evidence = (p.resolve() for p in (args.sources, args.builds, args.evidence))
        roots = [self.sources, self.builds, self.evidence]
        if len(set(roots)) != 3:
            raise ValueError("Source/build/evidence roots must be distinct")
        for a in roots:
            for b in roots:
                if a != b and (a in b.parents or b in a.parents):
                    raise ValueError("Source/build/evidence roots must be separate, non-nested directories")
        for root in roots:
            root.mkdir(parents=True, exist_ok=True)
        self.refs = {"baseline": args.baseline, "candidate": args.candidate}
        if args.comparison:
            self.refs["comparison"] = args.comparison
        self.commits = {label: run_git(self.repo, "rev-parse", ref + "^{commit}").decode().strip()
                        for label, ref in self.refs.items()}
        baseline_metadata = self.repo / "docs/performance/baselines/11b.json"
        expected_tree = json.loads(baseline_metadata.read_text()).get("tree") if baseline_metadata.exists() else None
        actual_tree = run_git(self.repo, "rev-parse", self.commits["baseline"] + "^{tree}").decode().strip()
        self.frozen_baseline_matches = expected_tree is not None and expected_tree == actual_tree
        self.paths = {label: self.sources / (label + "-" + commit[:12]) for label, commit in self.commits.items()}
        self.targets = {label: self.builds / (label + "-" + commit[:12]) for label, commit in self.commits.items()}
        self.env_base = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_PROFILE_DEV_DEBUG="line-tables-only",
                             CARGO_PROFILE_RELEASE_DEBUG="0", CARGO_TERM_COLOR="never")
        self.env_base.pop("CARGO_TARGET_DIR", None)
        self.env_base.pop("SCCACHE_RECACHE", None)
        self.records_path = self.evidence / "records.jsonl"
        self.rows = [json.loads(line) for line in self.records_path.read_text().splitlines()] if self.records_path.exists() else []
        self.controls = {key: getattr(args, key) for key in ("warm_pairs", "check_pairs", "extra_warm_passes", "cold_retries", "load_delta", "absolute_max_load")}
        config = dict(schema=SCHEMA, commits=self.commits, refs=self.refs, controls=self.controls,
                      sources=str(self.sources), builds=str(self.builds), load_delta=args.load_delta,
                      environment={key: self.env_base.get(key) for key in ("CARGO_INCREMENTAL", "CARGO_PROFILE_DEV_DEBUG", "CARGO_PROFILE_RELEASE_DEBUG", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN", "CARGO_BUILD_JOBS", "CARGO_BUILD_TARGET", "SCCACHE_DISABLE")})
        metadata = self.evidence / "run.json"
        if metadata.exists() and json.loads(metadata.read_text()) != config:
            raise ValueError("Evidence belongs to another configuration; use a new evidence directory")
        dump(metadata, config)
        for label, commit in self.commits.items():
            path = self.paths[label]
            manifest_path = self.evidence / ("source-" + label + ".json")
            if not path.exists():
                safe_export(self.repo, commit, path)
                dump(manifest_path, source_manifest(path))
            elif not manifest_path.exists():
                raise ValueError("Unowned source directory: " + str(path))
            verify_manifest(path, json.loads(manifest_path.read_text()))
            owner = self.targets[label].with_suffix(".owner.json")
            expected = dict(schema=SCHEMA, commit=commit, source=str(path))
            if self.targets[label].exists() and not owner.exists():
                raise ValueError("Unowned build directory: " + str(self.targets[label]))
            if owner.exists() and json.loads(owner.read_text()) != expected:
                raise ValueError("Build ownership mismatch: " + str(owner))
            dump(owner, expected)
        tools = {}
        for name, command in [("rustc", ["rustc", "-Vv"]), ("cargo", ["cargo", "-V"]),
                              ("clang", ["clang", "--version"]), ("task", ["task", "--version"]),
                              ("sccache", ["sccache", "--version"])]:
            try:
                tools[name] = subprocess.check_output(command, stderr=subprocess.STDOUT, text=True).strip()
            except (FileNotFoundError, subprocess.CalledProcessError) as error:
                tools[name] = str(error)
        tool_path = self.evidence / "tools.json"
        if tool_path.exists() and json.loads(tool_path.read_text()) != tools:
            raise ValueError("Tool versions changed; do not mix toolchains in one comparison")
        dump(tool_path, tools)
        self.capture_id = str(time.time_ns())

    def env(self, label, recache=False):
        value = dict(self.env_base, CARGO_TARGET_DIR=str(self.targets[label]))
        if recache:
            value["SCCACHE_RECACHE"] = "1"
        return value

    def guard(self):
        free = shutil.disk_usage(self.builds).free / 2**30
        if free < self.args.min_free_gib:
            raise RuntimeError(f"Disk guard: {free:.1f} GiB available, requires {self.args.min_free_gib}")

    def append(self, row):
        self.rows.append(row)
        with self.records_path.open("a") as stream:
            stream.write(json.dumps(row, sort_keys=True) + "\n")
        print(json.dumps({k: row[k] for k in ("label", "unit", "pair", "seconds", "exit", "load_before")}), flush=True)
        return row

    def command(self, label, unit, pair, command, recache=False, phase="measured"):
        self.guard()
        log = self.evidence / "logs" / f"{self.capture_id}-{pair}-{label}-{unit}.log"
        log.parent.mkdir(exist_ok=True)
        before, start = os.getloadavg()[0], time.monotonic()
        with log.open("wb") as stream:
            result = subprocess.run([str(s) for s in command], cwd=self.paths[label], env=self.env(label, recache),
                                    stdout=stream, stderr=subprocess.STDOUT)
        text = log.read_text(errors="replace")
        counts = [tuple(map(int, x)) for x in re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored", text)]
        row = dict(schema=SCHEMA, label=label, commit=self.commits[label], unit=unit, pair=str(pair), phase=phase,
                   cache_policy="recache" if recache else "normal", command=[str(s) for s in command], seconds=time.monotonic() - start, exit=result.returncode,
                   load_before=before, load_after=os.getloadavg()[0], log=str(log.relative_to(self.evidence)),
                   tests=dict(passed=sum(x[0] for x in counts), failed=sum(x[1] for x in counts), ignored=sum(x[2] for x in counts)))
        inline = re.findall(r"test result: ok\. (\d+) passed; 0 failed; 0 filtered out", text)
        if inline:
            row["inline_counts"] = list(map(int, inline))
        executables = re.findall(r"Executable [^\n]*\(([^\n]+)\)", text)
        if executables:
            files = [Path(name) for name in executables]
            row["executables"] = dict(count=len(files), bytes=sum(p.stat().st_size for p in files if p.exists()))
        return self.append(row)

    def comparisons(self):
        values = compare_rows(self.rows, load_delta=self.args.load_delta)
        if "comparison" in self.refs:
            values += compare_rows(self.rows, baseline="comparison", load_delta=self.args.load_delta)
        return values

    def failed_candidate(self):
        return any(r["label"] == "candidate" and r["exit"] for r in self.rows)

    def gate(self, label, pair, mode, recache=False, phase="measured"):
        before, start = os.getloadavg()[0], time.monotonic()
        failures = 0
        for name, command in GATE:
            row = self.command(label, f"{mode}-{name}", pair, command, recache, phase)
            failures += bool(row["exit"])
            if label == "candidate" and name == "workspace":
                journal = self.targets[label] / "tmp/run-state-item2-real/journal.jsonl"
                if journal.exists():
                    destination = self.evidence / f"restart-{self.capture_id}-{pair}.jsonl"
                    shutil.copy2(journal, destination)
        return self.append(dict(schema=SCHEMA, label=label, commit=self.commits[label], unit="gate-" + mode,
                                pair=str(pair), phase=phase, seconds=time.monotonic() - start, exit=failures,
                                load_before=before, load_after=os.getloadavg()[0]))

    def prepare(self):
        for label in self.refs:
            self.gate(label, "prepare-" + self.capture_id, "prepare", phase="prepare")
            self.command(label, "runtime-release", "prepare-" + self.capture_id,
                         ["cargo", "build", "-q", "--release", "-p", "ignite-rt", "--features", "runner", "--target-dir", self.targets[label] / "ignite-test/rt"], phase="prepare")
        if self.failed_candidate():
            raise RuntimeError("Candidate failure during preparation; fix and freeze a new candidate")

    def smoke(self):
        for label in self.refs:
            self.command(label, "smoke-compiler", "smoke", ["cargo", "build", "-q", "-p", "ignite"], phase="smoke")
            compiler = self.targets[label] / "debug/ignite"
            outputs = [self.evidence / f"smoke-{label}-{n}.json" for n in (1, 2)]
            for number, output in enumerate(outputs):
                self.command(label, "smoke-plan", f"smoke-{number}", [compiler, "plan", "examples/todo/todo.ignite", "--out", output], phase="smoke")
            if not all(p.exists() for p in outputs) or outputs[0].read_bytes() != outputs[1].read_bytes():
                raise RuntimeError("Non-deterministic or failed smoke plan: " + label)
            output = self.builds / "smoke" / self.capture_id / label
            self.command(label, "smoke-build", "smoke", [compiler, "build", "examples/todo/todo.ignite", "--out", output / "program", "--target-dir", output / "cargo"], phase="smoke")

    def cold(self):
        for attempt in range(self.args.cold_retries + 1):
            pair = self.capture_id + "-cold-" + str(attempt + 1)
            for label in self.refs:
                if self.targets[label].exists():
                    cleaned = self.command(label, "clean", pair, ["cargo", "clean", "--target-dir", self.targets[label]], phase="prepare")
                    if cleaned["exit"]:
                        raise RuntimeError("Cold preparation failed; no cold claim is valid")
                self.gate(label, pair, "cold", recache=True)
                if self.failed_candidate():
                    raise RuntimeError("Candidate cold failure; freeze corrected source before restarting")
            comparisons = self.comparisons()
            gates = [r for r in comparisons if r["unit"] == "gate-cold" or r["unit"].startswith("cold-")]
            if gates and all(gate["valid_pairs"] for gate in gates):
                return
            if any(r["label"] == "baseline" and r["pair"] == pair and r["exit"] for r in self.rows):
                return

    def warm(self):
        self.prepare()
        for number in range(1, self.args.warm_pairs + self.args.extra_warm_passes + 1):
            pair = self.capture_id + "-warm-" + str(number)
            for label in self.refs:
                (self.paths[label] / "crates/ignite-check/src/lib.rs").touch()
                self.gate(label, pair, "warm")
                if self.failed_candidate():
                    raise RuntimeError("Candidate warm failure; do not collect greens around it")
                self.command(label, "no-run-warm", pair, ["cargo", "test", "--workspace", "--no-run"])
                (self.paths[label] / "crates/ignite-check/src/lib.rs").touch()
                self.command(label, "no-run-touch", pair, ["cargo", "test", "--workspace", "--no-run"])
                self.command(label, "file-ignite_test", pair, ["cargo", "test", "-p", "ignite", "--test", "main", "ignite_test::"])
            values = self.comparisons()
            needed = [r for r in values if r["unit"].startswith("warm-") or r["unit"] in ("gate-warm", "no-run-warm", "no-run-touch", "file-ignite_test")]
            if number >= self.args.warm_pairs and all(r["valid_pairs"] >= self.args.warm_pairs for r in needed):
                return

    def programs(self):
        for pair in range(1, self.args.warm_pairs + self.args.extra_warm_passes + 1):
            for profile in ("dev", "release"):
                for target in ("rust", "llvm"):
                    for label in self.refs:
                        compiler = self.targets[label] / "debug/ignite"
                        if target == "llvm":
                            self.command(label, "runtime-" + profile, f"{self.capture_id}-programs-{pair}",
                                         ["cargo", "build", "-q", "-p", "ignite-rt", "--features", "runner", "--target-dir", self.targets[label] / "ignite-test/rt"] + (["--release"] if profile == "release" else []), phase="prepare")
                        output = self.builds / "programs" / self.capture_id / f"{pair}-{label}-{target}-{profile}"
                        command = [compiler, "build", "examples/todo/todo.ignite", "--out", output]
                        if profile == "release":
                            command += ["--release"]
                        if target == "rust":
                            command += ["--target-dir", str(output) + "-cargo"]
                        else:
                            command += ["--target", "llvm", "--runtime", self.targets[label] / f"ignite-test/rt/{'release' if profile == 'release' else 'debug'}/libignite_rt.a"]
                        self.command(label, f"todod-{target}-{profile}", f"{self.capture_id}-programs-{pair}", command, recache=True)
                        sizes = {name: len((output / name).read_text().splitlines()) for name in ("std/src/lib.rs", "std.ll", "program.ll") if (output / name).exists()}
                        dump(self.evidence / f"program-{pair}-{label}-{target}-{profile}.json", sizes)
            rows = [r for r in self.comparisons() if r["unit"].startswith("todod-")]
            if pair >= self.args.warm_pairs and all(r["valid_pairs"] >= self.args.warm_pairs for r in rows):
                return

    def cli(self):
        for number in range(1, self.args.check_pairs + self.args.extra_warm_passes + 1):
            for unit in ("check", "plan"):
                for label in self.refs:
                    command = [self.targets[label] / "debug/ignite", unit, "examples/terminal/terminal.design.ignite"]
                    if unit == "plan":
                        command += ["--out", self.evidence / f"plan-{number}-{label}.json"]
                    self.command(label, unit, self.capture_id + "-cli-" + str(number), command)
            values = [r for r in self.comparisons() if r["unit"] in ("check", "plan")]
            if number >= self.args.check_pairs and all(r["valid_pairs"] >= self.args.check_pairs for r in values):
                return

    def summarize(self):
        measured = [r for r in self.rows if r["phase"] == "measured"]
        results = compare_rows(measured, load_delta=self.args.load_delta)
        if "comparison" in self.refs:
            results += compare_rows(measured, baseline="comparison", load_delta=self.args.load_delta)
        required = {"cold-" + name for name, _ in GATE} | {"gate-cold", "gate-warm", "no-run-warm", "no-run-touch", "file-ignite_test", "check", "plan"} | {"warm-" + name for name, _ in GATE} | {f"todod-{target}-{profile}" for target in ("rust", "llvm") for profile in ("dev", "release")}
        missing, over = [], []
        for unit in sorted(required):
            row = next((r for r in results if r["baseline"] == "baseline" and r["unit"] == unit), None)
            count = 1 if unit.startswith("cold-") or unit == "gate-cold" else max(15, self.args.check_pairs) if unit in ("check", "plan") else max(3, self.args.warm_pairs)
            if row is None or row["valid_pairs"] < count:
                missing.append(unit)
            elif row["ratio"] > 1.10:
                over.append(unit)
        workspace = next((r for r in results if r["baseline"] == "baseline" and r["unit"] == "warm-workspace"), None)
        absolute = bool(workspace and workspace.get("candidate", float("inf")) <= 10.8)
        low_load = bool(workspace and workspace.get("loads", [float("inf"), float("inf")])[1] <= self.args.absolute_max_load)
        file = next((r for r in results if r["baseline"] == "baseline" and r["unit"] == "file-ignite_test"), None)
        file_ok = bool(file and file.get("candidate", float("inf")) <= 4.6)
        comparison_missing = [] if "comparison" in self.refs else ["published comparison not configured"]
        if "comparison" in self.refs:
            for unit in sorted(required):
                row = next((r for r in results if r["baseline"] == "comparison" and r["unit"] == unit), None)
                count = 1 if unit.startswith("cold-") or unit == "gate-cold" else max(15, self.args.check_pairs) if unit in ("check", "plan") else max(3, self.args.warm_pairs)
                if row is None or row["valid_pairs"] < count:
                    comparison_missing.append(unit)
        workspaces = [r for r in measured if r["label"] == "candidate" and r["unit"].endswith("workspace")]
        inlines = [r for r in measured if r["label"] == "candidate" and r["unit"].endswith("inline")]
        coverage = bool(workspaces and inlines) and all(r["tests"]["passed"] >= 647 and r["tests"]["ignored"] == 5 for r in workspaces) and all(len(r.get("inline_counts", [])) == 2 and min(r["inline_counts"]) >= 148 for r in inlines)
        failed = any(r["exit"] for r in self.rows)
        controls_pass = closure_eligible(self.controls, self.frozen_baseline_matches, "comparison" in self.refs)
        accepted = controls_pass and not missing and not comparison_missing and not over and absolute and low_load and file_ok and coverage and not self.failed_candidate()
        summary = dict(schema=SCHEMA, commits=self.commits, results=results, missing=missing, over_budget=over,
                       warm_workspace_absolute_pass=absolute, comparable_low_load=low_load,
                       ignite_test_absolute_pass=file_ok, candidate_failed=self.failed_candidate(),
                       any_command_failed=failed, frozen_baseline_matches=self.frozen_baseline_matches, closure_controls_pass=controls_pass, coverage_pass=coverage, comparison_missing=comparison_missing, accepted=accepted)
        dump(self.evidence / "summary.json", summary)
        lines = ["# Performance comparison", "", "unit | baseline | before s | candidate s | ratio | valid/excluded", "---|---|---:|---:|---:|---:"]
        for row in results:
            if "ratio" in row:
                lines.append(f"{row['unit']} | {row['baseline']} | {row['before']:.6f} | {row['candidate']:.6f} | {row['ratio']:.4f} | {row['valid_pairs']}/{len(row['excluded'])}")
        lines += ["", "Accepted: " + str(accepted), "Missing evidence: " + ", ".join(missing), "Over budget: " + ", ".join(over)]
        (self.evidence / "summary.md").write_text("\n".join(lines) + "\n")
        return summary

    def verify_sources(self):
        for label, path in self.paths.items():
            verify_manifest(path, json.loads((self.evidence / ("source-" + label + ".json")).read_text()))


def self_check():
    def row(label, pair, seconds, load=1, exit=0):
        return dict(unit="warm-workspace", label=label, pair=pair, seconds=seconds, load_before=load, exit=exit)
    rows = [row("baseline", "1", 10), row("candidate", "1", 11),
            row("baseline", "2", 100), row("candidate", "2", 50),
            row("baseline", "3", 10), row("candidate", "3", 100, load=20),
            row("baseline", "4", 10), row("candidate", "4", 1, exit=1)]
    value = compare_rows(rows)[0]
    assert value["valid_pairs"] == 2 and value["ratio"] == 0.8
    assert [v["reason"] for v in value["excluded"]] == ["load_mismatch", "command_failed"]
    assert statistics.median([11, 50]) / statistics.median([10, 100]) != value["ratio"]
    holder = object.__new__(Harness)
    holder.rows = [row("candidate", "1", 10, exit=1), row("candidate", "2", 1)]
    assert holder.failed_candidate(), "Later green samples cannot hide a candidate failure"
    holder.rows = [row("baseline", "1", 10, exit=1), row("candidate", "2", 1)]
    assert not holder.failed_candidate(), "Unchanged-baseline failure is tracked separately"
    controls = dict(warm_pairs=3, check_pairs=15, extra_warm_passes=2,
                    cold_retries=1, load_delta=5, absolute_max_load=9)
    assert closure_eligible(controls, True, True)
    assert not closure_eligible(controls, False, True), "Another baseline cannot redefine the budget"
    assert not closure_eligible(controls, True, False), "Published checkpoint comparison is required"
    for key, value in [("load_delta", 6), ("absolute_max_load", 10), ("cold_retries", 2),
                       ("extra_warm_passes", 3), ("warm_pairs", 2), ("check_pairs", 14)]:
        assert not closure_eligible(dict(controls, **{key: value}), True, True), key
    print("Self-check passed: paired ratios, exclusions, candidate failures, and strict closure provenance/controls")


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="mode", required=True)
    commands.add_parser("self-check")
    recovery = commands.add_parser("recover-baseline")
    recovery.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    recovery.add_argument("--source", type=Path, required=True)
    recovery.add_argument("--manifest", type=Path, required=True)
    recovery.add_argument("--parent", default="60aef38")
    recovery.add_argument("--ref", default="refs/heads/bench/11b-recovered")
    recovery.add_argument("--evidence", type=Path, required=True)
    recovery.add_argument("--publish", action="store_true")
    for mode in ("smoke", "prepare", "cold", "warm", "programs", "cli", "all", "summarize"):
        command = commands.add_parser(mode)
        command.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
        command.add_argument("--baseline", default="origin/bench/11b-recovered")
        command.add_argument("--candidate", default="HEAD")
        command.add_argument("--comparison", default="9273349")
        command.add_argument("--sources", type=Path, required=True)
        command.add_argument("--builds", type=Path, required=True)
        command.add_argument("--evidence", type=Path, required=True)
        command.add_argument("--warm-pairs", type=int, default=3)
        command.add_argument("--check-pairs", type=int, default=15)
        command.add_argument("--extra-warm-passes", type=int, default=2)
        command.add_argument("--cold-retries", type=int, default=1)
        command.add_argument("--load-delta", type=float, default=5)
        command.add_argument("--absolute-max-load", type=float, default=9)
        command.add_argument("--min-free-gib", type=float, default=20)
    return result


def main():
    args = parser().parse_args()
    if args.mode == "self-check":
        self_check()
        return 0
    if args.mode == "recover-baseline":
        recover(args)
        return 0
    if args.extra_warm_passes < 0 or args.cold_retries < 0 or args.load_delta <= 0 or args.absolute_max_load <= 0:
        raise ValueError("Retries must be nonnegative and load controls positive")
    if args.warm_pairs < 1 or args.check_pairs < 1:
        raise ValueError("Sample counts must be positive")
    harness = Harness(args)
    failure = None
    try:
        if args.mode == "all":
            harness.smoke()
            harness.cold()
            harness.warm()
            harness.programs()
            harness.cli()
        elif args.mode != "summarize":
            getattr(harness, args.mode)()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        failure = str(error)
        print(failure, file=sys.stderr)
    finally:
        harness.verify_sources()
        summary = harness.summarize()
    if failure:
        return 1
    if args.mode == "summarize":
        return 0 if summary["accepted"] else 2
    if any(row["exit"] for row in harness.rows):
        return 1
    if args.mode == "all" and not summary["accepted"]:
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
