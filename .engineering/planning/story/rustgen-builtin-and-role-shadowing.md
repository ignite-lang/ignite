---
format: aep.planning-md/3
id: story:rustgen-builtin-and-role-shadowing
kind: story
status: implemented
title: Generated Rust breaks when a type parameter is named like a protocol or a builtin type
summary: Type parameters named like a protocol, Option or Text fail on the Rust target; three ignored golden tests
relations:
- derived_from: epic:slice-15-protocols-p2
scope:
- confidence: cited
  path: crates/ignite-rustgen/src
- confidence: cited
  path: crates/ignite-rustgen/tests/golden.rs
- confidence: cited
  path: crates/ignite/src/apply/plan.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T22:38:33Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-03T22:38:33Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-04T04:35:32Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---

# Story: rustgen-builtin-and-role-shadowing

## Outcome

A type parameter named like a protocol, a builtin enum (`Option`) or a builtin type (`Text`)
either builds on the Rust target or is rejected by the checker with a diagnostic, instead of
failing in `rustc`. The LLVM target already builds these programs.

## Context

Found by the adversary on `story:rustgen-type-parameter-shadowing` (slice 15 wave 1, f3ad1e0),
outside that story's claim (records and enums declared in a program, std or a sibling):

- `golden::type_parameter_named_like_a_protocol`: inside `fn call<P>`, `P.serve(..)` is generated
  as `P::serve` (`role_path`, `crates/ignite-rustgen/src/lib.rs:3307`), E0599.
- `golden::type_parameter_named_like_a_builtin_enum`: a parameter `Option` makes the bare
  `Option<..>` of `rust_ty` resolve to the parameter (lib.rs:1988), E0109.
- `golden::type_parameter_named_like_a_builtin_type`: an annotation `Text` becomes `String` before
  the fn's type parameters are consulted (lib.rs:1451), E0308.

All three are `#[ignore]` in `crates/ignite-rustgen/tests/golden.rs`.

## Scope

`crates/ignite-rustgen/src/` and `crates/ignite-rustgen/tests/golden.rs`; or, for the checker
branch, `crates/ignite-check/` and a section-8 row in `docs/slice.md`.

## Acceptance

- The three tests pass without `#[ignore]`, or each is replaced by a checker fixture whose
  `.expected` names the diagnostic at the type parameter.
- `cargo test -p ignite-rustgen --test main` passes with no other golden changed.
