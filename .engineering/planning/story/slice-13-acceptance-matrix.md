---
format: aep.planning-md/3
id: story:slice-13-acceptance-matrix
kind: story
status: implemented
title: Slice 13 acceptance matrix on both targets
relations:
- decomposes: epic:slice-13-local-apply
- depends_on: story:slice-13-host-transport
- depends_on: story:slice-13-plan-admission
- depends_on: story:slice-13-transition-driver
- depends_on: story:slice-13-durability-recovery
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: crates/ignite/tests/apply.rs
- confidence: cited
  path: crates/ignite/tests/common
- confidence: cited
  path: docs/slice.md
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T20:47:22Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-02T20:47:23Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-03T02:58:02Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
## Scope

End-to-end tests for every 17.7 row, both targets where the row requires it; record the results in `docs/slice.md` and the gate-time row in `AGENTS.md`.

## Acceptance

17.7 rows 1, 2 (concurrent retry attaches to one operation) and 15; both targets for rows 1, 2, 4, 6, 7, 8, 9, 10, 11, 12; green `task gate`. First failing test: `apply::concurrent_retry_attaches_to_one_operation_{rust,llvm}`.

## State

Not started.

## Approval

Approved for implementation by Timo on 2026-11-02 17:33 ("Implement the plan"); the session brief: "That approval stands: implement slice 13, do not ask again".

## Source

`docs/slice.md` section 17; unit list of the slice-13 wave plan.
