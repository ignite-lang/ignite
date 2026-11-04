---
format: aep.planning-md/3
id: story:slice-16-rest-patterns
kind: story
status: draft
title: Rest patterns in list patterns
summary: Add the rest element to list patterns in every crate, with exhaustiveness split by length and E076
relations:
- decomposes: epic:slice-16-language-gaps
- depends_on: story:slice-16-rust-moves
- depends_on: story:slice-16-llvm-moves
scope:
- confidence: cited
  path: crates/ignite-check/src
- confidence: cited
  path: crates/ignite-check/tests
- confidence: cited
  path: crates/ignite-eval/src
- confidence: cited
  path: crates/ignite-llvm/src
- confidence: cited
  path: crates/ignite-llvm/tests
- confidence: cited
  path: crates/ignite-rustgen/src
- confidence: cited
  path: crates/ignite-rustgen/tests
- confidence: cited
  path: crates/ignite-syntax/src
- confidence: cited
  path: crates/ignite-syntax/tests
- confidence: cited
  path: crates/ignite-types/src
- confidence: cited
  path: crates/ignite-types/tests
- confidence: cited
  path: crates/ignite/src/apply/plan.rs
- confidence: cited
  path: crates/ignite/tests/fixtures/eval_equivalence.ignite
- confidence: cited
  path: crates/ignite/tests/fixtures/slice16/rest.ignite
- confidence: cited
  path: crates/ignite/tests/slice16.rs
revision: 4
---
# Story: slice-16-rest-patterns

## Outcome

List patterns take a rest element, as `docs/syntax.md` P9 proposes, in every crate from the parser
to both backends, with exhaustiveness split by length (review finding B3) and the diagnostic E076.

## Context

- Contract: `docs/slice.md` section 20 on `feat/slice16-language-gaps`, unit U5 of 20.10, rows 16
  to 20 of 20.0, decision D6. One unit adds `PatternKind::Rest` and every arm over it (20.11).
- First failing test: `fixtures::fixtures_match_expected_diagnostics` (ignite-syntax) on
  `valid_rest_patterns`.

## Acceptance

- Rows 16 to 20 of 20.0 pass on both targets and in the evaluator; rows 24 and 25 hold.
- Row 15's second step: `CODEGEN_REVISION` is one step past its value after W3.
- Adds `slice16/rest.ignite` to `slice16::fixtures_pass` (rustgen) and `slice16::llvm_fixtures_pass`
  (`crates/ignite/tests/slice16.rs`, after U2 and U4 merged), and the `rest_` family to
  `eval_equivalence.ignite`.

## Out of Scope

- Rest patterns in records or tuples.
