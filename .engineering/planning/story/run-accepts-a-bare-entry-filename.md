---
format: aep.planning-md/3
id: story:run-accepts-a-bare-entry-filename
kind: story
status: draft
title: '`ignite run` accepts an entry file named without a directory'
revision: 1
---
# Story: `ignite run` accepts an entry file named without a directory

## Outcome

`ignite run alpha.ignite` in the directory of the file behaves as `ignite run ./alpha.ignite` and as the absolute path.

## Why

IG-14, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

The released CLI rejected a valid `alpha.ignite` with exit 3 and an empty path in the filesystem error; `./alpha.ignite` succeeded.

`build_stamp` passes `file.parent()` to `ignite_files` (`crates/ignite/src/lib.rs:1619` to `:1621`). For a bare name the parent is the empty path, and `read_dir` on it fails (`:379`, `:380`). `parse_siblings` already maps an empty parent to `.` (`:1143` to `:1146`).

## Acceptance

- A test, red first: `ignite run` with a bare, a dot-relative and an absolute entry path succeeds for each.
- Control: the three spellings of one entry give the same stamp; `task gate` green.

## Scope

- `crates/ignite/src/lib.rs:1619` (`build_stamp`), `:379` (`ignite_files`), `:1143` (`parse_siblings`). Each read at a1d1407e, as the review states.
- `story:run-cache-keyed-by-entry-file` and `story:run-stamp-covers-the-imported-modules-only` change the same function.
