---
format: aep.planning-md/3
id: story:data-derived-codec
kind: story
status: draft
title: Compiler-derived encode and decode for every data type
relations:
- decomposes: epic:data-formats
- depends_on: story:data-value-model
revision: 1
---
# Story: data-derived-codec

## Outcome

`encode`, `decode`, `encode_with` and `decode_with` of `std.data` work for every data type a program uses: the frontend writes one typed encode and one typed decode function per reached type (`docs/serde.md` section 3.2, option A), both backends and the evaluator run them unchanged, and the mapping is the table of section 5.

## Acceptance

- A fixture with a record, a unit enum, a payload enum (positional and record payloads), `Option` fields, `List`, tuple, `Map<Text, _>`, `Map<Int, _>`, a recursive enum and `Bytes` round-trips on the evaluator, Rust and LLVM, and the three `Value`s are equal (differential test).
- `Shape { keys = Camel }`, `tagging = Internal("type")`, `unknown = Deny` and one `renames` entry each have a fixture with the expected `Value`.
- `decode` of a wrong member type gives `DecodeError` with the path (`$.items[2].title`).
- `encode` of a type containing a `TcpStream` or a function is a check diagnostic naming the field path, with a fixture and an `.expected`; a constant `Shape` whose `renames` names no field is a diagnostic too.
- `ignite emit --typed-json` lists the written functions.
- `CODEGEN_REVISION` bumped; `docs/slice.md` section; `task gate` green.

## Scope

- `crates/ignite-types` (instantiation record and the writing pass), `crates/ignite-check` (diagnostics), `crates/ignite/src/typed_json.rs`, `crates/ignite/src/apply/plan.rs` (`CODEGEN_REVISION`), `std/data.ignite` (builtin declarations), tests in `crates/ignite/tests/`. Inferred.
- Overlaps `story:mid-level-ir` on `crates/ignite-types` if they share a wave.
