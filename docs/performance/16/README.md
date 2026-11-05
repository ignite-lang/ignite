# Slice 16 gate time (`docs/slice.md` 20.12)

Measured 2026-11-05 03:00 to 04:05 local time (UTC+1). 20 cores, shared with other sessions;
start loads 5.7 to 15.5. sccache was on. Every command ran through the session's build-slot
wrapper (`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only`, `RUST_TEST_THREADS=4`,
`CARGO_BUILD_JOBS=8`, `nice -n 10`, `ionice -c3`). The record is in `AGENTS.md`, "Slice 16".

**The run is incomplete.** One cold pair and one warm pair of the gate were collected; the warm
pairs 2 and 3, and the harness's `programs` and `cli` modes, were not. The per-unit tables below
say which.

## Trees and harness

- **Before:** `31b738b`, `main` when the slice merged it for its close (`docs/slice.md` 20.12
  names the slice base after the rebase onto slice 15; the slice branch instead merged `main`
  three times, so the `main` side of the last merge is the tree that differs from "after" in
  slice 16 only, and in nothing else).
- **After:** `426449e`, the slice branch with `main` merged, the merge fix and U6's tests. The
  later commits of the slice change documents only.
- `ignite-perf` at `426449e`, harness SHA256 `bb2deecd…6621` (`records/run.json`), with
  `--comparison ""`.

## What ran

1. `scripts/perf.sh`: `cold` (one pair) completed. To keep the session's build directories
   under their 15 GB budget, the worktree's own target was cleaned while `cold` ran; that removed
   the harness binary, so the script's later modes exited 127 at once (`logs/modes.txt`).
2. `scripts/perf2.sh` rebuilt the harness and ran `warm`. Warm pass 1 completed. During pass 2
   the before tree's target passed 12 GB (the session's stop line; the after tree's was 11 GB),
   and the run was stopped (exit 143). Both targets were then removed with `cargo clean`.
3. `scripts/salvage.sh` measured, with copies of each tree's `debug/ignite` and runner archives
   taken before the clean, the units that need no workspace build: `ignite check` and `ignite
   plan` of the design file and `ignite check` of the todo example (15 interleaved pairs each),
   `todod` and `terminald` per target and profile (3 interleaved pairs, fresh `--out` and
   `--target-dir`, `SCCACHE_RECACHE=1`), and `ignite test slice16/moves_scale.ignite` per target,
   cold and warm, after only, each of its two tests also alone (3 runs). These are `STEP` lines
   (`records/salvage-steps.txt`), not `ignite-performance/1` records; `scripts/pairs.awk` applies
   the same rules (a pair is dropped when its start loads differ by more than 5 or an exit is not
   0; ratio = median of pair ratios) and wrote `records/salvage-pairs.txt`.

`summarize` refused the evidence afterwards ("belongs to another configuration": its build roots
were gone), so `records/summary.md` is the summary the `cold` mode wrote, and the warm pair below
is read from `records/records.jsonl`.

## Results

| unit | before s | after s | ratio | pairs kept/dropped | start loads |
|---|---|---|---|---|---|
| gate, cold | 605.56 (fmt 0.85, clippy 8.67, test 579.63, test-ignite 16.41) | 664.34 (0.93, 5.77, 629.57, 28.07) | 1.10 (1.10, 0.67, 1.09, 1.71) | 1/0 | 7.7 to 12.6 |
| gate, warm after the touch | 450.11 | 516.89 | 1.15 | 1/0 | 5.9, 8.7 |
| `cargo test --workspace`, warm | 440.21 | 507.38 | 1.15 | 1/0 | 8.9, 9.1 |
| fmt, clippy, warm | 1.063, 6.754 | 0.931, 6.430 | 0.88, 0.95 | 1/0 | 5.7 to 8.7 |
| `task test-ignite`, `--no-run` warm and after the touch, `ignite_test::`, rustgen tests, warm | 2.077; 0.042, 4.367; 3.594; 3.943 | 2.138; 0.032, 3.742; 2.708; 3.375 | none valid | 0/1 | 5.9 to 6.1 against 13.7 to 15.5 |
| `ignite check` of the design file | 0.089 | 0.120 | 1.32 | 15/0 | 8.7 to 8.8 |
| `ignite plan` of the design file | 0.090 | 0.121 | 1.34 | 15/0 | 8.7 to 8.8 |
| `ignite check examples/todo/todo.ignite` | 0.056 | 0.068 | 1.21 | 15/0 | 8.7 to 8.8 |
| todod Rust dev / release | 4.362 / 4.890 | 4.402 / 4.613 | 1.18 / 0.91 | 3/0 each | 6.7 to 9.4 |
| todod LLVM dev / release | 0.412 / 0.799 | 0.519 / 0.772 | 1.16 / 0.97 | 3/0 each | 6.7 to 9.0 |
| terminald Rust dev / release | 3.727 / 4.092 | 4.151 / 3.955 | 1.02 / 1.00 | 3/0 each | 7.6 to 9.2 |
| terminald LLVM dev / release | 0.463 / 0.594 | 0.506 / 0.662 | 1.07 / 1.19 | 3/0 each | 7.0 to 8.5 |

