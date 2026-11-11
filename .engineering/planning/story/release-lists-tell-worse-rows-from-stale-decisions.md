---
format: aep.planning-md/3
id: story:release-lists-tell-worse-rows-from-stale-decisions
kind: story
status: draft
title: A release's lists tell a worse row from a decision to tidy, and its trend row is against the release before
relations:
- informed_by: story:release-rule-names-a-cell-inside-its-tolerance
revision: 1
---
# Story: a release's lists tell a worse row from a decision to tidy, and its trend row is against the release before

## Outcome

What a release run prints as "worse than the last release, tracked" holds only rows that are worse. A decision that holds nothing, or one that must be withdrawn, is printed in a list of its own. The release's row of `quality/trend.md` gives its changes against the release before it, also when the label was run more than once.

## Why

Found by the adversary pass on the release rule of 2026-11-11 (`review-result:adversary-rule-pass-1`, findings 2 and 3):

- `tools/ignite-perf/src/remote.rs:1756` (item from `:1862`), `crates/ignite/tests/quality.rs:2566`, `:2634`, `:2701`: a size cell that is not above the base whose bar still carries a decision ("holds nothing: remove it"), and a bar a decision moved worse ("withdraw it"), are items of the same list as the worse rows. Since the rule of 2026-11-11 that list is printed as "worse than the base, tracked" and copied into the release notes. Red test on `wave/11-rule-adv` (`02b45b35`): `remote::rule_adv1_a_cell_that_is_not_worse_is_not_printed_as_worse_than_the_base`.
- `crates/ignite/tests/quality.rs:2029` (`render_trend`), `:2205` (the book page's reference): a row of the trend is compared with the run before it. A release label that is run twice (a candidate, then the release) gets two rows, and the second reads against the first. Pre-existing. Red test on the same branch: `quality::rule_adv1_a_second_run_of_a_release_label_is_compared_with_the_release_before`. At v0.16.0 the records of three candidates that were not released were relabelled `run` by hand.
- The label, the print and the exit status at the end of `remote::main` and in `quality::record` are executed by no test (finding 4).

## Acceptance

- The release check returns worse rows and decisions to tidy apart; each is printed under its own heading by both runs. The two red tests of `02b45b35` are taken over and pass.
- A release's row of the trend and the book page's reference are against the last `v*` label that is not its own.
- The end of a release run (label, lists, exit status) is a function a test calls without AWS, for the remote run and for `record`; a mutant of each of the three is killed.
- `docs/quality.md` describes the lists in the words the runs print; the word "refuses" stays only where a run still stops.
- `task gate` green.

## Scope

- `tools/ignite-perf/src/remote.rs` (`judge`, `Judged`, `judge_release`, the end of `main`), `crates/ignite/tests/quality.rs` (`release_judged_against`, `decided_against_the_tag`, `render_trend`, `record`), `docs/quality.md`. Inferred.
