---
format: aep.planning-md/3
id: story:llvm-lto-runtime
kind: story
status: active
title: Release builds link std, program and runtime with LTO
relations:
- decomposes: epic:native-parity
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:59:07Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T06:03:41Z", actor: "human:timo", revision: 3}
---
# Story: llvm-lto-runtime

## Outcome

`ignite build --release` optimises std, program and the runtime (as bitcode) together with LTO, so runtime calls in hot paths inline.

## Context

- Epic epic:native-parity (diagnosis and evidence in its body).

## Acceptance

- Release build time recorded (quality row 6.2 stays within its bar); todo get-64 user instructions down at least 5 % on top of `story:rt-hot-path`.
- Measured before and after locally with `perf stat` cost per reply against plain Rust (the method in the epic); the numbers go in the commit body; `task gate` green.

## Scope

- `crates/ignite/src/llvm_build.rs`, `crates/ignite/build.rs`, `crates/ignite-rt`. Inferred.
