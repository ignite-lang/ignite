---
format: aep.planning-md/3
id: story:substrate-built-once
kind: story
status: archived
title: The runtime archive is built once per gate, not again by build.rs
scope:
- confidence: inferred
  path: crates/ignite/build.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T19:44:37Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-05T19:44:37Z", actor: "human:timo", revision: 4}
- {from: "active", to: "archived", at: "2026-11-05T20:33:19Z", actor: "human:timo", revision: 6}
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

## Outcome of the attempt (2026-11-05)

Not met; archived. PR https://github.com/ignite-lang/ignite/pull/49 closed unmerged, branch `perf/substrate-built-once` (4894a6315) kept.

- Warm `--no-run` after touching ignite-io, 3 pairs base / after: 5.80 / 5.70, 5.11 / 5.40, 4.90 / 6.75 s.
- Cause (measured): the nested cargo built ignite-io and ignite-rt in parallel in about 0.5 s; as a build-dependency they run in series before build.rs, so the ignite lib starts later (1.24 / 1.41 s against 0.51 / 0.84 s).
- The gate still builds ignite-rt twice (feature panic-probe); only ignite-io is saved; the archive would be picked by mtime from target/deps.
- The premise of review recommendation 7 does not hold for the warm benchmark; cold and CPU savings were not measured.
