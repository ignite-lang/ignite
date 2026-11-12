---
format: aep.planning-md/3
id: story:tests-read-fixture-files
kind: story
status: draft
title: An inline test reads files of its package
relations:
- informed_by: story:expect-in-tests
revision: 1
---
# Story: an inline test reads files of its package

## Outcome

An inline test reads a file that lies beside its source, such as a recorded answer or an expected output, through a read-only `Directory` that `ignite test` gives it.

## Why

Requested on 2026-11-13 by the author of a command-line tool written in Ignite. A test body is typed as `fn() -> Unit` without parameters (`docs/slice.md` 12.3), and reading a file needs a `Directory` value with the `read` capability (`std/fs.ignite`), which only `main` receives. So a test cannot read a fixture, and larger inputs are written into the source as text literals.

## Acceptance

- The design is written first and states how a test names the directory (a parameter of the test, such as `test "…" (files: Directory) { .. }`, or a builtin), that it is rooted at the directory of the test's source file, and that it grants `read` only: a write or a path that leaves the root is refused, with the diagnostic or the `IoError` named.
- A test that reads a file beside its source passes under `ignite test` on the evaluator, the Rust target and the LLVM target, whatever the working directory of the `ignite test` call is.
- A test without the parameter is typed and run as before; the 12.3 rule for other effects in a test is unchanged or the change is stated.
- A missing fixture fails the test with the path in the message; it does not end the run of the other tests.
- `docs/slice.md` section 12 and `site/book` state the form; `task gate` green.

## Scope

- `crates/ignite-syntax` (the test declaration), `crates/ignite-types/src/typer.rs`, `crates/ignite-check/src/lib.rs`, `crates/ignite-io/src/test.rs`, the test drivers of both targets, `docs/slice.md`. Inferred.

## Not in this story

- Network, clock or process capabilities in a test; writing files from a test; `expect` (`story:expect-in-tests`).
