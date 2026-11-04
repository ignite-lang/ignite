---
format: aep.planning-md/3
id: story:process-spawn
kind: story
status: draft
title: Start and supervise child processes
summary: Spawn a child with arguments, directory and timeout, capture stdout, stderr and status, kill it
relations:
- decomposes: epic:playground-substrate
scope:
- confidence: inferred
  path: crates/ignite-check/src
- confidence: inferred
  path: crates/ignite-io/src
- confidence: inferred
  path: crates/ignite-rt/src
- confidence: inferred
  path: std
revision: 4
---
# Story: process-spawn

## Outcome

An Ignite program starts a child process with arguments, a working directory and a timeout,
captures its stdout, stderr and exit status, and can kill it, on both targets.

## Context

- Gap P1 of the playground session (2026-11-04): its run button must call `ignite check`, `test`
  and `build`, then the built program.
- Substrate in `crates/ignite-io` (one implementation, both targets; `crates/ignite-rt` adapts it
  over the C ABI), a std module over it, and a new effect the checker tracks.
- Constraint P3 (playground session, 2026-11-04): an `HttpApp` handler may use effects only on its state (`std/protocol/http.ignite:50`; `Clock.monotonic()` in a handler is E017, also in a task it spawns), so this capability arrives as a handle passed in the state, as `Store` does, not as an ambient static.

## Acceptance

- An inline test on both targets runs a child that writes to stdout and stderr and exits 3, and
  reads both outputs and the status 3.
- A child that outlives its timeout is killed and reported as timed out.
- A function that spawns without declaring the effect is rejected by `ignite check` with a
  diagnostic of `docs/slice.md` section 8.

## Out of Scope

- A shell, pipes between children, a pty.
