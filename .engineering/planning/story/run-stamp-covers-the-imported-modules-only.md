---
format: aep.planning-md/3
id: story:run-stamp-covers-the-imported-modules-only
kind: story
status: draft
title: The run stamp covers the modules the program loads, not the whole tree
revision: 1
---
# Story: the run stamp covers the modules the program loads, not the whole tree

## Outcome

The build stamp of `ignite run` is computed from the resolved module graph (the entry, the sibling modules it loads, the standard-library modules it reaches) and the actual toolchain inputs: a file added in an unimported subdirectory keeps the stamp, an edit to an imported module changes it.

## Why

IG-36, severity medium, class performance; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

Adding a file in an unimported subdirectory changed the stamp for the same entry and build options, so a warm run rebuilt.

`build_stamp` walks the entry's directory recursively (`crates/ignite/src/lib.rs:1619` to `:1621`, `ignite_files` at `:378` to `:392`, the recursion at `:385`, `:386`) and the whole standard library (`:1622`, `:1623`), and reads every file (`:1626`) before the previous stamp is compared (`:1696`). A cached run still pays for every unrelated file and directory entry.

## Acceptance

- Tests, red first: an unrelated file added in a subdirectory keeps the stamp and the run does not rebuild.
- Controls: an edit to an imported sibling, to a reached standard-library module and to the driver each change the stamp; `task gate` green.

## Scope

- `crates/ignite/src/lib.rs:378`, `:1619`, `:1696` (cited). Each read at a1d1407e, as the review states.
- `story:run-cache-keyed-by-entry-file` and `story:run-accepts-a-bare-entry-filename` change the same function; whichever lands later keeps the tests of the others.
