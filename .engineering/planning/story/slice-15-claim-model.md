---
format: aep.planning-md/3
id: story:slice-15-claim-model
kind: story
status: active
title: Catalogue evaluation, the claim model and E090
summary: Evaluator reads catalogue constants, computes requirements in scope and statuses, reports E090
relations:
- decomposes: epic:slice-15-protocols-p2
- depends_on: story:slice-15-frontend
scope:
- confidence: inferred
  path: crates/ignite-eval/Cargo.toml
- confidence: inferred
  path: crates/ignite-eval/src
- confidence: inferred
  path: crates/ignite-eval/tests
- confidence: inferred
  path: crates/ignite/tests/fixtures/claim_ok_complete
- confidence: inferred
  path: crates/ignite/tests/fixtures/claims_std
- confidence: inferred
  path: crates/ignite/tests/fixtures/e090
- confidence: inferred
  path: crates/ignite/tests/fixtures/e091
- confidence: inferred
  path: crates/ignite/tests/fixtures/e092
- confidence: inferred
  path: crates/ignite/tests/plan.rs
- confidence: inferred
  path: docs/layering.md
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T16:02:31Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":4}}}
- {from: "proposed", to: "active", at: "2026-11-03T16:07:59Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"review_outcome":4}}, correlation: "slice-15-wave-1"}
---

# Story: slice-15-claim-model

## Outcome

The compiler knows, for each protocol that `conforms`, which requirements are in scope and what
status each has, and `ignite check` rejects a claim that is incomplete or names requirements
outside its specs.

## Context

`docs/protocols.md` 3.2: "Each requirement in scope at MUST, MUST NOT, REQUIRED, SHALL or SHALL NOT
level needs exactly one of three statuses ... A missing status is E090 in `ignite check`. A SHOULD
or MAY with no status shows as `unclassified`." "Naming a requirement outside the protocol's specs
is E092." 3.1: a duplicate requirement id is E091. The P2 row's "literal-constant evaluation" is
this story: the evaluator (`crates/ignite-eval`, `docs/slice.md` 15) reads catalogue constants.

## Scope

- `crates/ignite-eval/`: evaluation of catalogue constants; the claim model, an add-only public API
  (spec, scope by parties and sections, per requirement: level, status, the declarations that
  satisfy it, the tests that verify it, a limit's configured value against `minimum`), which
  `story:slice-15-conformance-command` reads; E090, E092 and E091's second trigger (a duplicate
  requirement id); API tests in `crates/ignite-eval/tests/`.
- No change in `crates/ignite/src/`: the claim check runs inside `ignite_eval::Model::check`, which
  `Checked::check` (`crates/ignite/src/lib.rs:1115`) already calls.
- Fixtures: `crates/ignite/tests/fixtures/e090_*`, `e091_*`, `e092_*` with `.expected`, run by
  `plan::fixtures_match_expected` (`crates/ignite/tests/plan.rs:337`), whose name filter gains
  `e090` to `e092`. No new test file and no `mod` line in `crates/ignite/tests/main.rs`, which
  `story:slice-15-rfc-importer` edits in the same wave.

## Acceptance

- A fixture protocol conforming to a fixture catalogue that holds one requirement at each of MUST,
  MUST NOT, REQUIRED, SHALL and SHALL NOT, with no statuses: `ignite check` exit 1 with five E090,
  as its `.expected` says.
- The same fixture with a status on each: exit 0, no output.
- A requirement with two statuses: the diagnostic the contract's D3 names, per its fixture.
- The first fixture with one more MUST outside the `scope` sections and one more whose party the
  `scope` excludes: still exactly five E090, none for the two.
- A `satisfies`, a `verifies` and an `unsupported` status, each naming a requirement from a spec
  the protocol does not conform to: one E092 each, per their fixtures.
- A catalogue with two requirements of one id: E091, per its fixture.
- An API test reads a SHOULD with no status from the claim model as `unclassified`, and a limit
  configured below its requirement's `minimum` as not meeting it.

## Depends on

`story:slice-15-frontend` (clauses in the AST, resolved names).

## Contract

Unit U2 of `docs/slice.md` 19.16 (merged on `feat/slice15-protocols-p2` as c98a41f): its "owns" cell is this story's file list, its rows of 19.0 are 14 to 18, 38, 40, 41, its first failing test is `plan::fixtures_match_expected` on `e090_missing_status`. Where this body and section 19 differ, section 19 holds; the differences are listed in the contract report and in `docs/slice15-review.md` "Resolution".
