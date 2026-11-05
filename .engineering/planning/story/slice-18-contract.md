---
format: aep.planning-md/3
id: story:slice-18-contract
kind: story
status: active
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
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T08:56:51Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-05T08:56:51Z", actor: "human:timo", revision: 4}
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

- Section 22 has an acceptance table (22.0) with a command and an exact result per row.
- Its work split (22.13) gives each unit owned files; every file two units of one wave share is
  named with its merge order (W2: `crates/ignite/tests/plan.rs`, U1 then U2).
- Its decisions (22.12) each list lettered options and a default.
- Its new diagnostic codes are in section 8.
- An independent review is committed as `docs/slice18-review.md`, and 22.17 maps each of its
  blockers to the change that folds it.

## Out of Scope

- Implementation.
