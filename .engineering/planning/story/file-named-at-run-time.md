---
format: aep.planning-md/3
id: story:file-named-at-run-time
kind: story
status: draft
title: A program opens a file whose path arrives at run time
revision: 1
---
# Story: a program opens a file whose path arrives at run time

## Outcome

A command-line program reads and writes a file named by an argument, absolute or relative, through a declared resource, without starting a child process and without a `Directory` root fixed at launch that happens to contain it.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: a `Directory` root is fixed at launch from `path` or a `flag` (`std/fs.ignite`, `docs/slice.md` on `Directory`), a path that starts with `/` is refused as `absolute`, and `Directory.temporary()` is test-only (E051). A relative path goes through a working-directory resource; an absolute one has no route but `cat` through `std.process`.

## Acceptance

- A design step first, in `docs/slice.md` beside `Directory`: the resource a program declares for files named by its arguments (for example a `File` resource bound to a positional argument or a flag, or a `Directory` rooted at an argument's parent), what `ignite plan` shows for it, and what stays refused (a path computed from data that is no argument).
- A program declared that way reads `/abs/path/file` and `../rel/file` given as arguments; the same path given inside a file's content is refused, with a diagnostic or an error that names the resource.
- Symbolic links: the rule of `story:directory-reads-through-links-beneath-its-root` applies or the design says why not.
- One test per target runs the program on a file outside the working directory; `site/book` shows the form; `task gate` green.

## Scope

- `std/fs.ignite`, `crates/ignite-io/src/fs.rs`, the resource declarations in `crates/ignite-types/` and `crates/ignite/src/`, `docs/slice.md`. Inferred.

## Not in this story

- Ambient access to the whole file system.
