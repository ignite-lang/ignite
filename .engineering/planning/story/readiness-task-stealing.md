---
format: aep.planning-md/3
id: story:readiness-task-stealing
kind: story
status: draft
title: A session whose socket becomes readable while its worker is busy is run by a worker that has nothing to run
relations:
- informed_by: story:readiness-session-cost
- informed_by: story:readiness-scheduler
scope:
- confidence: inferred
  path: crates/ignite-io/src/reactor.rs
- confidence: cited
  path: crates/ignite-io/src/report.rs
- confidence: cited
  path: crates/ignite-io/src/sched.rs
- confidence: cited
  path: crates/ignite-io/src/wait.rs
- confidence: inferred
  path: crates/ignite-io/tests/scheduler.rs
- confidence: inferred
  path: crates/ignite-llvm/src/glue.rs
- confidence: inferred
  path: crates/ignite-llvm/src/ir.rs
- confidence: cited
  path: crates/ignite-rt/src/arena.rs
- confidence: cited
  path: crates/ignite-rt/src/net.rs
- confidence: inferred
  path: crates/ignite-rt/src/task.rs
- confidence: inferred
  path: crates/ignite-rt/tests
- confidence: cited
  path: docs/readiness-io.md
revision: 6
---
# Story: a session whose socket becomes readable while its worker is busy is run by a worker that has nothing to run

## Outcome

A task that is parked in its first read on a worker that runs another task is resumed by a worker that has none when the peer writes (`docs/readiness-io.md` 4.1 "Placement and stealing", 4.5, risk K2), and the p99 latency of `terminal/connect-8` on the dedicated host is at or below v0.15.0's on both targets.

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
- Read from the code on 2026-11-11 (a scoping pass, nothing run): each worker has its own epoll (`crates/ignite-io/src/sched.rs:268`, `:350`); a parked task's descriptor is registered with its worker's reactor only (`sched.rs:1481`); a worker reads its reactor only when its run queue is empty or after 128 resumes (`sched.rs:1214`, `:1247`). A session is bound to a worker when it starts, before its request has arrived. The event of its request is seen by no worker until the bound worker's running task parks or ends, and the task is parked, in no run queue: taking tasks from another worker's run queue cannot find it. No counter says how often this happens on the host.
- The first-session and per-session system calls were brought to v0.15.0's by `story:readiness-session-cost` and moved the p99 by nothing (157 µs before and after).

## Design the scoping pass recommends

A shared reactor instance for a task's first park in a read. The worker that owns the task gives it up on its own thread at that park (swaps its locals out, forgets its timer, frees its slot, lowers its counts, puts the coroutine into the task); the worker whose wait returns the event adopts it (slot, worker, run queue). A task moves at most once and no thread reaches into another worker's slots, timers or thread-locals. Later parks stay on the adopting worker's own reactor. Open: how sleeping workers wait on both instances (nesting the shared descriptor in every worker's epoll wakes every sleeper per event; one watcher needs a hand-off when it turns busy); it needs a host run. One shared epoll for every wake was read as larger and as a risk for the 64-connection cells.

What breaks today when a task resumes on another thread (risk K1), each with its site:

- `READ_BUFFER` is borrowed across the read (`crates/ignite-rt/src/net.rs:687`).
- Generated LLVM code caches the address of the arena state: `ignite_rt_arena_state` is declared `memory(none) speculatable` (`crates/ignite-llvm/src/ir.rs:26`, `glue.rs:51`, `crates/ignite-rt/src/arena.rs:609`).
- `CATCHING` is counted up on one thread and down on another (`crates/ignite-io/src/report.rs:190`).
- `wait::Lock` keeps a `MutexGuard` across parks (`crates/ignite-io/src/net.rs:771`, `:376`, `:416`, `pipe.rs:268`, `:295`, `tls.rs:196`; `wait.rs:455`).
- `park_from` indexes the thread-local `TIMERS` with the task's slot on both sides of the suspend (`sched.rs:454`, `:458`).
- corosensei's `Coroutine` is `!Send`; the hand-over needs its own proof that a parked coroutine has one taker.

## Acceptance

- A task parked in a read on a worker that another task holds busy returns from that read on a second worker within the test's bound after the peer writes; the readiness event, not a `wake`, makes it runnable. Held by a test of `ignite-io` that fails without the move. A task that moved waits for its descriptor, its timer and its channel on the worker it moved to.
- On a moved task, one test each, red without its fix: an allocation before and after the move in one generated function (LLVM target); a panic's report; a read lock released after the move; the read buffer. A debug build fails a test in which a thread-local of the substrate is borrowed across a park (`docs/readiness-io.md` 4.5, risk K1). `IGNITE_STEAL=0` switches the move off.
- Dedicated host: 3.7 `terminal/connect-8` on both targets not worse than v0.15.0 (135.9 and 129.0 µs in run `w11-session`); no cell of 3.3, 3.7 and 3.8 worse than at the release before, the 64-connection cells included. The host run is made as soon as the first test is green, before the K1 work is complete, because it decides whether the design stands.
- Instructions and system calls per session and per request round (the counts of `story:readiness-session-cost`) do not rise for a session that is alone on its worker; for a session that shares a worker the story states the added calls (expected: one `epoll_ctl` and one swap of locals).
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

