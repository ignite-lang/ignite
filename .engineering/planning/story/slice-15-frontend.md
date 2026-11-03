---
format: aep.planning-md/3
id: story:slice-15-frontend
kind: story
status: active
title: RFC vocabulary and linking clauses in the frontend
summary: std/rfc/rfc.ignite, the six clauses, E091 for a non-literal catalogue constant; clauses emit nothing on both targets
relations:
- decomposes: epic:slice-15-protocols-p2
- depends_on: story:slice-15-contract
- depends_on: story:rustgen-type-parameter-shadowing
scope:
- confidence: inferred
  path: crates/ignite-check/src
- confidence: inferred
  path: crates/ignite-check/tests
- confidence: inferred
  path: crates/ignite-check/tests/fixtures
- confidence: inferred
  path: crates/ignite-check/tests/layering.rs
- confidence: inferred
  path: crates/ignite-rustgen/tests/units.rs
- confidence: inferred
  path: crates/ignite-syntax/src
- confidence: inferred
  path: crates/ignite-syntax/tests
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: docs/layering.md
- confidence: inferred
  path: std/rfc/rfc.ignite
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T16:02:31Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":5}}}
- {from: "proposed", to: "active", at: "2026-11-03T16:07:58Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":5}}, correlation: "slice-15-wave-1"}
---

# Story: slice-15-frontend

## Outcome

The frontend accepts the RFC vocabulary and the linking clauses: `std/rfc/rfc.ignite` checks, a
catalogue constant that is not literal data is rejected, and clause-bearing programs build and test
on both targets with the clauses emitting nothing.

## Context

`docs/protocols.md` 3.1 (vocabulary types `Level`, `Party`, `Source`, `Requirement`, `Spec`;
"Literal data only", E091) and 3.2 (clauses `conforms`, `scope`, `satisfies`, `verifies`,
`unsupported`, `not_applicable`, contextual words); 6.1 (the clauses emit nothing). Constants
today: `docs/slice.md` 4.3. The vocabulary text is the contract's D1.

## Scope

- `crates/ignite-syntax/`: the six clauses on `protocol`, `fn`, `type`, record fields and `test`;
  AST fields added, none renamed (frozen skeletons).
- `crates/ignite-rustgen/tests/units.rs`: its full AST literals (`FnDecl` line 218, `RecordDecl`
  277, `Field` 281, `ConstDecl` 1019, `ProtocolDecl` 1148) get the new fields. Ordered after
  `story:rustgen-type-parameter-shadowing`, which edits `golden.rs` in the same test binary.
- `crates/ignite-check/`: resolution of the names a clause gives (a spec constant, a requirement
  constant) by name; E091's first trigger, a catalogue constant that is not literal data; fixtures
  with `.expected`.
- `crates/ignite-types/`: types of the clauses' arguments (a `Spec`, a `Requirement`).
- Tests and fixtures: `crates/ignite-syntax/tests/`, `crates/ignite-check/tests/fixtures/`
  (including its std module list asserted by `fixtures.rs`).
- `std/rfc/rfc.ignite`: the vocabulary module; its row in `docs/layering.md` and `LAYERS`
  (`crates/ignite-check/tests/layering.rs`) as the contract's D7 says.

## Acceptance

- `ignite check std/rfc/rfc.ignite` exit 0, no output.
- The E091 fixture of a non-literal catalogue constant reports exactly its `.expected` diagnostic.
- A fixture protocol with `conforms`, `scope`, `unsupported`, `not_applicable`, `satisfies` on a
  fn, on a type and on a limits-record field, and a `test .. verifies [..]` checks with exit 0 and
  no output.
- `ignite test` of that fixture passes with `--target rust` and with `--target llvm`.
- `ignite emit` of that fixture and of a copy with every clause deleted print byte-identical output,
  once per target.
- `cargo test --workspace` passes with no golden changed.

## Out of Scope

- E090, E092 and E091's second trigger (a duplicate requirement id): they need each spec's
  requirement set, which the evaluator computes (`story:slice-15-claim-model`).
