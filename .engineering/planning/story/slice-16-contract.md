---
format: aep.planning-md/3
id: story:slice-16-contract
kind: story
status: implemented
title: 'Slice 16 contract: docs/slice.md section 20'
summary: Write and review the wave-A contract that the implementation units work from
relations:
- decomposes: epic:slice-16-language-gaps
scope:
- confidence: cited
  path: docs/slice.md
- confidence: cited
  path: docs/slice16-review.md
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T22:40:55Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-03T22:40:55Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-05T03:28:31Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---

# Story: slice-16-contract

## Outcome

`docs/slice.md` has a section 20 that a set of implementation agents can work from in parallel:
the slice-16 contract for wave A (language gaps), in the form of sections 18 and 19.

## Context

- Parent: `epic:slice-16-language-gaps`. Model: section 19 (acceptance table 19.0 with a command
  and an expected result per row and an owning unit; decisions D1..; work split with an "owns"
  cell per unit; waves; gate time; open questions; review).
- Inputs: `docs/vision.md` 4.1 rows A and C and section 6; `docs/friction.md`; `docs/design.md`
  sections on fn values, patterns, enums and `Map`; `docs/actors.md` R1 (last-use moves) and P4;
  `docs/protocols.md` §5.2 (rest patterns); the current parser, checker, typer and both backends.

## Acceptance

- Section 20 exists with: a status line per wave-A item (already holds, in this slice, or
  deferred with reason), each "already holds" citing a test or fixture; an acceptance table whose
  every row names a command and an exact expected result; units whose "owns" cells do not overlap
  within a wave; decisions with options and the one taken; diagnostics added to section 8.
- An independent review against `docs/vision.md` section 6 is committed beside it (as
  `docs/slice15-review.md` was) and its blockers are folded.

## Out of Scope

- Implementation of any item.
