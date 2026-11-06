# Slice 18 gate time and build time (`docs/slice.md` 22.15)

Measured 2026-11-05 20:15 to 22:09 local time (UTC+1). 20 cores, shared with other sessions'
builds; start loads 5.3 to 29.6 (peaks of 36 during the cold gates, `logs/watch-2.txt`). sccache was on. Every harness call ran through the session's
build-slot wrapper (`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only`,
`RUST_TEST_THREADS=4`, `CARGO_BUILD_JOBS=8`, `nice -n 10`, `ionice -c3`). The record is in
`AGENTS.md`, "Slice 18".

**The run is incomplete.** `smoke`, `programs`, `cli`, `apply-cycle` and one `cold` attempt ran;
`warm` did not, and the before tree's apply cycle stopped at its third apply (below). No unit was
taken at a load under 5.

## Trees and harness

- **Before:** `421742b33`, the merge of slice 17 into `main` (PR 34), the slice base of 22.0.
  Its code is that of `6cb41c8ae`, where the slice's implementation started (documents and
  planning-store commits only between them).
- **After:** `661e55b42`, the slice branch with every unit (U1 to U7) and `main` up to
  `bc9dd720a` merged. Between the base and this tree `main` also brought PRs 37 to 47 (among
  them the LSP server, JSON decimals, the program argument list, `fix/rollout-drain-stuck` and
  `fix/llvm-stream-ownership`), so a ratio here is the slice plus those PRs. PR 48
  (`fix/rt-abi-panic-containment`), merged into the slice at the close (`1be6a4b0f`), is not in
  the measured tree.
- `ignite-perf` built at `661e55b42` (`tools/ignite-perf`, with U7's `apply-cycle`), run with
  `--comparison "" --cold-retries 0` (`records/run1/run.json`).

## What ran

1. `scripts/mode.sh run1 smoke programs cli apply-cycle` with `scripts/watch.sh run1` beside it
   (`logs/modes-1.txt`, `logs/watch-1.txt`): each tree's compiler, the todo program per target
   and profile (3 valid pairs), `ignite check` and `ignite plan` of the design file (15 pairs),
   then the apply cycle with both trees interleaved per sample. The before tree's third apply of
   the terminal (measured sample 2) did not complete: its host journal ends in
   `Control("rollout")` answered by `Note("rollout already in progress")`
   (`logs/apply-cycle-baseline-terminal-rust-host.txt`), the wedge `fix/rollout-drain-stuck`
   (PR 46) fixed on `main` after the base. The harness killed the apply at its 600 s bound and
   stopped the mode, so no other apply-cycle unit ran in this call. The before tree was not
   patched.
2. `scripts/sizes.sh run1` (`records/program-sizes.txt`): the line counts of the generated
   files and the executables of the first programs pass, per tree.
3. `scripts/mode.sh run1 programs` (`logs/modes-2.txt`): three more programs passes in the same
   evidence, so the program rows have 6 pairs. The mode exits 1 because the records already hold
   the failed apply-cycle row.
4. `BASE=661e55b42 ROOT=…/run2-root scripts/mode.sh run2 apply-cycle` (`logs/modes-4.txt`): the
   apply cycle of the after tree alone, so the todo units run. The harness has no
   candidate-only switch, so its `baseline` label is the after tree here too; those rows are
   extra after-tree terminal samples, not a before tree. A first attempt into the same roots was
   refused by the harness's ownership check before it ran anything (`logs/modes-3.txt`,
   `records/run2-refused/`).
5. `CLEAN_BASELINE=1 scripts/watch.sh run1` beside `scripts/mode.sh run1 cold`
   (`logs/modes-5.txt`, `logs/watch-2.txt`): the before tree's cold gate, then the after
   tree's; the watch cleaned the before tree's target once the after tree's cold rows began, so
   only one gate target existed at a time.

## Results

Ratios are medians of pair ratios; a pair is dropped when its start loads differ by more than 5
or an exit is not 0.

