---
format: aep.planning-md/3
id: story:mid-level-ir
kind: story
status: active
title: One typed mid-level IR for both backends and the evaluator
relations:
- depends_on: story:shared-ownership-analysis
- depends_on: story:quality-ratchet
scope:
- confidence: inferred
  path: crates/ignite-eval/src
- confidence: inferred
  path: crates/ignite-llvm/src
- confidence: inferred
  path: crates/ignite-rustgen/src
- confidence: inferred
  path: crates/ignite-types/src
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T12:35:13Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-11-07T12:35:13Z", actor: "human:timo", revision: 7}
---
# Story: mid-level-ir

## Outcome

Patterns, builtin operations with explicit copy and move, failure sites, constructors with defaults and executable mains are lowered once from the typed program into an IR that the Rust target, the LLVM target and the evaluator consume, so each rule has one implementation (design 8.5).

## Context

- Recommendation 1 of the architecture review (`docs/research/review-2026-11-05-architecture.md`), effort L.
- Evidence: the semantics of 96 builtins are implemented three times (`crates/ignite-rustgen/src/lib.rs:4061`, `crates/ignite-llvm/src/lower.rs:3687`, `crates/ignite-eval/src/eval.rs:1042`); estimate 13,700 to 11,500 lines (inferred).

## Acceptance

- a contract section and its first unit move one rule (pattern matching) to the IR on all three consumers with every golden and inline test unchanged
- `task gate` green.

## Out of Scope

- The other recommendations of the review.
