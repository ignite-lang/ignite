---
format: aep.planning-md/3
id: story:slice-18-records
kind: story
status: active
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
  path: README.md
- confidence: cited
  path: STATUS.md
- confidence: cited
  path: docs/performance/18
- confidence: cited
  path: docs/vision.md
revision: 13
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T19:13:06Z", actor: "human:timo", revision: 12, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "proposed", to: "active", at: "2026-11-05T19:13:06Z", actor: "human:timo", revision: 13, decided_on: {"recorded":{"review_outcome":3}}}
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
- Row 32: `ls docs/performance/18/` lists the records, logs and scripts of every unit of 22.15,
  and `AGENTS.md`'s gate-time section has one paragraph starting `Slice 18` naming the gate's
  head, exit, cargo counts and inline counts per target.
- S10: `AGENTS.md`'s layout table names `examples/todo/health.ignite` and the durable todo, and
  its commands table has a row for `ignite recover`.
- `STATUS.md`, `PLAN.md` and `docs/vision.md` (S1, S6, S12) each have a slice 18 row naming the
  gate commit and "merge into `main` pending", as the rows of slices 15 to 17 did.

## Out of Scope

- S11 of 22.11 (README and `site/` wording about what a check or build proves, review R6): no
  sentence of the epic asks for it; it stays for a later slice.
