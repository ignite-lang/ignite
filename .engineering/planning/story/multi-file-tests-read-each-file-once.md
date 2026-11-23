---
format: aep.planning-md/3
id: story:multi-file-tests-read-each-file-once
kind: story
status: draft
title: A multi-file test run reads each source file once
revision: 1
---
# Story: a multi-file test run reads each source file once

## Outcome

`ignite test` over N package files in one directory reads each file once per invocation, with or without sibling imports.

## Why

IG-32, severity medium, class performance; an operation count derived from source (the maintainer review of 2026-11-23 against main at a1d1407e). No wall-clock time was measured.

`prepare_tests` parses every root (`crates/ignite/src/test_build.rs:139` to `:147`). Each parse runs `parse_siblings` (`crates/ignite/src/lib.rs:1119`), which lists the directory (`:1148`) and reads the full text of every other `.ignite` file (`:1164`) before checking its package header (`:1165`). N independent files cost N squared reads before compilation.

## Acceptance

- A test with a counting reader, red first: N independent package files are read N times in total.
- Controls: sibling imports still resolve; a file named twice still runs once; `task gate` green.

## Scope

- `crates/ignite/src/test_build.rs:139`, `crates/ignite/src/lib.rs:1148`, `:1164` (cited). Each read at a1d1407e, as the review states.
- `story:sibling-package-line-read-as-the-lexer-reads-it` changes how the same header is read.
