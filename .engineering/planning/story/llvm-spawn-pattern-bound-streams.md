---
format: aep.planning-md/3
id: story:llvm-spawn-pattern-bound-streams
kind: story
status: draft
title: LLVM spawn hands pattern-bound streams to the task
summary: A failing task closes a stream bound by let (a, b) = ...; spawn in a test body behaves as in a helper
scope:
- confidence: inferred
  path: crates/ignite-llvm/src
revision: 2
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

- An inline test on both targets: a task spawned with a pattern-bound pipe end fails, and the
  other end reads `Eof`; the same test with the spawn in the test body passes on both targets;
  `task gate` green.

## Out of Scope

- The Rust target, unless the test shows the same failure there.
