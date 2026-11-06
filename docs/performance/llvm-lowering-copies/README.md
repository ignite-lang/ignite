# LLVM lowering without avoidable copies (story:llvm-lowering-copies)

The steps of `docs/research/review-2026-11-05-llvm.md` (P1 to P9), one section per step.
Base is `56dbd1ad7` (`main`). Machine: 20 cores shared with other sessions' builds, Linux
6.18, clang 23.1.1, rustc 1.99.1; the load average of each run is given with it.

Method:

- Programs: `bench.ignite` (this directory) and the tests of
  `crates/ignite-llvm/tests/golden/copies.ignite` at the step's commit, built with
  `ignite test FILE --release --target=llvm` (clang `-O2`) against the release runtime
  (`cargo build -p ignite-rt --features runner --release`); each test executable is kept
  per step.
- Timing: the two executables of a comparison run the same test with `--run-one NAME`,
  interleaved, order alternating per round, 11 rounds; wall time per run in ms; the
  median is given.
- Calls: `valgrind --tool=callgrind` of one `--run-one` run; call counts from
  `callgrind_annotate --tree=calling`.
- IR counts: `grep -c` over the `program.ll` that `ignite test` wrote for the review's
  reproducer `probe.ignite` (the review's text, with the expected order of the Text keys
  in "map key order by kind" corrected to byte order: `"aa"` sorts before `"a\u{FFFD}"`;
  the review's expectation fails on both targets before any change).

## P9: an allocation that fills its chunk exactly stays inline

`@ignite.alloc` compared `end < limit`, so a request ending at the chunk's last byte
took the runtime's slow path (which then served it from the same chunk). Now `end <=
limit`, and a null `limit` (no chunk last) still sends even a zero-size request to the
runtime, which returns a non-null pointer.

| measure | base | P9 |
|---|---|---|
| `program.ll` of `probe.ignite`: calls of `ignite_rt_alloc` | 2 sites | 2 sites (the IR has the same calls; the change is which path runs) |
| `ignite_rt_alloc` calls, copies test "one-byte texts that fill arena chunks to the last byte keep their bytes" (200,000 one-byte texts kept) | 71 | 63 |
| `ignite_rt_alloc` calls, `bench text concat` (4,000,000 rounds) | 1 | 1 |
| wall time, copies test, median of 11 (load 17.4) | 8 ms | 8 ms |
| wall time, `bench text concat`, median of 11 (load 16.3 to 17.4) | 692 ms | 724 ms |
| same, again, median of 21 (load 11.0 to 11.7) | 732 ms | 725 ms |

The slow path ran 8 times fewer per 200,000 allocations; no wall-time change beyond the
noise of the shared machine. Kept: it is the review's one-line fix, and the slow path it
avoids is a call with the runtime's state hand-over each time.

## P1: `Text` concatenation inline, one block per chain

`a + b` and `a.concat(b)` called `ignite_rt_text_concat`, which built a heap `Vec` of both
operands and copied it into the arena; a `+` chain then appended further operands with
`@ignite.append` (growing, so copying the prefix again), and a `.concat` chain made a new
text per step. Now the whole left spine of a chain, `+` and `.concat` mixed, is lowered
inline: operands evaluated left to right, adjacent literals joined into one constant, one
`@ignite.alloc` of the summed length and one `memcpy` per operand; past 16 non-literal
operands the text grows by `@ignite.reserve` (doubling) per further batch of 16. The
spawn thunks' error line (`log_error`) uses the same inline copy.

| measure | base | P1 |
|---|---|---|
| `probe.ignite` `program.ll`: calls of `ignite_rt_text_concat` | 24 | 0 |
| same: calls of `@ignite.append_text` | 4 | 2 (the debug glue's) |
| `probe2.ignite` `program.ll`: calls of `ignite_rt_text_concat` | 9 | 0 |
| `bench text concat` (4,000,000 rounds, 5 concatenations each): `ignite_rt_text_concat` calls | 20,000,000 | 0 |
| same: `malloc` calls, whole process | 20,000,029 | 32 |
| same: instructions (callgrind `Ir`, whole run) | 8,964,833,093 | 1,696,792,062 |
| wall time, `bench text concat`, median of 21 interleaved (load 17.5 to 18.4) | 877 ms | 157 ms |

`slice9::long_chains_100k_lower_quickly` and `slice9::long_chains_100k_run_on_llvm`
(`#[ignore]`, run by hand with `--include-ignored`) pass with the new lowering, as do the
10,000-term chains of the gate.

## P2: an aggregate constant is evaluated once

A mention of a constant called its function, which rebuilt the value (a `.concat`, a list
literal, a record) in the task's arena every time. Now a scalar constant stays a call LLVM
folds, and an aggregate one is evaluated once per process and IR module: a mention loads the
module's `"<NAME>.cache"` (acquire) and, while it is null, calls `"<NAME>.once"`, which
evaluates the constant, copies it to the heap with the `Store` glue (`copy_to_heap`) and
publishes it by compare-and-swap. The mention is borrowed, as a variable is: value positions
copy it, and an in-place builtin on a constant (`TAGS.push(x)` type-checks) changes a copy,
because `mutable_receiver` changes in place only a place rooted at a local. Constant
patterns use the same cache. LLVM target only: the Rust target (`crates/ignite-rustgen`)
is outside this story's scope and still evaluates a constant per mention.

| measure | base | P2 |
|---|---|---|
| `probe.ignite` `program.ll`: calls of `main.GREETING` / `main.TAGS` | 2 / 2 (one per mention) | 1 / 1 (inside `.once`) |
| `bench constants` (4,000,000 rounds, 4 mentions of 3 aggregate constants each): calls of the constants' functions (callgrind) | 16,000,000 (GREETING 4M, TAGS 4M, WORDS 8M) | TAGS 1, WORDS 1; GREETING's function inlined into its `.once` |
| same: instructions (callgrind `Ir`) | 3,596,432,908 | 172,439,498 |
| wall time, `bench constants`, median of 21 interleaved (load 14.5 to 15.6) | 333 ms | 9 ms |
| same, against P1 (P9 and P1 applied, constants not), median of 21 (load 7.1 to 7.3) | 262 ms (P1) | 10 ms |
| wall time, `bench text concat` against P1, median of 11 (load 13.7 to 14.5) | 124 ms (P1) | 121 ms |
| Rust target, `bench constants`, `ignite test --release`, 5 runs before any change (load 17 to 22) | 5.01 to 6.12 s | not changed by this step (no Rust generator change) |

## P7: join and trim write their result once

PR B's base is `d7d65b43f` (`main` after P9, P1 and P2); its programs are `bench-b.ignite`
(this directory), timed against base with 21 interleaved rounds.

`text_join` collected the parts in a `Vec`, joined them into a second heap `Vec` and copied
that into the arena: two `malloc`s and two copies per call. Now it sums the lengths and
writes parts and separators into one arena block. `text_trim` ran a UTF-8 pass over the
whole text; now it drops the ASCII whitespace of `White_Space` (tab, LF, VT, FF, CR, space)
from both ends and decodes only when an end left is a non-ASCII byte, which keeps
`str::trim`'s answer (unit test `trim_keeps_what_str_trim_keeps` compares the two on
Unicode spaces, NEL, U+2028/9, NBSP and invalid UTF-8). One difference on invalid UTF-8,
which no runtime or generated operation produces: a VT at an ASCII end is now dropped (the
old `trim_ascii` fallback kept it). `text_debug` of printable ASCII with no `"` or `\`
writes the quoted bytes straight into the arena instead of building a `String`.

The stdout part of the review's P7 (`Stdout::write_from(&[u8])`) is not done: it changes
`ignite-io`'s `Stdout::write_all`, outside this story's crates.

Both steps are runtime calls with no change to `program.ll`; the counts are runtime calls.

| measure | base | P7 |
|---|---|---|
| `bench text join` (1,000,000 joins of 16 words): `malloc` calls, whole process | 2,000,033 | 33 |
| same: instructions (callgrind `Ir`, whole run) | 1,708,435,891 | 1,370,435,334 |
| same: `Ir` inside `ignite_rt_text_join` | 1,665,000,147 | 1,327,000,000 |
| wall time, `bench text join`, median of 21 interleaved (load 11.4 to 11.7) | 112 ms | 87 ms |
| `bench text trim` (4,000,000 trims of a 24-byte ASCII text): `Ir`, whole run | 1,424,435,537 | 872,435,127 |
| same: `Ir` inside `ignite_rt_text_trim` | 1,336,000,000 | 784,000,000 |
| wall time, `bench text trim`, median of 21 interleaved (load 11.2 to 11.4) | 106 ms | 72 ms |

Tests that catch a wrong result: `crates/ignite-rt/tests/abi.rs` (join of multibyte parts
with a multibyte separator, an empty part, a 70,000-byte part, an empty separator; debug of
plain, empty and escaped texts) and the copies test "join, trim and their edge cases"
(NBSP and VT/FF ends, a non-whitespace control byte, empty and all-space texts), run on
both targets.

## P6: a read with a large maximum takes only the bytes read

A `read(maximum)` above 16 KiB (the HTTP server reads with 65,536) asked the arena for a
`maximum`-byte block, which as a large request is a zeroed `calloc` block of its own that is
not trimmed and ends the task's bump chunk. Now such a read goes into a per-thread buffer
(kept up to 64 KiB; a fresh `Vec` when the buffer is in use) and only the bytes read are
copied into the arena; reads up to 16 KiB still go straight into an arena block. The review
named a per-task buffer: the read does not yield between filling the buffer and copying out
of it, so one buffer per worker thread is enough.

| measure | base | P6 |
|---|---|---|
| `bench small reads of maximum 64 KiB` (20,000 request/answer round trips over a pipe, a 100-byte request and a 1-byte answer, both read with maximum 65,536): `calloc` calls (callgrind) | 40,000 (2,630,240,030 `Ir`) | 0 from reads |
| same: instructions (callgrind `Ir`, whole run) | 2,714,564,088 | 65,396,310 |
| same: wall time, median of 21 interleaved (load 16.4 to 17.8) | 311 ms | 179 ms |
| `bench pipe reads of 64 KiB` (reads that fill the maximum): `calloc` calls | 264 (17,358,280 `Ir`) | 2 (125,265 `Ir`) |
| same: `Ir`, whole run | 109,138,427 | 108,956,472 |
| same: wall time, median of 21 interleaved (load 11.2) | 15 ms | 15 ms |

When a read fills its maximum the saved zeroing is spent again on the copy, so that bench
does not change; a read that returns less than its maximum, the usual case of a server
reading a request, no longer pays for zeroing the whole maximum. Under callgrind the
`bench pipe reads of 64 KiB` run stops early on both builds (exit 101 after 264 reads; not traced, likely the
10 s idle timeout with the writer slowed by valgrind), so its counts cover the same
264 reads on both; outside valgrind it passes.

Tests that catch a wrong result: the copies test "reads over 16 KiB keep their bytes and
the texts made between them" (110,017 patterned bytes written as 40,000, 70,000 and 17,
read with maxima 65,536 and 20,000 alternating; each read checked against its maximum, the
bytes compared whole, and texts allocated between reads checked afterwards), on both
targets.

## P8: a map compares Text keys inline

Each probe of a `Map<Text, _>` binary search called `ignite_rt_text_cmp`, which entered the
runtime (`crate::entry`) to compare two slices. Now `glue::compare` of `Text` calls the
`alwaysinline` IR helper `@ignite.text_cmp`: `memcmp` of the common length (skipped when it
is 0, since an empty text's pointer may be null), then the shorter first. That is Rust's
`[u8]` order, the order `ignite_rt_text_cmp` gives, so a map's keys keep their order. The
O(n) `memmove` of `Map.put` (the review's known O11) is not changed.

| measure | base | P8 |
|---|---|---|
| `bench-b.ignite` `program.ll`: calls of `ignite_rt_text_cmp` | 1 (in `map.find.Map<Text,Int>`) | 0 |
| `bench map with text keys` (400 rounds of 512 puts and 512 gets): `ignite_rt_text_cmp` calls (callgrind) | 3,197,600 | 0 (3,197,600 `memcmp` calls inline) |
| same: instructions (callgrind `Ir`, whole run) | 438,246,089 | 276,452,453 |
| wall time, `bench map with text keys`, median of 21 interleaved (load 11.2) | 53 ms | 39 ms |

Tests that catch a wrong result: the copies test "map keys of multibyte and prefix texts
sort bytewise" (keys `""`, a prefix and its extensions, `é`, a text with byte 0xFF; `keys()`
order, `get` of present, absent and equal-but-distinct keys, overwrite and remove), on both
targets, and `copies::text_keys_compare_inline`, which checks that `map.find` over `Text`
keys calls `@ignite.text_cmp` and no runtime comparison is left.

## P3: a fn value that only reads a parameter gets the place

PR C's base is `586556ad6` (`main` after PR B); its programs are `bench-c.ignite` (this
directory), timed against base with 21 interleaved rounds.

A call through a fn value copied every borrowed aggregate argument, and `list.map` copied
each element before passing it, because a fn value's parameters are owned: the caller does
not know the callee. Now, where the caller does know it, a borrowed place goes as its own
pointer to a parameter the callee only reads (`Lower::known_borrows`): a closure literal,
or a `let` bound to one, only reads the parameters its body does not modify
(`region::modified_by`; nothing inside a closure body moves, so a read that keeps the value
copies it), and a named fn only reads the parameters it does not consume. `list.map(f)`
with such an `f` passes each element as its pointer; a call through such a `let` passes a
borrowed argument as its pointer unless a later argument modifies a variable or mentions
the place's root.

The review's fix, "borrow unmodified closure-literal parameters", was first tried as a
change of the fn-value convention (every fn value borrows; a closure copies a modified
parameter on entry, the `fnval.` thunk copies for a consuming named fn). The golden
`slice16.program.ll` showed the cost: an actor's state moved into `step` through a fn value
(`run_actor`) gained a copy in the thunk on every message, where it had none. That version
was dropped; the one above changes no callee and no convention.

| measure | base | P3 |
|---|---|---|
| `bench-c.ignite` `program.ll`: calls of `copy.` glue in the map, named-fn map and fn-value tests | 3 | 0 |
| `probe.ignite` `program.ll`: calls of `copy.` glue | 174 | 172 ("list map copies and transforms", and a fn-value call with a `Text` argument) |
| `bench map with a closure` (2,000 maps over 1,000 records of a text and 16 ints): `copy.main.Entry` calls (callgrind) | 2,000,000 | 0 |
| same: instructions (callgrind `Ir`, whole run) | 350,874,939 | 22,090,500 |
| same: wall time, median of 21 interleaved (load 12.7) | 34 ms | 2 ms |
| `bench map with a named fn` (the same with a named fn that reads its record) | 2,000,000 copies, `Ir` 350,874,951, 36 ms | 0, `Ir` 22,090,512, 2 ms |
| `bench fn value calls` (1,000,000 calls of a `let` closure on a 256-int list): `memcpy` calls | 1,000,007 | 0 (no `malloc` or `memcpy` above the threshold) |
| same: `Ir` | 272,442,229 | 20,442,283 |
| same: wall time, median of 21 (load 12.7 to 15.1) | 22 ms | 2 ms |

Tests that catch a wrong result: the copies test "fn values borrow what they only read and
copy what they change", on both targets: a closure and a named fn that change their
parameter leave the list's elements unchanged, a closure that returns its parameter gives
a list independent of the original (each changed afterwards), a later argument that
changes the variable or moves it into a consuming fn leaves the earlier argument as it was
evaluated. `copies::readers_and_spawns_take_places` checks the IR of `lengths`, `sums` and
`measured`: no list copy.

## P4: closure captures moved at their last use (skipped)

A closure copies each capture into its environment when it is created. The review proposed
a shallow `memcpy` when `TypedProgram::moves` marks the capture's last use. It never does:
the moves analysis (`crates/ignite-types/src/moves.rs`) records a closure's captures as
reads, not as move positions, and that crate is outside this story's scope. Deciding the
move in `ignite-llvm` would duplicate that analysis, and a shallow copy is unsafe for a
capture that does not own its memory (a borrowed parameter, a `for` element whose list is
changed in place later), which only the moves analysis knows. Not done; the cost it would
address, measured on base and P3 alike:

| measure | base | after P3 and P5 |
|---|---|---|
| `bench closure capturing a list` (100,000 closures capturing a 1,000-int list): `memcpy` calls, `Ir` share | 100,009, 78,300,923 of 85,157,151 `Ir` (92%) | the same |
| same: wall time, median of 21 (load 15.1) | 9 ms | 9 ms |

In this bench the list is used again in the next round, so its capture is no last use and
a move would not apply either; it shows what one capture copy costs.

## P5: a spawn hands over a borrowed argument's place

`spawn f(x)` copied a borrowed aggregate argument into an environment in the parent's
arena, and the task's thunk copied it again into the task's arena. The parent waits in
`ignite_rt_spawn` until the task has made its copy (`spawn_ready`, or the task ended), so
the place stays unchanged meanwhile. Now the environment holds the place's pointer and the
thunk copies from it; an owned argument (a temporary, a moved place) is moved into the
environment as before.

| measure | base | P5 |
|---|---|---|
| `bench-c.ignite` `program.ll`: calls of `copy.` glue in the spawn test | 1 | 0 |
| golden `bounds.program.ll`: the spawn of `Echo.serve` with a borrowed `Text` | `copy.Text` in the parent, `copy.Text` in the thunk | a pointer store in the parent, `copy.Text` in the thunk |
| `bench spawn with a borrowed list` (300 spawns of a task that reads a 100,000-int list, each waited for): `calloc` calls (large arena blocks) | 620 | 320 |
| same: `memcpy` calls of the list | 631 (481,035,820 `Ir` in `calloc`, 484,082,894 in `memcpy`) | 331 (240,939,687 and 244,080,158) |
| same: instructions (callgrind `Ir`, whole run) | 969,694,909 | 489,397,893 |
| same: wall time, median of 21 interleaved (load 15.1) | 41 ms | 22 ms |

Tests that catch a wrong result: the copies test "a spawned task gets the borrowed
arguments as they were at the spawn" (a variable, a record field and a parameter passed on
by a fn are changed or cleared right after the spawn; the task reports what it got through
a pipe), on both targets. `copies::readers_and_spawns_take_places` checks that `start`
copies no list and that the spawn thunk does.
