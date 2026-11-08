---
format: aep.planning-md/3
id: story:text-fast-search
kind: story
status: active
title: Text search and split use memchr and skip the UTF-8 re-check
relations:
- decomposes: epic:native-parity
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:59:06Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T03:59:07Z", actor: "human:timo", revision: 3}
---
# Story: text-fast-search

## Outcome

`find`, `contains` and `split` in `std.text` run on a memchr/memmem builtin, and splitting valid text at an ASCII separator does not re-check UTF-8, so HTTP parsing costs about what the reference pays.

## Context

- Epic epic:native-parity (diagnosis and evidence in its body).

## Acceptance

- Behaviour unchanged on both targets and in the evaluator (inline tests plus a differential test over random texts and separators, including multi-byte ones).
- get-64 user instructions per reply down at least 20 %.
- Measured before and after locally with `perf stat` cost per reply against plain Rust (the method in the epic); the numbers go in the commit body; `task gate` green.

## Scope

- `std/text.ignite`, `crates/ignite-rt/src/text.rs`, builtins in `ignite-check`/`ignite-types`/`ignite-eval`, both backends. Inferred.
