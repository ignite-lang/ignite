---
format: aep.planning-md/3
id: story:rustgen-redundant-clones
kind: story
status: implemented
title: Drop redundant clones in generated Rust
summary: The Rust target clones only where Ignite semantics need a copy
relations:
- depends_on: story:slice-16-moves-analysis
scope:
- confidence: inferred
  path: crates/ignite-rustgen/src
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T08:56:32Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":5}}}
- {from: "proposed", to: "active", at: "2026-11-06T10:00:22Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":6}}}
- {from: "active", to: "implemented", at: "2026-11-06T10:00:22Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":6}}}
---
# Story: rustgen-redundant-clones

## Outcome

The Rust target clones a value only where Ignite semantics need a copy, so its allocation count
on a protocol server falls toward the handwritten Rust baseline.

## Context

- Found by the playground session (2026-11-04, ember): the Rust target makes 748k allocations
  where the handwritten Rust baseline makes 4,797.
- Landed (merged to `main`): step 1, `for` over a list place by reference (PR 15; ember
  allocations 748,453 to 652,439); step 2, `Int.parse` of bytes and slices without a copy (PR 18;
  652,437 to 564,434); a match on a place by reference (PR 21); `to_lower`/`to_upper` as a loop
  and `to_bytes` of a fresh text (PR 23); borrowed interpolation parts, `at` elements and appended
  values (PR 25); plain closures for `update`, `upsert` and `map` literals (PR 27); a field of a
  list element through the borrowed element (PR 28). Slice 16's last-use moves (PR 30) cover
  `Parsed::Command(args.clone(), ..)` and `next.entries.push(entry.clone())`.
- Remaining sites, from the playground session's dhat profiles (shares measured before the PRs
  above, not re-measured since):
  - `Store` copies in `get`, `update`, `values`: the lending reads `read`, `read_values`,
    `replace_with` of `ignite_io::Store` (PR 17) are not used by the generator yet: a discarded
    `update` result as `replace_with`, `for x in store.values()` as `read_values` when the body
    does not touch the store (the lock is not reentrant), read-only `match db.get(id)` arms as
    `read`;
  - byte literals rebuilt as a `Vec` on every call (4.3 %);
  - `Int.to_text(n).to_bytes()` clones a temporary receiver (4.3 %);
  - untracked pattern bindings copied at branch tails (`story:moves-for-pattern-bindings`).
- Not met yet: the acceptance's dhat records per step; only
  `docs/performance/store-borrowing-reads.md` has one.

## Acceptance

- A dhat run of the ember server under the same load, before and after each step, recorded in
  `docs/performance/`, with each removed clone site named.

## Out of Scope

- The LLVM target's copies (story:store-borrowing-reads).
