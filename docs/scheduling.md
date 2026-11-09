# Scheduling: who chooses how tasks run

**Status: decided: option C (Timo, 2026-11-09); not implemented.**
**Date:** 9 November 2026
**Related:** `docs/readiness-io.md` (the readiness scheduler, 3.3 and 3.4), `docs/vision.md` T8
and T9, `story:io-mode-setting`, `story:readiness-blocking-pool` (wave 10),
`story:task-thread-bound`, `docs/actors.md` section 8.

"slice §n" is `docs/slice.md`, "readiness §n" is `docs/readiness-io.md`. Code is cited by symbol
and file (vision T3). Costs marked "inferred" are not measured: benchmarks run only on the
dedicated host (AGENTS.md).

## 1 The question and the decision

Every process has one scheduler today: stackful tasks on epoll workers, or one OS thread per
task when `IGNITE_IO=threads`. Timo asked whether programs should control scheduling (raw OS
threads when wanted, a sane default, several schedulers or one per server, later other models),
what other languages default to and how much control they give, or whether Ignite keeps one
way and stays simple.

Decision, option C of section 4: **one default scheduler, no async colouring, a raw OS-thread
escape hatch, and a scheduling property per executable component, declared in the system and
shown by `ignite plan`.** Options A, B and D were considered; section 4 keeps each with its
reasons, section 5 the reasons for C, section 6 the stories in order, section 7 the questions
those stories answer.

## 2 Today

| what | where |
|---|---|
| Two IO modes, `Threads` and `Readiness` | `IoMode` in `crates/ignite-io/src/sched.rs` |
| The mode is read once from `IGNITE_IO`; anything but `threads` is `readiness` | `io_mode` in `sched.rs`; slice §7 "IO modes" |
| Worker count: `IGNITE_WORKERS`, else `available_parallelism` (CPU affinity and cgroup quota), at least 1 | `worker_limit` in `sched.rs` |
| Task stack 2 MiB reserved, guard page, pooled | `TASK_STACK`, `STACK_POOL` in `sched.rs` |
| A worker runs at most 128 tasks before it polls without waiting | `BUDGET` in `sched.rs` |
| `try_spawn` picks the mode: `sched::spawn` or a thread per task | `try_spawn` in `crates/ignite-io/src/task.rs` |
| Placement: stay on the spawner's worker while it is at most one task busier than the least loaded, else an idle worker, a new one, the least loaded; no stealing, no migration | `choose` and the module docs of `sched.rs` |
| The only task of a worker may block in one `recv`; the worker then counts as busy | `block_alone` in `sched.rs` |
| One waiter for every wait: a thread (futex) or a task (switch to the worker loop); callers do not know which | `Waiter`, `park`, `wake` in `crates/ignite-io/src/wait.rs` |
| The readiness source is one interface (`register`, `deregister`, `wait`, `wake`); epoll, `poll` and a scripted double implement it | trait `Reactor` in `crates/ignite-io/src/reactor.rs` |
| `main` runs on the process's main thread in both modes | `run` in `crates/ignite-io/src/lib.rs`; slice §7 |
| The LLVM runtime calls the same `try_spawn` and swaps its per-task thread-locals at a switch | `ignite_rt_spawn` in `crates/ignite-rt/src/task.rs`; `TaskLocals` in `sched.rs` |
| Rust target: `spawn f(a)` becomes `::ignite_io::spawn(move \|\| ..)`; LLVM: `ignite_rt_spawn`; neither lowers suspension | slice §6 (the `spawn` row); readiness §4.10 |
| `spawn` takes a call of an effectful user fn or role and returns `Unit` at once (E013) | slice §5, §8 |
| No `async`, no `await`: `await e` is E001 since slice 6 | design §8.8; slice §10 |
| Inline on the worker in R1: `connect` and name resolution, `Directory`, `Launcher`, stdout; standard input parks on readiness | slice §7; `Stdin::read` in `crates/ignite-io/src/stdio.rs` |
| One executable component is one process; a native process hosts its member and the resources it `includes` | `Binding`, `nativeProcess`, `admits` in `std/deploy.ignite` |
| A server's accept loop is Ignite: `accept`, then `spawn Http.serve(..)` | `HttpServer.main` in `std/protocol/http.ignite` |
| `threads` mode is exercised only by `crates/ignite-io/tests/scheduler.rs`; no test of `crates/ignite`, rustgen or LLVM sets `IGNITE_IO` | grep, 2026-11-09 |
| No FFI: the case study's G11 was not taken | `.engineering/waves/plan-2026-11-07.md` |

Three facts shape the options.

