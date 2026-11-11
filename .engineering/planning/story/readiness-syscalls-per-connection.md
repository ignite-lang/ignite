---
format: aep.planning-md/3
id: story:readiness-syscalls-per-connection
kind: story
status: active
title: A connection costs no more system calls than at v0.15.0, and a silent session blocks no worker
scope:
- confidence: cited
  path: crates/ignite-io/src/idle.rs
- confidence: cited
  path: crates/ignite-io/src/net.rs
- confidence: cited
  path: crates/ignite-io/src/reactor.rs
- confidence: cited
  path: crates/ignite-io/src/sched.rs
- confidence: inferred
  path: crates/ignite-io/tests
- confidence: inferred
  path: docs/slice.md
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T00:24:52Z", actor: "human:timo", revision: 4, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T00:24:53Z", actor: "human:timo", revision: 5, executor: "agent:claude-coordinator", correlation: "wave-11"}
---
# Story: a connection costs no more system calls than at v0.15.0, and a silent session blocks no worker

## Outcome

Rows 3.7 and 3.3 `terminal/connect-8` (both targets) and 3.3 `llvm/terminal/rr-64` are at or
better than v0.15.0 on the dedicated host.

## Why

Dedicated-host run `iq-20261110t211023z-863a` (HEAD `4520b8cf`, BASE v0.15.0): 3.7 connect-8 p99
llvm 1.111x -> 1.467x, rust 1.052x -> 1.387x; 3.3 connect-8 llvm 0.930x -> 0.874x, rust 0.955x ->
0.899x; 3.3 llvm/terminal/rr-64 0.975x -> 0.871x.

Diagnosis, counts only (`.engineering/waves/2026-11-11-wave-11-diagnosis/connect.md`, `rss-rr.md`):

- connect: the serving side makes about 5.8 more system calls per connection than v0.15.0, all in
  the readiness path (`strace -f -c`, 8 clients x 25: llvm 14.49 -> 20.68, rust 13.62 -> 19.43):
  two failed `recvfrom`, an `epoll_wait` that returns only EPOLLOUT, an `EPOLL_CTL_DEL` before
  `close`, a failed `recvfrom` after a hand-over wake, three avoidable `setsockopt`. Inferred: 8
  connections on 4 workers share workers, so one session's wasted calls are its neighbour's wait.
- rr-64: a session that has not yet sent anything blocks the worker it is alone on in `recv`
  (`sched::block_alone` from `TcpStream::read_with`); spawns pass blocked workers, so a burst of
  64 connections lands 16/16/16/16, 21/21/21/1, 31/31/1/1 or 61/1/1/1 from round to round.
- Stack allocation, cross-thread wakes, locks and work stealing are ruled out by counts.

Eight patches were tried in a copy and pass the `ignite-io` tests; they are a starting point, not
the change: `fix-1` to `fix-7` (connect) and `B-first-read-does-not-block-its-worker` (rr-64).

## Acceptance

- A test in `ignite-io` counts the system calls of the serving side for a fixed number of
  sequential and concurrent connections (or counts the reactor's operations through its scripted
  double) and fails above the v0.15.0 count per connection: 13.62 on the Rust target with 8
  clients. Red on the wave base.
- A test opens a burst of connections that send nothing yet and fails when they are not spread
  evenly over the workers. Red on the wave base ([1, 1, 1, 13] in the diagnosis).
- No `unsafe` site is added (row 3.6).
- The idle timeout, urgent data, the exit flush and the drain behave as before: the existing
  `ignite-io` unit and integration tests pass unchanged.
- The dedicated-host run of the release candidate shows the five rows at or better than BASE.
  The coordinator reads this; the unit records its cost as "not measured locally".

## Scope

- `crates/ignite-io/src/net.rs`, `sched.rs`, `reactor.rs`, `idle.rs`, their tests. Cited (the
  patches).
- `docs/slice.md` where it states the readiness path. Inferred.
