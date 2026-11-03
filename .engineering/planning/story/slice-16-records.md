---
format: aep.planning-md/3
id: story:slice-16-records
kind: story
status: draft
title: Slice 16 records and close
summary: Holds fixture, evaluator families, performance records and the standing documents for wave A
relations:
- decomposes: epic:slice-16-language-gaps
- depends_on: story:slice-16-rest-patterns
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: PLAN.md
- confidence: cited
  path: STATUS.md
- confidence: cited
  path: crates/ignite/tests/fixtures/eval_equivalence.ignite
- confidence: cited
  path: crates/ignite/tests/fixtures/slice16/holds.ignite
- confidence: cited
  path: crates/ignite/tests/slice16.rs
- confidence: cited
  path: docs/actors.md
- confidence: cited
  path: docs/friction.md
- confidence: cited
  path: docs/performance/16
- confidence: cited
  path: docs/slice.md
- confidence: cited
  path: docs/syntax.md
- confidence: cited
  path: docs/vision.md
revision: 4
---
# Story: slice-16-records

## Outcome

Slice 16 is closed on record: the cross-target holds fixture, the evaluator equivalence families,
the performance records of 20.12 and the standing documents name what wave A delivered.

## Context

- Contract: `docs/slice.md` section 20 on `feat/slice16-language-gaps`, unit U6 of 20.10, rows 21
  to 23 and 27 of 20.0, and its share of rows 24 to 26.
- First failing test: `slice16::fixtures_pass_on_both_targets`.
- Appends to `crates/ignite/tests/slice16.rs` and `eval_equivalence.ignite` after U2, U4 and U5
  merged; edits the title and the stale points of `docs/slice.md` that 20.8 names.
- Ordering: `fix/source-doc-errors` edits `PLAN.md` and `docs/vision.md` (row X25). It lands on
  `main` first (release v0.4.1), and the slice branch merges `main` before W5 starts.
- Release: after the slice merges to `main`, the steering session tags v0.5.0 and publishes the
  GitHub release; that step is not this unit's.

## Acceptance

- Rows 21 to 23 and 27 of 20.0 pass; rows 24, 25 and 26 hold after this unit.
- `docs/friction.md` has a row for `assert` as a match-arm body (E001 today, 20.8 item 9).
- `STATUS.md`, `AGENTS.md`, `PLAN.md`, `docs/vision.md` §4.1 and §5, `docs/actors.md` R1 and
  §3.3 and `docs/syntax.md` P9 hold the content row 27 and 20.8 name; `docs/performance/16/` holds
  the 20.12 records.

## Out of Scope

- New language features.
