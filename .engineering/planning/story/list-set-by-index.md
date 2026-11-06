---
format: aep.planning-md/3
id: story:list-set-by-index
kind: story
status: draft
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
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/text.ignite
revision: 9
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

Met by PR 60 (checked against main 67dfc7b98 by the devtools session, 2026-11-06): `xs.set(i, v)` and `xs[i] = v` on a `List` (`crates/ignite-types` builtin `ListSet`), the evaluator (`crates/ignite-eval/src/eval.rs:1402`), both backends, `docs/slice.md` 25.4. Remaining:

- `Bytes` has `set` with the same bounds failure as `at`, in the typer, the evaluator and both backends.
- `to_lower` and `to_upper` (`std/text.ignite:603`, `:612`) use it instead of pushing into a fresh `Bytes`.
- An out-of-range `set` on a `List` and on `Bytes` fails with the same message, file, line and column on both targets (inline test).
- `task gate` green.

## Out of Scope

- Slices that share storage.
