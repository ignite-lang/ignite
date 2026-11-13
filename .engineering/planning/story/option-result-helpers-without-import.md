---
format: aep.planning-md/3
id: story:option-result-helpers-without-import
kind: story
status: draft
title: The helpers of std.option and std.result are in scope without an import
relations:
- informed_by: story:option-try-in-fns
revision: 1
---
# Story: the helpers of std.option and std.result are in scope without an import

## Outcome

A module calls `unwrap_or`, `map`, `and_then`, `is_some`, `is_none`, `ok_or`, `map_err`, `is_ok`, `is_err` and `ok` on an `Option` or a `Result` without an `import` line for `std.option` or `std.result`.

## Why

Requested on 2026-11-13 by the author of a command-line tool written in Ignite: `Option` and `Result` are builtin types that every module uses, and their helpers live in two std packages that each module imports again. Both packages declare `unwrap_or` and `map`, so a module that needs both for both types imports them under two names.

## Acceptance

- The design is written first and states one of: the helpers become builtin methods of `Option` and `Result` (as `len` is of `List`), or a prelude of named std items is in scope in every module. It states which names it brings into scope, and what a module's own fn of the same name means (the module's fn wins, or one diagnostic).
- `opt.unwrap_or(x)`, `opt.map(f)`, `res.map_err(f)` and the other names of the outcome type-check in a module with no import; the receiver's type selects between the two `unwrap_or` and the two `map`.
- Every program of the corpus and of `examples/` that compiles before the change compiles after it and prints the same; a program that declares its own `map` or `unwrap_or` is one of the tests.
- The generated Rust and the LLVM output of a program that already imports the helpers are unchanged, or the difference is stated.
- `docs/slice.md` and `site/book` state the rule; `task gate` green.

## Scope

- `crates/ignite-types/src/typer.rs` (name resolution, UFCS), `crates/ignite-types/src/builtins.rs`, `std/option.ignite`, `std/result.ignite`, `docs/slice.md`. Inferred.

## Not in this story

- `?` on an `Option` inside a fn (`story:option-try-in-fns`); a prelude for any other std package.
