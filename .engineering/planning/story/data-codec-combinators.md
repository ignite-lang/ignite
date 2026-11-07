---
format: aep.planning-md/3
id: story:data-codec-combinators
kind: story
status: draft
title: std.data Codec<T> and combinators, std only
relations:
- decomposes: epic:data-formats
- depends_on: story:data-value-model
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: crates/ignite/tests/main.rs
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/data.ignite
revision: 2
---
# Story: data-codec-combinators

## Outcome

`std.data` has `Codec<T>`, `encode`, `decode`, `encode_with`, `decode_with` and the combinators of `docs/serde.md` section 3.3 (`int`, `text`, `bool`, `bytes`, `duration`, `list`, `option`, `text_map`, `pair`, `record2` to `record8`, `field`, `variants`, `variant`), written in Ignite with no compiler change, following the mapping of section 5 and `Shape` of section 5.1.

## Acceptance

- A hand-built codec for a record, a unit enum, a payload enum (positional and record payloads), `Option` fields, `List`, a pair, `Map<Text, _>`, a recursive enum, `Bytes` and `Duration` gives the `Value` of section 5 and decodes it back, in inline tests on both targets.
- `Bytes` decodes from base64 `Text`; `Text` decodes from `Decimal` and `Moment`.
- `Shape { keys = Camel }`, `variants = Kebab`, `tagging = Internal("type")`, `unknown = Deny` and one `renames` entry each have an inline test with the expected `Value`; a `renames` entry naming no field gives a `DecodeError`.
- `decode` of a wrong member type gives a `DecodeError` with the path (`$.items[2].title`).
- The same codec run in the evaluator and on both targets gives equal `Value`s (a test in `crates/ignite/tests/`).
- `docs/slice.md` section; `task gate` green.

## Scope

- Own: `std/data.ignite` (after `story:data-value-model`).
- Shared with `story:data-yaml` and `story:data-toml` in one wave: `docs/slice.md`, `AGENTS.md` layout table, `crates/ignite/tests/main.rs`.
