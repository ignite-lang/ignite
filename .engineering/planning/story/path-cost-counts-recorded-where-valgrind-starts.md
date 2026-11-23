---
format: aep.planning-md/3
id: story:path-cost-counts-recorded-where-valgrind-starts
kind: story
status: draft
title: Path-cost counts recorded again where valgrind starts
revision: 2
---
# Story: path-cost counts recorded again where valgrind starts

## Outcome

`bench/path-cost/counts/` holds the counts of the latest release, recorded by `task path-cost RECORD=vX.Y.Z` on the development machine where `task gate` runs, and `task gate` there judges every merge against them without the container route.

## Why

v0.16.5 (wave 16, 2026-11-23) was cut without recording its counts. valgrind could not start on the development machine: its glibc 2.44+r50 loader is stripped, its debug file was no longer served (the distribution's debuginfod answers 404 for that build id, its package archive 503), and valgrind cannot set up the `memcmp` redirection in `ld-linux-x86-64.so.2` without it. Row 3.10 and the valgrind tests of `crates/ignite/tests` ran in a container instead (`AGENTS.md` "Commands", the paragraph on valgrind), comparing v0.16.4 and the release head under one glibc. The counts in the repository stay those of v0.16.4, recorded under the machine's old loader; the container's counts differ from them by its loader alone (the loader is 284,744 of the 321,791 instructions of `llvm/todo/start` at v0.16.4).

## Acceptance

- On the development machine after its glibc update, `valgrind -q true` exits 0, and `task path-cost` against the v0.16.4 counts prints each cell with both values; cells above their band only by the loader's change are named in the commit.
- `task path-cost RECORD=<the latest release>` writes `bench/path-cost/counts/`, the bars are blessed as `AGENTS.md` "Releases" says, and `task gate` on that commit is green with its path-cost step.
- The container paragraph in `AGENTS.md` says it is for a machine where valgrind cannot start, or is removed.

## Scope

- `bench/path-cost/counts/`, `quality/baseline.json` (row 3.10 bars), `docs/quality.md` "At vX.Y.Z", `AGENTS.md` "Commands". Inferred.
