---
format: aep.planning-md/3
id: story:structural-apply-identity
kind: story
status: active
title: Saved plans identify an implementation by structure, not Debug text
scope:
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T07:25:52Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-06T07:26:03Z", actor: "human:timo", revision: 4}
---
# Story: structural-apply-identity

## Outcome

Apply identities hash a canonical form of the declaration (or the IR) instead of `format!("{decl:?}")` with spans scrubbed by a string scan.

## Context

- Recommendation 6 of the architecture review (`docs/research/review-2026-11-05-architecture.md`), effort S/M.
- Evidence: `crates/ignite/src/apply/plan.rs:1538-1580`, pinned by a hand-bumped `CODEGEN_REVISION` (`:15-17`).

## Acceptance

- a span-only edit and a rename of a private helper keep the plan identity; a change of behaviour changes it; tests in `apply/plan.rs`
- `task gate` green.

## Out of Scope

- The other recommendations of the review.