| unit | before s | after s | ratio | pairs kept/dropped | start loads |
|---|---|---|---|---|---|
| gate, cold (fmt, clippy, workspace test, test-ignite) | 1246.63 (1.22, 20.27, 1182.49, 42.66) | 2366.65 (1.40, 12.76, 2313.06, 39.43), exit 1 | no valid pair: the after tree failed one timing test, and the start loads differ by 7.5 (below); test-ignite alone 0.92 (the harness's per-step pair) | 0/1 | 17.7 against 10.2; steps 10.0 to 20.4 |
| `ignite check`, `ignite plan` of the design file | 0.134, 0.137 | 0.136, 0.144 | 1.05, 1.06 | 15/0 each | 7.1 to 7.6 |
| todo program, Rust dev / release | 7.203 / 10.308 | 6.304 / 8.203 | 0.91 / 0.89 | 6/0 each | 5.3 to 16.2 |
| todo program, LLVM dev / release | 0.513 / 0.994 | 0.856 / 1.499 | 1.53 / 1.42 | 6/0 each | 5.3 to 16.2 |
| apply cycle, terminal design, Rust (run1, interleaved) | 90.536 (1 sample; apply 72.04) | 29.242 (1 sample) | 0.32, not valid (below) | 1/0 | 13.6, 14.2 |

The program rows are the harness's (`records/run1/summary.md`); `todod-<target>-<profile>` is
one `ignite build examples/todo/todo.ignite`, which writes `todod` in the before tree and `todod`
and `health` in the after tree (row 27, as landed). The terminal cycle's one pair is not a
comparison: the before tree's sample already ran into the wedge of PR 46 (its apply took 72 s
against 10 to 15 s for every other apply), and its next sample never completed.

### Row 27: the build is a fraction of rustc's

After tree, dev profile, per pass (LLVM / Rust, seconds): 0.752 / 4.353, 1.705 / 5.107,
0.732 / 7.916, 1.021 / 6.539, 0.848 / 6.069, 0.864 / 7.996. Median LLVM 0.856 s over median Rust
6.304 s = **0.136**, at most 0.15: row 27 holds. The median of the six per-pass ratios is 0.148;
single passes range from 0.092 to 0.334 (one LLVM build of 1.705 s at load 5.3). Before tree:
0.513 / 7.203 = 0.071.

### Row 28: the edit-build-restart cycle

Five measured samples per unit after one discarded warm-up, `records/run2/apply-cycle.json`;
medians in seconds (total; plan with its build, apply until `completed`, first answer), start
loads 7.4 to 29.6.

| unit | tree | samples | total | plan | apply | answer |
|---|---|---|---|---|---|---|
| `apply-cycle-terminal-rust` | after (run2, both labels) | 5 + 5 | 30.870, 28.497 | 14.145, 14.448 | 13.173, 11.889 | 0.000 |
| `apply-cycle-terminal-llvm` | after (run2, both labels) | 5 + 5 | 28.269, 24.667 | 13.930, 12.911 | 12.556, 11.756 | 0.000 |
| `apply-cycle-todo-rust` | after | 5 | 31.458 | 17.372 | 14.085 | 0.001 |
| `apply-cycle-todo-llvm` | after | 5 | 30.821 | 16.326 | 14.494 | 0.000 |
| `apply-cycle-terminal-rust` | before (run1) | 1 of 5 | 90.536 | 18.498 | 72.038 | 0.000 |
| `apply-cycle-terminal-llvm` | before | 0 | uncollected | | | |

The todo cycle's LLVM median (30.821 s) is below its Rust median (31.458 s): row 28 holds,
`"todo_llvm_below_rust": true`. The margin is 2 %: on both targets the cycle is dominated by
`plan --for-apply` (13 to 17 s) and the rollout (12 to 15 s), not by the program build (0.9 s
LLVM, 6.3 s Rust dev above). Where the plan's time goes beyond the build is not measured; the
plan logs are empty and the harness records no finer split.

## Rises over 10 %

