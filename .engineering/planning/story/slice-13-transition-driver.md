---
format: aep.planning-md/3
id: story:slice-13-transition-driver
kind: story
status: active
title: Transition driver with draining cutover
relations:
- decomposes: epic:slice-13-local-apply
- depends_on: story:slice-13-plan-admission
scope:
- confidence: cited
  path: crates/ignite/src/apply/host/controller.rs
- confidence: cited
  path: crates/ignite/src/apply/revision.rs
- confidence: cited
  path: crates/ignite/src/run/live.rs
- confidence: cited
  path: crates/ignite/tests/fixtures/control_trace.ignite
- confidence: cited
  path: std/control.ignite
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T20:47:22Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-02T20:47:22Z", actor: "human:timo", revision: 3}
---
## Scope

Drive add, replace, remove and detach in dependency order through a `Transition` reducer in `std/control.ignite`, replacement-first with draining cutover, probes and standings feeding readiness; a machine that gives up ends the operation.

## Acceptance

17.7 rows 4, 7, 8, 9 and the probe and standing part of row 1. First failing test: `apply_transition::add_remove_dependent_orders_provider_first_external_untouched`.

## State

Not started; needs the action schema of the admission story.

## Approval

Approved for implementation by Timo on 2026-11-02 17:33 ("Implement the plan"); the session brief: "That approval stands: implement slice 13, do not ask again".

## Source

`docs/slice.md` section 17; unit list of the slice-13 wave plan.
