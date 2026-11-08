---
format: aep.planning-md/3
id: story:readiness-scheduler
kind: story
status: implemented
title: Spawned tasks run as stackful tasks on epoll workers
relations:
- decomposes: epic:native-parity
- decomposes: story:readiness-io
- depends_on: story:readiness-waiters
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: crates/ignite-io/src/drain.rs
- confidence: cited
  path: crates/ignite-io/src/net.rs
- confidence: cited
  path: crates/ignite-io/src/reactor.rs
- confidence: cited
  path: crates/ignite-io/src/sched.rs
- confidence: cited
  path: crates/ignite-io/src/task.rs
- confidence: cited
  path: crates/ignite-rt/src/arena.rs
- confidence: cited
  path: crates/ignite-rt/src/net.rs
- confidence: cited
  path: crates/ignite-rt/src/task.rs
- confidence: cited
  path: docs/llvm.md
- confidence: cited
  path: docs/slice.md
- confidence: cited
  path: docs/vision.md
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T08:45:33Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-08T09:43:55Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-08T18:10:59Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: readiness-scheduler

## Outcome

Spawned tasks run as stackful tasks on a few worker threads, each worker a loop around the reactor interface of `story:readiness-waiters` with the epoll implementation; TCP, pipes, channels, sleep and child wait park the task instead of the thread, so the HTTP and Telnet servers' sessions run on readiness and context switches per reply approach the tokio reference. Phase R1 of `docs/readiness-io.md`.

## Context

- Design: `docs/readiness-io.md` sections 3.3, 3.4, 4.1, 4.3 to 4.6, 4.8 to 4.10, 6, 7 (R1), 10 (risks K1, K2, K4).
- Decision: Timo, 2026-11-08 (`docs/vision.md` T8). Neither backend changes its lowering; the reactor stays behind the interface R0 landed, so a tokio-driven reactor can replace epoll later.
- Stack switching through `corosensei` (Q2); worker count `available_parallelism`, `IGNITE_WORKERS` (Q3); `main` stays on its thread.
- `story:spawn-without-handshake` touches the same handshake and lands before or after this story, not beside it.

## Acceptance

- `docs/readiness-io.md` section 6 rows 1, 2 and 3: todo get-64 at most 0.1 context switches per reply, task-clock per reply at most 1.1x plain Rust, throughput at least 0.909 and p99 at most 1.1x (`ignite-load matrix`); row 4: terminal rr-1, rr-64, connect-8 and todo get-1, patch-16 no ratio worse than before; numbers in the commit body, measured with the epic's method (`perf stat`, `strace -c`).
- Row 6: idle terminald runs 2 threads, 3 with one session (`terminal::idle_terminald_runs_one_thread` unchanged).
- Row 8: `task gate` green on both targets; drain, failover, channel and `apply*` suites unchanged; counts and exit in the PR body.
- The epoll loop is reached only through the reactor interface: no `epoll` call outside the reactor's implementation in `crates/ignite-io/src` (grep), and the R0 contract suite still passes for both implementations.
- The texts of section 3.3 changed: `docs/slice.md` §7, `docs/llvm.md` §4, `docs/vision.md` wave D, D2 and D3, the `AGENTS.md` layout rule.

## Scope

- `crates/ignite-io/src/{sched,reactor,net,task,drain}.rs`, `crates/ignite-rt/src/{arena,task,net}.rs`, `docs/slice.md` §7, `docs/llvm.md` §4, `docs/vision.md` wave D and D2/D3, `AGENTS.md` layout rules. Cited from `docs/readiness-io.md` section 8.
