---
format: aep.planning-md/3
id: story:list-set-by-index
kind: story
status: implemented
title: Set a List or Bytes element by index
summary: xs.set(i, v) with the bounds failure of at, both targets (friction E10)
scope:
- confidence: inferred
  path: crates/ignite-eval/src
- confidence: inferred
  path: crates/ignite-io/src
- confidence: inferred
  path: crates/ignite-llvm/src
- confidence: inferred
  path: crates/ignite-rustgen/src
- confidence: inferred
  path: crates/ignite-types/src
- confidence: cited
  path: crates/ignite/tests/fixtures/tests/codespec_lists.ignite
- confidence: cited
  path: crates/ignite/tests/ignite_test.rs
- confidence: inferred
  path: docs/slice.md
- confidence: cited
  path: std/text.ignite
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T23:50:47Z", actor: "human:timo", revision: 12}
- {from: "proposed", to: "active", at: "2026-11-06T23:50:48Z", actor: "human:timo", revision: 13}
- {from: "active", to: "implemented", at: "2026-11-07T00:59:58Z", actor: "human:timo", revision: 14, decided_on: {"recorded":{"test_result":1}}}
---
# Story: list-set-by-index

## Outcome

A `List<T>` or `Bytes` element can be replaced by index (`xs.set(i, v)` on a `var`, same bounds
failure as `at`), so a value is changed in place instead of rebuilt.

## Context

- Row E10 of the playground session's friction list (`examples/friction.md` in the sibling
  examples repository, 2026-11-04): std `to_lower`/`to_upper` rebuild the text, 3 allocations per
  call where a copy changed in place would cost 1 (round 8, `perf/text-case-and-fresh-to-bytes`).
- Index failures and their text: `ignite_io::checked::at`, `docs/slice.md` section 7.

## Acceptance

## Acceptance

Scoped 2026-11-07 on main df7be28c: `Bytes` is `List<Byte>` in the typer (`crates/ignite-types/src/builtins.rs:15`), evaluator and both backends, so `set` on `Bytes` reaches the PR 60 `ListSet` path; no test exercises it. The List out-of-range case is checked by `crates/ignite/tests/fixtures/tests/codespec_lists.ignite:49` and `crates/ignite/tests/ignite_test.rs:691`.

- `b.set(i, v)` and `b[i] = v` on `Bytes` run on both targets and in the evaluator (inline test; an evaluator case if the eval equivalence fixture covers `set`).
- `to_lower` and `to_upper` (`std/text.ignite:603`, `:612`) change a copy of `text.to_bytes()` in place with `set` instead of pushing into `Bytes.empty()`; their existing tests pass.
- An out-of-range `set` on `Bytes` fails with `index failed at FILE:L:C: index N out of range for length M` on both targets: a failing-on-purpose test in `codespec_lists.ignite` and its line in `ignite_test.rs`'s `codespec_stderr_and_lists_on_both_targets`.
- `docs/slice.md` 25.4 says `set` applies to `Bytes`; `task gate` green.

## Acceptance

Met by PR 60 (checked against main 67dfc7b98 by the devtools session, 2026-11-06): `xs.set(i, v)` and `xs[i] = v` on a `List` (`crates/ignite-types` builtin `ListSet`), the evaluator (`crates/ignite-eval/src/eval.rs:1402`), both backends, `docs/slice.md` 25.4. Remaining:

- `Bytes` has `set` with the same bounds failure as `at`, in the typer, the evaluator and both backends.
- `to_lower` and `to_upper` (`std/text.ignite:603`, `:612`) use it instead of pushing into a fresh `Bytes`.
- An out-of-range `set` on a `List` and on `Bytes` fails with the same message, file, line and column on both targets (inline test).
- `task gate` green.

## Out of Scope

- Slices that share storage.
