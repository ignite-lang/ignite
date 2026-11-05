---
format: aep.planning-md/3
id: epic:agent-delivery-benchmark
kind: epic
status: draft
title: 'Agent delivery benchmark: verified software per agent hour and cost'
summary: The same requirements through direct Rust, ESS to service-sdk, direct Ignite and ESS to Ignite, measured in tokens, cost, time, builds and correctness
revision: 1
---
# Epic: agent-delivery-benchmark

## Outcome

A repeatable measurement answers whether Ignite delivers more verified software per unit of agent
time and cost than Rust: the same requirements go through direct Rust, ESS to service-sdk, direct
Ignite and (once an adapter exists) ESS to Ignite, and each run records tokens, model cost, wall
time, cold and warm builds, peak memory, build output on disk, repair iterations and runtime
performance at equal correctness.

## Context

- Source: `docs/research/review-2026-11-04-ecosystem.md` (findings E4, E6, E8) and Timo's
  motivation in `docs/vision.md` section 1: many concurrent agents compiling Rust bring the
  machine to a halt.
- Workloads proposed by the review: a CRUD service, an event-driven state machine, a small native
  networking program. Candidates in hand: the todo service (`examples/todo`, ignite-ess
  milestone M0), the `std.control` reducers, the ember server and its handwritten Rust baseline in
  the sibling examples repository.
- Tools: metaharness for comparable agent runs, ESS conformance suites for correctness,
  `tools/ignite-perf` and `docs/performance/` for builds.
- Token savings are a hypothesis to test, not a claim (E4).

## Acceptance

- `docs/performance/agent-delivery/` holds, per workload and path, at least 3 runs with every
  figure above, the harness and model named, and the conformance result each run reached; paths
  that cannot run yet say why.

## Out of Scope

- Building the ESS to Ignite adapter (ignite-ess milestones).
- Speeding up the compiler's own Rust build (self-hosting, wave H).
