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