1. **"One scheduler per server" already holds.** A server is an executable component, and an
   executable component is its own process (`admits` in `std/deploy.ignite`). Two servers never
   share workers. Several schedulers inside one process only matter once one process runs two
   workloads that must not slow each other: actors beside a server (actors P1), or a
   background job inside a server.
2. **Every blocking call is a substrate call.** Without FFI, user code cannot block a worker in
   the kernel except through `ignite-io`, which knows each such call and can route it (R2's
   blocking pool) without a decision by the program.
3. **The remaining hazard is CPU.** Scheduling is cooperative (readiness §4.1): a task that
   computes for 50 ms without waiting delays every task queued on its worker (K2 of readiness
   §10). Only user code causes it, and the substrate cannot route it.

## 3 Other languages

Defaults and control as each project documents them; sources below the table, with what could
not be verified on the project's own pages.

| language | unit | default scheduler | who chooses, how much | blocking, CPU, FFI escape | colouring |
|---|---|---|---|---|---|
| Go | goroutine | M:N; `GOMAXPROCS` from logical CPUs, the affinity mask and, on Linux, the cgroup quota | the runtime; one knob (`GOMAXPROCS`); no scheduler API | threads blocked in system calls do not count against `GOMAXPROCS`; `runtime.LockOSThread`; asynchronous preemption since 1.14 | none (not stated as such) |
| Erlang/BEAM | process | one scheduler thread per logical processor, preemptive by reductions | the VM; emulator flags (`+S`, `+SDcpu`, `+SDio`); no user scheduler found | dirty CPU and dirty IO schedulers for NIFs | none |
| Java (21+) | virtual or platform thread | a work-stealing `ForkJoinPool`, parallelism = available processors | the JDK; `jdk.virtualThreadScheduler.parallelism`; the program picks virtual or platform threads | platform threads; pinning on native frames, on `synchronized` until JDK 24 (JEP 491) | none |
| Kotlin | coroutine | `Dispatchers.Default`: CPU cores, at least 2 | the program, per coroutine: a dispatcher in the context; any `Executor` becomes one | `Dispatchers.IO`: 64 threads or the cores, whichever is larger; `limitedParallelism` | `suspend` |
| Rust | OS thread (std); futures under a library runtime | std: native OS threads, no async runtime; tokio: a worker per core | the program picks and builds a runtime (`current_thread`, multi-thread, `LocalSet`) | tokio `spawn_blocking`, `block_in_place` | `async` |
| .NET | `Task` | the .NET thread pool (`TaskScheduler.Default`) | the program: `TaskScheduler` subclasses, `SynchronizationContext` | `TaskCreationOptions.LongRunning`, a hint that an extra thread may be needed | `async` |
| Swift | task, actor | a width-limited global concurrent pool | the runtime; custom actor executors (SE-0392, Swift 5.9), task executor preference (SE-0417, Swift 6.0) | none documented for blocking | `async` |
| Zig (0.16) | what the `Io` implementation runs | none: all IO takes an `Io` value; `Io.Threaded` complete, `Io.Evented` experimental | the program, by the `Io` value it passes down each call chain | the implementation chosen; `io.concurrent` fails with `error.ConcurrencyUnavailable` where it cannot | no keywords; the `Io` parameter instead |

Sources:

- Go: <https://pkg.go.dev/runtime> (`GOMAXPROCS`, `LockOSThread`, blocked system calls),
  <https://go.dev/doc/faq> (goroutines moved off a blocked thread),
  <https://go.dev/doc/go1.14> ("Goroutines are now asynchronously preemptible").
  Not verified: that a cgo call gets its own thread (<https://pkg.go.dev/cmd/cgo> does not say),
  and a statement that Go has no colouring (inferred from the FAQ).
- Erlang: <https://www.erlang.org/doc/apps/erts/erl_cmd.html> (`+S`, dirty scheduler counts),
  <https://www.erlang.org/doc/apps/erts/erl_nif.html> (dirty NIFs). Not verified: that no user
  scheduler exists (inferred from the absence of an API).
- Java: <https://openjdk.org/jeps/444>, <https://openjdk.org/jeps/491>. Not verified: that a
  custom virtual-thread scheduler is not public API (JEP 444 does not discuss it).
- Kotlin: <https://kotlinlang.org/api/kotlinx.coroutines/kotlinx-coroutines-core/kotlinx.coroutines/-dispatchers/>
  (`Default`, `IO`, `Main`),
  <https://kotlinlang.org/api/kotlinx.coroutines/kotlinx-coroutines-core/kotlinx.coroutines/new-fixed-thread-pool-context.html>,
  <https://kotlinlang.org/docs/coroutines-basics.html> (suspending functions callable only from
  suspending functions).
