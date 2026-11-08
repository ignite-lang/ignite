---
format: aep.planning-md/3
id: story:llvm-alias-and-cold
kind: story
status: active
title: LLVM IR with alias facts, cold failure paths and no region check in non-allocating loops
relations:
- decomposes: epic:native-parity
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:59:06Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T03:59:07Z", actor: "human:timo", revision: 3}
---
# Story: llvm-alias-and-cold

## Outcome

The LLVM backend marks list and text parameters `noalias readonly` where the moves analysis proves it, declares failure calls `cold noreturn`, and drops the per-iteration region check in loops that allocate nothing, so hot loops keep values in registers.

## Context

- Epic epic:native-parity (diagnosis and evidence in its body).

## Acceptance

- Goldens updated, `CODEGEN_REVISION` bumped; the differential corpus agrees (quality row 1.1).
- codecs LLVM time from 2.94x to at most 2.0x of the reference; todo get-64 user instructions down at least 5 %.
- Measured before and after locally with `perf stat` cost per reply against plain Rust (the method in the epic); the numbers go in the commit body; `task gate` green.

## Scope

- `crates/ignite-llvm/src/lower.rs`, `region.rs`, `ir.rs`, `crates/ignite/src/apply/plan.rs`. Inferred.
