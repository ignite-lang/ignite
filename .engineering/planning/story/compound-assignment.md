---
format: aep.planning-md/3
id: story:compound-assignment
kind: story
status: draft
title: += and -=
revision: 1
---
# Story: `+=` and `-=`

## Outcome

`p += e` and `p -= e` are statements: `p = p + e` and `p = p - e` with the place `p` evaluated once, for the types `+` and `-` allow, on the evaluator and both targets.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: `x = x + y` is the most frequent workaround in its source. The statement grammar of `docs/slice.md` has only `place '=' expr`. `docs/syntax.md` P6 proposes the syntax and the typing.

## Acceptance

- `+=` for every type `+` accepts and `-=` for every type `-` accepts; any other type is the diagnostic `+` or `-` gives today.
- A `let` binding or a constant on the left is E006.
- The place is evaluated once: `xs[next()] += 1` calls `next` once (a test counts the calls).
- Overflow of `Int` fails as `p = p + e` does, with the same failure site.
- On a `var` of `Text`, `Bytes` or `List` the Rust target appends in place (`push_str`, `extend_from_slice`); a count test shows no copy of the left side per round at two sizes. The LLVM target's count for the same loop is written down.
- `ignite fmt` keeps the form; `docs/slice.md` and `site/book` state it; `docs/syntax.md` marks P6 as done; `task gate` green.

## Scope

- `crates/ignite-syntax/src/lexer.rs`, `parser.rs`, `crates/ignite-types/src/typer.rs`, `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, the formatter, `docs/slice.md`, `docs/syntax.md`. Inferred.

## Not in this story

- `*=`, `/=`, `%=` and the bitwise forms; `++` and `--`.
