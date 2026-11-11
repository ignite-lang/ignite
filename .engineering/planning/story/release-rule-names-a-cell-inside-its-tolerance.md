---
format: aep.planning-md/3
id: story:release-rule-names-a-cell-inside-its-tolerance
kind: story
status: implemented
title: A release tracks the rows that are worse than the last release and is not refused by them
relations:
- informed_by: story:readiness-task-stealing
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T21:27:01Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-11T21:27:01Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-11-11T23:22:42Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: a release tracks the rows that are worse than the last release and is not refused by them

## Outcome

`task quality RUN=vX.Y.Z` and `task quality-remote RUN=vX.Y.Z` compute the rows that are worse than the last release as before, print them as tracked, record the run under the release's label and exit 0. The release notes list each such row with both values and the id of the story that takes it. A merge is judged as before.

## Why

The operator on 2026-11-11: "the general idea is to get better every time, but the metrics, of course, need to make sense", then: "Let's just track the numbers, but let's not block the release, okay? So then cut the release now."

Run `w11-rss` on the dedicated host (`ec2:c7i.metal-24xl`, 5 passes, v0.15.0 against the integration head with waves 9 to 11), judged by `tools/ignite-perf/src/remote.rs` `judge` as a release run would be: 2 of 36 cells are refused, both by "every sample worse than every sample of the base", both inside the tolerance of 20 %: 3.7 `llvm/terminal/connect-8` 1099 → 1282 permille (1.167) and `rust/terminal/connect-8` 1047 → 1202 (1.148). In the same run, HEAD against BASE as the geometric mean over the six cells of each row:

| row | LLVM | Rust |
|---|---|---|
| 3.3 replies per second (higher is better) | 1.106 | 1.109 |
| 3.7 p99 latency (lower is better) | 0.655 | 0.673 |
| 3.8 peak RSS (lower is better) | 0.733 | 0.718 |

The row that holds the refused cell is a third better over its cells (the 64-connection cells are at 0.38 to 0.49 of v0.15.0), and the release that carries it was refused. The fix of the one cell is a change of the scheduler (`story:readiness-task-stealing`).

## Acceptance

- `ignite-perf quality-remote --run vX.Y.Z` with a row worse than BASE prints `release vX.Y.Z: worse than the base, tracked and not refused:` and the rows on stdout, writes its records under the label `vX.Y.Z` with the decisions of the held size cells, and exits 0. Held by the test of `remote::judge_release` in `tools/ignite-perf/tests/remote.rs` (the text) and by the release run of v0.16.0 (label and exit status).
- `quality::record` under `IGNITE_QUALITY_RUN=vX.Y.Z` with a row worse than at the last release prints `release vX.Y.Z: worse than the last release, tracked and not refused:` and the rows on stderr and records the run under the label. Held by `quality::a_release_prints_its_worse_rows_as_tracked`.
- The lists themselves are computed as before (`judge`, `release_judged`): their tests are unchanged.
- `AGENTS.md` § Releases and `docs/quality.md` state the rule once each at the head of their release text; the generated quality page says that a worse row is tracked.
- `task gate` green.

## Scope

- `tools/ignite-perf/src/remote.rs` (`judge_release`, `ReleaseJudgement`, the end of the run), `tools/ignite-perf/tests/remote.rs`, `crates/ignite/tests/quality.rs` (`record`, `tracked_section`, the generated page's text), `docs/quality.md`, `AGENTS.md`, `site/book/src/quality.md` (generated).

## Not done here

- `docs/quality.md` keeps the word "refuses" in the paragraphs that describe how the lists are computed; the rule at the head of its section says what the word means since 2026-11-11.
- A record of the tracked rows in `quality/history.jsonl` beyond the values themselves.
