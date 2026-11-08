# Fault injection and deterministic simulation in the language

**Status:** draft, not implemented, for decision.
**Date:** 8 November 2026
**Scope:** what the language, `std` and the substrate can give a test so that the failure paths
of a program run under `ignite test`: faults at the IO seam, named fault points, a simulated world
driven by one seed, a schedule generator with invariants and shrinking, and the trace that makes a
failure readable. Infrastructure-level injection (pausing, resetting, blackholing or delaying a real
process through containers or a proxy) is proposed separately for the specification tooling and is
not covered here.

This document proposes. Each section lists options with their trade-offs and a recommendation; the
maintainers decide. References cite `docs/slice.md` as "slice §n", `docs/design.md` as "design
§n", other documents and files by path.

## 1 Motivation

A WebSocket push service, with replicas fanned out over a message broker, was implemented in five
languages against one specification. Every implementation passed the full conformance suite. One
of them was still wrong in two ways that the suite never exercised:

- After a broker reconnect it delivered messages late. They had sat in the broker client's 8 MiB
  reconnect buffer and were flushed when the connection came back. The specification says such
  messages are dropped and counted after 1 s.
- It never sent the echoed WebSocket close frame when the client closed.

The suite simulated broker outages only by pausing the broker process. A paused process keeps its
TCP connections open, so the client saw a silent peer, never a disconnect, and the reconnect path
with its buffer never ran. The suite was green because the fault it could produce was not the
fault that mattered.

The lesson for Ignite: a conformance suite tests what its world can do to the program. A world
that can only pause tests the pause path. The infrastructure proposal widens the world from the
outside, against the unmodified binary. This document asks what the language can add from the
inside, where it knows every IO operation, every task and every clock read, and can make a failing
run repeat exactly.

## 2 What exists

| piece | where | what it gives fault testing today | what it lacks |
|---|---|---|---|
| one substrate, both targets | `AGENTS.md` layout rules; design §8.6; `crates/ignite-io/src/lib.rs` | every socket, pipe, file, child, clock and entropy call of a compiled program goes through one Rust crate; the LLVM target reaches it through the C ABI adapter `crates/ignite-rt` only | no seam inside it: each operation calls the OS directly |
| tasks | slice §7; `crates/ignite-io/src/task.rs` | one OS thread per task, reused; `spawn` deep-copies its arguments, so tasks share no values, only handles the substrate owns | scheduling is the kernel's; no ordering control, no virtual time |
| capabilities as values | slice §21.3 to §21.7, §21.19: `Directory`, `Launcher`, `Entropy`, `Setting`, `CommandLine` | a test chooses which capability a fn receives (`Directory.temporary()`, `Setting.of(..)`, `CommandLine.of(..)`), and `requires` shows which fn can touch it | each is a concrete builtin type; a test cannot hand in a faulty one (section 4.1) |
| ambient IO | `EFFECTFUL_STATICS` in `crates/ignite-check/src/lib.rs`: `TcpListener.bind`, `TcpStream.connect`, `Stdin.*`, `Stdout.write_all`, `Process.exit`, `Clock.monotonic`, `Endpoint.of` | effect-tracked (`net.connect`, `clock.read`, …) | not values at all: no language-level way to substitute them |
| `ByteStream` | slice §18.3 | protocol roles are generic over `S: ByteStream`, so the same role runs over TCP and over a pipe | "Nothing else implements it" (slice §18.3): no user-written faulty stream |
| `Pipe.pair()` | slice §18.4; `crates/ignite-io/src/pipe.rs` | an in-process stream with TCP's coalescing, a bounded queue (`PIPE_CAPACITY`), `pipe peer is gone`, idle timeout; the session tests of Telnet, HTTP and JSON-RPC run over it | no reset, no partial delivery, no latency, no silent peer on demand; real time |
| test runner | slice §12.7 | one executable per target, each test in its own child process (`--run-one`), timeout, captured output | no seed, no trace, no replay of a failure |
| pure reducers | `std/control.ignite`; slice §16.6 | supervisor, scheduler, evaluator and rollout are `step(machine, event) -> Step`; inline tests feed synthetic event lists, so their fault paths (exit, signal, timeout) are already tested by construction | only the reducers; effectful code has no such form |
| manual clock, journal, replay | slice §16.4, §16.5, §16.15 | `ignite run --clock manual` moves the reducers' time on `advance`; replay re-steps journaled events and compares byte for byte (E127) | the programs themselves read the real `Clock.monotonic`; the settle rule waits in real time; replay is exact for the reducers only, because process values enter as events |
| compile-time evaluator | slice §15.3 | pure, fuelled, deterministic | runs no IO, so no faults |
| host fault points | `IGNITE_DEBUG_HOST_FAULT` in `crates/ignite/src/run/process.rs` (slice §17.5) | named points (`after_intent`, `before_dispatch`, `after_dispatch`, `mid_transition`, `sync_intent`, `sync_effect`) in a debug build of `ignite host`; a release build never reads them; the fault matrix of slice §22.8 is built on them | Rust driver code only; nothing for a compiled Ignite program |
| seeded actor simulation | `docs/actors.md` §10 ("Testing", item Concurrency: `std.actor.sim`, a seeded single-threaded dispatcher), P5 | the plan already promises a seed, a trace and exact replay for actors | actors only, connection tasks explicitly not simulated; not built |
| model exploration | `docs/protocols.md` §3.5 (`std.check.explore`) | breadth-first exploration of two pure protocol machines over bounded channels | pure machines, not the code that does IO; not built |

