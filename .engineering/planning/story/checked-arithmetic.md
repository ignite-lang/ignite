---
format: aep.planning-md/3
id: story:checked-arithmetic
kind: story
status: implemented
title: Checked Int arithmetic
summary: checked_add, checked_sub and checked_mul returning Option<Int>, so overflow can be tested before it ends a task
scope:
- confidence: inferred
  path: crates/ignite-eval/src
- confidence: inferred
  path: crates/ignite-llvm/src
- confidence: inferred
  path: crates/ignite-rustgen/src
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: std
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T07:25:52Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-06T07:26:03Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-11-06T08:10:57Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
# Story: checked-arithmetic

## Outcome

`Int` has checked operations (`checked_add`, `checked_sub`, `checked_mul`, returning `Option<Int>`)
on both targets and in the evaluator, so a program tests for overflow instead of ending its task.

## Context

- Row E7 found by the example program ember (2026-11-04, compiler `9578530`), `examples/friction.md`
  in the sibling examples repository: `SET k v EX 9223372036854775807` overflowed, ended the
  session task, closed the connection with no reply, and the replies to earlier commands of the
  same read were lost too. Found by a differential check against a handwritten Rust
  implementation. Workaround in ember: divide first (`n <= (MAX_INT - now) / unit`).
- Overflow ends the task by design (`overflow_fails_the_same_everywhere`); the missing piece is a
  way to test first.
- The replies to earlier commands of the same read were lost because ember collects them in a
  local buffer and writes it once after the loop (ember's `session`), so the overflow dropped
  the unwritten buffer; the substrate dropped nothing written (playground session, 2026-11-04).

## Acceptance

- Inline tests on both targets and an evaluator equivalence family: `checked_add(MAX_INT, 1)` is
  `None`, `checked_mul(3, 4)` is `Some(12)`, the same for sub and the minimum.

## Out of Scope

- Wrapping or saturating arithmetic; changing what plain overflow does.
