---
format: aep.planning-md/3
id: story:data-toml
kind: story
status: draft
title: 'std.toml: TOML 1.0.0'
relations:
- decomposes: epic:data-formats
- depends_on: story:data-value-model
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: crates/ignite-check/tests/layering.rs
- confidence: inferred
  path: crates/ignite/tests/main.rs
- confidence: inferred
  path: docs/layering.md
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: docs/toml
- confidence: inferred
  path: std/toml.ignite
revision: 3
---
# Story: data-toml

## Outcome

`std.toml` reads (`read`, `read_with`) and writes (`write`) TOML 1.0.0 complete (`docs/serde.md` section 6.3): datetimes as `Value.Moment`, floats as `Value.Decimal`; `write` refuses a root that is not a `Map` and any `Null`.

## Acceptance

- `toml-lang/toml-test` vendored with its licence; a test runs every valid case (expected JSON matches) and every invalid case (a `FormatError`), on both targets.
- `write` of `Value.List([])` as root gives a `FormatError`; `write` of a `Map` with a `Null` member, and of a `List` holding `Null`, each give a `FormatError` naming the path.
- `read(write(v)) == v` for 200 `Map` roots without `Null` from a seeded generator, on both targets.
- 10 valid cases read to the same `Value` in the evaluator and on both targets.
- `DataLimits` bound depth and size, a test each.
- `docs/slice.md` section; `task gate` green.

## Scope

- Own: `std/toml.ignite` (new), `docs/toml/` (vendored), a test file under `crates/ignite/tests/`.
- Shared with `story:data-yaml` and `story:data-codec-combinators` in one wave: `docs/layering.md` and `crates/ignite-check/tests/layering.rs` (the `std.toml` row, layering rule 4), `docs/slice.md`, `AGENTS.md` layout table, `crates/ignite/tests/main.rs` (one `mod` line).
