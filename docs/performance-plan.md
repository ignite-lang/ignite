# Performance budget recovery

Status: implementation in progress, 2026-11-02. The first implementation checkpoint is
`631be24`, based on published slices 11–12b (`9273349`). Its full correctness close passed,
but cold validation then exposed a shutdown-journal race and registry contention; the
failed measurements are retained in `docs/performance/12c/failed-checkpoint/`.
Batch 12c remains open until every performance criterion below passes; do not start slice 13.

Checkpoint `d8a9cb7` recovered the strict generator-test and file-level CLI-test
budgets. Its remaining warm misses and all raw evidence are in
[`performance/12c/checkpoint-d8a9cb7/`](performance/12c/checkpoint-d8a9cb7/).
The next implementation uses complete-workspace parallel formatting, identical
JSON output with fewer tiny writes, and removal of six proven-empty Cargo
harnesses. The integrated gate passes 654 Rust tests, five ignored, 148 inline
cases per backend and four example builds. These are correctness results;
final frozen performance and LLVM readiness validation remain outstanding.

## Result required

Restore the original 11b budget with the current behaviour and coverage. A measured reason
for a regression helps choose a fix; it does not satisfy this batch's acceptance. Do not
raise the baseline, remove tests, increase ignored counts, relax diagnostics or restart
bounds, weaken effect durability, or substitute a smaller gate.

The final 12b measurements are recorded in `AGENTS.md` under “Slice 12b”. Approximate
ceilings below are 1.10 times the paired 11b absolute medians; the actual acceptance statistic
is the median of matched per-pair ratios, not a ratio of independently selected medians.

| Unit | 11b reference | Published 12b | Planning ceiling | Reduction needed from 12b |
|---|---:|---:|---:|---:|
| cold gate | 57.141 s | 66.111 s | 62.855 s | 3.256 s |
| warm gate after checker touch | 13.374 s | 16.786 s | 14.711 s | 2.075 s |
| warm workspace tests, build included | 10.142 s | 12.413 s | 10.800 s | 1.613 s |
| warm fmt | 0.447 s | 0.523 s | 0.492 s | 0.031 s |
| warm clippy | 2.166 s | 2.713 s | 2.383 s | 0.330 s |
| warm `task test-ignite` | 0.663 s | 1.140 s | 0.729 s | 0.411 s |
| design-file `check` | 0.062 s | 0.090 s | 0.068 s | 0.022 s |
| design-file `plan` | 0.063 s | 0.091 s | 0.069 s | 0.022 s |
| workspace `--no-run` after touch | 2.233 s | 2.594 s | 2.456 s | 0.138 s |

The warm workspace-test ceiling deliberately retains the earlier 10.8 s goal, stricter than
1.10 times the latest paired median. Establish it under comparable low load; never discard a
slow result merely because it misses the target. Preserve the already-passing limits too:
`ignite_test.rs` ≤4.6 s; all four todo build variants and warm `--no-run` ratio ≤1.10.

## 0. Freeze inputs and make the result reproducible

Owner: benchmark harness and repository documentation, following `docs/slice.md` 15.1.

- Preserve the exact recovered 11b tree currently at
  `/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-before` as a
  benchmark-only Git ref with a source manifest. Label it as a recovered snapshot based on
  `60aef38`, not as a historical commit that already existed. Verify its source against the
  retained 12b measurement manifest. Do not use `9273349` as a replacement baseline.
- Retain `9273349` as the published before-fix checkpoint, and preserve the slice-12-close
  snapshot for explaining the gains already obtained. Copy the small measurement records,
  phase summaries and restart samples needed for review into durable repository evidence;
  local cache paths alone are insufficient for another developer to reproduce the result.
- Add a repository-owned measurement entry point which accepts baseline/candidate refs and
  separate source/build/evidence directories. Reuse the existing measurement scripts after
  removing retired worktree paths. Record toolchain, environment, command, source revision,
  load before/after, wall time, exit code, test counts, sample exclusions and pair ratios.
- Keep `CARGO_INCREMENTAL=0`, dev debug `line-tables-only`, release debug 0, the same toolchain
  and the same sccache policy on both sides. Cold means fresh owned Cargo targets and fresh
  generated-program outputs with `SCCACHE_RECACHE=1`; warm means a prewarmed tree, then the
  same `ignite-check/src/lib.rs` touch before the gate. Include preparation consistently.
- First smoke-check the harness with one small program build and plan comparison; inspect
  the records before expanding. Then capture one short before-fix attribution run.

Acceptance: a fresh checkout can recover the exact baseline and run the same comparison;
records distinguish cold, warm, diagnostic and readiness runs. A failed command gives the
whole measurement run a nonzero aggregate exit even when later units are collected.

## 1. Remove repeated std preparation from generator tests

