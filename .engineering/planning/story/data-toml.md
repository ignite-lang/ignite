---
format: aep.planning-md/3
id: story:data-toml
kind: story
status: draft
title: 'std.toml: TOML 1.0.0'
relations:
- decomposes: epic:data-formats
- depends_on: story:data-value-model
revision: 1
---
# Story: data-toml

## Outcome

`std.toml` parses and emits TOML 1.0.0 complete (`docs/serde.md` section 6.3): datetimes as `Value.Moment`, floats as `Value.Decimal`, a non-`Map` root and any `Null` refused by `emit`.

## Acceptance

- `toml-lang/toml-test` vendored with its licence; a test runs every valid case (expected JSON matches) and every invalid case (a `FormatError`), on both targets.
- `parse(emit(v)) == v` for generated `Map` roots without `Null`.
- `DataLimits` bound depth and size, with a test each.
- `docs/slice.md` section; `task gate` green.

## Scope

- `std/toml.ignite` (new), `docs/toml/` (vendored), a test under `crates/ignite/tests/`, `docs/slice.md`, `AGENTS.md` layout row. Inferred.