Measured growth (`git diff --shortstat 421742b33 661e55b42`): see `records/growth.txt`.

- **The todo program on LLVM, dev 1.53 and release 1.42:** the after tree builds a second
  executable and a larger program (measured, `records/program-sizes.txt`): `program.ll` 1,743 to
  5,184 lines (the durable todo and `examples/todo/health.ignite`), `std.ll` 12,780 to 14,350,
  and a second main (`main_health.ll`, 278 lines) with its own link of `health`. How the time
  splits between the larger IR and the second link is inferred, not measured.
- **Rust dev 0.91 and release 0.89 fall** although its generated `ignite-std` grows from 1,700 to
  2,049 lines and the program crate from 100 to 463 (measured); the Rust rows vary from 4.3 to
  30.6 s within one tree at these loads, so the fall is not attributed.
- **Cold gate, 2366.65 against 1246.63 s:** no valid pair. The after tree's workspace step
  failed `slice16::llvm_state_threading_is_linear`, a timing bound of slice 16 (`200000 steps
  took 0.08 s, 50000 steps 0.01 s`, at the runner's 0.01 s resolution, load 10 to 36;
  `logs/cold-candidate-workspace-results.txt`); the test and its fixture are unchanged, while `crates/ignite-llvm/src` changed by 492 inserted lines (PRs 45 and 47 from `main`: the program argument list, stream ownership), so whether the time is load or generated code is not measured; the final gate's run of it is in `AGENTS.md`. Measured: the
  workspace step runs 1,188 cargo tests against 1,090 (11 ignored in both), and the `crates/ignite`
  end-to-end binary takes 2184.42 s (440 tests) against 1042.08 s (381); its new files are the
  slice's live suites (`slice18::`, `slice18_adversary::`, `apply_way_out::`, `apply_integrity::`, the durable
  `todo::`) and `main`'s (`rollout_repeat::`, `stream_ownership::`). The split of the rise
  between those tests and the load (10.1 at the after step's start against 20.4, with peaks of
  36 during it) is not measured. Inline tests: 233 per target before, 294 plus 28 (the LSP
  example's own executable) after, 39.43 against 42.66 s cold.

No baseline moved. The apply suites stay the slice-13 exception.

## Uncollected, with the reason

- **`warm`** (gate, `cargo test --workspace`, fmt, clippy, `task test-ignite`, `--no-run`,
  `ignite_test::`, rustgen tests): its interleaved passes need both trees' gate targets built at
  once (10 to 12 GB each); this unit's scratch and target are bounded at 15 GB together, so one
  tree's gate target at a time. Slice 17 left it uncollected for the same reason.
- **`apply-cycle-terminal-<target>` of the before tree:** Rust stopped after 1 of 5 measured
  samples, LLVM never started: the before tree lacks `fix/rollout-drain-stuck` (PR 46), and its
  third rollout of one host is refused as `rollout already in progress` (step 1). Not patched.
- **`health` timed apart from `todod`:** `ignite build` builds both; row 27 times the program
  (22.0 row 27, as landed).

## Files

| file | content |
|---|---|
| `records/run1/` | the `ignite-perf` records (`ignite-performance/1`) of smoke, programs, cli, the before tree's apply cycle and the cold attempt: `records.jsonl`, `run.json`, `summary.json`, `summary.md`, `tools.json`, `apply-cycle.json` (the source manifests, 0.6 MB each, stay in the scratch directory) |
| `records/run1-records-after-first-modes.jsonl` | `records.jsonl` as it stood after step 1 |
| `records/run2/` | the same for the after tree's apply cycle |
| `records/run2-refused/` | the refused first attempt of step 4 |
| `records/program-sizes.txt`, `growth.txt` | generated file sizes per tree; `git diff --shortstat` of the trees |
| `logs/` | mode and watch logs, the host log of the before tree's wedged cycle, the result lines of both cold workspace steps, as `.txt` (`*.log` is ignored by git) |
| `scripts/` | the scripts as run; their paths point into the session's scratch directory |