- Rust: <https://doc.rust-lang.org/std/thread/index.html>,
  <https://doc.rust-lang.org/reference/expressions/await-expr.html>,
  <https://docs.rs/tokio/latest/tokio/runtime/index.html>,
  <https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html>,
  <https://docs.rs/tokio/latest/tokio/task/fn.block_in_place.html>. Partly verified: "no async
  runtime in std" from <https://rust-lang.github.io/async-book/>, not from the std docs.
- .NET: <https://learn.microsoft.com/en-us/dotnet/api/system.threading.tasks.taskscheduler>,
  <https://learn.microsoft.com/en-us/dotnet/api/system.threading.tasks.taskcreationoptions>,
  <https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/operators/await>.
- Swift: the concurrency chapter of the Swift book, source at
  <https://github.com/swiftlang/swift-book/blob/main/TSPL.docc/LanguageGuide/Concurrency.md>,
  <https://github.com/swiftlang/swift-evolution/blob/main/proposals/0392-custom-actor-executors.md>,
  <https://github.com/swiftlang/swift-evolution/blob/main/proposals/0417-task-executor-preference.md>.
  Not verified: that the global pool's width equals the cores (SE-0417 says "width-limited").
- Zig: <https://ziglang.org/download/0.15.1/release-notes.html> (no `async` or `await`
  keywords, the `Io` interface "agnostic to the application's concurrency model"),
  <https://ziglang.org/download/0.16.0/release-notes.html> (`Io` required for all IO,
  `Io.Threaded`, `Io.Evented`, `io.async` and `io.concurrent`), <https://ziglang.org/devlog/2026/>.
  Not verified: the label "colorless", which the official notes do not use.

What the comparison says:

- **Every runtime has one default sized to the machine, and most control is a count.**
  `GOMAXPROCS`, `+S`, `jdk.virtualThreadScheduler.parallelism`, tokio's worker threads: the
  knob users touch is how many workers, not which algorithm. Go's default reads the affinity
  mask and the cgroup quota, as `worker_limit` does.
- **The languages without colouring keep the scheduler closed.** Go, BEAM and Java offer no
  user scheduler. Kotlin, Rust, .NET and Swift open it, and all four colour functions: a
  suspending function is a value the caller hands to an executor. Zig opens it without
  keywords by passing the scheduler as a parameter through every call chain that does IO.
- **Every M:N default ships an escape hatch, shaped by what can block.** Go and Java must run C
  code and give foreign calls a thread. BEAM gives long NIFs dirty schedulers. Kotlin, tokio and
  .NET give blocking library code a second pool. Go and BEAM answer CPU-bound code by
  preemption. Java's virtual threads, Kotlin, tokio and .NET do not preempt and tell the
  program to move such work to another thread or pool.

For Ignite: no colouring (design §8.8, readiness §3.4) puts it with Go, BEAM and Java, which
keep one scheduler and expose a count. Without FFI (fact 2) the blocking hatch is the
substrate's job. It does not preempt (fact 3), so CPU-bound work needs what Java, Kotlin and
tokio offer: a way for the program to put a task on a thread of its own.

## 4 Options

A is today. B, C and D each include the one before.

### A One scheduler per process, chosen by environment (considered: today)

`IGNITE_IO` and `IGNITE_WORKERS` at start; nothing in source or in the model. Cost: none.

Not taken because: the mode a server needs is not in what `ignite plan` shows or `ignite apply`
changes, so an operator sets it by hand on every host; `threads` mode is not tested above
`ignite-io` (section 2), so slice §7's "both modes give the same results" holds for the
substrate only; a CPU-heavy request stalls its worker's other sessions (K2).

### B Plus a dedicated thread for blocking and CPU work (considered: part of C)

- **B1, blocking calls leave the workers.** R2 as decided (readiness §4.7,
  `story:readiness-blocking-pool`, wave 10): name resolution, fork and exec, `Directory` and
  stdout writes that would block run on a pool of OS threads for the parked task. Invisible to
  the program.
- **B2, CPU work leaves the workers.** Explicitly: a spawn form that starts its task on a
  dedicated OS thread, written `spawn thread f(a)` here (the form is question Q1). Or
  automatically: a worker whose running task has not parked for a threshold hands its queued
  tasks to other workers, as Go's runtime moves goroutines off a blocked thread.

