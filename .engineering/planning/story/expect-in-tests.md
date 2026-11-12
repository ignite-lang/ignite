---
format: aep.planning-md/3
id: story:expect-in-tests
kind: story
status: draft
title: A test fails loudly on a missing value
relations:
- informed_by: story:option-try-in-fns
revision: 1
---
# Story: a test fails loudly on a missing value

## Outcome

Test code takes the value out of an `Option` or a `Result` with `expect(message)`: on `None` or `Err` the test fails with the message and the position, where today a fixture builds a fallback value that hides the failure.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: `?` works in a test body and not in a `test fn` that returns a value, which is where fixtures are built, so its fixtures use `unwrap_or(<a made-up record>)` and error-to-default fallbacks. `std/option.ignite` has `unwrap_or`, `map`, `and_then`, `is_some`, `is_none` and `ok_or`; `std/result.ignite` has `unwrap_or`, `map`, `map_err`, `is_ok`, `is_err` and `ok`. `docs/design.md` says a missing value and a failure cannot be silently discarded; a fallback record in a fixture is one.

## Acceptance

- `o.expect(message)` on an `Option<T>` and `r.expect(message)` on a `Result<T, E>` give the `T`; on `None` or `Err` the running test fails with the message, the source position of the call and, for `Err`, the error's debug text.
- It is accepted in a `test` body, a `test fn` and a `test const`; outside test code it is one diagnostic that names `?` and `match`.
- The failure is reported by `ignite test` in the format of a failed `assert` (`docs/slice.md` section 12), on the evaluator and both targets, and the other tests of the run continue.
- The decision on `?` inside a `test fn` that returns a plain value (allow it with the same failure, or leave it refused) is written into `docs/slice.md` with its reason.
- `docs/slice.md` section 12 and `site/book` state the form; `task gate` green.

## Scope

- `std/option.ignite`, `std/result.ignite` or the builtin table, `crates/ignite-check/src/lib.rs` (test-only rule), `crates/ignite-io/src/test.rs` and `report.rs`, both targets, `docs/slice.md`. Inferred.

## Not in this story

- A crash outside tests (`unwrap`, `panic`): the design has no unrecoverable failure in program code, and this story does not add one.
