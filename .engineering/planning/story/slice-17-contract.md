---
format: aep.planning-md/3
id: story:slice-17-contract
kind: story
status: active
title: 'Slice 17 contract: docs/slice.md section 21'
summary: The reviewed contract for processes, files, settings and entropy
relations:
- decomposes: epic:playground-substrate
scope:
- confidence: cited
  path: docs/slice.md
- confidence: cited
  path: docs/slice17-review.md
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T01:57:17Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-04T01:57:17Z", actor: "human:timo", revision: 4}
---
# Story: slice-17-contract

## Outcome

`docs/slice.md` section 21 is the slice-17 contract for processes, files, settings and entropy,
reviewed independently, so units can work from it in parallel.

## Context

- Parent: `epic:playground-substrate`. Written on `feat/slice17-processes-files`: commits
  `82ab6b3` (draft, 34 rows, E140 to E142), `212d2eb` (probes; row 35 closes a parameter named
  `clock` absorbing `Clock.monotonic()`'s effect), `eac295b` (review), `d3d95b0` (37 findings
  folded, 21.17).
- Steering answers to 21's open questions: Q1 to Q5 and Q7 take their defaults; Q6 yes, `Setting`
  belongs to `story:program-arguments`.

## Acceptance

- Section 21 has an acceptance table with a command and an exact result per row, units whose
  "owns" cells do not overlap within a wave, decisions with options, and E140 to E142 in section 8;
  `docs/slice17-review.md` is committed and its blockers are folded.

## Out of Scope

- Implementation.
