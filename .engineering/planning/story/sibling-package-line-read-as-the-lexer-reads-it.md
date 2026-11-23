---
format: aep.planning-md/3
id: story:sibling-package-line-read-as-the-lexer-reads-it
kind: story
status: draft
title: A sibling module whose package line ends with a comment is found
revision: 1
---
# Story: a sibling module whose package line ends with a comment is found

## Outcome

A sibling file whose first declaration is `package demo.ops // public helpers`, or carries a block comment or a tab in that line, is loaded as a sibling module by `check`, `build`, `run` and `test`, exactly as the same file without the comment.

## Why

IG-11, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

Synthetic reproducer: a sibling declaring `package demo.ops // public helpers` passed `ignite check` on its own; its importer failed with E031. Removing only the comment made the importer pass, and its native test ran.

`declares_package` (`crates/ignite/src/lib.rs:1066` to `:1072`) takes the first line that is not empty and does not start with `//`, strips `package ` and compares the rest, trimmed, to the expected path, so a trailing comment fails the comparison. The lexer drops comments and whitespace (`crates/ignite-syntax/src/lexer.rs:256`, `:257`). `docs/slice.md:2648` defines a sibling by its first declaration.

## Acceptance

- A test, red first: the importer with the commented sibling passes `ignite check` and its test runs on both targets; the same with a block comment before `package` and with a tab after it.
- Controls: a file whose first declaration names another package is still not loaded, and an unrelated file beside the program is still not parsed in full.
- `task gate` green.

## Scope

- `crates/ignite/src/lib.rs:1066` (`declares_package`), `:1164` and `:1165` (its use in `parse_siblings`), `crates/ignite-syntax/src/lexer.rs:256`, `docs/slice.md:2648`. Each read at a1d1407e, as the review states.
- `story:multi-file-tests-read-each-file-once` changes how the same headers are read.
