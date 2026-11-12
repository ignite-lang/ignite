---
format: aep.planning-md/3
id: story:module-qualified-calls
kind: story
status: draft
title: A fn is called through its module's name
relations:
- informed_by: story:glob-imports-resolve-by-receiver
revision: 1
---
# Story: a fn is called through its module's name

## Outcome

`import std.text` brings the module into scope, and `text.find(s, x)` calls its fn without importing the fn's name; the same for a program's own modules.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: a path in an expression (`std.text.find(x)`) is E003, and "importing a whole module is not in the slice" (`docs/slice.md` section 4), so every module lists each name it uses or takes a glob. Two globs that provide one name are E041 at the use (`docs/friction.md` row 21, open).

## Acceptance

- A design step first, written into `docs/slice.md` section 4: the import form (`import a.b` and whether `import a.b as c` exists), what `b.name` means when a local `b` exists (the local wins or it is a diagnostic), and how `b.f(x)` differs from a method call on a value `b`.
- `import std.text` then `text.find(s, x)`, `text.Pattern` as a type and `text.LIMIT` as a constant check and run on the evaluator and both targets.
- A module name that is not imported stays E003, with a hint that names the import.
- `import std.option` and `import std.result` together, with `option.unwrap_or(o, d)` and `result.unwrap_or(r, d)` in one module, check.
- `ignite fmt` keeps the form; `site/book` shows it; `task gate` green.

## Scope

- `crates/ignite-syntax/src/parser.rs` (imports), `crates/ignite-check/src/lib.rs` (name resolution, E003, E041), `crates/ignite-types/src/typer.rs` (path heads in expressions), `docs/slice.md` section 4. Inferred.

## Not in this story

- A third-party package mechanism.
- Choosing between two glob-imported fns by the receiver's type (`story:glob-imports-resolve-by-receiver`).
