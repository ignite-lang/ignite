---
format: aep.planning-md/3
id: story:rustgen-fewer-allocations
kind: story
status: active
title: Rust target allocates no String per literal and clones less
relations:
- decomposes: epic:native-parity
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:59:06Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T03:59:07Z", actor: "human:timo", revision: 3}
---
# Story: rustgen-fewer-allocations

## Outcome

The Rust target emits literals without a heap `String`, borrows in `to_bytes()` and builds `split_once`/`map` without an `Arc` closure, so a Rust-target todo build allocates about what the reference does.

## Context

- Epic epic:native-parity (diagnosis and evidence in its body).

## Acceptance

- Goldens updated, `CODEGEN_REVISION` bumped; the differential corpus agrees.
- Rust-target todo get-64 `malloc` share of user time under 10 %, throughput ratio recorded.
- Measured before and after locally with `perf stat` cost per reply against plain Rust (the method in the epic); the numbers go in the commit body; `task gate` green.

## Scope

- `crates/ignite-rustgen/src/lib.rs`, `crates/ignite/src/apply/plan.rs`. Inferred.
