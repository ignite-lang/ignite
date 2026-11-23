---
format: aep.planning-md/3
id: story:wildcard-import-offers-what-explicit-import-offers
kind: story
status: draft
title: A wildcard import offers what an explicit import of the same module offers
relations:
- informed_by: story:glob-imports-resolve-by-receiver
revision: 1
---
# Story: a wildcard import offers what an explicit import of the same module offers

## Outcome

`import std.net.*` makes `Connection` available as `import std.net.{Connection}` does, and `import demo.lib.*` of a sibling module makes `sum` available as `import demo.lib.{sum}` does; conflicts between globs, own declarations and test-only exports keep their rules.

## Why

IG-21, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

`std.net.*` failed to import `Connection` while `std.net.{Connection}` worked; a sibling wildcard `demo.lib.*` failed with E031 while `demo.lib.{sum}` worked.

`glob_imports` refuses every module outside `std` with E031 (`crates/ignite-check/src/lib.rs:952` to `:961`) and skips the substrate modules (`std.net`, `std.io`, `std.fs`, `std.process`, `:274`) without importing anything (`:962` to `:964`). The explicit path accepts sibling modules (`:1042`) and asks a substrate module for its exports (`:1052` to `:1074`). `docs/slice.md:2072` (row 6): a glob imports every non-test public item of the module.

`story:glob-imports-resolve-by-receiver` changes the conflict rule of the same function; it does not cover these two cases.

## Acceptance

- Tests, red first: `import std.net.*` then a use of `Connection`; a sibling module imported by `demo.lib.*` then a call of `sum`; both check and run on both targets.
- Controls: a name two globs provide stays E041 at its use; an own declaration wins over a glob; a test-only export is not imported by a glob outside tests; `task gate` green.

## Scope

- `crates/ignite-check/src/lib.rs:944` (`glob_imports`, cited `:952`), `:1042`, `:1053` (cited), `docs/slice.md:2072`. Each read at a1d1407e, as the review states.
