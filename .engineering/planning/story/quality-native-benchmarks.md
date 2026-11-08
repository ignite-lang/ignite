---
format: aep.planning-md/3
id: story:quality-native-benchmarks
kind: story
status: implemented
title: Benchmark Ignite programs against plain Rust and Go on the EC2 host
relations:
- depends_on: story:quality-timing-host
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T01:03:20Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T01:03:20Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-08T18:44:32Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: quality-native-benchmarks

## Outcome

On the dedicated EC2 host of story:quality-timing-host, `task quality-remote` also runs the same workloads written in Ignite (both targets), plain Rust and Go, and records each Ignite program's throughput, p99 latency, peak RSS and binary size as a ratio to the plain-Rust program, so row 3.3 is measured and the gap to native code is a number with a bar.

## Context

- Row 3.3 (throughput and p99 of terminald and todod) is unmeasured; no row compares an Ignite program with a hand-written one.
- The EC2 run of story:quality-timing-host measures compile and check times only (rows 5.3, 6.1, 6.2).
- Operator, 2026-11-08: the purpose of the EC2 host is benchmarks against plain Rust, Go and others, plus the quality bars.

## Acceptance

- Reference programs under `bench/native/` (Rust with the standard library and one widely used HTTP crate, Go with `net/http`): an HTTP JSON todo service with the todo example's routes and a line echo server with terminald's protocol; each passes the same functional check as the Ignite program before it is timed.
- `ignite-load` drives every program with the same workload matrix on the host (loopback, pinned CPUs for server and load), 3 ABBA passes, medians.
- Records per workload and implementation: requests per second, p50 and p99 latency, peak RSS, binary size (release, stripped); the trend and the quality page show the ratios Ignite LLVM / Rust, Ignite Rust-target / Rust, Go / Rust.
- Row 3.3 gets a measured value and a bar (initial bar: the first run's ratios, ratcheted); a release refuses when a ratio worsens beyond its tolerance against the last release in the same run.
- No account, profile, instance id or internal name in any committed file; the leftover check by tag finds nothing after the run.

## Scope

- `tools/ignite-perf` (remote and host modes), `crates/ignite-load`, `bench/native/` (new), `quality/baseline.json`, `docs/quality.md`, `crates/ignite/tests/quality.rs`, `Taskfile.yml`. Inferred.
