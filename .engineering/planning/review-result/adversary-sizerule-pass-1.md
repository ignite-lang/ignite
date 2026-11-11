---
format: aep.planning-md/3
id: review-result:adversary-sizerule-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit sizerule (wave 11)
relations:
- reviews: story:size-rows-grow-with-a-decision
revision: 1
---
red

Adversary pass 1 on unit sizerule, attacked at `7ac3bfc6`; the pass's cases are commit `dc0f508b` (`crates/ignite/tests/quality.rs`, `tools/ignite-perf/tests/remote.rs`).

verdict: red; cases executed 150→153, red 1; origin: introduced 7, pre-existing 0.

Findings:
1. `crates/ignite/tests/quality.rs:1874`, NEEDS-CHANGE, introduced, warning. The release's row of `quality/trend.md`, which the release notes carry, names a grown size cell and both values but not the decision that holds it; the decision is only on the run's output. Acceptance bullet 4 is not met. Test `quality::sizerule_adv1_the_release_row_of_the_trend_carries_the_decision_of_a_grown_cell`.
2. `quality/baseline.json:459` and `:683`, CONFIRMED, introduced, warning. All 12 bars of 3.2 and 3.9 carry decisions written on 2026-11-09 for the refused v0.16.0 candidate. They are new against v0.15.0, so the rule holds every cell with nobody deciding anything: the real `quality::record` with `RUN=v0.16.0`, fed the 3.2 values of `e009426f`, lists 8 cells and refuses none, including `llvm/terminald/dev` 7,831,016 → 15,560,616 B; the real `remote::judge` on the last dedicated-host run holds 4 cells. The blessing has to rewrite or remove the twelve decisions.
3. `crates/ignite/tests/quality.rs:2349`, CONFIRMED, introduced, note. `release_judged`, which `record` calls, had no test; a mutant that drops the tag baseline's decisions left the unit's suite green. Pinned by `quality::sizerule_adv1_the_release_check_reads_the_last_releases_decisions_from_the_tag`.
4. `tools/ignite-perf/src/remote.rs:1721`, CONFIRMED, introduced, note. An undecided 3.9 cell inside the 5 % tolerance passing unlisted left the suite green. Pinned by `remote::sizerule_adv1_a_size_cell_inside_the_tolerance_is_refused_without_a_decision`.
5. `tools/ignite-perf/src/remote.rs:1967`, CONFIRMED, introduced, note. No test reaches the release path of `remote::main` or the print in `quality::record` (`quality.rs:2594`); BASE's baseline reaching `judge` and both printed lists are held by reading and one manual run.
6. `crates/ignite/tests/quality.rs:2444` and `tools/ignite-perf/src/remote.rs:1734`, CONFIRMED, introduced, note. A decision is new by exact trimmed text: the last release's decision in upper case, without its final dot, with one character changed or with a sentence appended passes as new, and a zero-width space, `.` or `x` counts as a decision.
7. `AGENTS.md:523`, CONFIRMED, introduced, note. The quality-gate bullet still says a bar a decision moved worse refuses the release whatever the value, timing or benchmark bars included; the code skips 3.2 and 3.9 there. Line 550 says it correctly.

Held: no row other than 3.2 and 3.9 gets through. The mark on ids `3.20`, ` 3.2`, `3.2 `, `03.2`, `3.9 ` and any other value of the mark are errors; duplicate keys are a parse error. A hill-climb row, a count row, 3.8, and 6.2 on a target name that 3.9 has a decided bar for are refused. `check` refuses beyond the bar, `against_main` refuses a moved bar without a new decision, `bless` leaves a worse bar alone. 19 flipped or dropped conditions in `quality.rs` and 20 in `remote.rs` are killed by the unit's tests except the two of findings 3 and 4. `v0.15.0`'s baseline is read by this tree's reader (10 decisions); no `v*` tag, a second annotated tag on the same commit and an rc tag each give one refusal and no list; a depth-1 clone is the no-tag case. `baseline_at` reads a lightweight tag, an annotated tag, a branch, a short and a full hash in a path with spaces; an unknown commit is an error. No existing test was edited. Two grown cells are listed, an undecided one and one on the last release's decision are refused, a shrunk and an equal cell get nothing. The 10 % cap test's text is identical at `239a672d` and head, green on the real history and red with v0.15.0's reference lowered. Checks at `dc0f508b`: fmt and clippy exit 0; `quality::` 41 passed, 3 failed (the pass's red case and the two red on the base); `ignite-perf` 65 passed; `ignite-check` 10 + 34 passed.

```findings
[
{"file":"crates/ignite/tests/quality.rs","line":1874,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the release's row of quality/trend.md, which the release notes carry, names a grown size cell and both values but not the decision that holds it (test quality::sizerule_adv1_the_release_row_of_the_trend_carries_the_decision_of_a_grown_cell)"},
{"file":"quality/baseline.json","line":459,"category":"judgement","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"all 12 bars of 3.2 and 3.9 carry decisions written on 2026-11-09 for the refused v0.16.0 candidate, which are new against v0.15.0, so the rule holds every cell with nobody deciding anything; the blessing rewrites or removes them"},
{"file":"crates/ignite/tests/quality.rs","line":2349,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"release_judged, the function record calls, had no test; a mutant that dropped the tag baseline's decisions left the unit's suite green until quality::sizerule_adv1_the_release_check_reads_the_last_releases_decisions_from_the_tag"},
{"file":"tools/ignite-perf/src/remote.rs","line":1721,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"an undecided size cell inside the 5 % tolerance passing unlisted left the unit's suite green until remote::sizerule_adv1_a_size_cell_inside_the_tolerance_is_refused_without_a_decision"},
{"file":"tools/ignite-perf/src/remote.rs","line":1967,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"no test reaches the release path of remote::main or the print in quality::record, so BASE's baseline reaching judge and both printed lists are held by reading and by one manual run only"},
{"file":"crates/ignite/tests/quality.rs","line":2444,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"a decision is new by exact trimmed text, so the last release's decision with a changed case, dot or character passes as new, and a zero-width space or a single letter counts as a decision"},
{"file":"AGENTS.md","line":523,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the quality-gate bullet still says a bar a decision moved worse refuses the release whatever the value, timing or benchmark bars included, while the code skips 3.2 and 3.9 there"}
]
```