Design §6.3 already states the principle: "Time, random values, filesystem content, and external
responses enter through declared inputs or recorded effects." Design §13.1 lists "crash
before/after persistence and dispatch, duplicate results, lease loss, uncertain outcomes" as the
runtime verification layer. `docs/vision.md` R4 records a substrate risk found by "code reading,
no fault injection". The gap is a mechanism that brings effectful program code under the same
discipline the reducers already follow.

## 3 Goals and non-goals

Goals:

1. The failure paths of the motivating case (connection reset, silent peer, reconnect with a stale
   buffer, a missing close reply) are reachable from an inline `test` in milliseconds, on both
   targets.
2. A failing run is reproducible from one printed seed, on the same target and build.
3. Zero cost in release builds; a measured, small cost in test builds that do not inject.
4. One implementation: the seam lives in the substrate, so both targets get it unchanged (layout
   rule "The substrate is implemented once").
5. Invariants are Ignite code, checked by `ignite test`, not a separate model that drifts
   (`docs/protocols.md` §3.5's argument against P and TLA+).

Non-goals:

- Replacing infrastructure-level injection. The kernel's TCP stack, real DNS servers, containers
  and other people's binaries stay outside; the separate proposal covers them.
- Determinism of code outside the substrate's reach: child processes started through `Launcher`
  are real OS processes (section 5.5).
- Proving properties. A clean run of N seeds is evidence within its bounds, as `explore`'s report
  says of itself (`docs/protocols.md` §3.5).

## 4 The seam

### 4.1 How far capabilities as values get

Slice §21.3 made files, children, entropy and arguments values that a system declares and a fn
receives. That is half a seam: the test decides which instance a fn gets, and `requires [state.*]`
shows the authority. It falls short in three places:

1. **No substitution.** `Directory`, `Launcher`, `Entropy` are concrete builtins. A test can give
   a temporary directory, not a directory whose next write fails with `ENOSPC`. Ignite has one
   interface, `ByteStream`, and only builtins implement it (slice §18.3).
2. **Ambient IO.** The operations the motivating case needs are not capabilities: `TcpStream.connect`
   (effect `net.connect`), `TcpListener.bind` and `Clock.monotonic` (`clock.read`) are statics.
3. **No scheduler.** Even with every operation substitutable, the interleaving of tasks and the
   passage of time stay the kernel's. A test of "dropped after 1 s" would sleep in real time and
   still race.

### 4.2 Options

| option | how | gets | costs |
|---|---|---|---|
| A: interfaces in Ignite | user-implementable interfaces (`Network`, `Clock`, `Files`), every IO operation reached through a capability value, test doubles written in Ignite | substitution at the language level; authority and seam are one concept; doubles in Ignite are readable | a language change (impls of interfaces, slice §18.3's "the only interface" lifted), every ambient static becomes a resource, every program and std module rewritten to thread them; still no scheduler and no virtual time |
| B: one substrate seam | inside `crates/ignite-io`, every operation goes through a `World`: the real one (today's code) or a simulated one, chosen when a test executable starts | every operation, ambient or capability, both targets, no source change; the simulated world owns tasks and time, so it can be deterministic | Rust work in one crate; the simulated world is a second implementation of the substrate's semantics that must match the first (section 9) |
| C: process interposition | `LD_PRELOAD`, seccomp or ptrace shims, or a deterministic hypervisor (Antithesis) under the unmodified binary | any binary, any language | outside the language; that is the infrastructure proposal |
| D: fault points only | named points in the code that a test arms (FoundationDB's `BUGGIFY`, the `fail` crate's failpoints, `IGNITE_DEBUG_HOST_FAULT`) over the real substrate | cheapest; works on today's substrate; reaches logic no IO fault reaches | nondeterministic: a fault point in a race fires differently each run; no virtual time |

**Recommendation: B, with D on top.** B is the shape the layout rules already force: every IO call
of a compiled program already crosses `ignite-io`, and since slice §16.13 even the clock does. D's
named points add faults that are not IO (section 6). A is not needed for testing; whether `Clock`
and `TcpStream.connect` should become capability resources for authority reasons is a separate
question (Q6). C stays with the infrastructure proposal.

### 4.3 The `World` in the substrate

- `crates/ignite-io` gains a private `World` with one method per substrate operation group:
  network (bind, accept, connect, read, write, close, urgent data, idle timeout), pipe, clock,
  entropy, files (`fs`), children (`child`), stdio, task spawn and end, store.
- `RealWorld` is today's code, moved behind it. `SimWorld` is section 5.
- Selection: once, at `ignite_io::test_main` (slice §12.7) from runner flags. Compiled programs
  outside tests have only `RealWorld`; `SimWorld` is behind a cargo feature `sim`, built like the
  existing `runner` feature, so `libignite_rt.a` for release and `ignite build` output does not
  contain it.
- Dispatch: a test executable checks one process-global after selection. Option: an enum branch
  per operation (predictable, no allocation) rather than a trait object. Its cost against a syscall
  of microseconds is inferred to be negligible; phase 1 measures it under the gate-time rules of
  `docs/vision.md` invariant 9.
- `ignite-rt` changes nothing: it is an adapter over `ignite-io`'s functions, which keep their
  names and signatures (the module doc of `crates/ignite-io/src/lib.rs` already promises that an
  async substrate changes this crate and the two adapters only; a `World` changes even less).

## 5 Deterministic simulation as a first-class mode

### 5.1 Prior art

| system | how it is deterministic | what it injects | lesson for Ignite |
|---|---|---|---|
| FoundationDB simulation | single-threaded actor runtime (Flow); network, disk and time are simulated interfaces; one seed | machine, rack and datacenter failures, network partitions, disk faults, `BUGGIFY` points that fire randomly only in simulation | a whole cluster in one process; fault points in production code that cost nothing outside simulation |
| TigerBeetle VOPR | single-threaded by design; IO behind interfaces chosen at compile time; one seed | storage corruption and latency, network loss, delay, reordering, partitions, replica crashes | invariants checked continuously by a cluster-wide checker; storage faults are first-class, not an afterthought |
| turmoil (Rust) | runs simulated hosts on one thread over tokio, simulated network and time | partitions, hold and release of links, host crash and bounce | host crash as "drop every task of a host"; works with ordinary async Rust |
| madsim (Rust) | replaces tokio and `std` time, net and random at compile time via `cfg`, one seed | network faults, node kill and restart | a swap at the runtime layer, invisible to the program's source |
| Antithesis | deterministic hypervisor under arbitrary containers | process, network, clock faults, guided exploration | infrastructure-level; the counterpart of this document |
| Jepsen | real clusters; a nemesis schedules faults; history checkers (Knossos, Elle) | partitions, clock skew, kills, pauses | invariants over a recorded history, not over internal state |

Ignite has an advantage the Rust and C systems lack: tasks share no values (spawn deep-copies,
slice §7, `docs/llvm.md` §4), so **every interaction between tasks crosses the substrate** (a pipe,
a socket, a store, a file). Code between two substrate calls is invisible to every other task.
Making the substrate deterministic therefore makes the program deterministic, with no change to
the generated code.

### 5.2 Scheduling tasks deterministically

Tasks are OS threads today, and slice 5's async substrate has not landed. Options:

| option | how | for | against |
|---|---|---|---|
| S1: baton over threads | tasks stay OS threads; `SimWorld` holds one baton; only the holder runs; every substrate call parks the caller and hands the baton to the task the seeded scheduler picks among the runnable ones | no change to codegen or to `task.rs`'s model; works on both targets now; non-preemptive scheduling is exact because code between substrate calls cannot be observed | one context switch per substrate call (microseconds); a task that computes forever without a substrate call holds the baton |
| S2: coroutines | lower tasks to coroutines (LLVM) and futures (Rust), drive them from one thread | the slice-5 plan; fastest | waits for slice 5 on both targets; two lowerings to make deterministic |
| S3: record and replay real interleavings | run on real threads, record the order of substrate calls, replay it | finds what the kernel happens to produce | replay of a recorded order is fragile, and exploration is the kernel's, not the seed's |

**Recommendation: S1.** It works with today's substrate and survives slice 5: under an async
substrate the same scheduler picks the next ready task instead of the next parked thread.

Rules of S1:

- Scheduling points: every substrate call, `spawn`, task end. A call that would block (an empty
  pipe, an `accept` with no connection, a read from a silent peer) leaves the task not runnable
  until the simulated world makes it so.
- The scheduler draws from a PRNG seeded by the test's seed; ties are broken by the draw, never by
  thread identity.
- Virtual time advances only when no task is runnable: the clock jumps to the earliest pending
  timer (an idle timeout, a child timeout, a scheduled fault, a latency delivery). `Clock.monotonic`
  returns virtual time. A test of "dropped after 1 s" therefore runs in microseconds and is exact.
- A runaway task: the baton holder is charged with each substrate-free stretch only by wall time.
  Option: a wall-clock watchdog that fails the run as `no scheduling point for 5 s` with the trace
  so far, rather than fuel instrumentation in generated code (Q4).
- Deadlock: no runnable task and no pending timer while the test body has not returned fails the
  run as `deadlock` with every task's parked operation.

### 5.3 What `SimWorld` simulates

| substrate area | in simulation |
|---|---|
| network | an address table in the process: `bind` registers `address:port`, `connect` finds it or is refused; a connection is a pair of ends with `Pipe.pair()`'s delivery model (slice §18.4), plus a per-link latency and the faults of section 6; names resolve through a table the test fills (DNS) |
| pipe | unchanged semantics, virtual time for idle timeouts, faults available |
| clock | virtual monotonic time per simulated node, optionally with a rate (skew) |
| entropy | bytes from the seed's PRNG, so a program's random choices repeat |
| files | an in-memory tree per `Directory` root with the same refusals as `crates/ignite-io/src/fs.rs` (symlinks, bounds, atomic replace), plus faults and a crash model (section 6) |
| store | as today; lost with its node on a crash |
| stdio | captured, timestamped in virtual time |
| children | not simulated (section 5.5) |

### 5.4 Nodes and systems

A test that exercises the motivating case needs at least two parties: the program and a broker
that can be reset. In simulation a **node** is a group of tasks with its own clock, its own stores
and its own directories:

- The test body is node 0. A test-only `Sim.node(name, entry)` starts a further node whose first
  task runs `entry` (a fn value), as `spawn` does. A fake broker is an ordinary Ignite fn that
  binds an address and speaks the protocol; it runs in the same simulated network.
- Crash: every task of the node is parked inside a substrate call (S1 guarantees that), so a crash
  marks the node dead, never resumes its threads, drops its stores and connections (peers see a
  reset or, under a partition, nothing), and applies the crash model to its directories. Restart
  runs `entry` again with the surviving directories. The parked threads of a dead node stay parked
  until the test process exits; unwinding through LLVM-generated frames is not attempted.
- Whole systems: running every member of a `system` as nodes in one test executable is possible
  because the runner already links several programs into one executable (slice §12.7). It needs a
  way to call a member kind's `main` with its resources from a test. Deferred to phase 4 (Q7).

### 5.5 What stays outside

- Child processes (`Launcher`, slice §21.5): real processes cannot be scheduled by the seed. In
  simulation `Launcher.execute` and `start` fail with `not available in simulation` unless the
  test registers a fake for the program name (`Sim.program("/bin/tool", handler)`, a fn from
  `Invocation` to `ChildExit`), which is deterministic.
- `ignite run` and `ignite apply`: their driver is Rust, and its processes are real. The driver's
  reducers are already deterministic and replayable (slice §16.15); faults at that level (kill,
  stop, port blackhole of a member) belong to the infrastructure proposal and to the existing
  `IGNITE_DEBUG_HOST_FAULT` points.
- The compile-time evaluator runs no IO (slice §15.3). A fault or simulation builtin in a property
  expression is E116, as `Pipe.pair()` is (slice §18.4, "As landed").

### 5.6 Reproduction

- `ignite test FILE --sim [--seed N] [--runs K]`: each selected test runs K times with seeds
  derived from N (default: a seed drawn and printed). A failure prints
  `seed 0x… (rerun: ignite test FILE --sim --seed 0x… --exact NAME)`.
- The seed reproduces the run on the same target, build profile and compiler. It does not promise
  equal traces across the two targets: they already differ in when a handle is dropped (slice §18.4,
  "When an end is dropped differs between the targets"), which changes when a peer sees `Eof`.
  Option: canonicalise drop events so the traces agree (Q3).

## 6 Fault kinds at the seam

| fault | where it is injected | simulated world | real world (phase 1) | motivating case |
|---|---|---|---|---|
| connect refused | `connect` | no listener at the address, or forced | forced `Err` before the syscall | broker down at start |
| reset | read or write on an established stream | both ends see `connection reset` at the next operation; queued bytes discarded | the substrate closes the socket and returns the error | the disconnect path the suite never ran |
| silent peer | a link | bytes are accepted and never delivered; reads block until an idle timeout | reads block, writes buffer (the substrate stops delivering) | what pausing the broker produced |
| partial read and write | `read`, `write_all` delivery | a delivery split into pieces the PRNG chooses | reads return short counts | framing bugs |
| latency, jitter | a link | delivery at `now + delay` in virtual time | real sleep before delivery | the 1 s staleness rule |
| loss | a link | TCP is reliable: loss appears as latency spikes, then reset or idle timeout; a datagram transport, when one exists, drops | as latency | |
| partition and heal | a set of links | silent in both directions until healed, then delivery resumes or the ends reset (chosen per fault) | as silent peer | reconnect with a stale buffer |
| DNS failure | name resolution in `connect` and `bind` (`socket_address`, `crates/ignite-io/src/lib.rs`) | the test's name table answers not found or a different address | forced error | |
| clock jump, skew | `Clock.monotonic` | a node's clock jumps forward or runs at a rate; monotonic time never runs backwards (slice §16.13) | not offered: the real monotonic clock cannot be moved | timeouts that assume one clock |
| disk full, EIO | `Directory` writes and reads | the operation fails with that error | forced error | the durable todo (slice §22.4) |
| torn or lost write on crash | `Directory` at a node crash | crash model: writes since the last atomic replace may be lost or cut | not offered | todo shard files |
| allocation failure | allocation | not offered as a recoverable fault: Ignite has no recoverable out-of-memory (Rust aborts; the LLVM arenas end the process). Modelled as a node crash at an allocation point | not offered | |
| process kill and restart with state loss | a node | section 5.4 | not offered (infrastructure) | replica restart |
| child exit, signal, timeout | `Launcher` | a registered fake returns any `ChildExit` | not offered | |

A fault is a value: `Fault { at: Trigger, target: Target, kind: FaultKind }` in a new test-only
module (section 8), where `Trigger` is a virtual instant, the Nth operation on the target, or a
fault point's Nth hit; `Target` names an address pair, a node, a directory root or a fault point.

## 7 Named fault points

### 7.1 Form

A fault point marks a place in program code where a test may make something go wrong that no IO
fault reaches: "the broker accepted the publish but the acknowledgement is lost", "the buffer is
full".

| option | form | for | against |
|---|---|---|---|
| F1: builtin static | `Fault.point("broker.publish")?`, an `IoResult<Unit>` that is `Ok(())` unless a test armed the point | no keyword (`docs/vision.md` invariant 6 counts keywords); resolves like `Clock.monotonic`; composes with `?` | a string id; typos found only by the arming test (Q2) |
| F2: keyword | `fault "broker.publish"` as a statement | reads as the request's sketch | a keyword for a test feature; needs grammar, parser, both lowerings |
| F3: declared points | `fault_point publish_lost: Unit` at module level, used as `publish_lost.check()?` | ids checked by the compiler; listable by `ignite test --list-faults` | a new declaration form |

**Recommendation: F1**, with the ids collected by the checker so that arming an id no point in the
program names is a diagnostic (Q2), which removes F1's main weakness without a declaration form.

### 7.2 Semantics and cost

- Release and non-test builds: the call lowers to `Ok(())` and disappears. The generators already
  distinguish test builds (slice §12.6, §12.7), so no runtime flag is read.
- Test builds: the call is one substrate entry `fault_point(id)` that looks up the armed set; in
  the simulated world it is also a scheduling point and is recorded in the trace.
- Effects: none. A fault point that needed `requires` would spread a test concern into every
  signature above it. Its behaviour differs from `Ok(())` only when a test armed it, and arming is
  test-only (E051 elsewhere, as `Pipe.pair()`).
- Arming: `Sim.arm(point, FaultKind)`, also usable in the real world of phase 1. Random firing in
  simulation, FoundationDB's `BUGGIFY`, is the nemesis's job (section 8), not the point's.
- Precedent: `IGNITE_DEBUG_HOST_FAULT` gives the Rust host the same thing for crash points, read
  only by debug builds (`crates/ignite/src/run/process.rs`).
- std modules opt in by placing points at their own failure-prone steps (for example a framing
  boundary in `std.protocol.jsonrpc`). Each point costs nothing outside tests, so no opt-in flag
  per module is needed; the layering rule (`docs/layering.md`) keeps protocol-named ids inside
  protocol modules.

## 8 Nemesis, invariants and shrinking

### 8.1 The nemesis

A nemesis generates fault schedules. In a test-only module (working name `std.sim`, a base-layer
module beside `std.net`, since it is substrate builtins plus Ignite helpers):

```ignite
import std.sim.{Sim, Nemesis, Fault, Reset, Partition, Latency, check}

test "stale buffered messages are dropped after a reconnect" {
  let report = check(Nemesis {
    nodes = ["broker"]
    kinds = [Reset, Partition(2 seconds), Latency(300 ms)]
    faults_per_run = 3
    runs = 200
  }, fn(sim: Sim) -> IoResult<History> = scenario(sim), invariants)
  assert report.passed
}
```

- `scenario` starts the fake broker as a node, runs the client under test, and returns the
  history of observations it recorded (section 8.2).
- Each run draws a schedule from the seed: which fault, when (virtual time or operation count),
  against which target, for how long. Runs are independent and could use the runner's `--jobs`.
- A failing run is shrunk (section 8.3), and the report carries the seed, the minimal schedule and
  the trace of the minimal run.

### 8.2 Where invariants live

| option | form | for | against |
|---|---|---|---|
| I1: predicates over a history | `fn(History) -> Bool` in Ignite, history a list of the scenario's observations with virtual timestamps | Jepsen's model: checks what a client saw, not internal state; any invariant ("no duplicate", "no message delivered more than 1 s after publish", "the `dropped_stale` counter equals the number dropped", "a close frame answers every close") is ordinary code | the scenario must record what it observes |
| I2: contracts and probes | reuse `std.contract`'s kinds over `std.sli` samples: a probe in simulation emits `Sample`s, the contract's `classify` (slice §22.3) judges them, `std.control`'s `Evaluate` reducer computes the standing over a window in virtual time | the declared SLO of a system becomes its test oracle; the reducers already run in virtual time (slice §16.4) and are pure | contracts are ratios over windows (availability, latency): liveness and service level, not safety. "No duplicate" or "never delivered late" is not a ratio |
| I3: assertions inside the program | `assert` in code under test | already exists | fires in the program's task, loses the cross-node view; a failed assert ends the test process |

**Recommendation: I1 as the carrier, I2 as an adapter for recovery.** Safety invariants are
predicates over the history. Recovery-time properties reuse the contract machinery: a helper
`recovers_within(history, fault, contract)` steps `std.control`'s evaluator over the samples
after each injected fault and reports the virtual time until the standing is met again. That gives
"recovery within N s" a single definition shared by the test and the running system.

### 8.3 Shrinking

Given (seed, schedule) the run is deterministic, so shrinking is re-running:

1. Keep the scheduler's seed, remove faults one at a time (delta debugging) while the invariant
   still fails.
2. Shorten durations and move triggers earlier, halving, while it still fails.
3. Optionally vary the scheduler's seed with the minimal schedule to find a run with fewer steps.

The minimal schedule is printed as Ignite source (`[Fault { at = After(3, connection("client",
"broker")), kind = Reset }]`), so a developer pastes it into a regression test that no longer
needs the nemesis.

## 9 Observation

- **Trace.** The simulated world records one line per scheduling decision, substrate operation and
  injected fault: virtual `at`, node, task, operation, outcome, fault. Program output (`debug`, log
  lines, stdout) is captured with the same virtual timestamps, so a fault and the program's own
  reaction sit next to each other. Encoding: JSON lines in the model encoding of slice §15.4, as the
  run journal is (slice §16.5). Printed for a failing run, written by `--trace FILE` on request.
- **Recovery time.** `Sim.now()` gives virtual time in tests; `recovers_within` (section 8.2)
  measures from a fault's record to the first good standing.
- **Coverage of faults.** The report counts, per fault kind and fault point, how often it fired
  and in how many runs an invariant was checked after it. A point that never fired is reported, so
  "the disconnect path never ran" becomes visible instead of silent.
- **Fidelity.** The simulated world is a second implementation of the substrate's semantics. A
  differential suite runs the existing pipe and session tests (the session helpers of slice §18.12, the Telnet,
  HTTP and JSON-RPC session tests) in both worlds and requires equal results; disagreement is a
  bug in the simulation.

## 10 The two targets

| question | Rust target | LLVM target |
|---|---|---|
| where the simulated world lives | `crates/ignite-io` with feature `sim` | the same code, reached through `crates/ignite-rt`; the test runtime archive is built with `--features runner,sim` |
| codegen change for simulation | none: substrate calls stay calls | none |
| fault points | rustgen emits `ignite_io::fault_point(id)` in test builds, nothing elsewhere | ignite-llvm emits a call to an `ignite_rt` entry in test builds, nothing elsewhere |
| scheduling | S1 over the threads of `ignite_io::spawn` | S1 over the same threads (`ignite-rt`'s task entry forwards to `ignite-io`) |
| node crash | parked threads never resumed | the same; arenas of dead tasks leak until the test process exits |
| cost when off | release: none (feature absent, points removed); test builds without `--sim`: one branch per substrate call, to be measured | the same |
| a later wasm target | the runtime in Ignite (`docs/wasm.md` §4) calls the same substrate through its host bridge, so it inherits `SimWorld`; its interpreter's fuel (wasm §8) would also solve the runaway task of section 5.2 | |
| `CODEGEN_REVISION` | fault-point lowering is a generation change; `apply::plan::CODEGEN_REVISION` moves once, in the phase that adds it | |

## 11 Phased plan

Smallest useful first. Each phase is usable on its own.

| phase | content | outcome | depends on |
|---|---|---|---|
| 1 | `World` seam in `ignite-io` with `RealWorld` only; test-only arming of IO faults on the real world (connect refused, reset, silent peer, partial read, DNS failure, disk full, EIO) by target address or root; `Fault.point` with test-build lowering on both targets; the fault trace | the motivating disconnect and close-frame paths are testable today with real time: a reset of the broker connection runs the reconnect path; cost of the seam measured | none |
| 2 | `SimWorld` for one node: S1 scheduler, virtual clock, seeded entropy, simulated pipes and in-process network, `--sim`, `--seed`, `--runs`, seed printed on failure; differential suite against the real world | "dropped after 1 s" asserted exactly in microseconds; any failure replays from its seed | 1 |
| 3 | nodes: `Sim.node`, crash and restart, partitions, latency, the in-memory directory with a crash model, `Sim.program` fakes | broker restarts and replica crashes with state loss in one test executable | 2 |
| 4 | `std.sim`'s nemesis, history invariants, shrinking, fault coverage in the report; `recovers_within` over `std.control` | generated fault schedules with minimal reproductions; recovery time defined once | 3 |
| 5 | whole systems: members of a `system` as nodes from their declaration | a system's assembly runs simulated under `ignite test` | 4, Q7 |

## 12 Draft stories

Not filed. Titles and one-line outcomes.

| story | outcome |
|---|---|
| `substrate-world-seam` | every `ignite-io` operation dispatches through a `World`; `RealWorld` is today's code; no behaviour change; dispatch cost measured in the gate |
| `io-fault-arming` | a test arms reset, refused, silent peer, partial read, DNS failure, disk full and EIO on the real world by address or root, on both targets |
| `fault-points` | `Fault.point(id)` lowers to nothing outside test builds and to an armed lookup in tests, on both targets; unknown armed ids are a diagnostic |
| `fault-trace` | a failing test prints the injected faults beside its captured output, in the model encoding |
| `sim-scheduler` | under `--sim` one task runs at a time, the next chosen by the seed; deadlock and runaway tasks fail with a report |
| `sim-clock-entropy` | `Clock.monotonic`, idle timeouts and child timeouts use virtual time; `Entropy` draws from the seed |
| `sim-network` | `bind`, `connect`, streams and name resolution run in-process under `--sim` with latency, partition and reset |
| `sim-seed-replay` | `--seed` and `--runs`; a failing seed is printed with the rerun command and reproduces the trace byte for byte |
| `sim-differential` | the pipe and protocol session tests pass identically in the real and the simulated world |
| `sim-nodes-crash` | `Sim.node`, crash and restart with state loss and a directory crash model |
| `sim-program-fakes` | `Launcher` in simulation runs registered fakes and refuses the rest |
| `nemesis-shrink` | `std.sim.check` generates schedules, checks history invariants and prints the minimal failing schedule as Ignite source |
| `recovery-within` | recovery time after each fault computed with `std.control`'s evaluator over a contract |
| `push-reconnect-regression` | an example test reproduces the motivating case: a stale reconnect buffer and a missing close reply both fail it, the corrected client passes |

## 13 Open questions

| id | question | recommended default |
|---|---|---|
| Q1 | Is simulation an `ignite test` flag (`--sim`) or a property of a test (`sim test "…"`, or a test whose body takes a `Sim`)? | a test whose body calls `Sim` builtins runs simulated; `--sim` forces every test of the run into the simulated world for the differential suite |
| Q2 | Fault point ids: free strings or checked? | free strings in source; the checker collects them per program and arming an id no point names is a diagnostic |
| Q3 | Must a seed give the same trace on both targets? | no, per target; canonicalising drop events is revisited when the differential suite shows it matters |
| Q4 | How is a runaway task (no substrate call) handled under S1? | wall-clock watchdog, 5 s, fails with the trace; no fuel instrumentation in generated code |
| Q5 | Which real-world faults belong to phase 1 and which only to simulation? | phase 1: refused, reset, silent peer, partial read, DNS, disk full, EIO; latency, skew, crash and partition only in simulation |
| Q6 | Should `Clock` and `TcpStream.connect` become capability resources (slice §21.3) for authority? | separate decision; not needed for testing, since the seam is in the substrate |
| Q7 | How does a test start a system member as a node? | defer to phase 5; until then a node runs a fn value the test chooses |
| Q8 | Crash model for `Directory` without `fsync` (slice §22.4 notes writes do not `fsync`) | atomic replace survives a crash; any other write since the last replace may be lost or cut; revisited when `fsync` lands |
| Q9 | Does the seeded actor dispatcher of `docs/actors.md` §10 become a part of `SimWorld`? | yes: actors are tasks with mailboxes in the substrate, so one scheduler serves both; `actors.md` P5 then depends on phase 2 |
| Q10 | Where does `std.sim` sit in `docs/layering.md`? | base layer, test-only items, beside `std.net`; the layer table and `layering::` change together |

## 14 References

Project pages, not specific sections:

- FoundationDB, simulation and testing: <https://apple.github.io/foundationdb/testing.html>
- TigerBeetle (VOPR in its repository): <https://github.com/tigerbeetle/tigerbeetle>
- turmoil: <https://github.com/tokio-rs/turmoil>
- madsim: <https://github.com/madsim-rs/madsim>
- Antithesis: <https://antithesis.com>
- Jepsen: <https://jepsen.io>
- fail-rs (failpoints): <https://github.com/tikv/fail-rs>
