---
format: aep.planning-md/3
id: story:random-bytes
kind: story
status: active
title: Secure random bytes
summary: Obtain OS random bytes through a tracked effect, for per-launch tokens
relations:
- decomposes: epic:playground-substrate
- depends_on: story:slice-17-frontend
scope:
- confidence: cited
  path: crates/ignite-io/src/entropy.rs
- confidence: cited
  path: crates/ignite-rt/src/entropy.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T05:11:07Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-11-05T05:11:07Z", actor: "human:timo", revision: 6}
---
# Story: random-bytes

## Outcome

An Ignite program obtains cryptographically secure random bytes, so a local server can mint a
per-launch token.

## Context

- Gap P7 of the playground session (2026-11-04): no effect or builtin yields random bytes
  (`crates/ignite-check/src/lib.rs:128-147` lists the effects).
- Under `epic:playground-substrate`; a new effect, its source in `crates/ignite-io` (the OS
  generator), passed to handlers as a value (P3).
- Contract: `docs/slice.md` section 21 on `feat/slice17-processes-files`, unit U5 of 21.12, row 20 of 21.0 (the `Entropy` handle); first failing test "two draws differ"; after `story:slice-17-frontend`.

## Acceptance

- On both targets, two calls for 32 bytes return 32 bytes each and differ; a function using it
  without the effect is rejected by `ignite check`.

## Out of Scope

- Seeded or deterministic generators for simulation.
