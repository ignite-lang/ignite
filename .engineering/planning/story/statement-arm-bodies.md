---
format: aep.planning-md/3
id: story:statement-arm-bodies
kind: story
status: draft
title: A match arm body may be a statement
relations:
- informed_by: story:pattern-test-is
revision: 1
---
# Story: a match arm body may be a statement

## Outcome

A `match` arm whose body is one assignment, one `assert` or one `for` is written without braces: `None => next.state = Stopped`, `Some(h) => assert h == 1`.

## Why

Three rows of `docs/friction.md` are open: 38 (an arm body cannot be an assignment, E001, since 2026-11-02), 42 (it cannot be an `assert`, since 2026-11-04) and 58 (`for` and assignment are not expressions, since 2026-11-06). Reported again on 2026-11-12 by the author of a command-line tool written in Ignite: a test that expects an `Err` wraps each `assert` in a block arm. `assert e is P` (`story:pattern-test-is`) shortens that test differently; this story is the arm body itself.

## Acceptance

- `arm := pattern '=>' (block | expr | assignment | assert | for)`; such an arm has type `Unit`, and a `match` with one is a statement or a `Unit` expression. Mixing a statement arm with a value arm is one diagnostic that names both arms.
- `return`, `break` and `continue` as arm bodies behave as today.
- A block's value position is unchanged: an assignment as the last statement of a block gives `Unit` as today, or the rule that changes is stated.
- It is a desugar to the braced arm; a differential test holds one program on the evaluator and both targets.
- Rows 38, 42 and 58 of `docs/friction.md` move to solved or their remainder is stated (row 58 names a block's value too); items 9 of section 20.8 and 7 of section 20.13 of `docs/slice.md`, which row 42 cites as open, are closed.
- The braced one-statement arms of `std/` and `examples/` are rewritten; `docs/slice.md` grammar; `task gate` green.

## Scope

- The parser and checker crates, `std/`, `examples/`, `docs/slice.md`, `docs/friction.md`. Inferred.

## Not in this story

- `for` and assignment as expressions with a value.