Derived 2026-11-11 by a scoping pass. Every line is cited (read from the tree) or inferred.

- Primary surface: `crates/ignite-io/src/sched.rs` (`Task`, `Local`, `Worker::resume`, `Worker::start`, `enqueue`, `park_from`, `TIMERS`, `fire_timers`, `work`, `Readiness::wait`, `wait_alone`). Cited.
- `crates/ignite-io/src/wait.rs` (`wait::Lock`), `crates/ignite-io/src/report.rs` (`catch`), `crates/ignite-rt/src/net.rs` (`read_through_buffer`), `crates/ignite-rt/src/arena.rs` (`TaskArena::swap`, `ignite_rt_arena_state`), `docs/readiness-io.md` 4.1, 4.3, 4.5, K1, K2. Cited.
- `crates/ignite-io/src/reactor.rs` (a shared instance, a wait for one event), `crates/ignite-rt/src/task.rs`, `crates/ignite-llvm/src/ir.rs` and `glue.rs` (only if the arena-state contract changes form), `crates/ignite-io/tests/scheduler.rs`, `crates/ignite-rt/tests`. Inferred.
- Collides with any unit that touches `sched.rs`, `reactor.rs`, `wait::Lock`, the task-locals contract or the `ignite_rt_arena_state` contract.

## Not established

- Kernel behaviour for a shared or nested epoll (which sleepers wake; the limits of `EPOLLEXCLUSIVE`).
- Whether a task can park while unwinding (`Pending::drop` at `net.rs:271` into `send_all`), which would split std's per-thread panic count.
- Whether a `setjmp` buffer in the failure handlers is valid on another thread.
- What a per-task arena state address costs per request round in instructions (LLVM).

## Out of scope

- `main` on a worker (`docs/readiness-io.md` Q7).
- More workers than CPUs.
- Moving a task at any park other than its first read.

## Measured 2026-11-12: refuted as built

The design of this draft was built as a prototype (branch `measure/x-move`, `19fecaeb`, one commit on `1e16dfba`: a task's first read park waits on a shared epoll instance that every worker registers in its own epoll; the worker that gets the event adopts the task; a task moves at most once; `IGNITE_STEAL=0` switches it off) and measured on the dedicated host against v0.16.0 (run `iq-20261112t004412z-1b07`, c7i.metal-24xl, 5 passes, 0.6810 USD). It is refuted as built: every cell it was meant to help is worse, and so are the cells at 16 and 64 connections.

Row 3.7, p99 against plain Rust, lower is better (v0.16.0 → prototype):

- `terminal/connect-8`: LLVM 1.307 → 1.518, Rust target 1.216 → 1.318.
- `terminal/rr-64`: LLVM 0.393 → 0.629, Rust target 0.402 → 0.624.
- `todo/get-64`: LLVM 0.496 → 0.909, Rust target 0.551 → 0.991 (passes from 0.629 to 1.858).
- `todo/patch-16`: LLVM 0.571 → 1.107, Rust target 0.747 → 0.966.
- one connection (`rr-1`, `get-1`): unchanged within 0.03.

Row 3.3, throughput against plain Rust, higher is better:

- `terminal/connect-8`: LLVM 0.913 → 0.829, Rust target 0.958 → 0.899.
- `todo/patch-16`: LLVM 1.198 → 0.904, Rust target 1.189 → 0.667.
- `terminal/rr-64`: LLVM 1.251 → 1.198, Rust target 1.218 → 1.124; `todo/get-64`: 1.290 → 1.238, 1.177 → 1.109.

Rows 3.8 (peak RSS) and 6.x (build time) moved by at most 0.07 and 0.013.

What the prototype established beside the timing:

- The sites that keep per-thread state across a park (the read buffer, the arena state address the generated LLVM caches, the open `catch` count, the timer slot, `wait::Lock` guards) can each be made correct for a task that changes threads; each has a test that fails without its fix. All thread-locals a task reaches go through a non-inlined accessor.
- The move needs one `unsafe impl Send` for the carried coroutine (row 3.6: Rust target 12 → 13, LLVM 79 → 80).
- A soak of 18,000 sessions on four servers was correct with the move on and off; about 2950 of 4500 sessions were given up and adopted per server.
- Counted in the soak: every sleeping worker wakes for a shared event (`shared_polls_empty` 184 on the Rust target's terminal server, 3012 on the LLVM target's), and while any task is adrift every worker gives up its lone-wait and blocking-read paths.

Not established: whether one watcher of the shared instance (handed over with `EPOLL_CTL_DEL`/`ADD`) instead of all workers changes the result; which of the two counted effects carries the loss.

The story's premise (the first read of a new session waits behind its worker's run queue, and moving it to an idle worker shortens the p99 of `connect-8`) is not supported by this run. Before another design, the p99 of `connect-8` needs to be located by measurement on the dedicated host (where the reply's time goes between the client's send and the server's read), not inferred from counts.
