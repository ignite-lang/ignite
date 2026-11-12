---
format: aep.planning-md/3
id: story:declaration-words-as-identifiers
kind: story
status: draft
title: A declaration word is an identifier where no declaration can start
revision: 1
---
# Story: a declaration word is an identifier where no declaration can start

## Outcome

`package`, `requires`, `protocol`, `contract`, `policy`, `resource`, `component`, `system`, `assembly` and `test` are accepted as the name of a local, a parameter, a fn, a field and a variant; each stays a keyword only at the place its declaration or clause starts.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite, which renamed values called `package` and `requires`. The lexer holds 31 hard keywords (`crates/ignite-syntax/src/lexer.rs`); a keyword is a name only in the few places `name_or_keyword` allows (`parser.rs`: paths, record fields, record-literal keys, field access). `docs/friction.md` rows 26 (`protocol`) and 36 (`contract`, `policy`) are open.

## Acceptance

- A design step first: for each of the 31 keywords, whether it becomes contextual, with the grammar position that keeps it a keyword; the expression and statement words (`let`, `var`, `if`, `match`, `for`, `return`, `true`, `false` and the like) stay hard.
- `let package = ..`, `fn requires(..)` and a parameter `protocol: Text` parse, check and run on the evaluator and both targets.
- Every program of `examples/`, `std/` and the golden corpus parses to the same tree as before (a test compares the printed trees).
- The generated Rust escapes a name that is a Rust keyword as today.
- `ignite fmt` output is unchanged for the corpus; `docs/syntax.md` lists the contextual words (the file says 29 reserved words, the lexer has 31); rows 26 and 36 of `docs/friction.md` move to solved; `task gate` green.

## Scope

- `crates/ignite-syntax/src/lexer.rs`, `parser.rs`, `docs/slice.md`, `docs/syntax.md`, `docs/friction.md`. Inferred.

## Not in this story

- A raw-identifier form (`r#name`).