After only, medians of three runs at loads 7.5 to 7.8:

| unit | Rust s | LLVM s |
|---|---|---|
| `ignite test slice16/moves_scale.ignite`, cold (fresh `--out` and `--target-dir`) | 3.345 | 0.181 |
| same, warm | 0.031 | 0.017 |
| the test "50000 steps through a call" alone, the runner's `finished in` | 0.00 to 0.01 | 0.01 |
| the test "200000 steps through a call" alone, the runner's `finished in` | 0.00 to 0.02 | 0.01 |

The runner prints `finished in` to 0.01 s, so the two tests' times are under its resolution on
both targets and give no ratio; rows 9 and 10 of 20.0 check linearity with their own runs.

Test counts in the cold and warm gates: before 979 cargo tests passed, 10 ignored; after 1009
passed, 10 ignored; 211 inline tests per target on both trees.

## Rises over 10 %

Measured: `std/` is unchanged between the trees; `crates/*/src` of the compiler crates +2,131
and -113 lines in 13 files (`crates/ignite-types/src/moves.rs` is 1,389 of them);
`crates/*/tests` +6,520 and -512 lines in 83 files (`git diff --shortstat 31b738b 426449e`).

Causes, each inferred unless it says measured:

- **`ignite check` and `ignite plan` of the design file, 1.32 and 1.34; `ignite check` of the
  todo example, 1.21:** the moves analysis runs on every program and on every std module it
  reaches, with one fixpoint per loop and per module (the cost 20.12 expected); list columns of
  E046 are split by length. Not split by phase. std did not grow (measured), so the frontend
  work per line did.
- **todod dev, Rust 1.18 and LLVM 1.16; terminald LLVM release 1.19:** the same frontend cost
  inside each build. The Rust dev ratio comes from two of three pairs (4.36 to 5.15 s and 3.33 to
  4.31 s); the absolute medians are 4.36 and 4.40 s. The release builds of todod fall (0.91,
  0.97): fewer copies and parameters taken by value, inferred from 20.3.4 and 20.3.5, generated
  code size not measured.
- **Warm gate and warm `cargo test --workspace`, 1.15, one pair:** 30 more cargo tests
  (measured), among them the `slice16::` end-to-end tests, each of which builds fixtures with
  `ignite test --target all`, and the `eval::` agreement tests over a larger fixture.
- **Cold `task test-ignite`, 1.71, one pair:** the step builds the compiler and the runner
  before the 211 inline tests per target, whose execution stayed at 0.33 to 0.35 s (measured);
  the rise is the build and the frontend on std, not split.

Known cost not measured here: since wave 3, a pattern binding that is the tail of a branch or an
arm whose value is used is copied, because the analysis does not track pattern bindings
(`slice16::untracked_tails_keep_their_values_on_both_targets`; the draft story
`moves-for-pattern-bindings` would move them).

No baseline moved. The apply suites stay the slice-13 exception.

## Uncollected, with the reason

- Warm pairs 2 and 3, and the warm `task test-ignite`, `--no-run`, `ignite_test::` and rustgen
  units of pair 1 (dropped by the load rule): the run was stopped at the 12 GB target limit.
- The harness's `programs` and `cli` modes as `ignite-performance/1` records: replaced by
  `salvage.sh`'s `STEP` lines over the same commands, without the harness's records.

## Files

| file | content |
|---|---|
| `records/records.jsonl`, `run.json`, `summary.json`, `summary.md`, `tools.json` | the `ignite-perf` records (`ignite-performance/1`) of the cold pair and warm pass 1 and the start of pass 2; `summary.md` is the cold mode's |
| `records/salvage-steps.txt`, `salvage-pairs.txt` | the `STEP` lines of `scripts/salvage.sh` and their pair summary (unit, before, after, ratio, pairs kept/dropped, load range) |
| `logs/modes.txt` | the mode and exit lines of both harness runs |
| `scripts/perf.sh`, `perf2.sh`, `salvage.sh`, `pairs.awk` | the invocations, as run (recorded evidence); `perf.sh` calls an `extra.sh` that a guard stopped (exit 3) and that `salvage.sh` replaces |
