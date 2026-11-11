---
format: aep.planning-md/3
id: story:size-rows-grow-with-a-decision
kind: story
status: implemented
title: A size row may grow at a release when a decision names the code
relations:
- informed_by: story:release-refuses-every-worse-row
- informed_by: story:binary-size-back
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: crates/ignite/tests/quality.rs
- confidence: inferred
  path: docs/quality.md
- confidence: inferred
  path: quality/baseline.json
- confidence: inferred
  path: tools/ignite-perf/src/remote.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T11:42:06Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T11:42:07Z", actor: "human:timo", revision: 4, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "active", to: "implemented", at: "2026-11-11T23:22:42Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: a size row may grow at a release when a decision names the code

## Outcome

`task quality RUN=vX.Y.Z` and `task quality-remote ... RUN=vX.Y.Z` no longer refuse a release for a binary-size row (3.2, 3.9) that is above the last release, provided the row's bar carries a `decision` that names the code that added the bytes; the run prints each grown cell. Every other row is refused as before.

## Why

Timo, 2026-11-11: "increase in binary size isn't a big deal ... if we're adding more code and more logic, then obviously the size will increase ... what would be bad is execution time, compile time". Recorded in `AGENTS.md` § Releases.

What the code does today, read on `b3d92b83`:

- `release_refusals` (`crates/ignite/tests/quality.rs`) refuses every count and size row worse than the last release by any amount, and refuses a bar a `decision` moved worse, whatever the value.
- `refusals` (`tools/ignite-perf/src/remote.rs`) refuses row 3.9 (stripped binary size against plain Rust, permille) beyond its tolerance or when every sample is worse, and no `decision` holds a row.
- Unit size of wave 11 ends with six Rust cells of 3.2 and 3.9 above v0.15.0 after it reverted every change that bought bytes with instructions.

Coordinator's reading of the rule, not Timo's words: growth stays deliberate. A size row that grows without a `decision` is still refused, so a release cannot grow by accident; the per-merge bar is unchanged, so a merge that grows a binary still has to move the bar with a `decision`.

## Acceptance

- A metric of `quality/baseline.json` can be marked as a size row by one field (name it; 3.2 and 3.9 carry it, no other row does), and `docs/quality.md` and the generated quality page say what the field means.
- `release_refusals`: a cell of a marked row above the last release is not refused when its bar holds a `decision`, and is refused when it holds none; the refusal says that a decision naming the added code is missing. The check that refuses a bar a `decision` moved worse skips marked rows. Tests: a grown cell with a decision passes and is listed; a grown cell without one is refused; an unmarked row with a decision is refused as today; a marked row that shrank needs nothing.
- `refusals` in `tools/ignite-perf`: the same for row 3.9 against the bars of `quality/baseline.json`, with the same four tests; timing and benchmark rows are judged as today.
- The release run prints every grown cell of a marked row: cell, value at the last release, value now, and the decision text; the list is also in the record the release notes are written from, where one exists.
- A merge is judged as today: `quality::static_metrics_hold_the_bar` and the per-merge bars are unchanged.
- `AGENTS.md` § Releases states the rule in place of the sentence that says the check still refuses a size row.
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `crates/ignite/tests/quality.rs`, `tools/ignite-perf/src/remote.rs`, `quality/baseline.json` (the field on 3.2 and 3.9 only), `docs/quality.md`, `AGENTS.md`, the generated `quality/trend.md` and book quality page if their text changes. Inferred.
