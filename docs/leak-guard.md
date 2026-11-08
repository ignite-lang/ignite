# Leak guard: a leak fails a test, not a server

**Status: design, not implemented.**
**Date:** 8 November 2026
**Epic:** `epic:leak-guard`. **Related:** `story:channel-release-on-close` (the fix of G30).

"slice §n" is `docs/slice.md`, "llvm §n" is `docs/llvm.md`. Costs marked "inferred" are not
measured: benchmarks run only on the dedicated host (AGENTS.md), so each story records its cost
as not measured locally and the next release's `task quality-remote` run measures it.

## 1 The problem

A program that leaks runs green under `ignite test`: every test is a child process of a few
milliseconds (slice §12.7), and a leak of a few hundred bytes per operation never shows. It
shows in production, as a server that grows.

The motivating case is G30 of the case study, a WebSocket push service
(`docs/research/case-study-gaps.md`): a closed channel's memory is never freed. A spawned task
running

```ignite
let (r, b) = Channel.bounded(1)
r.close()
b.receive_within(0 ms)
```

grows the resident size by about 370 B per task: 200,000 tasks take the process from 14 MB to
88 MB, as the case study reported it on v0.15.0. The service works around it with a list of
reusable channels.

These three lines alone do not reproduce it on `main` (2026-11-08): 200,000 tasks one after
another and at once, and 200,000 loop iterations, peak at 2.0 to 6.5 MB on both targets, in dev,
`--release` and `--static`, the same as a control without the channel. A reproducer that leaks
is requested from the case study (`story:channel-release-on-close`), which finds and fixes the
cause.

Fixing G30 removes one leak. This design makes the next one fail a test. Its acceptance has two
first cases (section 7): the case study's G30 reproducer, once it arrives, fails under the
guard before the fix and passes after it; and a seeded leak, a runtime object deliberately
never released, proves the guard red before any real reproducer exists.

## 2 Where runtime state lives

The substrate is one implementation for both targets (AGENTS.md): every object below is created
and dropped in `crates/ignite-io`, and the LLVM runtime `crates/ignite-rt` adds a handle table,
arenas and heap copies on top.

| state | where | Rust target | LLVM target | live means |
|---|---|---|---|---|
| task | `ignite-io/src/task.rs` (`LIVE`, `live_tasks`) | one pooled worker thread per running task | the same; `ignite-rt/src/task.rs` adds the spawn's transfer arena | spawned and its fn not yet returned |
| worker | `task.rs` (`IDLE`, `LINGER` 1 s) | a thread parked on the idle stack | the same | a pool, never a leak |
| channel | `ignite-io/src/channel.rs` (`Shared`: mutex, queue, two condvars) | freed when the last `Arc<End>` of both ends drops | each end is a handle-table entry holding a clone; freed when both entries are released | `Shared` allocated and not dropped |
| value in a channel | `channel.rs` queue | a Rust value | a `HeapValue` (`ignite-rt/src/store.rs`) made by the element's glue with `malloc` | queued, freed with the channel |
| listener, stream, pipe end, child, directory | `ignite-io` `net.rs`, `pipe.rs`, `child.rs`, `fs.rs` | an `Arc` the program's Rust values hold | a clone in a handle-table slot | the inner object not dropped |
| capability (launcher, entropy, setting, command line, environment) | `ignite-io` `child.rs`, `entropy.rs`, `args` | a value | a handle-table entry | as above |
| handle | `ignite-rt/src/net.rs` (`TABLE`, segments that never move, `FREE`, owners per slot) | none | a slot holding a `Handle`, released by the last owner (`tcp_release`, `own_release`, `own.rs`) | a filled slot; a free slot and the segment memory are a pool |
| store | `ignite-io/src/store.rs`, `ignite-rt/src/store.rs` (`RtStore`) | dropped with its last value | never freed: "a store is a system resource and lives until the process exits" | resident by design |
| heap copy | `ignite-rt/src/store.rs` `HeapValue` | none | `malloc` through generated glue (`ignite-llvm/src/glue.rs`), freed by `free_heap` | made and not freed |
| arena | `ignite-rt/src/arena.rs` | none (Rust heap) | per thread: blocks of the running task, `SPARE` chunks, one chunk kept per thread after a task, `TRANSFER_SPARE`, up to 8 MiB of spare large blocks | blocks of a live task; spares are a pool |
| heap bytes | the allocator | Rust's global allocator (`malloc`) | the runtime's Rust allocations and generated `@malloc` | allocated and not freed |
| resident size | the kernel | `VmRSS` | `VmRSS` | not a count: includes pools, fragmentation, `malloc_trim` timing |

