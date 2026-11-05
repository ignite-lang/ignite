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
revision: 9
---
# Story: slice-18-records

## Outcome

The performance records of slice 18 are in `docs/performance/18/`, and `AGENTS.md`, `STATUS.md`,
`PLAN.md` and `docs/vision.md` say what slice 18 landed. Unit U8 of `docs/slice.md` 22.13.

## Context

- Contract: 22.15 (method and units) on branch `feat/slice18-one-service` (`19c70564`).
- Depends on `story:slice-18-fault-matrix` and `story:apply-cycle-timing`.

## Acceptance

- Row 27: in the after tree the median LLVM dev build of `todod` alone is at most 0.15 of the
  median Rust dev build in the same passes, recorded under `docs/performance/18/`.
- Row 28: `apply-cycle` records exist for the todo (after tree, both targets) and the terminal
  design (both trees, both targets); the todo cycle's LLVM median is below its Rust median.
- Row 32: `AGENTS.md`'s gate-time section has a "Slice 18" paragraph naming the gate's exit and
  counts.
- `STATUS.md`, `PLAN.md` and `docs/vision.md` (S1, S6, S12) each name slice 18 with its merge
  commit.

## Out of Scope

- S11 of 22.11 (README and `site/` wording about what a check or build proves, review R6): no
  sentence of the epic asks for it; it stays for a later slice.
