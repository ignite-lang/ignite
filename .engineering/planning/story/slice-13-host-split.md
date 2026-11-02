---
format: aep.planning-md/3
id: story:slice-13-host-split
kind: story
status: implemented
title: Split the apply host into transport, admission, controller, journal and recovery
relations:
- decomposes: epic:slice-13-local-apply
scope:
- confidence: cited
  path: crates/ignite/src/apply/host
- confidence: cited
  path: crates/ignite/src/apply/host.rs
- confidence: cited
  path: crates/ignite/tests/apply.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T20:47:21Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-02T20:47:21Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-02T20:47:40Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
## Scope

Move `crates/ignite/src/apply/host.rs` into `apply/host/{mod,transport,admission,controller,journal,recovery}.rs` with no behaviour change, and share one host build directory across the apply end-to-end tests.

## Acceptance

Baseline only: the existing `apply::` tests pass before and after; `cargo check` and clippy clean.

## State

Done: commits c511da9 and 7ee4c46, fast-forwarded onto `feat/slice13-local-apply` and pushed.

## Approval

Approved for implementation by Timo on 2026-11-02 17:33 ("Implement the plan"); the session brief: "That approval stands: implement slice 13, do not ask again".

## Source

`docs/slice.md` section 17; unit list of the slice-13 wave plan.
