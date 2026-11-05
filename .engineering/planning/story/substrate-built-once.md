---
format: aep.planning-md/3
id: story:substrate-built-once
kind: story
status: active
title: The runtime archive is built once per gate, not again by build.rs
scope:
- confidence: inferred
  path: crates/ignite/build.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T19:44:37Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-05T19:44:37Z", actor: "human:timo", revision: 4}
---
# Story: substrate-built-once

## Outcome

`crates/ignite/build.rs` no longer compiles `ignite-rt` and `ignite-io` a second time in its own target dir during the gate, and the carried runtime of PR 33 keeps working.

## Context

- Recommendation 7 of the architecture review (`docs/research/review-2026-11-05-architecture.md`), effort S.
- Evidence: `crates/ignite/build.rs:13-68` runs a nested cargo with `rerun-if-changed` on both crates.

## Acceptance

- warm `cargo test --workspace --no-run` after a touch of `crates/ignite-io/src/lib.rs` is faster than on the PR 33 tree, measured in 3 pairs; `llvm_build::a_driver_alone_links_the_runtime_it_carries` stays green
- `task gate` green.

## Out of Scope

- The other recommendations of the review.