Owner: `crates/ignite-rustgen/tests/common/mod.rs`. Keep production loading in the driver
and semantic checking in `ignite-check`/`ignite-types`; do not create another checker.

Evidence: normal workspace logs show rustgen integration execution rising 1.60→3.13 s,
while Ignite end-to-end execution is essentially unchanged at 4.93→4.91 s. The helpers
`generate_source`, `lower_with` and `lower_tests_with` each call `load_std` and `type_std`.
The expanded control module is consequently prepared repeatedly inside one test process.

- Share immutable parsed/checked/typed std fixtures, their exported modules and source maps
  for the fixed repository and golden std roots. Use a test-process cache and borrowed data
  where practical; measure whether cloning the cached structures erases the gain.
- Keep the raw loading/typing helpers available for tests that exercise them. Custom or
  mutable fixture roots must reload, so a source edit or invalid module cannot be hidden by
  the fixture cache. Preserve typed/untyped fallback behaviour for older goldens.
- Do not cache generated std per root alone: item reachability, selected tests, program
  source locations and emitter options differ between calls.

Acceptance: all rustgen goldens/build tests pass unchanged; the crate's integration run is
at most its matched 11b time (planning target ≤1.6 s). Re-run the workspace test step to
verify the gain survives integration. This is the first implementation checkpoint because
its measured regression accounts for much of the required workspace recovery. The expected
saving is a hypothesis until measured; do not subtract isolated timings to declare a gate
pass.

## 2. Reduce full inline-test overhead without reducing execution

Owners: preparation/emission in `crates/ignite/src/test_build.rs`; execution in
`crates/ignite-io/src/test.rs`; scheduling in `Taskfile.yml`. Both backends keep their existing
emission owners. The runner already executes isolated children concurrently using `--jobs`;
adding a second runner or merely increasing concurrency is not the default fix.

The aggregate 0.663→1.140 s rise is measured, but its internal split is not yet established.
Profile one fixed, prebuilt inline suite in an isolated example: separate CLI startup, std
parse/check/type, Rust/LLVM emission, up-to-date compiler/link checks, runner startup, child
waiting and test bodies. Measure both targets separately and together. Keep instrumentation
out of normal programs and the final timing binary.

Select the changes the profile supports, in this order:

1. Remove repeated immutable preparation and source-map construction inside one invocation.
2. If duplicate frontend work across the two target invocations is material, design a shared
   prepared-input path owned by `test_build`. Review the additive API/CLI contract before
   implementation; preserve existing single-target invocations. Both emitters must consume
   the same checked input, and both sets of tests must actually execute.
3. If child waiting dominates, inspect the existing 5 ms `try_wait` polling and stdout/stderr
   reader setup. Use existing process/wait facilities to reduce detection/reader overhead
   only if profiling supports it. Preserve child isolation, timeout enforcement, cleanup,
   stable report ordering and the existing `--jobs` cap. No busy waiting or hidden serial
   fallback in the measurement path.

Acceptance: ≤0.729 s illustrative warm ceiling and matched ratio ≤1.10; all 148 existing
inline cases execute on each target. Filters, list mode, failure output, timeout and exit
status behaviours stay covered. No tests are moved out of the measured task.

## 3. Cut the design frontend's actual hot paths

Owners: `ignite-syntax`, `ignite-check`, `ignite-types`, then `ignite-eval`; driver orchestration
stays in `crates/ignite/src/lib.rs`. Preserve O45 and the canonical `std.control` type names.
Splitting the control module into cyclic pieces or renaming its public types would break
ownership and journal/model compatibility, rather than resolve the measured cost.

Evidence from disposable profiling, in milliseconds:

| std phase | 11b | 12b | `std.control` share in 12b |
|---|---:|---:|---:|
| parse | 10.12 | 13.71 | 6.65 |
| name/effect check | 11.97 | 16.20 | 7.74 |
| type check | 24.55 | 36.65 | 21.41 |

Program preparation stays around 0.58/0.72/1.85 ms. Evaluation check rises 3.68→6.73 ms.
The design legitimately reaches the reducer module; excluding it from the roots is invalid.

- Obtain function/allocation profiles of that module on the fixed design input. Investigate
  repeated export/signature cloning, name resolution, repeated inference/effect walks and
  exhaustiveness work using the existing checker extension points. Also profile model and
  hook evaluation; optimizing only parse will not recover the required ≈22 ms.
- Implement the largest measured repeated work first: share immutable export/signature data
  within a compilation and memoize only queries whose context is part of their key. Check
  whether the production API or the caller is causing repetition before adding a cache.
- Preserve full checking of reached modules, including unused bodies in those modules.
  `module("…")`, transitive roots, custom std roots, diagnostics, fuel limits and source
  positions must remain correct. Keep reducers in Ignite and the evaluator independent of
  compiled-program execution.
