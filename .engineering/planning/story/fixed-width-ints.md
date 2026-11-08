---
format: aep.planning-md/3
id: story:fixed-width-ints
kind: story
status: proposed
title: Fixed-width unsigned integers, rotate and fixed-size lists
relations:
- decomposes: epic:native-parity
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:59:06Z", actor: "human:timo", revision: 2}
---
# Story: fixed-width-ints

## Outcome

Ignite has `U32` (and `U8`, `U64`) with wrapping arithmetic, `rotate_left`/`rotate_right`, fixed-size lists without bounds checks on constant indices, and a capacity hint for `List`, so SHA-1 and base64 compile to what hand-written Rust does.

## Context

- Epic epic:native-parity (diagnosis and evidence in its body).

## Acceptance

- Design in `docs/slice.md`; codecs LLVM time within 1.1x of the reference.
- Measured before and after locally with `perf stat` cost per reply against plain Rust (the method in the epic); the numbers go in the commit body; `task gate` green.

## Scope

- `crates/ignite-syntax`, `ignite-types`, `ignite-eval`, both backends, `std/`. Inferred.
