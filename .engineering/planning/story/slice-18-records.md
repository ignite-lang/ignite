---
format: aep.planning-md/3
id: story:slice-18-records
kind: story
status: draft
title: Slice 18 records and close
relations:
- decomposes: epic:slice-18-one-service-operated
- depends_on: story:slice-18-contract
- depends_on: story:slice-18-fault-matrix
- depends_on: story:apply-cycle-timing
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: PLAN.md
- confidence: cited
  path: STATUS.md
- confidence: cited
  path: docs/performance/18
- confidence: cited
  path: docs/vision.md
- confidence: cited
  path: site
revision: 7
---
# Story: slice-18-records

## Outcome

The performance records of slice 18 are in `docs/performance/18/` and the standing documents say what landed.

## Context

- Unit U8 of `docs/slice.md` 22.13 (branch `feat/slice18-one-service`, contract at `19c70564`): owned files, wave and shared-file rules are there; decisions in 22.12.
- Depends on: story:slice-18-fault-matrix, story:apply-cycle-timing.

## Acceptance

- The first failing test `none (records)` is red before and green after, on both targets where it has a target.
- Rows 27, 28, 32 of 22.0 pass with the exact results the table names; rows 8 and 31 hold; `task gate` green.

## Out of Scope

- Files 22.13 gives to another unit.
