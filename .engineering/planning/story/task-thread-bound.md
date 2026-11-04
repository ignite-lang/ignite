---
format: aep.planning-md/3
id: story:task-thread-bound
kind: story
status: draft
title: Bound the OS threads of tasks
summary: A program bounds its task threads or waits on several streams from one task, so N connections stop costing N OS threads
scope:
- confidence: inferred
  path: crates/ignite-io/src
- confidence: inferred
  path: crates/ignite-rt/src
- confidence: inferred
  path: std
revision: 2
---
# Story: task-thread-bound

## Outcome

A program can bound the OS threads its tasks run on, or wait on several streams from one task, so N
open connections no longer cost N OS threads on either target.

## Context

- Row E9 found by the playground session (2026-11-04), `examples/friction.md` in the sibling
  examples repository (ember).
- Each `spawn` runs its task on its own OS thread for the task's whole life; only idle workers are
  reused (`crates/ignite-io/src/task.rs:76` `spawn`, `:91` `try_spawn`; `docs/slice.md` section 7).
  The LLVM runtime's spawn calls the same `try_spawn` (`crates/ignite-rt/src/task.rs:48`).
- No pool size, environment variable, compiler flag or `system` setting bounds the thread count.
- No readiness wait over several streams exists: `TcpStream` has a blocking `read` and
  `set_idle_timeout` only, so a program cannot spread connections over fewer tasks by hand.
- Measured by the playground session: emberd on both targets runs one thread per open connection
  plus 2 (3 threads at 1 client, 18 at 16); emberload does the same per client. A handwritten Rust
  baseline does 6.0M ops/s with 1 thread and 16 pipelined clients, 3.3M with one thread per
  connection; the LLVM build of emberd does 0.74M (`results/2026-11-04T02-03-44Z.json` in the
  examples repository).
- Related: `AGENTS.md` layout rules plan an async substrate (tokio inside `crates/ignite-io`,
  Rust futures and LLVM coroutines on it); that change would also remove the thread per task.
  This story records the gap; the contract decides between a bounded pool, a multi-stream wait,
  and the async substrate.

## Acceptance

- emberd with 16 open clients runs on at most a configured number of OS threads on both targets,
  read from `/proc/<pid>/task`, with every client still served.
- The same program's throughput with 16 pipelined clients is recorded before and after on both
  targets in `docs/performance/`.

## Out of Scope

- Typed actors and supervision (`docs/actors.md`).
