---
format: aep.planning-md/3
id: story:rustgen-type-parameter-shadowing
kind: story
status: draft
title: Generated Rust names a type parameter where a record of the same name is meant
summary: A type parameter named like a record in scope breaks the generated Rust (turbofish and record literals)
relations:
- derived_from: epic:slice-14-protocols-p1
revision: 1
---

# Story: rustgen-type-parameter-shadowing

## Outcome

A program in which a type parameter has the same name as a record or enum in scope builds on the
Rust target, or is rejected by the checker with a diagnostic, instead of failing in `rustc`.

## Context

Found by the adversary pass on slice 14 U3 (`epic:slice-14-protocols-p1`, released as v0.3.0) and
left open on purpose:

- Inside `fn outer<S: ByteStream>`, a call `tagged(stream, make())` whose second type argument is
  the user record `S` is generated as `tagged::<S, S>(..)`, and `rustc` reads both as the type
  parameter. The test `golden::bounded_turbofish_keeps_a_record_named_like_a_bound` in
  `crates/ignite-rustgen/tests/golden.rs` is `#[ignore]` with this story's id.
- The same name clash in a record literal (`S { .. }` inside `fn outer<S>`) already failed at
  bd05809, before slice 14: the generator spells record and enum types by their bare name
  (`enum_name`, `rust_ty`) and does not know the enclosing fn's type parameters.

## Acceptance

The ignored test passes without `#[ignore]`, and a record literal of a shadowed record builds too;
or the checker reports the shadowing at the type parameter's declaration with a fixture.
