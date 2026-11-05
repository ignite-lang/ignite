---
format: aep.planning-md/3
id: story:slice-17-records
kind: story
status: implemented
title: Slice 17 records and close
summary: The handler-through-state test on both targets and the standing documents for slice 17
relations:
- decomposes: epic:playground-substrate
- depends_on: story:file-system
- depends_on: story:process-spawn
- depends_on: story:random-bytes
- depends_on: story:program-arguments
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: PLAN.md
- confidence: cited
  path: STATUS.md
- confidence: cited
  path: crates/ignite/tests/slice17.rs
- confidence: cited
  path: docs/performance/17
- confidence: cited
  path: docs/vision.md
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T07:34:51Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-05T07:34:51Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-05T09:41:59Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: slice-17-records

## Outcome

Slice 17 is closed on record: a handler using all four handles through its state serves on both
targets, and the standing documents and performance records name what landed.

## Context

- Contract: `docs/slice.md` section 21 on `feat/slice17-processes-files`, unit U6 of 21.12, rows
  24 and 34 of 21.0; first failing test `slice17::rust::studio_serves_through_its_state`.
- Release: after the slice merges, the steering session tags the next minor version and tells
  the playground session that P1, P2, P7 and P8 landed.

## Acceptance

- Rows 24 and 34 pass; rows 30 and 33 hold; `STATUS.md`, `AGENTS.md`, `PLAN.md`,
  `docs/vision.md` and `docs/performance/17/` hold what 21.12 and the stale points name.

## Out of Scope

- New capabilities.
