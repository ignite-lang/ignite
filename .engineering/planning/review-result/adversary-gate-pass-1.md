---
format: aep.planning-md/3
id: review-result:adversary-gate-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit gate (wave 11)
relations:
- reviews: story:release-refuses-every-worse-row
revision: 1
---
red

Adversary pass 1 on unit gate, `wave/11-gate` at `79d65191`, test commit `10af7471`.

verdict: red; cases executed 74→76, red 1; origin: introduced 3, pre-existing 1, undecided 1.

Findings:
1. `crates/ignite/tests/quality.rs:2106`, CONFIRMED, introduced, blocker. "The last release" is the last history run named `bar` or `v*`; the history holds local `v0.16.0` records (`1293ce62`, `e009426f`) of a release the dedicated-host run refused, and no `v0.16.0` tag exists. Fed the real history and `e009426f`'s 55 values, `release_refusals` returns 0 refusals; with the history cut at v0.15.0 it returns 14 (1.1, 1.6, 3.6 x2, 7.2, 7.3, eight 3.2 rows).
2. `quality.rs:2128`, CONFIRMED, pre-existing, note. A decision that moved a bar worse does not block a release once the value is back, against `AGENTS.md` § Releases. Red case `quality::a_release_waits_until_a_decision_that_moved_a_bar_worse_is_withdrawn`.
3. `quality.rs:2120`, NEEDS-CHANGE, undecided, warning. Hill-climb row 1.3 is skipped by the release check while `AGENTS.md` and the new release paragraph say any row; no test pins either reading. 1.3 from v0.15.0 to the v0.16.0 candidate: eval/builtins 133→149, llvm/patterns 30→46, rust/builtins 54→57.
4. `tools/ignite-perf/src/remote.rs:1637`, CONFIRMED, introduced, note. The strict comparisons of the disjoint-samples rule survived `>=` and `<=` mutants; pinned by the adversary's `remote::a_row_whose_samples_touch_the_base_is_left_to_the_tolerance` (green).
5. `docs/quality.md`, CONFIRMED, introduced, note. "1 of 252" is the per-row chance; over 41 noisy rows two equal builds are refused in about 15 % of runs.

Held: 3.2 four percent worse is refused (tolerance 0 for a release); the per-merge rule is unchanged (mutant red); all-samples to any-sample and dropping the disjoint rule each go red; direction comes from the baseline row; per-target cells judged separately; five passes reach the host (`Settings.passes` → `host_command --passes` → `rows`); `--passes 3` is stricter, `--passes 0` yields no timing rows; a run cut off by the 3 h shutdown returns `Err` before the history append; the unit's tests build their own baselines; no new ignored test or dependency; `quality.rs` 3,411 → 3,390 lines.

Coordinator's reading of finding 3: the owner's rule is "no release is worse than the one before it on any row"; a hill-climb row is a row. The release check judges it; the per-merge rule stays. Row 1.3 becomes a story of the wave.

```findings
[
{"file":"crates/ignite/tests/quality.rs","line":2106,"category":"acceptance","severity":"blocker","verdict":"CONFIRMED","origin":"introduced","message":"the last release is taken as the last history run named bar or v*, and the history holds local v0.16.0 records of a refused, untagged release: the real history gives 0 refusals where the history cut at v0.15.0 gives 14"},
{"file":"crates/ignite/tests/quality.rs","line":2128,"category":"acceptance","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"a decision that moved a bar worse does not block a release once the value is back, against AGENTS.md Releases (red case quality::a_release_waits_until_a_decision_that_moved_a_bar_worse_is_withdrawn)"},
{"file":"crates/ignite/tests/quality.rs","line":2120,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"undecided","message":"hill-climb row 1.3 is skipped by the release check while AGENTS.md says any row; 1.3 from v0.15.0 to the candidate: eval/builtins 133 to 149, llvm/patterns 30 to 46, rust/builtins 54 to 57"},
{"file":"tools/ignite-perf/src/remote.rs","line":1637,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the strict comparisons of the disjoint-samples rule survived >= and <= mutants; pinned by remote::a_row_whose_samples_touch_the_base_is_left_to_the_tolerance"},
{"file":"docs/quality.md","line":150,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"1 of 252 is the per-row chance; over 41 noisy rows two equal builds are refused in about 15 % of runs"}
]
```
