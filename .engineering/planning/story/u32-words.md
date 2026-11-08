---
format: aep.planning-md/3
id: story:u32-words
kind: story
status: proposed
title: 'U32: modular 32-bit words with rotate and explicit conversions'
relations:
- decomposes: epic:native-parity
- depends_on: story:list-capacity
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T08:07:15Z", actor: "human:timo", revision: 2}
---
# Story: u32-words

## Outcome

Ignite has `U32`: `+ - *` wrap, `>>` is logical, `rotate_left`/`rotate_right`, conversions only by explicit methods, so SHA-1 rounds compile to what hand-written Rust does.

## Context

- Epic epic:native-parity; umbrella story:fixed-width-ints; design `docs/fixed-width-ints.md` (PR 120), its sections 2 to 5 and 8.
- codecs (SHA-1 and base64 over 1 MiB) is 2.10x plain Rust on LLVM after the alias work; page faults 5.8x.

## Acceptance

- Agreement of evaluator, Rust target and LLVM target with Rust's `u32` on every operator at the edges of design row 4.
- Diagnostics fixtures for E044 and E040 (mixed widths) per design row 5; the typed document (`docs/slice.md` 24) names the type.
- The `docs/slice.md` section for the feature written from the design; numbers before and after in the commit body (method of the epic); `task gate` green on both targets.

## Scope

- `crates/ignite-syntax`, `ignite-check`, `ignite-types`, `ignite-eval`, both backends, `crates/ignite/src/typed_json.rs`, `std/`. Inferred.