One retention is no leak by this definition: the handle table's segments are never freed. A
segment is allocated when the first slot in it is issued and stays for the life of the
process; released slots are reused (`FREE`). The table therefore keeps the memory of the most
handles ever open at once (each slot a lock, an entry and its `waited` list), not of the number
ever issued. A burst of 200,000 concurrent tasks with two channel ends each leaves 400,000 slots
behind after they end; the same work done one task at a time leaves the first segment. Repeated
at the same concurrency it does not grow, so a soak after its warm-up stays flat and the
per-test check sees only filled slots. The guard reports it (`handle_slots`, `pooled_bytes`)
and does not judge it; whether the table should shrink after a burst is Q11. A slot's
`waited` list is different: it grows by 16 bytes per waited child and is kept by design until
the process exits (slice §21.5), so a soak that waits children grows by that (Q11).

Readiness IO (`docs/readiness-io.md`) moves tasks onto stackful workers with a stack pool; a
task stays "spawned and not returned" and a pooled stack becomes one more pool. Nothing here
depends on one thread per task.

The evaluator (`crates/ignite-eval`) holds no runtime object: a substrate builtin has no value
at compile time (slice §30.4), and its own memory is bounded by fuel. The `ignite run` driver
and `ignite host` are Rust processes of the CLI, outside this design.

## 3 What "leak" means here

Two kinds of evidence, with different false positives:

- **Object counts per kind** are exact. A pool never shows as a live object: an idle worker is
  not a task, a spare chunk is not a block of a live task, a free handle slot is not a filled
  one. After a test's body returns and its tasks end, every kind except the resident ones (a
  store) must be back at its count before the body. A difference is a leak, with no tolerance.
- **Bytes** (heap bytes, arena bytes) catch a leak that no counted kind holds (a growing
  per-slot list, a thread-local that grows per task). They are noisy across one run: first-use
  caches (handle-table segments, thread-local read buffers, the spare lists) allocate once.
  Bytes are compared only across repetitions: after a warm-up, N more runs of the same work
  must not add more than a fixed slack. Resident size is reported, never compared: it moves
  with `malloc_trim` (once per second at most, `ignite_io::released_memory`) and with pools.

## 4 The options

| | idea | catches | false positives | cost | targets |
|---|---|---|---|---|---|
| (a) | leak check in `ignite test`: per test, counts per kind before the body and after it and its tasks end | an object kept after its owner is gone (G30 if a channel or an end stays live), on every test without writing anything | tests that leave a server task running on purpose; resident kinds; nothing from pools, since pools are not counted | one counter read before and after each test; a wait only while tasks are still live | both: the counters are in `ignite-io`, the runner is `ignite-io`'s (slice §12.7); LLVM-only kinds from `ignite-rt` |
| (b) | per-kind live counters exposed through `std.runtime`, for tests and as a server's metrics | everything (a) and (c) read; in production, the growth a test did not foresee | none by itself: it reports, it does not judge | a relaxed atomic increment per object created and dropped; for bytes, a counting allocator (inferred small, per-thread counters, measured by the next release) | both; the evaluator has no value, as every substrate builtin |
| (c) | soak in a test: run a fn N times, after a warm-up the counts must be equal and bytes within a slack | per-operation leaks that hold no counted object (bytes), and growth (a) cannot see because the body's own values are still live | first-use caches, absorbed by the warm-up; pools, bounded by their caps; resident kinds | N runs of the body inside the test; G30 at 1,000 runs is milliseconds, not a load test | both |
| (d) | compile-time ownership of resources: a dropped end releases its state | an end the compiler can prove unowned | none, but it cannot see runtime retention: a slot kept by the table, a value kept in a store, a leak inside `ignite-io` or `ignite-rt`. G30's lines own both ends by the rules already in `own.rs` and slice §30.3 | linear resource types: new checks and diagnostics in every resource API, and a rewrite of the moves analysis (slice §20.3) | both, but a leak in the substrate is invisible to it |

Verdict:

- (b) is the base the others read; without counters there is nothing to compare.
- (c) is the guard for G30: whatever the cause, it either holds a counted object or bytes, and a
  soak sees both.