| aspect | B1 | B2 explicit (`spawn thread`) | B2 automatic hand-over |
|---|---|---|---|
| syntax | none | one contextual word after `spawn` (slice §3) | none |
| std | none | none | none |
| checker, effects | none | E013 unchanged for the operand; effects as `spawn` (slice §5) | none |
| Rust target | none | calls `ignite_io::spawn_thread` instead of `spawn`: another entry, no lowering change | none |
| LLVM target | none | `ignite_rt_spawn_thread` beside `ignite_rt_spawn`, same handshake | none |
| ignite-io | `pool.rs`; routing in `net`, `child`, `fs`, `stdio` | the thread path of `task.rs` reachable in `readiness` mode; its waits are thread waits, which `Waiter` already handles | a stuck-worker check in the worker loop; queued tasks handed over |
| plan, model | none | none | none |
| tests | the story's acceptance | a fixture on both targets: a 100 ms computation on its thread, a session on the workers answers meanwhile; `/proc/<pid>/task` shows the thread | one task loops 100 ms, another on the same worker answers within a bound, both targets |
| risks | pool sizing; fork from a pool thread | a language word for an implementation concern, used by habit | K1 (a thread-local borrowed across a park) for tasks that move; hand-overs per reply if the threshold is low |
| stories | 1 (planned) | 1 | 1 |

### C Plus a scheduling property of the executable component (decided)

The executable kinds (`HttpServer`, `TelnetServer`, `TelnetClient`, `TelnetPrompt`, `HttpHealth`)
get one field, `schedule: Schedule = Schedule {}`, with `Schedule` in `std.runtime` (base layer).
A first shape, which `story:schedule-property` fixes:

```ignite
enum IoMode { Readiness, Threads }

type Schedule {
  mode: IoMode = Readiness
  workers: Option<Int> = None
}
```

A component that wants raw threads for the whole process writes
`schedule = Schedule { mode = Threads }`. The value is part of the component, so `ignite plan`
prints it with every other evaluated property (slice §15.5), `ignite explain` explains it, and a
change of it is a property change that `ignite apply` rolls out like any other.

How the value reaches the substrate (question Q3):

| | C1 std calls a substrate static | C2 the compiler reads a known field |
|---|---|---|
| mechanism | each kind's `main` starts with `Runtime.schedule(self.schedule)?`; the substrate sets mode and worker limit, and fails when a task was already spawned | the generated `main` passes the executable's `schedule` field to `run` when the kind has one, as the compiler knows `main`, `machine`, `validate` |
| syntax | none | none |
| std | `Schedule`, `IoMode`; one line per kind's `main` | `Schedule`, `IoMode` |
| checker, effects | one builtin static and its effect entry, as `Runtime.census` (slice §33) | a rule for the field's type and a diagnostic for a wrong one |
| backends | a builtin call on both, as every substrate static; no lowering change | the generated `main` of both targets passes one value to `run` and `ignite_rt_run` |
| ignite-io | `io_mode` and `worker_limit` read a value set once before the first spawn | same, through `run`'s argument |
| kinds outside std | must write the line (`examples/todo/health.ignite`) | get it for free |
| plan, model | the field appears; no model change | same |

Tests: every end-to-end suite that matters for scheduling runs in both modes on both targets,
one fixture's transcript byte-identical between modes. The `plan` goldens gain the field, and a
test checks the chosen precedence between declaration and environment. Risks: the gate grows by
part of the end-to-end binary per mode (inferred: that binary is most of the gate, AGENTS.md
slice 18); the C1 line can be forgotten in a kind written outside std.

C gives one scheduler per executable, and an executable is a process, so per-server isolation
is the process boundary (fact 1). Several schedulers inside one process come with actors: the
`Actor` kind (actors §2.1) can carry the same `Schedule`, and the substrate then keeps one
worker set per property value (question Q6).

### D Scheduler values in code, or user-written schedulers (considered: not taken)

**D1, pools as values**: a capability `Pool` created in `main` (`Pool.new(workers = 2)`, effect
`pool.spawn`) and a spawn form that names it, `spawn(on = pool) f(a)`. **D2, user
schedulers**: a scheduler interface written in Ignite that decides which task runs next.

| aspect | D1 pools as values | D2 user schedulers |
|---|---|---|
| syntax | `spawn(on = e) call` | a run-queue interface and a way for Ignite code to resume a suspended task |
| std | `Pool` in `std.runtime` | the interface and a default implementation in Ignite |
| checker, effects | a new capability kind; `pool.spawn` in effect inference (slice §5); E013 extended; pools as handles across tasks (ownership as channels, slice §30) | first-class suspension in the type system |
| backends | a new entry `ignite_rt_spawn_on(pool, fn, env)` on both; handle release on both (one more census kind) | suspension exposed per target |
| ignite-io | `WORKERS` per pool; placement, `LIVE`, the drain's task count across pools | the run queue handed to Ignite code |
| plan, model | pools made in code are invisible unless also declared as resources | invisible |
| tests | per-pool isolation on both targets; drain with tasks on two pools | a contract suite for user schedulers |
| risks | a value threaded through every call that spawns, as Zig's `Io`, for a choice most programs never make; deadlocks between pools sized too small | against "no target suspends a task itself" (AGENTS.md) |
| stories | 3 to 4 | not costed |

