---
format: aep.planning-md/3
id: story:pattern-test-is
kind: story
status: draft
title: e is P, a pattern test that is a Bool
revision: 1
---
# Story: `e is P`, a pattern test that is a Bool

## Outcome

`e is P` is a `Bool` expression that tests `e` against the pattern `P` and binds the pattern's names in the branch the test guards, so a `match` with one real arm and one empty arm is one `if`.

## Why

`docs/syntax.md` holds it as proposal P1 with its syntax and typing, rank 2 of its priority table (section 6), unscheduled. Requested on 2026-11-12 by the author of a command-line tool written in Ignite: statement matches that exist to run one arm, with the other arm empty, rank second in its report by lines saved, after the one-line early exits (S0 of `docs/syntax.md`). `assert e is P` also replaces a `match` in a test whose other arm fails the test (rows 38, 42 and 58 of `docs/friction.md` name the arm bodies that cannot be written).

## Acceptance

- The syntax and typing of `docs/syntax.md` P1: `compare := additive (cmp additive | 'is' pattern)?`; `is` becomes the 30th reserved word; a program that uses `is` as a name gets one diagnostic that says so.
- Names bound by `P` are in scope in the then-branch of an `if` whose condition is the test or a `&&` chain that holds it, and in the right operands of that `&&`; they are not in scope after `||`, in the else-branch or after a `!`, each with a diagnostic that names the rule.
- `assert e is P` reports a failure with the value of `e` in the format of a failed `assert a == b`.
- It lowers to the `match` a user writes today: no new node reaches the evaluator or the targets, or the node's lowering is the existing one; a differential test holds one program on all three.
- Exhaustiveness: an `is` whose pattern cannot fail is accepted (or refused) by a stated rule.
- The rewrites of `docs/syntax.md` section 9 that use `is` are applied to `std/` and `examples/` where they shorten the code; the lines saved are counted with the document's own method and written into its section 6.
- `docs/slice.md` grammar and section, `site/book`; `task gate` green.

## Scope

- The lexer, parser and checker crates, `crates/ignite-types/`, `std/`, `examples/`, `docs/slice.md`, `docs/syntax.md`, `docs/friction.md`. Inferred.

## Not in this story

- `let … else` (`story:let-else`), match guards (`story:match-guards`).
