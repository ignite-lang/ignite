---
format: aep.planning-md/3
id: story:typed-json-lowercase-constant-pattern-is-a-constant
kind: story
status: draft
title: Typed JSON writes a lowercase constant pattern as a constant
relations:
- informed_by: story:or-pattern-lowercase-constant
- informed_by: story:typed-program-json
revision: 1
---
# Story: typed JSON writes a lowercase constant pattern as a constant

## Outcome

In `ignite emit --typed-json`, a pattern that names a constant is written with the resolved constant identity whatever its case: a match against `const limit: Int = 3` gives the same pattern object as against `const LIMIT: Int = 3`, never a `binding`.

## Why

IG-23, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

The lowercase constant produced a `binding` with no constant and a null binding type; the capitalized one an explicit constant identity. A consumer can read a value test as a catch-all.

The emitter writes every `PatternKind::Binding` as a binding (`crates/ignite/src/typed_json.rs:935` to `:939`) and consults `pattern_const` only for `PatternKind::Variant` (`:941`). The type checker treats a lowercase name of a constant in an arm as a constant pattern (`crates/ignite-types/src/typer.rs:4698`) and records it (`:4819`). `docs/slice.md:309` to `:311`: a constant pattern, whatever its case.

`story:or-pattern-lowercase-constant` is the checker side of the same spelling (E049 in an or-pattern); it does not touch the emitter.

## Acceptance

- A typed-JSON golden test, red first: the lowercase constant carries its constant identity.
- Controls: an ordinary binding stays a binding with its type; the uppercase constant is unchanged; `task gate` green.

## Scope

- `crates/ignite/src/typed_json.rs:935` (cited), `crates/ignite-types/src/typer.rs:4698`, `:4819` (cited), `docs/slice.md:309`. Each read at a1d1407e, as the review states.
