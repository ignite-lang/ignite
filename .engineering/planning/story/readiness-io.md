---
format: aep.planning-md/3
id: story:readiness-io
kind: story
status: active
title: Connections served by readiness IO instead of a blocked thread each
relations:
- decomposes: epic:native-parity
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:59:06Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T06:03:41Z", actor: "human:timo", revision: 3}
---
# Story: readiness-io

## Outcome

The substrate serves connections from a small pool of worker threads driven by epoll instead of one blocked OS thread per connection, so context switches per reply approach tokio's.

## Decision

Timo, 2026-11-08: "A now - then make both possible" (`docs/readiness-io.md` status, sections 3.3 and 3.4; `docs/vision.md` T8; PR 121).

- Taken: stackful tasks on a few worker threads, one epoll loop per worker, inside `crates/ignite-io`; neither backend changes its lowering. Replaces slice 5's intent (tokio inside `ignite-io`, Rust-target futures, LLVM-target coroutines).
- Kept possible: (1) the reactor sits behind one interface in `crates/ignite-io` (the R0 waiter plus the reactor trait), so a tokio-driven reactor can replace the epoll loop without touching either backend; (2) stackless coroutine lowering per target stays a later alternative, gated on task failure ending by return instead of `longjmp`, and chosen only if stack memory per idle task (8 to 32 KiB, design estimate) is measured to matter (R3's number).
- Long run (Timo, 2026-11-08): the program chooses its IO mode, `threads` or `readiness`, as a build or executable setting, not by syntax (no `async`/`await`); both modes pass the same tests with the same results (`story:io-mode-setting`).

## Context

- Epic epic:native-parity (diagnosis and evidence in its body). Design: `docs/readiness-io.md`.
- Phases: `story:readiness-waiters` (R0) <- `story:readiness-scheduler` (R1) <- `story:readiness-blocking-pool` (R2) and `story:readiness-memory` (R3); `story:io-mode-setting` after R1. `story:spawn-without-handshake` lands before or after R1, not beside it.

## Acceptance

- `docs/readiness-io.md` section 6 rows 1 to 3: todo get-64 at most 0.1 context switches per reply, task-clock per reply within 1.1x of plain Rust, throughput at least 0.909 and p99 at most 1.1x.
- Measured before and after locally with `perf stat` cost per reply against plain Rust (the method in the epic); the numbers go in the commit body; `task gate` green.

## Scope

- Through its phase stories: `crates/ignite-io`, `crates/ignite-rt`, `docs/slice.md`, `docs/llvm.md`, `docs/vision.md`, `AGENTS.md`. No backend lowering changes. Cited from `docs/readiness-io.md` section 8.
