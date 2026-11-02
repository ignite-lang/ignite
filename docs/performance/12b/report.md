Slice 12b (2026-11-02, completed after recovering the quota-interrupted work). The 11b
baseline stays unchanged. Functionality passes; the timing exceptions below are measured and
recorded, not a new speed baseline. Evidence is retained outside build output at
`/home/timo/.cache/scratch/ignite12b-codex/measure/`: `records.jsonl`, `summary-final.md`,
`report.md`, source hashes, per-step logs, Cargo timing HTML/JSON, and restart journals.

Method: one exact-final cold before/final/slice-12 pass with fresh owned targets and
`SCCACHE_RECACHE=1`, starting at loads 3.71, 4.66 and 0.98; three interleaved warm passes over
`s12-before`, final and `s12-p4-after2` (loads 5.1–7.0). Every paired unit discards a start-load
change over 5 or a nonzero exit; ratios are medians of pair ratios, beside paired absolute
medians. Final gate/test/cache samples follow the minimal three-build ownership regression
and the runtime journal fix. Program variants (three pairs), check/plan (15 pairs), no-run
and `ignite_test.rs` timings precede those two changes, which do not alter their execution
paths. The initial and intermediate source snapshots, invalid cold comparisons and failed
samples remain in `final-v1`, `final-v2`, `records-historical.jsonl`, `records-pre-final.jsonl`
and `historical-logs/`; they are excluded from final gate numbers. Both final cold baseline comparisons meet the
start-load rule and have no failed step.

| measurement | 11b → 12b; ratio | slice 12 → 12b; ratio |
|---|---|---|
| gate, cold | 57.141 → 66.111 s; ×1.157 (1 kept/0 discarded) | 68.727 → 66.111 s; ×0.962 (1 kept/0 discarded) |
| gate, warm after touch | 13.374 → 16.786 s; ×1.255 (3 kept/0 discarded) | 18.705 → 16.627 s; ×0.889 (2 kept/1 discarded) |
| warm cargo test --workspace | 10.142 → 12.413 s; ×1.233 (3 kept/0 discarded) | 14.594 → 12.313 s; ×0.844 (2 kept/1 discarded) |
| warm fmt | 0.447 → 0.523 s; ×1.169 (3 kept/0 discarded) | 0.523 → 0.523 s; ×1.009 (3 kept/0 discarded) |
| warm clippy | 2.166 → 2.713 s; ×1.271 (3 kept/0 discarded) | 2.508 → 2.713 s; ×1.042 (3 kept/0 discarded) |
| warm task test-ignite | 0.663 → 1.140 s; ×1.709 (3 kept/0 discarded) | 1.099 → 1.140 s; ×1.047 (3 kept/0 discarded) |
| todod Rust dev | 4.952 → 3.998 s; ×0.990 (3 kept/0 discarded) | 4.836 → 3.998 s; ×0.892 (3 kept/0 discarded) |
| todod Rust release | 5.611 → 4.740 s; ×0.843 (3 kept/0 discarded) | 5.371 → 4.740 s; ×0.934 (3 kept/0 discarded) |
| todod LLVM dev | 0.379 → 0.309 s; ×0.892 (3 kept/0 discarded) | 0.528 → 0.309 s; ×0.585 (3 kept/0 discarded) |
| todod LLVM release | 0.862 → 0.820 s; ×0.946 (3 kept/0 discarded) | 1.052 → 0.820 s; ×0.791 (3 kept/0 discarded) |
| check design file | 0.062 → 0.090 s; ×1.447 (15 kept/0 discarded) | 0.115 → 0.090 s; ×0.769 (15 kept/0 discarded) |
| plan design file | 0.063 → 0.091 s; ×1.437 (15 kept/0 discarded) | 0.117 → 0.091 s; ×0.778 (15 kept/0 discarded) |
| --no-run warm | 0.029 → 0.030 s; ×0.961 (3 kept/0 discarded) | 0.032 → 0.030 s; ×0.931 (3 kept/0 discarded) |
| --no-run after touch | 2.233 → 2.594 s; ×1.162 (3 kept/0 discarded) | 2.751 → 2.594 s; ×0.901 (3 kept/0 discarded) |
| ignite_test.rs file | 7.735 → 2.185 s; ×0.399 (3 kept/0 discarded) | 8.233 → 2.185 s; ×0.259 (3 kept/0 discarded) |

