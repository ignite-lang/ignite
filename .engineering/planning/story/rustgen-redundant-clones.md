---
format: aep.planning-md/3
id: story:rustgen-redundant-clones
kind: story
status: draft
title: Drop redundant clones in generated Rust
summary: The Rust target clones only where Ignite semantics need a copy
relations:
- depends_on: story:slice-16-moves-analysis
scope:
- confidence: inferred
  path: crates/ignite-rustgen/src
revision: 2
---
# Story: rustgen-redundant-clones

## Outcome

The Rust target clones a value only where Ignite semantics need a copy, so its allocation count
on a protocol server falls toward the handwritten Rust baseline.

## Context

- Found by the playground session (2026-11-04, ember): the Rust target makes 748k allocations
  where the handwritten Rust baseline makes 4,797; the generated `Resp::serve` holds
  `more.clone()`, `bucket.entries.clone()` and `out.clone()`. That these clones cause the count
  is inferred, not measured.
- Slice 16's moves analysis (`story:slice-16-moves-analysis`, last-use moves on the Rust target)
  removes clones at a value's last use; this story measures what is left after it.

## Acceptance

- A dhat run of the ember server under the same load, before and after, recorded in
  `docs/performance/`, with each removed clone named.

## Out of Scope

- The LLVM target's copies (story:store-borrowing-reads).
