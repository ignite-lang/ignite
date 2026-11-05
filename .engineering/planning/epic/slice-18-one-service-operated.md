---
format: aep.planning-md/3
id: epic:slice-18-one-service-operated
kind: epic
status: active
title: 'Slice 18, F5: one service operated end to end'
summary: The todo service built on LLVM, its data durable, probed by an HTTP probe kind written outside std, deployed and recovered by ignite apply under fault injection
relations:
- depends_on: epic:playground-substrate
- depends_on: epic:slice-13-local-apply
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T08:56:51Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-05T08:56:51Z", actor: "human:timo", revision: 3}
---
# Epic: slice-18-one-service-operated

## Outcome

One real HTTP service, the todo service, is built, deployed, measured and recovered from one Ignite
specification on one host: its data survives restarts and rollouts, an HTTP health probe written
outside `std/` feeds an `Availability` contract, `ignite apply` never reports success it did not
observe and never repeats an effect it cannot prove undone, and the service builds on the LLVM
target in a fraction of the Rust target's time.

## Context

- Source: the external review of 2026-11-04 (`docs/research/review-2026-11-04.md`), priorities 1,
  2 and 4 (findings R2, R10, R11, R13), and Timo's correction there (section 2): Ignite is also a
  Rust replacement and exists because of rustc's compile times.
- Builds on slice 13 (`ignite apply`, `docs/slice.md` 17), slice 12 (`ignite run`, probes,
  contracts, 16) and slice 17 (`Directory`, `Setting`, `docs/slice.md` 21, epic
  `epic:playground-substrate`).
- Measured baseline: `todod` dev build 0.309 s LLVM, 3.998 s Rust (`AGENTS.md`, slice 12b rows).
- Extensibility today: the probe driver runs any probe executable
  (`crates/ignite/src/run/probe.rs:1`), but a contract's `good` is a `Text` matched against two
  Telnet outcomes (`std/sli.ignite:22`, `:30`), and the machine set is closed
  (`std/control.ignite:25`).

## Acceptance

- `examples/todo` declares its data in a `Directory`; a todo created before a SIGKILL of `todod`
  and before an `ignite apply` rollout to a changed build is listed after both, on the LLVM target.
- An HTTP probe kind and its outcome type live under `examples/todo/`; an `Availability` contract
  over them reaches each `Status` (`Compliant`, `Violated`,
  `InsufficientEvidence`, `std/control.ignite:199`) under `ignite run`
  with a manual clock; no file under `crates/` or `std/` names the probe or its outcome.
- The fault-injection matrix of the contract passes on the todo system: a host killed after an
  effect and before its receipt, a plan saved before a source change, and a journal with a
  truncated or altered last record each end in a stated recovery or refusal, never in `completed`
  over an unobserved effect or in a second start of the same operation.
- The todo service's dev build on the LLVM target, and its edit-build-restart cycle under
  `ignite apply`, are timed before and after the slice and recorded in `docs/performance/18/`.

## Out of Scope

- More than one host, fencing and distributed authority (review R8).
- User-defined controller machines (R4) and a new protocol (R13's other two parts).
- Binding executables written in other languages (R12).
- A licence decision; that one is Timo's.
