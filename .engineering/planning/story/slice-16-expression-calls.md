---
format: aep.planning-md/3
id: story:slice-16-expression-calls
kind: story
status: implemented
title: Calls of fn-typed expressions
summary: Check and type a call whose callee is an expression of fn type, with E040, E042 and E043
relations:
- decomposes: epic:slice-16-language-gaps
- depends_on: story:slice-16-contract
scope:
- confidence: cited
  path: crates/ignite-check/src/lib.rs
- confidence: cited
  path: crates/ignite-check/tests/fixtures
- confidence: cited
  path: crates/ignite-types/src/typer.rs
- confidence: cited
  path: crates/ignite-types/tests/fixtures
- confidence: cited
  path: crates/ignite/tests/fixtures/slice16/calls.ignite
- confidence: cited
  path: crates/ignite/tests/fixtures/slice16/closures.ignite
- confidence: cited
  path: crates/ignite/tests/main.rs
- confidence: cited
  path: crates/ignite/tests/slice16.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T23:35:37Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-03T23:35:37Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-04T07:01:36Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: slice-16-expression-calls

## Outcome

A call whose callee is an expression of fn type, such as `adder(1)(2)`, checks and types instead
of E003, with the diagnostics E040, E042 and E043 of `docs/slice.md` 20.4 and 20.7.

## Context

- Contract: `docs/slice.md` section 20 on `feat/slice16-language-gaps`, unit U2 of 20.10, rows 6
  to 8 of 20.0, decisions D4 and D5.
- First failing test: `fixtures::fixtures_match_expected` (ignite-check) on
  `valid_expression_calls`.

## Acceptance

- Rows 6 to 8 of 20.0 pass with the commands and results the table names.
- Rows 24 and 25 hold.

## Out of Scope

- Inferring a closure parameter's type from its body; passing a closure that uses a captured
  capability as a fn value (both deferred, 20.13).
