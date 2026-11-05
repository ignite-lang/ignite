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