- If local hot-path fixes cannot recover the budget, require a separate reviewed design
  before introducing a prevalidated std image/cache. It must validate exact source contents,
  transitive dependencies, compiler semantics and paths; report cold/miss and warm/hit costs
  separately. Its generation and invalidation costs count in the cold gate. A daemon,
  prewarmed hidden cache, or skipped validation cannot make this acceptance pass.

Acceptance: design `check` and `plan` each ratio ≤1.10 (illustrative ceilings 68/69 ms),
byte-identical model/explain goldens and unchanged diagnostics. Verify the same declaration
through both native builds and `run`/replay, not only through a frontend microbenchmark.

## 4. Close compiler-tooling and cold-build residuals

Owners: the measured Rust packages and existing Cargo/Taskfile configuration.

After steps 1–3, rerun only the still-failing units and attribute them before another patch.
For fmt/clippy/touch rebuilds, retain package timings and Cargo unit timings: the measured
warm clippy driver library grew 0.17→0.48 s, driver test library 0.19→0.50 s, and E2E check
0.52→0.72 s. Durations overlap and cannot be added as wall time.

- Separate test execution savings from compiler/link savings. Remove duplicated test
  preparation and unnecessary work/instantiations in their existing owners. Profile
  repeated conversion/clone work and test-helper monomorphization before restructuring it.
- Use the existing integration-binary layout. Do not split crates simply to move cost out
  of one measurement: require evidence that any dependency-boundary change reduces the
  complete cold and warm gate, and preserves architectural ownership.
- Formatting must still check every Rust source and clippy must still check the whole
  workspace/all targets with warnings denied. Do not exclude new files, change the checker
  touch, turn off checks, or alter profile settings on only the candidate side.
- If an optimization trades a faster warm path for extra startup, code generation or image
  construction, charge the latter to cold builds. Preserve all four already-passing todo
  build budgets and measure terminal builds as an integrated guard.

Acceptance: every remaining row in the budget table is ≤1.10, plus warm workspace ≤10.8 s
under comparable low load. If a unit still misses, continue with its next measured cause;
recording an explanation alone does not close 12c.

## Execution and final proof

Run step 1 first; use its result to size steps 2–4. Independent review may inspect the frontend
and benchmark contracts while a single owner runs builds. Keep tightly coupled diagnosis
under one owner and run only one Cargo/Task build at a time on the shared machine. Use managed
worktrees, leases and separate bounded build directories. Preserve benchmark evidence outside
ignored output and retain the recovered baselines.

After each meaningful change, run its focused correctness check and the smallest affected
integrated workflow. Advance to broad measurements only after it passes. When a failure
recurs, revisit the cause; retain failed samples rather than retrying until a green one
appears. Never edit a source snapshot while measuring it.

Final closure requires:

1. Full format/clippy/workspace gate, at least 647 Rust tests with the existing five ignored
   cases unchanged, and all 148 existing inline cases on both targets. Additional regressions
   may increase counts; no behaviour is dropped to save time. All four example builds pass.
2. The terminal runs on Rust and LLVM: kill/restart, draining rollout, probe timeout and
   standing updates, stop, and replay every resulting journal. Keep the real-clock restart
   bound 100–500 ms, manual-clock exact backoff, and sync after every effect before dispatch.
3. Three interleaved warm pairs per timed unit and 15 design check/plan pairs against the
   frozen 11b source. Target comparable low load for the absolute 10.8 s goal, but use the
   established start-load-delta ≤5 rule, preserve raw samples, and exclude every failed pair.
   Never wait indefinitely for low load.
4. One fresh cold pair against 11b, plus a comparison against the published `9273349` tree;
   include the whole gate, all component steps, todo's four build variants, test executable
   count/size and no-run/touch costs. Allow one documented retry for a load-mismatched cold
   pair. If evidence remains invalid or insufficient, leave the performance gate open.
5. Every warm unit has at least three valid warm pairs (15 for check/plan), all ratio
   medians ≤1.10, warm workspace ≤10.8 s at comparable load, and `ignite_test.rs` ≤4.6 s.
   Use at most two additional warm passes to replace load-mismatched pairs or documented
   unchanged-baseline failures; then report an evidence gap rather than select favourable
   runs. Any candidate correctness failure blocks closure until diagnosed and fixed; freeze
   the corrected candidate and restart its affected validation batch. Extra passes cannot
   collect three green samples around an unresolved candidate failure. Aggregate gate savings
   never excuse a remaining required-unit regression.
6. Record the exact source refs, commands, paired results, counts and retained evidence in
   `AGENTS.md`; update `PLAN.md` to performance complete only after these criteria pass.
   Publish the fixes and finish/archive managed trees. Explanations cannot move the baseline.
7. After all required acceptance evidence is retained and the fixes are merged
   to the default branch, validate that merged version and publish a new tagged
   GitHub release, as requested on 2026-11-02. Follow the repository's package
   version and release history; release notes must report verified results.
