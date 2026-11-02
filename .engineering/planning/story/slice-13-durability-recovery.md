---
format: aep.planning-md/3
id: story:slice-13-durability-recovery
kind: story
status: active
title: Host journal durability and crash recovery
relations:
- decomposes: epic:slice-13-local-apply
- depends_on: story:slice-13-transition-driver
scope:
- confidence: cited
  path: crates/ignite/src/apply/host/journal.rs
- confidence: cited
  path: crates/ignite/src/apply/host/recovery.rs
- confidence: cited
  path: crates/ignite/src/run/process.rs
- confidence: cited
  path: crates/ignite/tests/apply_recovery.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T20:47:22Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-02T20:47:22Z", actor: "human:timo", revision: 3}
---
## Scope

Sync-before-ack journal separate from `ignite-journal/0.1`; resume a half-applied plan after a crash; `outcome_unknown` where an effect cannot be confirmed; PID-reuse and foreign-listener protection; debug-only fault injection for the tests.

## Acceptance

17.7 rows 10, 11, 12 (PID reuse, foreign listener), 13 (sync failure) and 14. First failing test: `apply_recovery::crash_after_intent_before_dispatch_yields_one_server_or_outcome_unknown`.

## State

Not started; needs the transition driver's action and generation identity.

## Approval

Approved for implementation by Timo on 2026-11-02 17:33 ("Implement the plan"); the session brief: "That approval stands: implement slice 13, do not ask again".

## Source

`docs/slice.md` section 17; unit list of the slice-13 wave plan.
