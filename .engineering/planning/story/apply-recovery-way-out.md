---
format: aep.planning-md/3
id: story:apply-recovery-way-out
kind: story
status: implemented
title: An operator path out of a non-resolvable recovery
summary: A host holding a later reason no corrective resolves has a stated command back to accepting plans
relations:
- depends_on: story:slice-13-recovery-gaps
- decomposes: epic:slice-18-one-service-operated
- depends_on: story:slice-18-contract
- depends_on: story:host-journal-integrity
- depends_on: story:typed-probe-outcomes
scope:
- confidence: cited
  path: crates/ignite/src/apply/host/admission.rs
- confidence: cited
  path: crates/ignite/src/apply/host/controller.rs
- confidence: cited
  path: crates/ignite/src/apply/host/recovery.rs
- confidence: cited
  path: crates/ignite/src/apply/host/transport.rs
- confidence: cited
  path: crates/ignite/src/apply/mod.rs
- confidence: cited
  path: crates/ignite/src/main.rs
- confidence: cited
  path: crates/ignite/tests/apply_way_out.rs
- confidence: cited
  path: crates/ignite/tests/main.rs
- confidence: cited
  path: docs/slice.md
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T12:43:30Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "proposed", to: "active", at: "2026-11-05T12:43:30Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "active", to: "implemented", at: "2026-11-05T18:06:35Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":3}}}
---
# Story: apply-recovery-way-out

## Outcome

A host whose recovery holds a later reason that no corrective plan can resolve has an operator
path back to accepting plans: `ignite recover --state S --operation <id>` stops every owned
instance, restarts the committed revision, journals `operatorRecovered`, and refuses what it
cannot prove (D11). `docs/slice.md` 17.5 names the command (S8). Unit U5 of `docs/slice.md` 22.13.

## Context

- Found by the adversary on `fix/slice13-recovery-gaps` (2026-11-04, judgement finding at
  `docs/slice.md` line 4225 on that branch): once a non-resolvable later reason is kept beside
  `gave_up`, every apply is refused, corrective and retry alike ("operation requires recovery"),
  and 17.5 names no path beyond `ignite inspect`.
- Contract: 22.9 and 22.12 D11 on branch `feat/slice18-one-service` (`19c70564`).
- Order: after `story:host-journal-integrity` (U3 owns `apply/mod.rs` first) and after
  `story:typed-probe-outcomes` (U1 rewrites the terminal design that row 25 drives).
- Shared file: `docs/slice.md`. U1 edits sections 16 and 19 (S2, S3), this story only the 17.5
  line; the edits are section-disjoint and merge in wave order.

## Acceptance

- Row 25: `apply_way_out::{rust,llvm}_operator_recover_returns_the_host_to_accepting_plans` is red
  before and green after, with the exits and messages 22.0 row 25 names (corrective exit 2, retry
  exit 1, `ignite recover` exit 0, then a plan applied to `completed`, the failed operation
  `abandoned`, one `operatorRecovered` record).
- Row 26: `apply_way_out::recover_refuses_an_unaccounted_process`: `ignite recover` exits 1 with
  `recovery refused: unaccounted process <pid> runs <path>`, signals and journals nothing; for an
  operation not `recovery_required` it exits 2 with `operation <id> requires no recovery`.
- `docs/slice.md` 17.5 names `ignite recover` (S8).
- Rows 8 and 31 hold; `task gate` green.

## Out of Scope

- The three contract-drift findings on the same branch; that branch's fixer takes them.
- Files 22.13 gives to another unit.
