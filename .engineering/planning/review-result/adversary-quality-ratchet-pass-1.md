---
format: aep.planning-md/3
id: review-result:adversary-quality-ratchet-pass-1
kind: review-result
status: active
title: Adversary, quality-ratchet, pass 1
relations:
- reviews: story:quality-ratchet
revision: 1
---
unit: story:quality-ratchet, branch feat/quality-ratchet at 9a775116 plus 5 uncommitted adversary cases
verdict: NEEDS-CHANGE
cases: executed 13→18, red 5
origin: introduced 5 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 (scratch unit-quality/adv1, 1.8 GB)
needs-coordinator: whether the 7/7 `task ux` failures need another A/B; 6 interleaved runs at load 25 to 58 were all green on base (279) and head (284)

## Cases (uncommitted)

- `differential::adversary_every_test_of_two_files_with_one_module_path_runs_on_a_target`: red, 13 units of `slice16/moves.ignite` with no LLVM run.
- `quality::adversary_a_stale_decision_does_not_let_a_bar_worsen_again`: red.
- `quality::adversary_a_tolerance_loosened_needs_its_own_decision`: red.
- `quality::adversary_a_metric_relabelled_as_timing_is_not_silently_ungated`: red.
- `quality::adversary_eval_cannot_run_reasons_are_held_exactly`: red.

## Findings

- A1 (blocker) `crates/ignite/tests/differential.rs:425`: `run_tests` keys runs by module path with `or_insert`; fixtures without `module` share `main`, so 186 units (144 evaluated, 42 cannot run; 34 files under `slice9/`, `slice16/`, `streams/`, `tests/unnamed_{a,b}`) never run on Rust or LLVM and are never compared. Mutant at `crates/ignite-eval/src/eval.rs:1840` (`[..all]` binds `[]`) fails 2 units in the evaluator and the corpus test stays green (615.90 s). Positive control (`starts_with` → `ends_with` at `:1223`) caught with 11 refusals.
- A2 `quality.rs:492-498`: a decision is only checked for presence, so one decision lets a bar worsen in every later merge.
- A3 `quality.rs:511-513`: a looser tolerance passes on any old decision of any bar of the metric.
- A4 `quality.rs:477`: relabelling a count metric `source: perf`, `unit: ms` turns off every check for it.
- A5 `quality.rs:727-738`: `eval_cannot_run` compared by key only; a reason edited to "over the time limit" also drops the unit from merge runs (`differential.rs:752`).
- A6 `quality/baseline.json`: the intermittent `store_load` LLVM entry accepts any regression of that test indefinitely.

## Not broken

Ratchet basics per target; bless never writes worse; release refusal per metric and target against the last bar/release; generated pages fail on drift; bars spot-checked (7.2 21, 2.5 319, 2.6 5, 7.3 4, 7.4 34, 3.1 rust 168 / llvm 90, 3.6 rust 7); differential bars reproduce (629, 4, 194). Gate time: `differential::corpus…` 95.8 s at load 24 to 30, 615.9 s at load 43 to 61; `quality::` plus the page 21.7 s; the differential's Rust builds update the crates.io index (network in the gate).

```json
[
 {"id":"A1","severity":"blocker","title":"Differential runner never runs or compares 186 units: fixtures sharing module path main lose their target runs","evidence":"differential.rs:425; 186 units rust/llvm missing; mutant eval.rs:1840 not caught","test":"differential::adversary_every_test_of_two_files_with_one_module_path_runs_on_a_target","pre_existing":false},
 {"id":"A2","severity":"warning","title":"A stale decision lets a bar worsen again against main","evidence":"quality.rs:492-498","test":"quality::adversary_a_stale_decision_does_not_let_a_bar_worsen_again","pre_existing":false},
 {"id":"A3","severity":"warning","title":"A loosened tolerance passes on any old decision","evidence":"quality.rs:511-513","test":"quality::adversary_a_tolerance_loosened_needs_its_own_decision","pre_existing":false},
 {"id":"A4","severity":"warning","title":"Relabelling a count metric as perf/ms turns off every check","evidence":"quality.rs:477","test":"quality::adversary_a_metric_relabelled_as_timing_is_not_silently_ungated","pre_existing":false},
 {"id":"A5","severity":"note","title":"eval_cannot_run reasons are not compared","evidence":"quality.rs:727-738; differential.rs:752","test":"quality::adversary_eval_cannot_run_reasons_are_held_exactly","pre_existing":false},
 {"id":"A6","severity":"note","title":"The intermittent store_load entry accepts any LLVM regression indefinitely","evidence":"quality/baseline.json known_differences","test":null,"pre_existing":false}
]
```
