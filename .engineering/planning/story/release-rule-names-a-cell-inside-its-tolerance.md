---
format: aep.planning-md/3
id: story:release-rule-names-a-cell-inside-its-tolerance
kind: story
status: draft
title: A release is refused by a cell beyond its tolerance or a row that is worse, and names a cell that is worse inside its tolerance
relations:
- informed_by: story:readiness-task-stealing
revision: 1
---
# Story: a release is refused by a cell beyond its tolerance or a row that is worse, and names a cell that is worse inside its tolerance

## Outcome

`ignite-perf quality-remote` with `RUN=vX.Y.Z` refuses a release when a cell's HEAD median is worse than BASE's beyond the metric's tolerance, or when a row's geometric mean over its cells is worse than BASE's. A cell whose samples are all worse than BASE's but whose median is inside the tolerance no longer refuses: the run names it, the release notes list it, and it has a story in the plan.

## Why

The operator on 2026-11-11: "the general idea is to get better every time, but the metrics, of course, need to make sense", when one cell held v0.16.0.

Run `w11-rss` on the dedicated host (`ec2:c7i.metal-24xl`, 5 passes, v0.15.0 against the integration head with waves 9 to 11), judged by `tools/ignite-perf/src/remote.rs` `judge` as a release run would be: 2 of 36 cells are refused, both by "every sample worse than every sample of the base", both inside the tolerance of 20 %: 3.7 `llvm/terminal/connect-8` 1099 → 1282 permille (1.167) and `rust/terminal/connect-8` 1047 → 1202 (1.148). In the same run, HEAD against BASE as the geometric mean over the six cells of each row:

| row | LLVM | Rust |
|---|---|---|
| 3.3 replies per second (higher is better) | 1.106 | 1.109 |
| 3.7 p99 latency (lower is better) | 0.655 | 0.673 |
| 3.8 peak RSS (lower is better) | 0.733 | 0.718 |

The row that holds the refused cell is a third better over its cells (the 64-connection cells are at 0.38 to 0.49 of v0.15.0), and the release that carries it is refused. The fix of the one cell is a change of the scheduler (`story:readiness-task-stealing`). The every-sample rule shows that a difference is real; it does not say that the release is worse.

## Acceptance

- `judge` refuses a cell of 3.3, 3.7, 3.8 only beyond its tolerance; it refuses a row (per metric and target, the cells the run measured) whose geometric mean of HEAD median over BASE median is worse than 1.000 by more than 3 % (a bound on noise: between the runs `w11-session` and `w11-rss` the median of an unchanged cell moved by 2 % (3.7 `llvm/terminal/connect-8` at v0.15.0: 1121 and 1099 permille); the implementor states the spread of the row mean of two equal trees from the records and lowers the bound if it allows). Held by tests in `tools/ignite-perf/tests/bench.rs` that fail on the rule of today: a cell apart inside the tolerance with a better row is named and not refused; the same cell with a worse row is refused by the row; a cell beyond the tolerance is refused whatever the row is.
- A named cell is printed on stdout (`release vX.Y.Z: worse inside the tolerance`: the cell, BASE, HEAD, the ratio) and written into the run's record; `AGENTS.md` § Releases says that each named cell is listed in the release notes with the id of its story, and that a release with a named cell and no story for it is not cut.
- The timing rows 5.3, 6.1, 6.2 and the size row 3.9 keep the rule of today. The second and third run of a candidate (`AGENTS.md` § Releases) apply to a refusal by the row mean.
- `docs/quality.md` "Releases" states the rule; no other restatement.
- `task gate` green.

## Scope

- `tools/ignite-perf/src/remote.rs` (`judge`, `Judged`, the release output), `tools/ignite-perf/tests/bench.rs`, `docs/quality.md`, `AGENTS.md`. Inferred.

## Out of scope

- The tolerances themselves.
- Local `task quality` (it judges no timing row).
