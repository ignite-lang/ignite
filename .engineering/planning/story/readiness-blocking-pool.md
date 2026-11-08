---
format: aep.planning-md/3
id: story:readiness-blocking-pool
kind: story
status: proposed
title: Name resolution, child start, files and blocking writes leave the workers
relations:
- decomposes: epic:native-parity
- decomposes: story:readiness-io
- depends_on: story:readiness-scheduler
scope:
- confidence: cited
  path: crates/ignite-io/src/child.rs
- confidence: cited
  path: crates/ignite-io/src/fs.rs
- confidence: cited
  path: crates/ignite-io/src/net.rs
- confidence: cited
  path: crates/ignite-io/src/pool.rs
- confidence: cited
  path: crates/ignite-io/src/stdio.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T08:45:33Z", actor: "human:timo", revision: 3}
---
# Story: readiness-blocking-pool

## Outcome

Name resolution, child start (fork, exec), files under a `Directory` and stdout or stderr writes that would block run on a small pool of OS threads for the parked task, so a slow disk, a fork or a resolver never stalls a worker. Phase R2 of `docs/readiness-io.md`.

## Context

- Design: `docs/readiness-io.md` sections 4.4 (the R2 column), 4.7 (blocking pool), 7 (R2), Q7 (`main` on a worker only if connect-8 shows the accept wakeup).
- Builds on `story:readiness-scheduler` (workers and the reactor interface).

## Acceptance

- A pool in `crates/ignite-io/src/pool.rs`: default 4 threads, grows to 64 on demand, a thread lingers 1 s idle; it runs a closure for a parked task and wakes it through the waiter.
- `getaddrinfo` of `connect`, `Launcher.start` and `execute`, every `Directory` call and a stdout or stderr write that would block run on the pool; a test shows a task on the same worker keeps answering while another task's file or child call blocks (a pipe-backed child or a slow fs fixture).
- `docs/readiness-io.md` section 6 row 4: no ratio worse than before R2, numbers in the commit body.
- `task gate` green on both targets, counts and exit in the PR body; the `std/fs.ignite` and `std/process.ignite` inline tests unchanged.

## Scope

- `crates/ignite-io/src/{pool,net,child,fs,stdio}.rs`. Cited from `docs/readiness-io.md` section 8.
