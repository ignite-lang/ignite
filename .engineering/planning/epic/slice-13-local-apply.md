---
format: aep.planning-md/3
id: epic:slice-13-local-apply
kind: epic
status: active
title: 'Slice 13, F4: ignite apply on one local host'
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T20:47:20Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-02T20:47:20Z", actor: "human:timo", revision: 3}
---
## Outcome

`ignite host` runs a foreground host over a state directory; `ignite plan --for-apply` writes a
source-bound, digest-pinned plan; `ignite apply` admits it, drives replacement-first transitions
with draining cutover, journals every step durably, and recovers a half-applied plan after a
crash; `ignite inspect` reports the host's standing. A second `apply` of the same plan is a no-op.

## Contract

`docs/slice.md` section 17 (merged to main as 50beccb). Acceptance is the matrix in 17.7, every
row on both targets where the row says so, plus a green `task gate`.

## Approval

Approved for implementation by Timo on 2026-11-02 17:33 ("Implement the plan"); the session brief: "That approval stands: implement slice 13, do not ask again".

## Integration

Branch `feat/slice13-local-apply`; units merge into it, the branch merges to main on a green
`task gate`, then a release is tagged.
