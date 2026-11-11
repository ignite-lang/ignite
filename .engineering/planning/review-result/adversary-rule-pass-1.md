---
format: aep.planning-md/3
id: review-result:adversary-rule-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit rule (wave 11)
relations:
- reviews: story:release-rule-names-a-cell-inside-its-tolerance
revision: 1
---
red

Adversary pass 1 on unit rule (`story:release-rule-names-a-cell-inside-its-tolerance`), attacked at `a45d5cd3`; the pass's cases are commit `02b45b35` on `wave/11-rule-adv` (`crates/ignite/tests/quality.rs` +82, `tools/ignite-perf/tests/remote.rs` +99).

verdict: red; cases executed 123→127 (ignite-perf 72→74, `quality::` 51→53), red 3; origin: introduced 4, pre-existing 1.

Findings:
1. `crates/ignite/tests/quality.rs:2858` (strings at `:2527`, `:2539`), CONFIRMED, introduced, warning. A release check that compared no row (the last release's tag could not be read, no tag reachable, the tag without a record in the history) was printed by `record` as "worse than the last release, tracked", kept the release's label and passed. Red test `quality::rule_adv1_a_release_that_compared_no_row_is_not_printed_as_tracked`. Fixed in the unit's second round: `Release.unjudged`; with it what the check says stops the release and the run is labelled `run`. The test was moved onto that field. Not reached at v0.16.0 (`git describe` finds v0.15.0 and its record is in the history).
2. `tools/ignite-perf/src/remote.rs:1756` (item from `:1862`), CONFIRMED, introduced, warning. A size cell that is not above the base with a decision left on its bar is printed under "worse than the base"; the local twin is `quality.rs:2566` and the "withdraw it" items at `:2634` and `:2701`. Red test `remote::rule_adv1_a_cell_that_is_not_worse_is_not_printed_as_worse_than_the_base`, left on the adversary's branch. Not reached at v0.16.0: no bar of 3.2 or 3.9 carries a decision. Filed as `story:release-lists-tell-worse-rows-from-stale-decisions`.
3. `crates/ignite/tests/quality.rs:2029`, CONFIRMED, pre-existing, warning. A second run under one release label gets its own row of the trend, compared with the run before it and not with the release before; `quality/trend.md` held two `v0.16.0` rows of candidates that were not released, and an EC2 row of a third. Red test `quality::rule_adv1_a_second_run_of_a_release_label_is_compared_with_the_release_before`, left on the adversary's branch. For v0.16.0 the records of the three candidates (`1293ce62`, `e009426f`, `66a3d5b9`: 175 lines of `quality/history.jsonl`, 36 of `quality/bench.jsonl`) are relabelled `run`, which the rule before 2026-11-11 gave a candidate that was not released; the trend's comparison with the run before stays and is filed with finding 2's story.
4. `tools/ignite-perf/src/remote.rs:2145`, INFERRED, introduced, note. The label, the print of the tracked rows and the exit status in `remote::main` and in `record` are executed by no test (the first sits behind the AWS run, the second in an ignored test); mutants there survive. The release run of v0.16.0 is what holds them; the story's acceptance says so.
5. `Taskfile.yml:62`, `:79`, CONFIRMED, introduced, note. The descriptions of `quality` and `quality-remote` said that a release is refused when worse. Fixed in the second round.

Held: the next release's reference is taken by git tag, never by the last `v*` label; duplicate labels panic no reader; nothing in the repository parses the remote run's stdout; the `defer` step of `task quality-remote` behaves as before; `AGENTS.md` § Releases contradicts the code in no sentence; the texts of `tracked` and `tracked_section` are held by literal assertions.

Not run: mutants in place, `quality::record`, any task, benchmark or AWS call.

```findings
[
{"file":"crates/ignite/tests/quality.rs","line":2858,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"a release whose check compared no row (tag unreadable, no tag, tag without a record) is printed as tracked, labelled as the release and passed; red test quality::rule_adv1_a_release_that_compared_no_row_is_not_printed_as_tracked"},
{"file":"tools/ignite-perf/src/remote.rs","line":1756,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"a size cell not above the base with a stale decision is printed under worse than the base; red test remote::rule_adv1_a_cell_that_is_not_worse_is_not_printed_as_worse_than_the_base"},
{"file":"crates/ignite/tests/quality.rs","line":2029,"category":"wrong-result","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"a second run of a release label is a second trend row compared with the run before it, not with the release before; red test quality::rule_adv1_a_second_run_of_a_release_label_is_compared_with_the_release_before"},
{"file":"tools/ignite-perf/src/remote.rs","line":2145,"category":"mutant","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"label, print of tracked and exit 0 in remote::main and record are executed by no test; the release run itself holds them"},
{"file":"Taskfile.yml","line":62,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the desc of quality and quality-remote still say a release is refused when worse"}
]
```
