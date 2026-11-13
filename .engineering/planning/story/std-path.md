---
format: aep.planning-md/3
id: story:std-path
kind: story
status: draft
title: std has path functions
relations:
- informed_by: story:file-named-at-run-time
revision: 1
---
# Story: std has path functions

## Outcome

A program takes a path apart and puts one together with functions of `std`: the directory, the last component, the extension, a normal form and a join.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: it wrote its own `dirname`, `basename`, a normalizer and a join. `std/fs.ignite` has `join_path` beside `read_text_file` and `write_text_file`; the program did not find it, and `docs/slice.md` does not point to it from the text on `Directory`.

## Acceptance

- `parent(path) -> Text`, `file_name(path) -> Text`, `extension(path) -> Option<Text>`, `join(a, b) -> Text`, `normalize(path) -> Text`, on `/`-separated paths as text, without touching the file system.
- `normalize` removes `.` components and empty components, resolves `..` against the component before it, keeps a leading `/` and never leaves the root; the cases (`a/../..`, `/..`, a trailing `/`, the empty path) are tests.
- A property test on generated component lists: `normalize` is idempotent, and `file_name(join(a, b))` is `file_name(b)` for a `b` without `/`.
- `join_path` of `std.fs` stays and is the same function as `join`, or is replaced with its callers changed; the module decision (a new `std/path.ignite` or `std.fs`) is part of this story and goes into `docs/layering.md`.
- The `Directory` text of `docs/slice.md` names the functions; `task gate` green.

## Scope

- `std/fs.ignite` or a new `std/path.ignite`, `docs/layering.md`, `docs/slice.md`. Inferred.

## Not in this story

- Windows separators and drive letters.
- Opening a path named at run time (`story:file-named-at-run-time`).
