---
format: aep.planning-md/3
id: story:io-mode-setting
kind: story
status: draft
title: A program chooses threads or readiness IO as a build or executable setting
relations:
- decomposes: epic:native-parity
- depends_on: story:readiness-scheduler
scope:
- confidence: inferred
  path: crates/ignite-io/src/lib.rs
- confidence: inferred
  path: crates/ignite/src
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std
revision: 2
---
# Story: io-mode-setting

## Outcome

A program chooses its IO mode as a build or executable setting, `threads` (one OS thread per task, today's substrate) or `readiness` (stackful tasks on epoll workers, later possibly a tokio reactor behind the same seam), not by language syntax: no `async` or `await`, functions are not split into two kinds, and user code reads the same in both modes (blocking-looking IO, concurrency through `spawn` and channels). Both modes pass the same tests and give the same results; only cost differs.

## Context

- Timo, 2026-11-08: "we give the lang user the option to use either async or non async IO? - on the long run"; recorded in `docs/readiness-io.md` section 3.4 and `docs/vision.md` T8.
- Builds on `story:readiness-waiters` (the waiter seam keeps the thread-per-task implementation) and `story:readiness-scheduler` (the readiness mode).
- Which surface carries the setting (a flag such as `ignite build --io threads|readiness`, a field of the executable kind, or both, and which wins) is open and decided in this story's design.

## Acceptance

- The setting exists on the chosen surface, documented in `docs/slice.md`; the default is stated there.
- `task test-ignite` and the end-to-end suites of `crates/ignite/tests` (terminal, drain, failover, channels, todo) run in both modes on both targets with identical results; `task gate` covers both modes, counts and exit in the PR body.
- No program source changes between modes: one fixture builds in each and its transcript is byte-identical.
- The cost of each mode recorded once (todo get-64 context switches, task-clock and RSS per mode) in the commit body.

## Scope

- `crates/ignite-io` (mode selection at `run`), `crates/ignite/src` (the build flag or the kind field's reading), `std/` executable kinds if a field is chosen, `docs/slice.md`. Inferred; no design yet.
