---
format: aep.planning-md/3
id: story:match-guards
kind: story
status: draft
title: Match guards
relations:
- informed_by: story:pattern-test-is
revision: 1
---
# Story: match guards

## Outcome

A `match` arm takes a condition, `P if cond => …`: the arm is chosen when the pattern matches and the condition holds, and the next arms are tried otherwise.

## Why

`docs/syntax.md` holds it as proposal P4 with its syntax, rank 7 of its priority table (section 6), unscheduled; row 20 of `docs/friction.md` ("No match guards …") is open since 2026-11-01. Requested on 2026-11-12 by the author of a command-line tool written in Ignite, which ranks it last of its proposals: `e is P && cond` (`story:pattern-test-is`) covers its common case, and a guard proper remains for an arm that is an `if` without `else` beside an empty arm.

## Acceptance

- The syntax of `docs/syntax.md` P4: `arm := pattern ('if' expr)? '=>' (block | expr)`; the guard is a `Bool` expression that sees the arm's bindings.
- Exhaustiveness (E046) does not count a guarded arm as covering its pattern; the diagnostic names the guard.
- A guard performs the effects the enclosing fn may perform, and they are counted as the fn's; the guard is evaluated once per tried arm, in arm order, and a test pins the order with a counter.
- Both targets lower it without evaluating the scrutinee twice; a differential test holds one program on the evaluator and both targets.
- The `name @ lo..=hi` binding of row 20 is in this story or is split off with its own draft, stated in the report.
- The rewrite of `docs/syntax.md` section 9.2 is applied; row 20 of `docs/friction.md` moves to solved or its remainder is stated.
- `docs/slice.md` grammar and section, `site/book`; `task gate` green.

## Scope

- The parser and checker crates, `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, `std/`, `docs/slice.md`, `docs/syntax.md`, `docs/friction.md`. Inferred.

## Not in this story

- `e is P` (`story:pattern-test-is`), `let … else` (`story:let-else`).
