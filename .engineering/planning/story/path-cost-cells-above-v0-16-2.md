---
format: aep.planning-md/3
id: story:path-cost-cells-above-v0-16-2
kind: story
status: draft
title: The path-cost cells that v0.16.3 raised are back at v0.16.2
relations:
- informed_by: story:path-cost-counted-at-every-merge
- informed_by: story:rust-target-constants-built-once
revision: 1
---
# Story: the path-cost cells that v0.16.3 raised are back at v0.16.2

## Outcome

The three cells of row 3.10 of `docs/quality.md` that v0.16.3 recorded above v0.16.2 beyond their bands are at their v0.16.2 values or below, or the story states for a cell why its v0.16.3 value stays.

## Why

`task path-cost` on the release head of v0.16.3 against the counts of v0.16.2 (recorded with `IGNITE_WORKERS=2`, commit `a3abbba7`) refused three cells, each measured three times:

- `rust/design/build` instructions: 1,155,668,462 at v0.16.2, 1,163,103,404 at v0.16.3 (+0.64 %, band 1,155,668). The cell is the Rust driver built from the terminal example's design file. The functions that grew are spread over 264 symbols; the named ones are `ignite_rustgen::rust_identifiers` in `StdDependencies::new` (8,922,743 to 9,634,161), string compares and slice iteration under it, and `ignite_types::moves::Live::join` (under 115,566 to 515,338). The unit `constants` alone (PR #146, without the other units of the release) read 1,167,683,033, so most of the rise is that unit's: the generated Rust of `std` gained one static per constant that is built once.
- `rust/todo/get` instructions: 302,422 at v0.16.2, 303,865 at v0.16.3 (+0.48 %, band 302), the same value in three runs. `malloc` 21,612 to 21,942 and libc 35,022 to 36,195. The unit `constants` alone read the same 303,865, so the cell is that unit's; which constant's first use or copy the GET of a todo pays was not looked for.
- `llvm/terminal/session` instructions: 244,745 at v0.16.2, 245,028 at v0.16.3 (+0.12 %, band 244), the same value in three runs. Functions that appear where v0.16.2 had them under 24 instructions each: `ignite_rt::arena::TaskArena::swap` 1,140, `ignite_rt::IgniteList::from_bytes` 1,020, the closure of `ignite_rt_spawn` 996, `TaskArena::new` 480. Inferred, not checked: the runtime of v0.16.3 holds the merge sort, and the release build inlines these functions differently. The first recording of v0.16.2 (2026-11-12, another server environment) read this cell 245,441, so its v0.16.2 value has two readings.

The release was cut with these cells tracked and not refused (the rule of 2026-11-11), and its `RECORD` wrote the v0.16.3 values as the values a merge is held to.

The other 33 of the 36 cells were inside their bands or below v0.16.2; below it by more than the band: `all/design/check` 854,153,551 to 851,995,208, `all/design/plan` 859,996,322 to 857,731,455, `llvm/design/build` 2,236,463,944 to 2,227,491,893, `rust/todo/delete` 541,888 to 532,880, `rust/todo/patch` 800,543 to 798,633, `rust/todo/start` 388,903 to 386,829, `llvm/todo/connection` 471,700 to 469,930.

## Acceptance

- `rust/design/build`, `rust/todo/get` and `llvm/terminal/session` read at most 1,155,668,462, 302,422 and 244,745 instructions by `task path-cost`, or the story records for a cell the cause and why the value stays, with the count of what the rise buys.
- The allocations per round that the unit `constants` removed stay removed (`crates/ignite/tests/constants.rs` unchanged).
- `task gate` green.

## Scope

- `crates/ignite-rustgen/src/lib.rs` (`StdDependencies::new`, the statics of constants built once), `crates/ignite-rt/src/` (the task arena and the list functions the session calls). Inferred.