- Final validation: 647 cargo tests passed, 0 failed, 5 ignored; 148 inline tests per target.
  Parent close: fmt, clippy, workspace tests, inline tests, `task gate` and all four compile
  tasks pass. Three subsequent untraced final warm gates and the exact-final cold gate all
  pass. The slice-12 baseline's first final warm pass hit its existing `args_port` race and
  is discarded (2 valid gate/test pairs); the baseline was not patched. Actual executable
  lines count 20 per tree (the earlier count 21 included Cargo's `target(s)` text), about
  247 MB before and 266 MB in 12b.
- Program-build gain: generated todo `ignite-std` is 993 lines, against 2,191 in 11b and
  3,792 at slice-12 close. All four program-build ratios meet 1.10 against 11b. The whole
  `ignite_test.rs` file is 2.185 s median (range 2.131–3.875), below its 4.6 s target;
  each named test's isolated before/after timing is in `report.md`. Shared fixture setup is
  paid once per full-file run, so isolated per-test times are not additive.
- Remaining frontend exception, measured in separate disposable instrumented compilers
  (15 interleaved runs; `profile-summary.json`): design-file std parse/check/type costs
  10.12/11.97/24.55 ms before and 13.71/16.20/36.65 ms after. `std.control` alone costs
  6.65/7.74/21.41 ms; the design legitimately reaches it through its kind hooks and imports.
  Program parse/check/type stays about 0.58/0.72/1.85 ms; evaluation check is 3.68 → 6.73 ms.
  Thus selective module loading removes unused HTTP/JSON but retains the reducers required
  by this system. No profiler output was added to evaluated programs or normal CLI output.
- Remaining gate/test exception is attributed per crate in the normal second final warm
  workspace logs: Ignite E2E execution is 4.93 → 4.91 s, rustgen integration tests are
  1.60 → 3.13 s (their helper still loads/types the full std per program), and the new driver
  unit tests add 0.25 s. The full inline suite grows from 102 to 148 tests per target. Cold
  test/inline steps are 44.37 → 51.26 s and 8.05 → 9.68 s. These remain explicit exceptions;
  their exact split beyond the measured crate/phase figures is not claimed.
- Formatting attribution at matched load: package `ignite` 0.087 → 0.146 s, evaluator
  0.070 → 0.087 s; runtime and rustgen unchanged. Warm clippy attribution (`--timings`):
  total 2.26 → 2.95 s; driver library 0.17 → 0.48 s, test library 0.19 → 0.50 s, E2E check
  0.52 → 0.72 s, evaluator 0.40 → 0.57 s, LLVM 0.67 → 0.80 s. Unit durations overlap and
  must not be added as wall time. The attribution predates removal of redundant cache-test
  cases; source growth correlates with these costs, but no finer causal split is asserted.
- Findings and fixes: five frontend reachability tests and the shared-target cache regression
  cover module literals, transitive imports, unused broken std, and program ownership.
  Four existing E031/E032/E003 CLI fixtures now explicitly import their broken std modules;
  the emission fixture asserts an unused nested module is absent. All 21 std/example files
  check silently. Recovered rustgen goldens `consts.std.rs` and `variants.std.rs` only drop
  unreachable items; this completion adds no golden changes. The ownership regression was
  mutation-checked by removing the build claim (it printed the wrong third program), then
  restored; its final minimal A/B/A test takes 0.538 s median over three normal runs.
- Integrated failures are retained, not hidden: real-time restart exceeded 500 ms twice in
  pre-fix timed suites (1,341.69 and 869.97 ms); three isolated unchanged tests passed, so
  that alone did not establish readiness. The runtime now avoids separate durability syncs
  for deterministic signal/standing/consequence follow records; every originating effect
  remains synced before dispatch under O30. Journal order/bytes and replay behaviour stay
  unchanged. Three final normal suite journals measure 106.72, 107.28 and 104.51 ms, inside
  the unchanged 100–500 ms bound; the final cold run measures 104.40 ms and replays. Raw
  journals and parsed values are in `restart-samples.json`. The separate shared-ARGS ABI-test
  race (`Some(0)` versus `None`) is fixed with a test-only mutex. A syscall-traced diagnostic
  run interfered with HTTP concurrency tests and is not readiness evidence.


Named `ignite_test.rs` timings, one isolated pass per test on slice-12 close and the
12b compiler snapshot. These precede the cache/runtime fixes, which do not change this file.

| test | slice 12 s | 12b s |
|---|---:|---:|
| assert_and_try_reports | 1.038 | 1.213 |
| both_targets_print_the_same_report | 1.021 | 2.253 |
| captured_output_only_for_failures | 0.693 | 1.180 |
| compile_error_exits_2 | 0.294 | 0.092 |
| counts_results_and_failures | 0.659 | 1.158 |
| each_unnamed_file_runs_its_own_tests | 3.230 | 1.238 |
| endpoint_of_reads_the_ref_binding_on_both_targets | 0.670 | 0.048 |
| files_share_one_executable | 0.537 | 0.072 |
| filter_exact_and_list | 1.515 | — |
| filter_exact_passing_and_none | — | 0.390 |
| overflow_fails_the_same_everywhere | 1.709 | 1.093 |
| std_module_as_the_file | 0.404 | 0.285 |
| substrate_builtins_run_on_both_targets | 0.539 | 1.292 |
| target_bugs_of_slice_11b_are_fixed | 0.592 | 1.098 |
| timeout_kills_a_test | 1.661 | 1.084 |

`filter_exact_and_list` became `filter_exact_passing_and_none`; list assertions are
shared with the fixture-run coverage. Per-file and per-crate figures must be compared with
whole-suite wall time; the concurrently executing file times are not an additive budget.
