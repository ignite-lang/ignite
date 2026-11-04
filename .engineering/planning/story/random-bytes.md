---
format: aep.planning-md/3
id: story:random-bytes
kind: story
status: draft
title: Secure random bytes
summary: Obtain OS random bytes through a tracked effect, for per-launch tokens
relations:
- decomposes: epic:playground-substrate
scope:
- confidence: inferred
  path: crates/ignite-check/src
- confidence: inferred
  path: crates/ignite-io/src
- confidence: inferred
  path: crates/ignite-rt/src
- confidence: inferred
  path: std
revision: 2
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

## Acceptance

- On both targets, two calls for 32 bytes return 32 bytes each and differ; a function using it
  without the effect is rejected by `ignite check`.

## Out of Scope

- Seeded or deterministic generators for simulation.
