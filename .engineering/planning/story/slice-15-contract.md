---
format: aep.planning-md/3
id: story:slice-15-contract
kind: story
status: proposed
title: 'Slice 15 contract: docs/slice.md section 19, reviewed against vision section 6'
summary: Acceptance table, decisions, work split and E090 to E092 rows for protocols P2
relations:
- decomposes: epic:slice-15-protocols-p2
scope:
- confidence: inferred
  path: docs/protocols.md
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: docs/slice15-review.md
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T16:02:31Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"review_outcome":2}}}
---

# Story: slice-15-contract

## Outcome

`docs/slice.md` has a section 19, the slice-15 contract for protocols P2, and a review of it against
`docs/vision.md` section 6 is recorded, so implementors can be dispatched against one text.

## Context

- `docs/vision.md` 4.1: "Each slice starts with a contract section in `docs/slice.md`, reviewed by
  the steering session against section 6 before agents are dispatched." Slice 14 did the same:
  section 18 and `docs/slice14-review.md`.
- Design input: `docs/protocols.md` 3.1 to 3.3, 3.7 and the P2 row of section 7.

## Scope

Files: `docs/slice.md` (new section 19 and new rows in section 8 for E090, E091, E092),
`docs/slice15-review.md` (new), `docs/protocols.md` (stale points the contract finds, only).
No code.

The contract takes these decisions, each a numbered entry of its decisions subsection:

- D1 the vocabulary text of `std/rfc/rfc.ignite` (types, variants and field names), which the
  frontend writes and the importer targets;
- D2 whether catalogue constants are compile-time data only (never emitted by either backend) or
  compiled on both targets, and the diagnostic for a runtime use if they are compile-time only;
- D3 the grammar of the six clauses, and for each of E090, E091 (both triggers) and E092 the crate
  that reports it and the story that owns it; the code for two statuses on one requirement;
- D4 the catalogue extent: RFC 9112 truncated to sections 2 to 9 at import or imported whole and
  narrowed by `scope`; the list of RFC 9110 sections and parties that make the "server subset";
- D5 the RFCXML sources: vendored into the repository (path) or not, and their licence note;
- D6 the `ignite rfc import` input (a local RFCXML path, any section or party filter D4 needs),
  its XML reader (hand-written or a dependency, with the line the commit message needs) and the
  digest algorithm;
- D7 the layer of `std.rfc` and its catalogues in `docs/layering.md` and `LAYERS`
  (`crates/ignite-check/tests/layering.rs`): one row per module or a prefix rule as for
  `std.protocol.*`;
- D8 how `ignite conformance` obtains test results (which target runs the `verifies` tests, a
  `--target` flag or not) and its exact output format, including the claim line;
- D9 the work split that names the units and their files; where it differs from the drafted stories
  under `epic:slice-15-protocols-p2`, the stories' bodies are updated to match.

## Acceptance

- `docs/slice.md` section 19 has an acceptance table with one row per protocols.md P2 acceptance
  test, each naming its test and unit.
- Section 19's decisions subsection has an entry for each of D1 to D9 above.
- Section 8 has rows for E090, E091 and E092.
- `docs/slice15-review.md` lists each `docs/vision.md` section 6 item checked and its result; no
  finding of it is open.
- Every story under the epic names the section-19 rows it implements.

## Out of Scope

Any code or test change.
