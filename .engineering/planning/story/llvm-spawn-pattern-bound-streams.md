---
format: aep.planning-md/3
id: story:llvm-spawn-pattern-bound-streams
kind: story
status: active
title: LLVM spawn hands pattern-bound streams to the task
summary: A failing task closes a stream bound by let (a, b) = ...; spawn in a test body behaves as in a helper
scope:
- confidence: inferred
  path: crates/ignite-llvm/src
- confidence: cited
  path: crates/ignite-rustgen/src/lib.rs
- confidence: cited
  path: crates/ignite/src/apply/plan.rs
- confidence: cited
  path: crates/ignite/tests/fixtures/streams
- confidence: cited
  path: crates/ignite/tests/stream_ownership.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T15:04:13Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-11-06T15:04:13Z", actor: "human:timo", revision: 6}
---
# Story: llvm-spawn-pattern-bound-streams

## Outcome

On the LLVM target a stream bound by a pattern (`let (a, b) = Pipe.pair()`) and used by a spawned
task is handed to that task like a stream bound by name, so the task's failure closes it; a
`spawn` directly in a test body behaves as one in a helper `test fn`.

## Context

- Found while landing `story:store-atomic-upsert` (PR 19, 2026-11-04), not fixed there:
  - a pattern-bound stream is not passed to the spawned task, so it is not closed when the task
    fails (read from the LLVM lowering of `spawn`, `crates/ignite-llvm`);
  - the upsert overflow test timed out when it spawned its task from the test body and passes
    when it spawns from a helper `test fn`. That the cause is the first item is inferred, not
    verified.

## Acceptance

## Acceptance

Scoped 2026-11-06 from main 317955736d: the LLVM half is met by `a1065a37e8` (story `llvm-stream-ownership`): `crates/ignite-llvm/src/own.rs:37`, `lower.rs:5021`, tests `crates/ignite/tests/stream_ownership.rs:61` (helper spawn, both targets) and `:69` (test-body spawn, LLVM only). What remains is the Rust target: `crates/ignite/tests/fixtures/streams/test_body_spawn.ignite:6` records that Rust clones the end into the task and keeps the test body's clone until the body ends, so the test waits for its timeout (`crates/ignite-rustgen/src/lib.rs:3909`, `Generator::spawn`).

- On the Rust target a pipe end whose last use is a `spawn` argument is moved into the task (the moves analysis, `TypedProgram::moves`), so the test body holds no clone.
- `stream_ownership::` runs `test_body_spawn.ignite` on both targets, and it passes on both without reaching its timeout; the fixture's Rust comment is removed.
- An end used after the `spawn` is still cloned (a case in the same fixture or a rustgen golden).
- `CODEGEN_REVISION` bumped; `task gate` green.

## Out of Scope

- The Rust target, unless the test shows the same failure there.
