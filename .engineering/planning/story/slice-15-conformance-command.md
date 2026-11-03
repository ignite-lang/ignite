---
format: aep.planning-md/3
id: story:slice-15-conformance-command
kind: story
status: proposed
title: ignite conformance prints the matrix
summary: Rows and verdicts per protocols.md 3.3 from the claim model and verifies test results
relations:
- decomposes: epic:slice-15-protocols-p2
- depends_on: story:slice-15-claim-model
- depends_on: story:slice-15-rfc-importer
scope:
- confidence: inferred
  path: crates/ignite/src/conformance.rs
- confidence: inferred
  path: crates/ignite/src/lib.rs
- confidence: inferred
  path: crates/ignite/src/main.rs
- confidence: inferred
  path: crates/ignite/src/test_build.rs
- confidence: inferred
  path: crates/ignite/tests/conformance.rs
- confidence: inferred
  path: crates/ignite/tests/fixtures/conformance
- confidence: inferred
  path: crates/ignite/tests/main.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T16:02:31Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":2}}}
---

# Story: slice-15-conformance-command

## Outcome

`ignite conformance <file>` prints one row per requirement in scope with its verdict and a claim
line, from the claim model and the results of the `verifies` tests.

## Context

`docs/protocols.md` 3.3: verdicts pass, fail, gap, deviation, inconclusive, n/a and out of scope;
"The claim holds when no MUST-level row is fail, gap or inconclusive"; a skip makes the result
inconclusive. The output format and how tests are run are the contract's D8.

## Scope

- `crates/ignite/src/conformance.rs` (new) and its `mod` line in `crates/ignite/src/lib.rs`.
- The `conformance` subcommand in `crates/ignite/src/main.rs` and a `mod` line in
  `crates/ignite/tests/main.rs`: both files are also edited by `story:slice-15-rfc-importer`, which
  this story depends on for that reason.
- Running the `verifies` tests through the existing `ignite test` path
  (`crates/ignite/src/test_build.rs`).
- `crates/ignite/tests/conformance.rs` (new) with fixture protocols and catalogues under
  `crates/ignite/tests/fixtures/conformance/`.

## Acceptance

- One fixture per verdict (pass, fail, gap, deviation, inconclusive, n/a, out of scope), each
  printing its row as the golden says.
- A failing `verifies` test turns its row to fail and the claim line to no.
- A requirement that is satisfied but has no passing test prints inconclusive.
- A fixture whose MUST rows all pass prints a claim line that says yes, with its counts of gaps,
  failures and inconclusive rows (all 0), in D8's format.
- A fixture whose only non-pass MUST row is inconclusive prints a claim line that says no, with
  1 inconclusive in its counts.
- Two runs print byte-identical output.

## Depends on

`story:slice-15-claim-model` (the claim model API), `story:slice-15-rfc-importer` (shared
`main.rs` and `tests/main.rs`).
