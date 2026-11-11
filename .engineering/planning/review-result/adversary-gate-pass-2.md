---
format: aep.planning-md/3
id: review-result:adversary-gate-pass-2
kind: review-result
status: active
title: Adversary pass 2, unit gate (wave 11)
relations:
- reviews: story:release-refuses-every-worse-row
revision: 1
---
red

Adversary pass 2 (the last) on unit gate, `wave/11-gate` at `03292a98` (fix commits `70cc63df`, `03292a98`), test commits `8d1bf064`, `366ddc1e`.

verdict: red; cases executed 81→85, red 1; origin: introduced 4, pre-existing 3.

Findings:
1. `crates/ignite/tests/quality.rs:2204` (also `AGENTS.md:517`, `:531`), NEEDS-CHANGE, pre-existing, warning. Decisions that moved benchmark and timing bars worse than v0.15.0 are read by neither the release check nor `quality-remote`, although `AGENTS.md` says such a decision blocks the release until it is withdrawn. Six real ones ("Accepted for v0.16.0"): 3.9 1378→1820, 700→869, 1263→1547, 692→850; 3.7 1105→1467, 1055→1387. Red case `quality::gate_adv2_a_benchmark_bar_a_decision_moved_worse_refuses_the_release`. The story's acceptance needs it, so it goes to the implementor.
2. `quality.rs:2186`, `:2213`, CONFIRMED, introduced, note. The reverse search for the last run and the decided-bar comparison had no test; now pinned by `gate_adv2_the_reference_is_the_last_local_run_under_the_tag` and `gate_adv2_a_decision_on_a_bar_not_worse_than_the_last_release_refuses_nothing`.
3. `tools/ignite-perf/src/remote.rs:1740`, CONFIRMED, introduced, note. Removing the `release_samples` call from `remote::main` left the unit's suite green; now pinned by `remote::gate_adv2_quality_remote_refuses_a_one_pass_release_before_anything_else`.
4. `docs/quality.md:154`, `AGENTS.md:528`, CONFIRMED, introduced, note. The rerun rule is an operator instruction only, does not cover a second run that refuses a different row, and read literally lets a row through unless both runs refuse it.
5. `AGENTS.md:517`, INFEASIBLE, introduced, note. The code takes the nearest `v*` tag `git describe` finds from HEAD (an rc tag, an annotated tag over a newer lightweight one on one commit, never an unreachable hotfix tag); `AGENTS.md` says the newest `v*` tag. No such tag exists in the repository.
6. `tools/ignite-perf/src/bench.rs:184`, CONFIRMED, pre-existing, note. A release run with `--bench-passes 5` judges a benchmark row on as few as one sample per side when passes have no clean run.
7. `remote.rs:1845`, CONFIRMED, pre-existing, note. Only the `RUN=v*` label makes a dedicated-host run a release run, and no release step requires a `v*` dedicated-host record before tagging.

Held: the real history, e009426f's 55 values and `last_release_tag("v0.16.0")` give `Tag("v0.15.0")` and 17 refusals, the 17 rows of `owed.txt` and no other. A rerun of v0.16.0 resolves to v0.15.0. Tag order follows the commit graph. A shallow clone, a repository with no commits or no repository is refused, or uses `bar` when the history holds no `v*` run. A dedicated-host record under the same label is never the reference. A decision on a row that did not exist at the last release refuses nothing. A whitespace-only decision is no decision. Mutants: hill-climb skip put back and `climbing = false` are caught by the unit's tests. 41 noisy rows (5 timing + 36 benchmark), about 15 %, 1 in 252 check out. `cargo test -p ignite --test main -- quality::` 29 passed, 1 failed (the red case), 1 ignored; `cargo test -p ignite-perf` 55 passed; clippy and fmt exit 0.

```findings
[
{"file":"crates/ignite/tests/quality.rs","line":2204,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"pre-existing","message":"decisions that moved benchmark and timing bars worse than v0.15.0 (six real ones on rows 3.9 and 3.7) are read by neither the release check nor quality-remote, although AGENTS.md says such a decision blocks the release until it is withdrawn"},
{"file":"crates/ignite/tests/quality.rs","line":2213,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"removing the reverse search for the last run or the decided-bar comparison left the unit's suite green; both are now pinned by the adversary's green cases"},
{"file":"tools/ignite-perf/src/remote.rs","line":1740,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"removing the release_samples call from remote::main left the unit's suite green; now pinned by remote::gate_adv2_quality_remote_refuses_a_one_pass_release_before_anything_else"},
{"file":"docs/quality.md","line":154,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the rerun rule exists only as an operator instruction, does not cover a second run that refuses a different row, and read literally lets a row through unless both runs refuse it"},
{"file":"AGENTS.md","line":517,"category":"contract-drift","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the code takes the nearest v* tag git describe finds from HEAD, AGENTS.md says the newest v* tag; no rc tag and no two release tags on one commit exist in the repository"},
{"file":"tools/ignite-perf/src/bench.rs","line":184,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"a release run with --bench-passes 5 judges a benchmark row on as few as one sample per side when passes have no clean run"},
{"file":"tools/ignite-perf/src/remote.rs","line":1845,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"only the RUN=v* label makes a dedicated-host run a release run, and no release step requires a v* dedicated-host record before tagging"}
]
```
