---
format: aep.planning-md/3
id: story:llvm-release-all-handles
kind: story
status: active
title: LLVM releases listener, directory and capability handles
relations:
- decomposes: epic:leak-guard
scope:
- confidence: inferred
  path: crates/ignite-llvm/src/glue.rs
- confidence: inferred
  path: crates/ignite-llvm/src/lower.rs
- confidence: cited
  path: crates/ignite-llvm/src/own.rs
- confidence: inferred
  path: crates/ignite-llvm/tests/streams.rs
- confidence: inferred
  path: crates/ignite-rt/src/fs.rs
- confidence: inferred
  path: crates/ignite-rt/src/net.rs
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
- confidence: cited
  path: crates/ignite/tests/fixtures/leak/seeded.ignite
- confidence: cited
  path: crates/ignite/tests/leak.rs
- confidence: inferred
  path: docs/llvm.md
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/runtime.ignite
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T17:01:38Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T21:40:02Z", actor: "human:timo", revision: 3}
---
# Story: llvm-release-all-handles

## Outcome

The LLVM target releases listener, stream, directory, launcher and other capability handles when their owner ends, as the Rust target does, so the leak census soaks of those kinds hold flat on both targets.

## Context

- Epic epic:leak-guard. Found by story:leak-census: on LLVM the soaks `listener`, `stream` (listeners grow), `child` (its launcher: capabilities grow), `directory` and `capability` grow their counts; they are asserted as known failures in `leak::` (`KNOWN_ON_LLVM`). The same leak class as G30 (design docs/leak-guard.md 1.2, Q8).

## Acceptance

- Each of those soaks passes on LLVM; its entry leaves `KNOWN_ON_LLVM`; the seeded checks run on both targets.
- `task gate` green.

## Scope

- `crates/ignite-llvm/src/own.rs`, `lower.rs`, `crates/ignite-rt` handle release, `crates/ignite/tests/leak.rs`. Inferred.
