---
format: aep.planning-md/3
id: story:let-else
kind: story
status: draft
title: let ... else
relations:
- informed_by: story:pattern-test-is
revision: 1
---
# Story: `let … else`

## Outcome

`let P = e else { … }` binds the names of a refutable pattern for the rest of the block and leaves through the `else` block when the pattern does not match, so an early exit on `None` or `Err` does not nest the rest of the function in a `match`.

## Why

`docs/syntax.md` holds it as proposal P2 with its syntax, rank 6 of its priority table (section 6), unscheduled. Requested on 2026-11-12 by the author of a command-line tool written in Ignite: a `let x = match … { Some(v) => v, None => return … }` remains at the sites that neither `map_err` nor `?` on an `Option` (`story:option-try-in-fns`) covers, because the exit returns a value built at that site or is a `continue`.

## Acceptance

- The syntax of `docs/syntax.md` P2: `stmt := 'let' pattern (':' type)? '=' expr 'else' block`; no new reserved word. Only here may a `let` pattern be refutable; elsewhere it stays E001.
- The `else` block must leave: it ends in `return`, `break`, `continue` or a call that does not return; a block that can fall through is one diagnostic.
- Names of the pattern are in scope after the statement and not in the `else` block.
- An irrefutable pattern with an `else` is a diagnostic that says the `else` cannot run.
- It lowers to the `match` a user writes today; a differential test holds one program on the evaluator and both targets, with `return`, `break` and `continue` exits.
- The rewrites of `docs/syntax.md` section 9 that use it are applied to `std/` and `examples/`; the lines saved are counted with the document's method and written into its section 6.
- `docs/slice.md` grammar and section, `site/book`; `task gate` green.

## Scope

- The parser and checker crates, `std/`, `examples/`, `docs/slice.md`, `docs/syntax.md`. Inferred.

## Not in this story

- `e is P` (`story:pattern-test-is`), match guards (`story:match-guards`).