Not taken because: D2 breaks the substrate rule that suspension is written once, in one
language (AGENTS.md, readiness §3.3), and the comparison shows open schedulers only where
functions are coloured or the scheduler is passed as a parameter. D1 is within the rules but
puts a deployment choice into code, where `ignite plan` cannot see it, for a case Ignite does
not have yet (fact 1). The reactor seam (`Reactor`) stays the extension point, for substrate
implementers, not programs.

## 5 Why C

- The languages without colouring keep one scheduler and expose a count; Ignite has no
  colouring by decision (T8). C keeps one default and makes the count and the mode declarable.
- The executable kind is where Ignite already declares what differs between processes (limits,
  recovery, rollout), and design §8.11 asks to "choose linked facilities according to the
  executable's declared role". There the setting is reviewable in the plan and changed by
  `apply`, which an environment variable is not.
- Every M:N runtime in the comparison ships a hatch to a plain thread. Ignite's blocking cases
  go to the substrate's pool (B1); the CPU case gets the raw-thread hatch, which Java, Kotlin
  and tokio also leave to the program.
- Later models (stackless lowering per target as in readiness §3.4, a tokio-driven reactor,
  actor dispatchers) are new values of the same property or new implementations behind `Reactor`,
  not new syntax.

## 6 Stories, in order

| order | id | title | depends on | surfaces |
|---|---|---|---|---|
| 1 | `story:spawn-thread` (new) | A spawn form starts its task on a dedicated OS thread in either IO mode | readiness-scheduler | `crates/ignite-syntax` (the form), `crates/ignite-check` (E013), `crates/ignite-rustgen`, `crates/ignite-llvm` (`ignite_rt_spawn_thread`), `crates/ignite-rt/src/task.rs`, `crates/ignite-io/src/task.rs`, slice §3, §5, §6, §7, `crates/ignite/src/apply/plan.rs` (`CODEGEN_REVISION`) |
| 2 | `story:schedule-property` (re-scopes `story:io-mode-setting`) | An executable component declares its IO mode and worker count in `schedule`, shown by `ignite plan` | spawn-thread | `std/runtime.ignite`, the executable kinds in `std/protocol/*.ignite`, `examples/todo/health.ignite`, `crates/ignite-io/src/sched.rs`, the builtin tables of the checker and both backends, plan goldens, slice §7 and §33 |
| 3 | `story:io-modes-in-the-gate` (new) | The end-to-end suites run in both IO modes on both targets | schedule-property | `crates/ignite/tests/common/mod.rs`, `Taskfile.yml` |

Beside them, already planned: `story:readiness-blocking-pool` (wave 10, B1), which C assumes.
`story:task-thread-bound` (draft) is answered by R1 (N connections no longer cost N threads);
its acceptance is re-checked and the story retired by whoever plans story 1.

## 7 Questions for the stories

| # | story | question | recommended default |
|---|---|---|---|
| Q1 | spawn-thread | The form of the hatch: `spawn thread f(a)`, or a field of a spawn option record? | `spawn thread f(a)`: one contextual word, the operand rule of E013 unchanged |
| Q2 | spawn-thread | Add the automatic hand-over of a stuck worker's queue (B2 automatic) as well, or only when a program shows a stall the hatch does not fix? | only then; measured on the dedicated host |
| Q3 | schedule-property | C1 (std calls `Runtime.schedule`) or C2 (the compiler reads the field)? | C1; C2 if kinds outside std forget the line |
| Q4 | schedule-property | When the declaration and `IGNITE_IO` or `IGNITE_WORKERS` disagree, which wins? | the environment, logged once at start, as an operator's override for diagnosis |
| Q5 | schedule-property | `workers` as a number only, or also a share of the CPUs? | a number; cgroup and affinity limits already shrink the default (`worker_limit`) |
| Q6 | schedule-property, actors P1 | Several worker sets in one process, one per `Schedule` value, for the `Actor` kind? | deferred to actors P1 |
| Q7 | io-modes-in-the-gate | Every end-to-end suite in both modes, or a set? | terminal, drain, failover, channels and todo in both modes, the rest in `readiness` |
