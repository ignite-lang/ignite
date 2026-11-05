---
format: aep.planning-md/3
id: story:llvm-stream-ownership
kind: story
status: draft
title: LLVM streams live as long as on the Rust target
relations:
- informed_by: story:llvm-spawn-pattern-bound-streams
scope:
- confidence: cited
  path: crates/ignite-llvm/src/lower.rs
- confidence: cited
  path: crates/ignite-llvm/src/region.rs
- confidence: cited
  path: crates/ignite-rt/src/net.rs
- confidence: cited
  path: crates/ignite/src/apply/plan.rs
revision: 2
---
# Story: llvm-stream-ownership

## Outcome

On the LLVM target a stream lives as long as on the Rust target: a stream a task passes on to a
second task stays open until the last task using it ends, and a stream never moved into a task is
released at its last use, so its fd and handle-table slot do not outlive it.

## Context

- Found by the LLVM review of 2026-11-05 (origin/main `39e08768`), inferred from code, not run:
  - M1 miscompile: `crates/ignite-llvm/src/lower.rs:4361-4366` releases every stream moved into a
    task when that task returns; `region.rs:268-283` (`moved_to_task`) never moves a parameter.
    `spawn hand_on(server)` then `spawn echo_once(stream)` inside `hand_on`: `hand_on` returns,
    releases the handle, and `echo_once`'s next lookup fails with "was released"; the peer reads
    Eof. The Rust target clones the `Arc` into each task (`crates/ignite-rustgen/src/lib.rs:3857-3871`).
  - L1 leak: `lower.rs:4365` and `:4388` are the only release sites; `crates/ignite-rt/src/net.rs:344-347`
    `close()` shuts down the write half only. Accepting and handling a connection without `spawn`,
    or connecting in a loop, keeps one fd and one table slot per connection.
- Same lowering as `story:llvm-spawn-pattern-bound-streams` (a pattern-bound stream not handed to
  the task); one fix of handle ownership likely covers both (inferred).
- Proposed fix: an owner count per handle-table entry; `spawn_with` registers every stream argument
  of the child and releases it at the child's end; release a fresh stream binding at its last use
  (the moves analysis marks it).

## Acceptance

- Inline test on both targets, from the review's `probe3.ignite`: a stream spawned into a task that
  spawns a second task with it and returns; the client writes `hi` and reads back `hi` (fails on
  LLVM before the fix).
- A test that accepts 2,000 connections on one task without `spawn` keeps the LLVM server's open
  fd count (`/proc/<pid>/fd`) within 16 of its start; `task gate` green.

## Out of Scope

- P1 to P9 of the same review (`story:llvm-lowering-copies`).
