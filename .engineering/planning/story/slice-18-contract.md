---
format: aep.planning-md/3
id: story:slice-18-contract
kind: story
status: draft
title: 'Slice 18 contract: docs/slice.md section 22'
summary: The reviewed contract for one service operated end to end
relations:
- decomposes: epic:slice-18-one-service-operated
- depends_on: story:slice-17-contract
scope:
- confidence: cited
  path: docs/slice.md
- confidence: cited
  path: docs/slice18-review.md
revision: 2
---
# Story: slice-18-contract

## Outcome

`docs/slice.md` section 22 is the reviewed contract of slice 18, so units can work from it in
parallel.

## Context

- Parent: `epic:slice-18-one-service-operated`; source `docs/research/review-2026-11-04.md`.
- Written after slice 17's contract lands on `main` (it defines `Directory`), on a branch
  `feat/slice18-one-service`.
- Open points the contract settles: how a probe kind declared outside `std/` names its outcome
  type and how `good` becomes a typed predicate over it while keeping the two Telnet outcomes;
  which rows of the 17.7 matrix already cover R11 and which are new; how a corrupt journal record
  is detected (checksum or length frame) and what `ignite inspect` reports for it; the build-time
  method and its comparison with the Rust target.

## Acceptance

- Section 22 has an acceptance table with a command and an exact result per row, units whose owned
  files do not overlap within a wave, decisions with options, and its diagnostic codes in section 8;
  an independent review is committed as `docs/slice18-review.md` and its blockers are folded.

## Out of Scope

- Implementation.
