---
format: aep.planning-md/3
id: story:host-journal-integrity
kind: story
status: implemented
title: The host detects a torn, altered or cut journal
relations:
- decomposes: epic:slice-18-one-service-operated
- depends_on: story:slice-18-contract
scope:
- confidence: cited
  path: crates/ignite/src/apply/host/journal.rs
- confidence: cited
  path: crates/ignite/src/apply/host/mod.rs
- confidence: cited
  path: crates/ignite/src/apply/mod.rs
- confidence: cited
  path: crates/ignite/tests/apply_integrity.rs
- confidence: cited
  path: crates/ignite/tests/main.rs
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T09:47:32Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-11-05T09:47:33Z", actor: "human:timo", revision: 8}
- {from: "active", to: "implemented", at: "2026-11-05T12:43:29Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1}}}
---
# Story: host-journal-integrity

## Outcome

The host drops a torn last record and journals the repair, refuses an altered record or a detected cut with exit 3, and `ignite inspect` reports the journal state offline; the host records its working directory (D8 to D10, D7).

## Context

- Unit U3 of `docs/slice.md` 22.13 (branch `feat/slice18-one-service`, contract at `19c70564`): owned files, wave and shared-file rules are there; decisions in 22.12.
- Depends on: story:slice-18-contract.

## Acceptance

- The first failing test `apply::host::journal::tests::an_altered_record_names_its_index` is red before and green after, on both targets where it has a target.
- Rows 23, 24, 33 and the unit level of 20 to 22 of 22.0 pass with the exact results the table names; rows 8 and 31 hold; `task gate` green.

## Out of Scope

- Files 22.13 gives to another unit.
