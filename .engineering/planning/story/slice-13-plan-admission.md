---
format: aep.planning-md/3
id: story:slice-13-plan-admission
kind: story
status: active
title: Apply plan identity and admission
relations:
- decomposes: epic:slice-13-local-apply
- depends_on: story:slice-13-host-split
scope:
- confidence: cited
  path: crates/ignite/src/apply/digest.rs
- confidence: cited
  path: crates/ignite/src/apply/host/admission.rs
- confidence: cited
  path: crates/ignite/src/apply/plan.rs
- confidence: cited
  path: crates/ignite/src/apply/revision.rs
- confidence: cited
  path: crates/ignite/src/main.rs
- confidence: cited
  path: crates/ignite/tests/apply_admission.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T20:47:22Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-02T20:47:22Z", actor: "human:timo", revision: 3}
---
## Scope

Envelope `ignite-apply-plan/0.1` with every 17.3 field; SHA-256 with domain separation for every digest; operational identity versus admission pins; changed external members are `detach`, never `replace`; stale base, changed source and wrong identity rejected before mutation; receipt lookup before stale-base rejection; codegen-revision override for the compiler-upgrade case; migration and cycle rejection at plan time; the frozen action schema the transition driver consumes.

## Acceptance

17.7 rows 3, 5, 6, and the cycle and migration parts of 13. First failing test: `plan::tests::changed_external_member_is_detach_never_replace`.

## State

In progress on branch `s13/u3-admission`.

## Approval

Approved for implementation by Timo on 2026-11-02 17:33 ("Implement the plan"); the session brief: "That approval stands: implement slice 13, do not ask again".

## Source

`docs/slice.md` section 17; unit list of the slice-13 wave plan.
