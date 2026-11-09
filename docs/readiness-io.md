# Readiness IO: connections without a blocked thread each

**Status: decided (Timo, 2026-11-08: option A, with the reactor swappable and stackless
lowering kept possible); not implemented.** "Option A" is the choice as it was put to Timo:
section 3.3's stackful tasks over epoll, column B of table 3.1, not its column A.
**Date:** 8 November 2026
**Story:** `story:readiness-io` of `epic:native-parity` (goal set by Timo, 2026-11-08: Ignite
programs within 1.1x of plain Rust).

This document decides. Each section states the chosen design and the alternatives it rejects.
"slice §n" is `docs/slice.md`, "llvm §n" is `docs/llvm.md`, "vision" is `docs/vision.md`.

## 1 The problem, measured

Every task is an OS thread (`crates/ignite-io/src/task.rs`): an HTTP or Telnet server runs
`accept`, then `spawn serve(stream, ..)`, and each session blocks its thread in `read`.

| measurement (epic body, 2026-11-08, LLVM target) | Ignite | plain Rust |
|---|---|---|
| context switches per reply, todo get-64 | 1.13 | 0.024 (tokio reference) |
| task-clock per reply, todo get-64, after wave P1's fixes (expected) | about 1.15x | 1 |
| RSS gap, get-64 | 64 threads x (128 KiB read buffer + spare arena chunk) = about 8 MiB | |

Each reply costs a futex or socket wakeup and a scheduler switch into the session's thread
and out again. The other wave P1 stories remove instructions; they do not remove the switch.

## 2 Goals and non-goals

Goals:

- G1. todo get-64 within 1.1x of plain Rust in task-clock per reply, throughput at least 0.909,
  p99 at most 1.1x; context switches per reply at most 0.1.
- G2. No source change to any Ignite program and no change to the language: `spawn` stays the
  only concurrency form, no `await` (design §8.8).
- G3. One substrate in `crates/ignite-io`; both targets call the same functions (AGENTS.md).
- G4. Every behaviour of slice §7, §16.13 (drain), §21 (capabilities) and §30 (channels) kept,
  each existing test green on both targets.

Non-goals: 10,000 idle actors at tokio's memory (vision wave D) is measured, not promised
(section 10, K3); io_uring; preemption of a task that never waits.

## 3 Options

Two independent choices: how a task suspends, and what tells the substrate that a descriptor
is ready.

### 3.1 Suspension

| | A stackless (recorded intent of slice 5) | B stackful tasks in the substrate |
|---|---|---|
| what suspends | Rust target: every effectful fn becomes `async fn`, each call `.await`; LLVM: every effectful fn a coroutine (`llvm.coro.*`, CoroSplit) | the substrate switches stacks inside its own blocking calls; generated code is unchanged |
| codegen cost | both backends rewritten for effectful fns: ramp/resume/destroy functions, frames on the heap or arena, `Box::pin` for recursion, `Send` bounds on generic `ByteStream` roles, every runtime entry gets a poll form with a waker (llvm §5 ABI doubled) | none; the C ABI of llvm §5 is unchanged |
| failure path | LLVM tasks end by `longjmp` to the task wrapper (llvm §4); a `longjmp` across coroutine frames is undefined | `longjmp` stays within one task's stack: unchanged |
| per-switch cost | a poll return chain | save and restore callee-saved registers, about 10 to 20 ns (inferred) |
| memory per idle task | its frames, hundreds of bytes | the touched pages of its stack, 8 to 32 KiB (inferred; risk K3) |
| size | vision: L (two slices), plus both backends | M (estimate): `ignite-io` and `ignite-rt` only |

### 3.2 Readiness source

