---
format: aep.planning-md/3
id: story:data-derived-codec
kind: story
status: draft
title: Compiler-derived encode and decode for every data type
relations:
- decomposes: epic:data-formats
- depends_on: story:data-value-model
- depends_on: story:data-codec-combinators
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: crates/ignite-check/src
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
- confidence: inferred
  path: crates/ignite/src/typed_json.rs
- confidence: inferred
  path: crates/ignite/tests/main.rs
- confidence: inferred
  path: docs/layering.md
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/data.ignite
revision: 4
---
# Story: data-derived-codec

## Outcome

`std.data.codec<T>()` returns, for every data type `T` a program uses, a `Codec<T>` with the mapping of `docs/serde.md` section 5: the frontend writes one `Codec` value per reached type (section 3.2), built from the `story:data-codec-combinators` combinators where they reach and written directly for what they do not (a record of more than eight fields, which `record2` to `record8` do not cover); a program-declared `const` of type `Codec<T>` is used instead for its `T`; both backends and the evaluator run the result unchanged.

## Acceptance

- For each type of the combinators story's fixture (record, unit enum, payload enums, `Option`, `List`, pair, `Map<Text, _>`, recursive enum, `Bytes`, `Duration`) plus `Map<Int, _>`, `Map` keyed by a unit-variant enum and a record of 12 fields, on the evaluator, Rust and LLVM:
  - `encode(codec(), x)` equals the `Value` the hand codec gives (for the 12-field record, the `Value` written out in the fixture);
  - `decode(codec(), v)` equals `decode(hand_codec, v)` for the encoded `v` and for three wrong `Value`s per type (wrong member type, missing member, unknown variant), including the `DecodeError` path.
- A program-declared `const` `Codec<Duration>` is used by `codec<Record>()` for a `Duration` field (fixture).
- Check diagnostics, each with a fixture and an `.expected`: `codec()` for a type containing a `TcpStream` or a function (names the field path); a `Map` key type other than `Text`, `Int` or a unit-variant enum; `codec()` with no fixed `T` (design O1); a constant `Shape` whose `renames` names no field or variant of a type the call reaches (section 5.1).
- `ignite emit --typed-json` lists the written `Codec` values.
- `CODEGEN_REVISION` bumped; `docs/slice.md` section; the layering rule 6 row for the `std.data` name in the crate; `task gate` green.

## Scope

- Own: `crates/ignite-types` (instantiation record, the writing pass), `crates/ignite-check` (diagnostics), `crates/ignite/src/typed_json.rs`, `crates/ignite/src/apply/plan.rs` (`CODEGEN_REVISION`), the builtin declaration in `std/data.ignite`, tests under `crates/ignite/tests/`.
- Shared: `docs/layering.md` ("Std names in the crates"), `docs/slice.md`, `AGENTS.md`, `crates/ignite/tests/main.rs`.
- Overlaps `story:mid-level-ir` on `crates/ignite-types`: not in one wave with it (recorded in the `scope` field, so `aep plan artifact waves` sees it).
