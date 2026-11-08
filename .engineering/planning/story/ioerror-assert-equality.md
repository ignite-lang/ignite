---
format: aep.planning-md/3
id: story:ioerror-assert-equality
kind: story
status: implemented
title: IoError equality inside assert
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T01:26:00Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T01:26:48Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-08T06:51:09Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: ioerror-assert-equality

## Outcome

Inside an `assert`, `==` compares `IoError` values by their message, so a test can compare a `Result<T, IoError>` with the expected error; outside an `assert` the rule of `docs/slice.md` 12.3 is unchanged.

## Context

- Case study gap G18 (`docs/research/case-study-gaps.md`, friction row 74): `assert port("x") == Err(IoError.new("bad port"))` is E045; the service returns its errors as `Text` to test them.

## Acceptance

- The G18 assertion checks and passes on both targets and in the evaluator; a different message fails with both values in the report.
- `IoError` inside `List`, `Option`, tuples and records compares the same way inside an `assert`.
- `==` on `IoError` outside an `assert` stays E045 (fixture).
- `docs/slice.md` 12.3 updated; `task gate` green.

## Scope

- `crates/ignite-types` (E045 rule), both backends' assert equality, `crates/ignite-eval`, `docs/slice.md`. Inferred.