| | tokio | hand-written epoll | io_uring |
|---|---|---|---|
| fit with B | tokio drives futures; a stackful task would wrap each park in a future: two schedulers | direct: a worker loop around `epoll_wait` | completion based: buffers owned by the kernel until done; fits B, harder than epoll |
| dependency | tokio, compiled into every Rust-target program build (`ignite-io` is already 2.4 s of 3.2 s of a todod dev build, AGENTS.md slice 11; tokio's share inferred, not measured) | none: `nix` is already a dependency (feature `event` added) | a new crate or raw syscalls |
| syscalls per reply | `epoll_wait` amortised, `read`, `write`, `read` to `EAGAIN` | same | one `io_uring_enter` per batch |
| deployability | everywhere | everywhere | refused by Docker's default seccomp profile and by some hardened kernels (verify per host) |
| files | `spawn_blocking` pool | blocking pool (section 4.7) | true async file IO |

The reference that reaches 0.024 switches per reply is tokio, which is epoll underneath. epoll
is therefore enough for 1.1x; io_uring buys fewer syscalls the goal does not need.

### 3.3 Decision

**B with hand-written epoll**: stackful tasks on a few worker threads, one epoll instance per
worker, all inside `crates/ignite-io`. Stack switching through the `corosensei` crate (it
replaces hand-written context-switch assembly; guard pages, x86_64 and aarch64, used in
production by Wasmer).

This departs from slice 5's recorded intent ("tokio inside `ignite-io`, Rust-target futures,
LLVM-target coroutines", Timo 2026-11-01; vision wave D, D2, D3; AGENTS.md layout rules). It
keeps the rule behind it: the substrate is written once and both targets do IO through the
same functions. It is stronger on one point: both targets also suspend the same way, so "a
target differs only in how it suspends a task" becomes "targets do not differ". Timo took it on
2026-11-08 ("A now - then make both possible", Q1), on the terms of section 3.4; the texts to
change are in section 8, `story:readiness-scheduler`.

### 3.4 Keeping both possible

Two later replacements stay open, each without touching what the other parts depend on, and
the program keeps a choice between two IO modes.

**The reactor is swappable.** Every wait goes through the waiter of section 4.2, and the
scheduler drives readiness through one reactor interface in `crates/ignite-io`
(`story:readiness-waiters` lands it with the epoll implementation and the test double
`ScriptedReactor` under one contract suite, `story:readiness-scheduler` drives its workers
through it). The interface is what a worker needs from a readiness source:

- register a descriptor once with its interest (read, write, urgent, hang-up) and deregister it;
- wait for readiness events until a deadline, returning the waiters to make runnable;
- wake a worker that is waiting, from any thread (today's `eventfd`).

The hand-written epoll loop is the first implementation. A tokio-driven reactor (tokio's
`mio` registry and its timer wheel, driven by the worker loop) would implement the same three
operations; replacing one with the other changes `crates/ignite-io` only, neither backend and
no generated code.

**Stackless lowering stays a later alternative per target** (slice 5's futures and
coroutines, table 3.1 column A). Prerequisite: a failing task ends by returning an error
through its frames to the task wrapper instead of the LLVM target's `longjmp` (llvm §4), which
is undefined across coroutine frames. That change is useful alone (no `setjmp` per task) and is
not scheduled. Stackless lowering is chosen only if the memory cost of stacks is measured to
matter: the touched stack per idle task (8 to 32 KiB, inferred, K3) and the RSS of 10,000 idle
Telnet sessions (section 6 row 7, recorded in R3), against a program that needs that many idle
tasks (vision's 10,000 idle actors, before actors P5).

**The IO mode is a setting, in the long run** (Timo, 2026-11-08). A program chooses its IO mode
as a build or executable setting (for example `ignite build --io threads|readiness`, or a field
of the executable kind; `story:io-mode-setting` decides which), not by language syntax: no
`async` or `await`, so functions are not split into two kinds, and user code reads the same in
both modes (blocking-looking IO, concurrency through `spawn` and channels). `threads` is
today's one OS thread per task; `readiness` is stackful tasks on epoll workers, later possibly a
tokio reactor behind the same seam. Both modes pass the same tests and give the same results;
only cost differs. The waiter seam therefore keeps the thread-per-task implementation as one of
its implementations, not as a stage to remove.

## 4 Design

### 4.1 Workers and tasks

- A **worker** is an OS thread running a loop: run each runnable task until it parks, then
  `epoll_wait` until a descriptor, a timer or a wake is due. Workers start lazily, one per
  `spawn` placement up to `IGNITE_WORKERS` (default `std::thread::available_parallelism`,
  cgroup-aware), and stay.
- A **task** is a stack and a control block (its arena state, section 4.5; its owned handles;
  its parked reason). `spawn` creates a task; `task_end` frees it and returns the stack to the
  worker's pool. As landed (`story:readiness-connect-latency`): each worker keeps two free
  stacks, the rest go to one shared pool of 64; a spawn takes the stack from the cache of the
  worker it places the task on, from `main` too, else from another worker's cache (a worker
  it starts has none), else from the pool, and maps one, without the placement lock held,
  only when none is free. So a connection accepted in `main` takes no pool lock and maps no
  stack while the worker it is placed on holds a free one, or, for a worker it starts, while
  any worker does. A running worker with an empty cache still maps one when only another
  worker caches a free stack (case `scheduler_adversary::a_connection_accepted_in_main_maps_no_stack_while_another_worker_holds_one`,
  a known gap: taking from other workers' caches for running workers raised the pool takes
  of the connect-8 shape from 0 to 2 to 2 to 19 per run).
- `main` stays on the process main thread outside the workers in phase R1: its waits block the
  OS thread, as today. An accept loop in `main` (`TelnetServer`, `HttpServer`) therefore costs
  one wakeup per connection, not per reply. R2 may move `main` onto a worker.
- Scheduling is cooperative: a task runs until it waits. A task that never waits holds its
  worker (Q4).
- **Placement and stealing.** A spawn from a worker runs the child first until
  `spawn_ready` (llvm §4's handshake, kept), then resumes the parent; the child is runnable
  on that worker. A spawn from `main` places the child on the next worker round-robin. An idle
  worker steals runnable tasks from the longest queue. A task migrates only while parked or
  runnable, never while running (section 4.5 makes that safe).

### 4.2 One waiter for every wait

Every place the substrate waits today (`Condvar` in channels and pipes, child
`wait`, task handshake, `Clock.sleep`, a blocking socket call) goes through one primitive:

```rust
// crates/ignite-io/src/wait.rs (target form)
pub struct Waiter { /* the parked thread or task */ }

/// Park the caller until `woken` or `deadline`. On a worker this switches to the
/// worker's loop; on any other thread it blocks the thread (a futex), as today.
pub fn park(waiter: &Waiter, deadline: Option<Instant>) -> Woken;

/// Make a parked waiter runnable; from any thread. A task on another worker is
/// queued there and that worker's eventfd is written once.
pub fn wake(waiter: &Waiter);
```

A wait list (`WaitList`, a `Mutex<VecDeque<Waiter>>`) replaces each `Condvar`. Because a waiter
is either a thread or a task, tasks on workers, `main`, the drain thread, child watcher threads
and the test runner's threads interoperate through the same channels and pipes. Phase R0 lands
this with threads only, so its behaviour change is none and its tests are the existing ones.
Off a worker, a wait for a descriptor is a `poll` on the descriptor and the thread's own
`eventfd`, so `main` can accept on the same non-blocking listener the drain wakes.

### 4.3 The reactor

- Per worker: one epoll instance, one `eventfd` for cross-thread wakes, one timer heap.
- Sockets are non-blocking from creation (`SOCK_NONBLOCK` on `accept4` and `socket`).
- A call that would block registers the descriptor once per stream (`EPOLLIN | EPOLLOUT |
  EPOLLRDHUP | EPOLLPRI`, edge-triggered), records the task as the stream's reader or writer,
  and parks. Readiness wakes it; the call retries.
- Deadlines (`set_idle_timeout`, `connect_within`, `receive_within`, `Clock.sleep`) are timer
  entries; today's `SO_RCVTIMEO` is replaced, `timed_out()` keeps its meaning (slice §26.2).
- As landed in R1: accepted sockets stay blocking and a task's `recv` and `send` pass
  `MSG_DONTWAIT`, so the only task of a worker reads with one blocking `recv` (idle timeout
  `SO_RCVTIMEO`) and its worker counts as busy for placement (slice §7). The lone task's
  `poll` and wake cost more per reply than a blocked thread's `recv` (`rr-1`).

### 4.4 The substrate, call by call

| call | R1 | R2 |
|---|---|---|
| `TcpListener.accept` | `accept4` non-blocking, park on `EPOLLIN` | same |
| `TcpStream.read`, `read_into` | read until `EAGAIN`, park on `EPOLLIN`; the read stops before the urgent byte as today (`SIOCATMARK`) | same |
| `write_all`, `write_urgent` | write until done, park on `EPOLLOUT` between partial writes | same |
| `urgent_pending` | `EPOLLPRI` state, no `poll` syscall (removes the terminal's extra `poll(POLLPRI)` per read, epic "fairness") | same |
| `TcpStream.connect`, `connect_within` | non-blocking `connect`, park on `EPOLLOUT` with the deadline; name resolution (`getaddrinfo`) inline, blocking the worker | resolution on the blocking pool |
| `PipeStream` | wait lists instead of condvars | same |
| channels (slice §30) | wait lists; `receive_within` a timer | same |
| `Clock.sleep` | a timer | same |
| `ChildProcess.wait` | the watcher thread wakes the waiter | same |
| `Launcher.start`, `execute` (fork, exec) | inline, blocking the worker for the fork | blocking pool |
| `Directory` (fs) | inline: short, bounded, blocks the worker only | blocking pool |
| stdin | a stdin reader thread feeds a wait list: an unbounded read must never hold a worker | same |
| stdout, stderr | inline writes | blocking pool when the write would block |
| `Store` | unchanged: its lock is held only around pure callbacks (an effectful closure is E017), so no task parks under it; a contended lock stalls a worker for one critical section | same |
| `Entropy` | inline (`/dev/urandom` does not block once seeded) | same |

"Inline" is correct, not only slow: it blocks one worker for a bounded time and waits on no
other task, so it cannot deadlock. Only stdin is unbounded, so only stdin leaves the workers
in R1.

### 4.5 Per-task state and thread-locals

- The arena state (`STATE`, `TASK` in `crates/ignite-rt/src/arena.rs`) moves from thread-local
  storage into the task's control block. The thread-local keeps one pointer, the running task's
  block, set at each switch. Generated code that caches the state's address is then right after
  a migration, because the address is the task's, not the thread's.
- The other substrate thread-locals are per worker and never live across a park: pending writes
  (`net.rs`; a task already flushes before it waits, slice §30.1, so they are empty at every
  switch), the read scratch buffer, report and callback guards. A park inside a `with` closure
  of a thread-local is a bug; `debug_assertions` builds check that none is borrowed at `park`.
- glibc's per-thread malloc arenas stay correct under migration (they are a cache).

### 4.6 Stacks

- Each task stack is reserved as today's thread stack is (2 MiB, `mmap` with `MAP_NORESERVE`
  and a guard page), so recursion depth does not change. RSS is the touched pages.
- A worker keeps a pool of free stacks; a stack returned with more than 64 KiB touched is
  `madvise(MADV_DONTNEED)`'d above 64 KiB before reuse.
- Overflow: the substrate's `SIGSEGV` handler (on `sigaltstack`, chained to Rust's) recognises
  a task's guard page and prints Rust's `has overflowed its stack` line before aborting, as a
  thread overflow does today.

### 4.7 Blocking pool (R2)

A small pool of OS threads (default 4, grows to 64 on demand, lingers 1 s as `task.rs` workers
do) runs a closure for a parked task and wakes it. Used by name resolution, fork/exec, fs and
stdout writes that would block.

### 4.8 Drain and `at_exit`

- The drain thread stays an OS thread with `sigwait` (slice §16.13). `begin` marks each
  listener draining, shuts it down and wakes its parked acceptor through the reactor; `accept`
  then fails with `draining`, as now.
- `wait_for_tasks` counts tasks as today (`LIVE`): incremented at `spawn`, decremented at task
  end, on any worker.
- `at_exit` hooks (kill live children) are unchanged: they run on the thread that calls
  `exit`, after or without the workers.

### 4.9 Test runner

The runner process is unchanged: it starts each test as a `--run-one` child from its own OS
threads (slice §12.7). Inside the child the test body is the root, as `main` is, and its spawns
run on workers. Timeouts still kill the child.

### 4.10 Codegen

None on either target. The Rust target keeps calling `ignite_io::spawn` and the blocking
methods; the LLVM target keeps calling `ignite_rt_spawn`, `spawn_ready`, `task_end` and the
stream entries of llvm §5. The only generated-code requirement is section 4.5's: no cached
copy of a state field across a runtime call, which holds today because a runtime call clobbers
memory for LLVM.

### 4.11 Memory

- The read scratch buffer (`READ_BUFFER`, 64 KiB kept, `crates/ignite-rt/src/net.rs`) becomes
  per worker: a read happens only when the descriptor is readable, then the bytes are copied
  into the task's arena. 64 sessions share `IGNITE_WORKERS` buffers instead of 64.
- The spare arena chunk a thread keeps for its next task (llvm §4) becomes a per-worker pool.
- Expected get-64 RSS: workers x (64 KiB + one chunk) + 64 x touched stack (inferred, to be
  measured in R3).

## 5 What a program sees

Nothing changes in source. The HTTP accept loop of `std/protocol/http.ignite` stays:

```ignite
let stream = self.listener.accept()?
spawn Http.serve(stream, self.limits, self.application)
```

Its `serve` blocks in `stream.read(..)` as before; the substrate parks the task instead of the
thread. Observable differences: fewer threads (`/proc/<pid>/task` holds workers, not
sessions), and a task that loops without waiting delays the other tasks of its worker (Q4).

## 6 Acceptance

Measured with the epic's method (`perf stat` per reply against plain Rust, `strace -c`) before
and after each phase, numbers in the commit body:

| # | measurement | target |
|---|---|---|
| 1 | todo get-64, context switches per reply | at most 0.1 |
| 2 | todo get-64, task-clock per reply / plain Rust | at most 1.1 |
| 3 | todo get-64 throughput / plain Rust (`ignite-load matrix`) | at least 0.909; p99 at most 1.1x |
| 4 | terminal rr-1, rr-64, connect-8; todo get-1, patch-16 | no ratio worse than before the phase |
| 5 | get-64 peak RSS / plain Rust | at most 1.1x (after R3) |
| 6 | idle terminald threads | main and drain (2), unchanged; with one session 3, unchanged (`terminal::idle_terminald_runs_one_thread`) |
| 7 | 10,000 idle Telnet sessions | RSS recorded, no target (risk K3) |
| 8 | `task gate` | green on both targets; drain, failover, channel and `apply*` suites unchanged |

## 7 Phases

| phase | content | useful alone because |
|---|---|---|
| R0 | the waiter and wait lists (4.2) replace every `Condvar` wait in `ignite-io`; threads only | no behaviour change; every later phase only adds a waiter kind |
| R1 | workers, stackful tasks, epoll, timers (4.1, 4.3, 4.5, 4.6); TCP, pipes, channels, sleep, child wait on the reactor; stdin reader thread; everything else inline; `main` on its thread | the HTTP and Telnet servers' sessions run on readiness: G1 is measured here |
| R2 | blocking pool (4.7); name resolution, fork/exec, fs, blocking stdout off the workers; optionally `main` on a worker | no worker stall on a slow disk or a fork |
| R3 | per-worker read buffer, stack pool trimming, per-worker spare chunks (4.11) | the RSS cells of the epic |

## 8 Draft stories

| id | title | phase | surfaces |
|---|---|---|---|
| `story:readiness-waiters` | Every substrate wait goes through one waiter that parks a thread or a task | R0 | `crates/ignite-io/src/{wait,channel,pipe,child,task}.rs` |
| `story:readiness-scheduler` | Spawned tasks run as stackful tasks on epoll workers | R1 | `crates/ignite-io/src/{sched,reactor,net,task,drain}.rs`, `crates/ignite-rt/src/{arena,task,net}.rs`, `docs/slice.md` §7, `docs/llvm.md` §4, `docs/vision.md` wave D and D2/D3, AGENTS.md layout rules (the decision of 3.3) |
| `story:readiness-blocking-pool` | Name resolution, child start, files and blocking writes leave the workers | R2 | `crates/ignite-io/src/{pool,net,child,fs,stdio}.rs` |
| `story:readiness-memory` | Per-worker read buffers and stack pools close the RSS gap | R3 | `crates/ignite-rt/src/{net,arena}.rs`, `crates/ignite-io/src/sched.rs` |

`story:readiness-io` stays the umbrella whose acceptance is section 6 rows 1 to 3;
`story:spawn-without-handshake` touches the same handshake (4.1) and lands before or after R1,
not beside it.

## 9 Open questions

| # | question | recommended default |
|---|---|---|
| Q1 | Replace slice 5's "tokio, futures, coroutines" with stackful tasks over epoll (3.3)? | answered yes (Timo, 2026-11-08): stackful tasks over epoll now, the reactor behind one interface so tokio can replace epoll, stackless lowering kept possible behind failure by return (3.4) |
| Q2 | `corosensei` or hand-written context switch? | `corosensei`; replace only if it blocks musl `--static` (slice §23) or shows in a profile |
| Q3 | Worker count | `available_parallelism`, override `IGNITE_WORKERS`; no command-line flag (the program owns its flags) |
| Q4 | A task that never waits starves its worker's queue | idle workers steal runnable tasks (4.1); add `Task.yield_now` (vision §5) only when a program needs it |
| Q5 | Task stack reservation | 2 MiB as today's threads; revisit with R3's 10,000-session number |
| Q6 | io_uring later? | no plan; revisit for files only if a profile shows fs on the blocking pool |
| Q7 | `main` on a worker (R2)? | only if a measurement of connect-8 shows the accept wakeup |

## 10 Risks

- K1. A thread-local borrowed across a park after migration corrupts state silently. Mitigation:
  4.5's rule and its debug check; stealing can be switched off (`IGNITE_STEAL=0`) to bisect.
- K2. Cooperative scheduling changes p99 under mixed load (a CPU-heavy request delays its
  worker's other sessions). Mitigation: stealing; row 4 of section 6 includes patch-16's p99.
- K3. Stack RSS per idle task (8 to 32 KiB, inferred) is about 50 times a stackless frame. At
  10,000 idle sessions that is 80 to 320 MiB. Vision's 10,000 idle actors would be measured
  against this before actors P5.
- K4. Edge-triggered epoll loses a wakeup if a call stops reading before `EAGAIN`. Mitigation:
  a stream keeps a `maybe_readable` flag, cleared only by `EAGAIN`.
