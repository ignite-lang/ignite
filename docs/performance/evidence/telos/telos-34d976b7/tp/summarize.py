#!/usr/bin/env python3
"""Median table of telos-load JSON lines.

usage: summarize.py RESULTS.jsonl [RESULTS2.jsonl ...]

A later file's runs of a cell replace an earlier file's runs of that cell
(re-runs). Prints markdown tables, one per client mode, release first.
"""
import json
import statistics
import sys

ORDER = [("rr", 1), ("rr", 16), ("rr", 256), ("pipe", 1), ("pipe", 16), ("pipe", 256), ("connect", 1), ("connect", 16)]
BUILDS = ["rust-release", "llvm-release", "rust-debug", "llvm-debug"]


def load(paths):
    cells = {}
    for path in paths:
        fresh = {}
        with open(path) as handle:
            for line in handle:
                line = line.strip()
                if not line:
                    continue
                run = json.loads(line)
                key = (run["quickack"], run["label"], run["workload"], run["connections"])
                fresh.setdefault(key, []).append(run)
        cells.update(fresh)
    return cells


def med(runs, field):
    return statistics.median(run[field] for run in runs)


def fmt_rate(value):
    if value >= 1e6:
        return f"{value / 1e6:.2f} M"
    if value >= 1e3:
        return f"{value / 1e3:.1f} k"
    return f"{value:.1f}"


def fmt_us(value):
    if value >= 1000:
        return f"{value / 1000:.2f} ms"
    return f"{value:.0f} µs"


def table(cells, quickack, builds):
    out = []
    out.append("| workload | conns | build | replies/s | MB/s | p50 | p99 | p999 | CPU s | CPU ms / 1000 | peak RSS MiB | threads | runs | contention |")
    out.append("|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|")
    for workload, conns in ORDER:
        for build in builds:
            runs = cells.get((quickack, build, workload, conns))
            if not runs:
                continue
            nproc = runs[0]["nproc"]
            contended = [r for r in runs if r["runnable_mean"] > nproc / 2]
            loads = ", ".join(f"{r['load1']:.1f}/{r['runnable_mean']:.1f}" for r in runs)
            flag = f"{len(contended)}/{len(runs)} contended (load1/runnable: {loads})" if contended else f"no (load1/runnable: {loads})"
            errors = sum(r["errors"] for r in runs)
            if errors:
                flag += f"; client errors {errors}"
            out.append(
                f"| {workload} | {conns} | {build} | {fmt_rate(med(runs, 'replies_per_s'))} | {med(runs, 'reply_mb_per_s'):.2f} | "
                f"{fmt_us(med(runs, 'p50_us'))} | {fmt_us(med(runs, 'p99_us'))} | {fmt_us(med(runs, 'p999_us'))} | "
                f"{med(runs, 'server_cpu_s'):.2f} | {med(runs, 'cpu_ms_per_1000'):.2f} | {med(runs, 'peak_rss_kib') / 1024:.1f} | "
                f"{int(med(runs, 'threads_max'))} | {len(runs)} | {flag} |"
            )
    return "\n".join(out)


def contended_cells(cells):
    for key, runs in sorted(cells.items(), key=str):
        nproc = runs[0]["nproc"]
        if any(r["runnable_mean"] > nproc / 2 for r in runs):
            yield key


def main():
    cells = load(sys.argv[1:])
    for quickack, title in ((True, "client with TCP_QUICKACK"), (False, "stock client")):
        for builds, profile in ((BUILDS[:2], "release"), (BUILDS[2:], "debug")):
            print(f"### {profile}, {title}\n")
            print(table(cells, quickack, builds))
            print()
    print("### cells with a contended run\n")
    for key in contended_cells(cells):
        print(f"- quickack={key[0]} {key[1]} {key[2]} {key[3]}")


main()
