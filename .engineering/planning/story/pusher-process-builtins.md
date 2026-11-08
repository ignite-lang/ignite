---
format: aep.planning-md/3
id: story:pusher-process-builtins
kind: story
status: active
title: Environment, sleep, wall clock and flush builtins
scope:
- confidence: inferred
  path: crates/ignite-check/src/lib.rs
- confidence: inferred
  path: crates/ignite-io/src
- confidence: inferred
  path: crates/ignite-llvm/src
- confidence: inferred
  path: crates/ignite-rt/src
- confidence: inferred
  path: crates/ignite-rustgen/src
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/process.ignite
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T22:11:33Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-07T22:11:33Z", actor: "human:timo", revision: 3}
---
# Story: case-study-process-builtins

## Outcome

A service written in Ignite can read environment variables, sleep, read the wall clock and flush its writes, on both targets, so the case-study drops its `/proc/self/environ`, loopback "bells" for sleeping, `/proc/stat` clock and write-order workarounds.

## Context

- case-study case study (session `case-study`), `docs/research/case-study-gaps.md` G2, G3 (sleep only), G8, G13; `docs/friction.md` rows 61, 62, 67, 72. Timo, 2026-11-07: high priority.
- G2: `docs/slice.md` 21.6 has `Setting` (flags) only; reproducer `docs/research/case-study-gaps/G2.ignite`. Follow the capability style of `Setting` and `CommandLine` (`std/process.ignite`, 21.5 to 21.7), with a test-only constructor.
- G3: `Clock` has no `sleep` (`G3.ignite`). The `ignite run` manual clock (`docs/slice.md` 16) must stay deterministic: say what `sleep` does under it. Timers and channels stay out (`docs/actors.md`).
- G8: no wall clock (`G8.ignite`); epoch milliseconds are enough.
- G13: `ignite_io::flush` exists (`docs/slice.md` 7) but is no builtin (`G13.ignite`).

## Acceptance

- Each of the four is a builtin on both targets with inline tests, documented in `docs/slice.md` and its std module; the four reproducers (adapted to the chosen names) run on both targets.
- The effect rules cover the new capabilities (a `requires` like the existing ones); a missing effect gives the existing diagnostic.
- `CODEGEN_REVISION` bumped if generation changes; `task gate` green.

## Out of Scope

- Timers, condition variables, channels (G3 beyond sleep); `setrlimit` (G10).
