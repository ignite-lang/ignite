---
format: aep.planning-md/3
id: story:readiness-task-stealing
kind: story
status: draft
title: A worker without a runnable task takes one from another worker
relations:
- informed_by: story:readiness-session-cost
- informed_by: story:readiness-scheduler
revision: 1
---
# Story: a worker without a runnable task takes one from another worker

## Outcome

A runnable task that waits behind another task on its worker is run by a worker that has none (`docs/readiness-io.md` 4.1 "Placement and stealing", 4.5, risk K2), and the p99 latency of `terminal/connect-8` on the dedicated host is at or below v0.15.0's on both targets.

## Why

Row 3.7, `terminal/connect-8`, is the one latency cell that the readiness scheduler of waves 9 and 10 made worse than one thread per connection. Six runs on the dedicated host (`ec2:c7i.metal-24xl`, 2026-11-11, 5 benchmark passes each, medians, LLVM / Rust target; the server runs on 4 CPUs, 8 clients connect, ask once and close):

| run | what the head is | p50 µs | p99 µs | p99.9 µs |
|---|---|---|---|---|
| `w11-session` base | v0.15.0, one thread per connection | 79.2 / 77.0 | 135.9 / 129.0 | 168.4 / 156.7 |
| `w11-session` head | readiness, 4 workers (`ccffba40`) | 76.5 / 73.2 | 157.2 / 148.0 | 217.7 / 200.9 |
| `x-threads` | the same head with one thread per task | 80.7 / 77.2 | 135.4 / 126.5 | 164.0 / 155.9 |
| `x-workers` | the same head with 8 workers | 85.8 / 81.5 | 138.6 / 131.1 | 167.1 / 159.4 |
| `x-sleep` | a spawn prefers a sleeping worker | 79 / 76 | 157 / 149 | not read |
| `x-sleep1` | a worker that runs counts as one task more | 79 / 76 | 157 / 147 | not read |
| `x-steal` | an idle worker takes an unstarted task | 78 / 75 | 162 / 152 | not read |

- The thread mode of the same head gives v0.15.0's values: the scheduler is the cause, nothing else of the wave.
- With one worker per client (8 workers) the tail is the thread mode's; that variant is no fix, because 8 workers on 4 CPUs give the 64-connection cells a p99 of 3,065 µs (130 µs with 4 workers).
- Where a session is placed when it starts does not move the tail (three variants), and moving sessions that have not started does not either.
- Inferred from these: the tail is a started session whose socket became readable while its worker runs another session and a different worker has nothing to run. The design names this as risk K2 with stealing as its mitigation; R1 was built without stealing.
- The first-session and per-session system calls were brought to v0.15.0's by `story:readiness-session-cost` and moved the p99 by nothing (157 µs before and after).

## Acceptance

- A worker with no runnable task takes a runnable task from the worker with the longest queue; a task moves only while it is runnable or parked, never while it runs. Held by tests of `ignite-io` that fail without the move: a task made runnable on a worker that is held busy by another task runs on a second worker within the test's bound; a task that moved waits for its descriptor, its timer and its channel on the worker it moved to; a moved task's pending writes, arena and failure handler are its own (LLVM target) after the move.
- A debug build fails a test in which a thread-local of the substrate is borrowed across a park (`docs/readiness-io.md` 4.5, risk K1). `IGNITE_STEAL=0` switches the move off.
- Dedicated host: 3.7 `terminal/connect-8` on both targets not worse than v0.15.0 (135.9 and 129.0 µs in run `w11-session`); no cell of 3.3, 3.7 and 3.8 worse than at the release before.
- Instructions and system calls per session and per request round (the counts of `story:readiness-session-cost`) do not rise for a session that never moves.
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `crates/ignite-io/src/sched.rs` (the run queue of a worker is thread-local today; a task holds its worker for its life; timers and descriptor registrations are per worker), `crates/ignite-io/src/wait.rs`, `crates/ignite-io/src/net.rs`, `crates/ignite-rt/src/arena.rs` and `process.rs` (per-task state on the LLVM target), `docs/readiness-io.md`. Inferred.

## Out of scope

- `main` on a worker (`docs/readiness-io.md` Q7).
- More workers than CPUs.
