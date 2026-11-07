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
revision: 2
---
# Story: data-derived-codec

## Outcome

`std.data.codec<T>()` returns, for every data type `T` a program uses, the `Codec<T>` that `story:data-codec-combinators`' combinators would build by hand: the frontend writes one `Codec` value per reached type (`docs/serde.md` section 3.2), a program-declared `const` of type `Codec<T>` is used instead for its `T`, and both backends and the evaluator run the result unchanged.

## Acceptance

- For each type of the combinators story's fixture (record, unit enum, payload enums, `Option`, `List`, pair, `Map<Text, _>`, recursive enum, `Bytes`, `Duration`) plus `Map<Int, _>` and `Map` keyed by a unit-variant enum, `encode(codec(), x)` equals `encode(hand_codec, x)` on the evaluator, Rust and LLVM.
- A program-declared `const` `Codec<Duration>` is used by `codec<Record>()` for a `Duration` field (fixture).
- Check diagnostics, each with a fixture and an `.expected`: `codec()` for a type containing a `TcpStream` or a function (names the field path); a `Map` key type other than `Text`, `Int` or a unit-variant enum; `codec()` with no fixed `T` (design O1).
- `ignite emit --typed-json` lists the written `Codec` values.
- `CODEGEN_REVISION` bumped; `docs/slice.md` section; the layering rule 6 row for the `std.data` name in the crate; `task gate` green.

## Scope

- Own: `crates/ignite-types` (instantiation record, the writing pass), `crates/ignite-check` (diagnostics), `crates/ignite/src/typed_json.rs`, `crates/ignite/src/apply/plan.rs` (`CODEGEN_REVISION`), the builtin declaration in `std/data.ignite`, tests under `crates/ignite/tests/`.
- Shared: `docs/layering.md` ("Std names in the crates"), `docs/slice.md`, `AGENTS.md`, `crates/ignite/tests/main.rs`.
- Overlaps `story:mid-level-ir` on `crates/ignite-types`: not in one wave with it.
