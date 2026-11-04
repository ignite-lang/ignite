---
format: aep.planning-md/3
id: story:apply-recovery-way-out
kind: story
status: draft
title: An operator path out of a non-resolvable recovery
summary: A host holding a later reason no corrective resolves has a stated command back to accepting plans
relations:
- depends_on: story:slice-13-recovery-gaps
scope:
- confidence: inferred
  path: crates/ignite/src/apply
- confidence: inferred
  path: crates/ignite/tests/apply_recovery.rs
- confidence: inferred
  path: docs/slice.md
revision: 2
---
# Story: apply-recovery-way-out

## Outcome

A host whose recovery holds a later reason that no corrective plan can resolve has an operator
path back to accepting plans, stated in `docs/slice.md` 17.5 and reached by a command.

## Context

- Found by the adversary on `fix/slice13-recovery-gaps` (2026-11-04, judgement finding at
  `docs/slice.md` line 4225 on that branch): once a non-resolvable later reason is kept beside
  `gave_up`, every apply is refused, corrective and retry alike ("operation requires recovery"),
  and 17.5 names no path beyond `ignite inspect`.

## Acceptance

- An e2e test drives a host into that state, runs the stated operator command, and then applies
  a plan to completion; the journal records the operator's step.

## Out of Scope

- The three contract-drift findings on the same branch; that branch's fixer takes them.
