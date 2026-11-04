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
revision: 3
---
# Story: rustgen-redundant-clones

## Outcome

The Rust target clones a value only where Ignite semantics need a copy, so its allocation count
on a protocol server falls toward the handwritten Rust baseline.

## Context

- Found by the playground session (2026-11-04, ember): the Rust target makes 748k allocations
  where the handwritten Rust baseline makes 4,797.
- Step 1, in landing (branch `perf/rustgen-for-borrow`): a `for` over a `List<T>` place whose body
  mutates neither the element nor the root iterates by reference; ember allocations 748,453 to
  652,439 (−12.8 %), throughput at pipeline 50 494,590 to 566,371 ops/s (tournament
  `results/2026-11-04T02-34-13Z.json` to `02-36-02Z.json` in the examples repository).
- Remaining sites, dhat share of the Rust target's allocations after step 1, measured by the
  playground session:
  - text conversions (`parse_int` `slice().to_text()`, int and text `to_text`) 13.1 %;
  - `Store` copies in `get`, `update`, `values` 10.4 % (story:store-borrowing-reads);
  - `match place.clone()` scrutinees 10.2 %;
  - interpolation copies and `format!(..).clone().into_bytes()` 5.1 %;
  - byte literals rebuilt as a `Vec` on every call 4.3 %;
  - `&x.clone()` where a borrow is enough 3.2 %.
- Slice 16's U3 (`story:slice-16-moves-analysis`, Rust last-use moves) covers
  `Parsed::Command(args.clone(), ..)` (4.3 %) and `next.entries.push(entry.clone())` (3.2 %).

## Acceptance

- A dhat run of the ember server under the same load, before and after each step, recorded in
  `docs/performance/`, with each removed clone site named.

## Out of Scope

- The LLVM target's copies (story:store-borrowing-reads).
