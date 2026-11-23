---
format: aep.planning-md/3
id: story:rust-target-sorts-tuples-wider-than-twelve
kind: story
status: draft
title: The Rust target sorts by tuples wider than twelve elements
relations:
- informed_by: story:list-sort
revision: 1
---
# Story: the Rust target sorts by tuples wider than twelve elements

## Outcome

`sort` and `sort_by` with tuple elements or keys of thirteen or more elements, and nested tuples that contain them, compile and sort on the Rust target in the order the LLVM target gives.

## Why

IG-26, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

A list of 13-integer tuples passed `check` and sorted on the LLVM target; Rust compilation failed with E0277. A 12-element sort and equality of the 13-element tuples both worked.

`ordered` accepts a tuple of ordered types of any width (`crates/ignite-types/src/lib.rs:323` to `:331`). The Rust generator emits `.sort()` for `sort` (`crates/ignite-rustgen/src/lib.rs:5799` to `:5802`) and `.cmp` on the keys for `sort_by` (`:5840`), both relying on the Rust tuple `Ord`, which the standard library implements up to twelve elements. `docs/slice.md:12068` to `:12071`: a tuple of ordered types is ordered element by element.

## Acceptance

- A native test on both targets, red first on Rust: `sort` of 13-tuples, `sort_by` with a 13-tuple key, and a nested tuple holding one.
- Controls: the 12-element sort keeps its generated form; equality of 13-tuples unchanged; `task gate` green; `CODEGEN_REVISION` bumped if generated code changes.

## Scope

- `crates/ignite-types/src/lib.rs:323`, `crates/ignite-rustgen/src/lib.rs:5802`, `:5840`, `docs/slice.md:12068` (cited). Each read at a1d1407e, as the review states.