- (a) makes every existing and future test a leak test, with no code in the test. It sees only
  counts, which is what keeps it free of false positives from caches.
- (d) is not part of the guard. The ownership rules exist (`own.rs`, slice §30.3); what is
  missing is evidence that the runtime keeps them. One known gap is by design: an end carried
  inside a value or sent on a channel is left unowned on LLVM ("never released", `own.rs`).
  The census makes that visible, and a fix for it is a story of its own once a test shows it
  (Q8).

## 5 The recommendation

Three stories, in this order:

1. **Census and soak** (`story:leak-census`): per-kind live counters in `ignite-io` and
   `ignite-rt`, heap bytes counted on both targets, `std.runtime` with `Runtime.census()`,
   `Runtime.settle(..)` and `soak(..)`. Its first failing cases are a seeded leak per compared
   kind and, once the case study supplies it, G30's reproducer as a soak.
2. **Leak check in `ignite test`** (`story:leak-check-tests`): on by default for every test, on
   counts only; a test opts out with `Runtime.leaves_running(reason)`; every test of `std/` and
   `examples/` passes it or opts out with a reason.
3. **Runtime metrics** (`story:runtime-metrics`): the census as metric lines a server exports;
   the todo example exports them.

### 5.1 The census

`crates/ignite-io/src/census.rs`: one `AtomicI64` per kind, incremented where the object is
created and decremented in its `Drop`, so both targets count the same objects. `ignite-rt`
registers a source for its own kinds (filled handles, live heap copies, arena bytes of tasks,
pooled bytes) at its first entry. Kinds:

| kind | Rust | LLVM | per-test check | soak |
|---|---|---|---|---|
| `tasks` | yes | yes | compared | compared |
| `channels` | yes | yes | compared | compared |
| `listeners`, `streams`, `pipes`, `children`, `directories` | yes | yes | compared | compared |
| `capabilities` (launcher, entropy, setting, command line, environment) | yes | yes | compared | compared |
| `handles` (filled slots) | no | yes | compared | compared |
| `handle_slots` (slots ever issued: the table's high-water mark) | no | yes | reported | reported |
| `heap_copies` | no | yes | not compared (a store's values live with it) | compared |
| `stores` | yes | yes | resident, reported | reported |
| `heap_bytes` | yes | yes | not compared | compared, 64 KiB slack |
| `arena_bytes` (blocks of tasks other than the caller) | no | yes | not compared | compared, 64 KiB slack |
| `pooled_bytes` (spare chunks and blocks, transfer spares, handle-table segments, later the stack pool) | no | yes | reported | reported |
| `workers` | yes | yes | reported | reported |
| `resident_bytes` (`/proc/self/statm`) | yes | yes | reported | reported |

Heap bytes: a counting global allocator over the system allocator, with one counter block per
thread (no shared cache line) summed on read. Rust target: the generated program crate and the
test crate declare `#[global_allocator] static ALLOC: ::ignite_io::census::Counting`. LLVM
target: `ignite-rt` declares it for its own Rust, and generated code calls
`ptr @ignite_rt_heap_alloc(i64)` and `void @ignite_rt_heap_free(ptr)` instead of `@malloc` and
`@free` (`ignite-llvm/src/glue.rs`, `ir.rs`), so the same counter covers it. Both are
generation changes: `apply::plan::CODEGEN_REVISION` moves once, in story 1.

Cost (inferred, not measured locally): one relaxed atomic per object created or dropped, which
is far below the mutex of a channel or the syscall of a socket; one per-thread add per
allocation. If the release's dedicated-host run shows the allocator beyond its tolerance, the
counting allocator stays in test builds only and `heap_bytes` is `None` elsewhere (Q4).

### 5.2 `std.runtime`

A base-layer module (`docs/layering.md`, `layering::` change together) with substrate builtins
(no value in the evaluator) and the soak in Ignite:

```ignite
package std.runtime

// Live objects of this process, per kind (docs/leak-guard.md 5.1).
type Census {
  tasks: Int
  workers: Int
  channels: Int
  listeners: Int
  streams: Int
  pipes: Int
  children: Int
  directories: Int
  capabilities: Int
  handles: Int
  handle_slots: Int
  heap_copies: Int
  stores: Int
  heap_bytes: Option<Int>
  arena_bytes: Int
  pooled_bytes: Int
  resident_bytes: Int
}

// A kind that grew: its value after the warm-up and after every run.
type Growth {
  kind: Text
  before: Int
  after: Int
}

type Soak {
  runs: Int
  warmup: Int
  growth: List<Growth>
}

//   Runtime.census() -> Census                    every program; the counters, no wait
//   Runtime.settle(tasks: Int, within: Duration) -> Bool
//                                                 waits until at most `tasks` tasks are live;
//                                                 false at the deadline
//   Runtime.leaves_running(reason: Text) -> Unit  test-only (E051 elsewhere, as Pipe.pair()):
//                                                 this test skips the leak check

// Run `body(i)` for i in 0..runs; after each run wait for the tasks it spawned (5 s).
// Warm-up: max(10, runs / 10) runs. Then compare the census after the warm-up with the
// census after the last run: every counted kind equal, heap and arena bytes within 64 KiB.
fn soak(runs: Int, body: fn(Int) -> Unit) -> Soak

// The census as metric lines (section 5.4).
fn lines(census: Census, prefix: Text) -> Text
```

`Census` has `==` and `debug()` like any record; `growth` is the list a test asserts empty, so
the failure report of `assert` (slice §12.5) prints what grew without a message argument.

### 5.3 The leak check in `ignite test`

`run_one` (`ignite-io/src/test.rs`) takes the census before the body. When the body returns,
it waits until the live tasks are back at the count before, at most 2 s, then until every
compared kind is back, at most the same deadline, then compares. A body that failed is not
checked (it failed already). A body that called `Runtime.leaves_running` is not checked.

A difference exits the child with `LEAK_EXIT_CODE` 102 after printing the table on stderr; the
runner reports it as `FAILED (leaked N objects)`. `--no-leak-check` turns the check off for a
run (diagnosis of a failure, not for the gate). Both targets print the same lines: the counts
come from `ignite-io`, except `handles` and `heap_copies`, which only the LLVM report shows.

### 5.4 Metrics

`lines(Runtime.census(), "ignite_")` gives one line per kind in the Prometheus text format
(`ignite_tasks 12`), with no dependency: text in `std.runtime`. The todo example serves it at
`GET /runtime`. A server that grows shows which kind grows, in production, without a debugger.

## 6 Target form

### 6.1 A soak test (G30)

In `std/sync.ignite`, beside the channel tests. The body below is the reported three lines; it
is replaced by the case study's reproducer when that arrives (section 1):

```ignite
test fn poll_closed() -> Result<Unit, IoError> {
  let (r, b) = Channel.bounded(1)
  r.close()
  b.receive_within(0 ms)
  Ok(())
}

test fn spawn_poll_closed(run: Int) -> Unit {
  spawn poll_closed()
}

test "a closed channel is freed (G30)" {
  assert soak(1000, spawn_poll_closed).growth == []
}
```

Before the fix, on a target that leaks (illustrative numbers):

```
running 1 test
test std.sync/a closed channel is freed (G30) ... FAILED

failures:

---- std.sync/a closed channel is freed (G30) stderr ----
assertion failed: soak(1000, spawn_poll_closed).growth == []
  left: [Growth { kind: "channels", before: 100, after: 1000 }, Growth { kind: "handles", before: 200, after: 2000 }, Growth { kind: "heap_bytes", before: 61440, after: 394440 }]
  right: []
  at std/sync.ignite:142:3

failures:
    std.sync/a closed channel is freed (G30)

test result: FAILED. 0 passed; 1 failed; 8 filtered out; finished in 0.31s
```

When the leak holds no counted object, only `heap_bytes` (or `arena_bytes`) appears.

### 6.2 Any test, checked with no code

```ignite
test "a session ends and closes its stream" {
  let (client, server) = Pipe.pair()
  spawn serve_one(server)
  client.write_text("quit\r\n")?
}
```

If `serve_one` keeps a channel or a stream after it returns:

```
test examples.terminal/a session ends and closes its stream ... FAILED (leaked 2 objects)

failures:

---- examples.terminal/a session ends and closes its stream leaks ----
kind       before  after
channels        0      1
pipes           0      1
(tasks back at 0 after 0.01 s)
```

### 6.3 A test that leaves a server running

```ignite
test "the server answers on TCP" {
  Runtime.leaves_running("the accept loop runs until the process ends")
  let listener = TcpListener.bind("127.0.0.1:0")?
  spawn accept_loop(listener)
  ...
}
```

### 6.4 A server exporting its census

```ignite
fn runtime(request: Request) -> Response =
  Response.plain(200, lines(Runtime.census(), "todo_"))
```

## 7 Acceptance of the guard

- Seeded leaks, red before any real reproducer: debug builds of `ignite-io` and `ignite-rt`
  read `IGNITE_DEBUG_LEAK=<kind>` (precedent: `IGNITE_DEBUG_HOST_FAULT`,
  `crates/ignite/src/run/process.rs`) and then skip one release per object of that kind:
  `channel` forgets a channel's `Shared` (both targets), `handle` registers an end and never
  releases its slot (LLVM), `heap_copy` never frees a received value's heap copy (LLVM),
  `bytes` leaks 370 B per closed channel and holds no counted object (both targets). A fixture
  under `crates/ignite/tests/fixtures/leak/` with one soak test and one plain test passes
  without the variable and fails with each value, naming that kind (`bytes`: `heap_bytes`), on
  both targets where the kind exists. This is the guard's first failing case.
- G30: the case study's reproducer (requested, see `story:channel-release-on-close`) as the soak
  of 6.1 fails under the guard before the fix, naming the kind that grew, and passes on both
  targets with it. The two stories land in one wave, the guard first; the wave's record shows
  both runs. If the reproducer turns out to need concurrency the soak does not give (a burst
  that only raises the handle table's high-water mark, section 2), the record says so and the
  case is Q11's, not a missed leak.
- `task test-ignite` runs every test of `std/` and `examples/` under the leak check on both
  targets; each test that opts out names a reason, and their count is a count row in
  `quality/baseline.json` (lower is better).
- No new diagnostic code: `Runtime.leaves_running` outside a test is E051, as `Pipe.pair()`.
- `docs/slice.md` gets a section for the census, `std.runtime` and the check, with 12.7's
  output and exit codes extended; `task gate` green.

## 8 Draft stories

Filed in the planning store under `epic:leak-guard`.

| order | story | outcome | depends on |
|---|---|---|---|
| 1 | `leak-census` | per-kind counters and heap bytes on both targets, `std.runtime` (`census`, `settle`, `soak`); seeded leaks red, then G30's reproducer red before the fix | none; `channel-release-on-close` depends on it |
| 2 | `leak-check-tests` | `ignite test` fails a test that leaves a counted object behind; std and examples pass or opt out with a reason | 1 |
| 3 | `runtime-metrics` | `lines(census, prefix)` and the todo example's `GET /runtime` | 1 |

## 9 Open questions

| id | question | recommended default |
|---|---|---|
| Q1 | Leak check on by default or opt-in per test? | on by default; an opt-in check would miss exactly the next leak nobody wrote a test for |
| Q2 | How does the child report a leak to the runner? | exit code 102 and the table on stderr; a test that calls `Process.exit(102)` reads as a leak, documented |
| Q3 | Settle bound after the body | 2 s, no flag; spent only when something is still live |
| Q4 | Counting allocator in production builds | yes, both targets; moved to test builds only if the release's dedicated-host run shows it beyond tolerance, then `heap_bytes` is `None` |
| Q5 | Report where a leaked object was created? | not in story 1 or 2; a later `--leak-sites` could record the source position per object in test builds |
| Q6 | Soak warm-up and slack | warm-up `max(10, runs / 10)`, slack 64 KiB on bytes, counts exact |
| Q7 | Stores never freed on LLVM | resident kind: reported, not compared; `Store.new()` per run in a soak is a known LLVM difference, not a guard failure |
| Q8 | Ends carried in values or sent on a channel are unowned on LLVM by design (`own.rs`) | the census will show them; story 2's sweep files a fix story if a std or example test hits it |
| Q9 | Compare resident size anywhere? | no, report only; the fix story's end-to-end RSS test stays the check of the whole process |
| Q10 | Zero-argument closures for `soak` | `fn(Int) -> Unit` with the run index, which needs none; revisit when closures without parameters exist |
| Q11 | Handle-table segments keep the high-water mark of open handles for the process's life, and `waited` keeps 16 B per waited child | no leak for the guard: reported as `handle_slots` and `pooled_bytes`, not compared; a story to free empty trailing segments is filed only when a server's metrics (5.4) show a burst's memory kept and it matters; `waited` stays by design and a soak over child waits is expected to grow by it |
