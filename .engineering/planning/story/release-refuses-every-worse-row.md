---
format: aep.planning-md/3
id: story:release-refuses-every-worse-row
kind: story
status: active
title: A release is refused for every row worse than the last release
relations:
- supersedes: story:remote-refusals-honour-decisions
scope:
- confidence: cited
  path: crates/ignite/tests/quality.rs
- confidence: cited
  path: docs/quality.md
- confidence: inferred
  path: tools/ignite-perf/src/quality.rs
- confidence: cited
  path: tools/ignite-perf/src/remote.rs
- confidence: inferred
  path: tools/ignite-perf/tests
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T00:24:52Z", actor: "human:timo", revision: 4, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T00:24:52Z", actor: "human:timo", revision: 5, executor: "agent:claude-coordinator", correlation: "wave-11"}
---
# Story: a release is refused for every row worse than the last release

## Outcome

`task quality RUN=vX.Y.Z` and `task quality-remote ... RUN=vX.Y.Z` refuse a release for every row
that is worse than the last release, and no `decision` in `quality/baseline.json` holds a row for
a release.

## Why

Timo, 2026-11-10: "every release should be an improvement in quality, not even allowed to accept
10% degregation", and "you must check the results which were run on the remote EC2 - not local".
Recorded in `AGENTS.md` § Releases.

What the code does today, read on `aa1defb2`:

- `release_refusals` (`crates/ignite/tests/quality.rs`) takes a bar that a `decision` moved worse
  since the last release as the row's reference: within it the row is "held by the decision" and
  the release is cut. v0.16.0 moved eight bars of 3.2 that way.
- `refusals` (`tools/ignite-perf/src/remote.rs`) refuses a row only when HEAD's median is worse
  than BASE's beyond the metric's tolerance (5 to 20 %). A row 9 % worse in every sample passes;
  the run of `4520b8cf` holds four such rows on 3.8 (+4.8 to +8.5 %) and two on 3.3 connect-8
  (-6 %).
- The dedicated host takes three samples for 6.1 and 6.2.

Coordinator's reading of the rule, not Timo's words: a tolerance stays only as a bound on noise.
Where HEAD and BASE samples overlap, the difference is not shown and the tolerance decides; where
they do not overlap, or a row has one deterministic value, any worse value is a worse row.

## Acceptance

- `release_refusals` refuses a count row (3.2 included) that is worse than the last release's
  value, with tolerance 0 for a deterministic value, whether or not a `decision` moved its bar; a
  test gives it a worsened row with a new decision and expects the refusal. The `decided` list no
  longer lets a release pass; the refusal names the decision so the reader sees what to withdraw.
- `refusals` in `tools/ignite-perf` refuses a row when HEAD's median is worse than BASE's beyond
  the tolerance (as today), and also when HEAD's median is worse and every HEAD sample is worse
  than every BASE sample (a row with one sample each: any worse value). Tests: +3 % with disjoint
  samples refused; +3 % with overlapping samples not refused; a one-sample row one unit worse
  refused; equal not refused.
- The dedicated host takes five samples for 6.1 and 6.2 (`quality-host`), so two equal builds
  have disjoint samples by chance in 1 of 252 runs, not 1 of 20.
- The per-merge rule is unchanged: a merge may still move a bar with a `decision`; the release
  check is what refuses it.
- `docs/quality.md` states the rule in the section on the release gate; its first paragraph
  already says "no release is worse than the release before it".
- `task gate` green.

## Out of scope

- The values themselves: the other stories of the wave bring them back.
- `story:remote-refusals-honour-decisions` asked for the opposite and is superseded by the rule.

## Scope

- `crates/ignite/tests/quality.rs` (`release_refusals`, its tests). Cited.
- `tools/ignite-perf/src/remote.rs` (`refusals`), `tools/ignite-perf/src/quality.rs` (sample
  counts on the host), `tools/ignite-perf/tests/`. Cited for the first, inferred for the others.
- `docs/quality.md`. Cited.
