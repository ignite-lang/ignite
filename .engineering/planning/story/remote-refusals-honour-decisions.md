---
format: aep.planning-md/3
id: story:remote-refusals-honour-decisions
kind: story
status: archived
title: quality-remote refusals honour recorded decisions
revision: 2
transitions:
- {from: "draft", to: "archived", at: "2026-11-11T00:24:56Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "wave-11"}
---
# Story: remote-refusals-honour-decisions

## Outcome

`ignite-perf quality-remote` refuses a release only for a row that is worse than the base and not covered by a `decision` in `quality/baseline.json`, the same rule `release_refusals` in `crates/ignite/tests/quality.rs` applies, so a decision taken after an EC2 run lets the release proceed without measuring again.

## Context

- v0.15.0: the EC2 run refused on 3.8 llvm/terminal/connect-8 (2.637x to 2.938x); a decision was recorded afterwards, but `refusals` in `tools/ignite-perf/src/remote.rs` compares HEAD with BASE inside the run and reads no decisions.

## Acceptance

- A test in `tools/ignite-perf` gives `refusals` a worsened row with and without a matching decision: refused without, accepted with.
- `task gate` green.

## Scope

- `tools/ignite-perf/src/remote.rs`, `tools/ignite-perf/tests/`. Inferred.
