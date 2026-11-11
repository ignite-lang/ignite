---
format: aep.planning-md/3
id: story:llvm-task-move-aliases
kind: story
status: draft
title: An owner moved to a task on LLVM leaves no holder of a released handle
relations:
- informed_by: story:llvm-release-handle-temporaries
revision: 1
---
# Story: an owner moved to a task on LLVM leaves no holder of a released handle

## Outcome

On the LLVM target, a binding handed to a spawned task is moved only when nothing else in the block still holds it; otherwise the task gets a retained owner. No program accepted by the checker uses a handle after its release.

## Why

Adversary pass 2 on unit handles of wave 11 (`review-result:adversary-handles-pass-2`), all four pre-existing on the unit's base:

- `crates/ignite-llvm/src/own.rs:204`: `moves_apart` looks for mentions only after the `let`. An alias, a record or a closure made before it still holds the end; the task takes the only owner and the holder then uses a released handle (three cases). The rule in `docs/llvm.md:574` does not hold.
- `crates/ignite-llvm/src/own.rs:170`: every builtin result other than `copy()` counts as fresh. `xs.at(i)` returns the list's own element, not a new owner; `docs/llvm.md:572` says it is fresh.
- `crates/ignite-llvm/src/lower.rs:5737`: an owned binding that is not handed over because it has an alias is still moved to the task without a retain; the block releases it a second time. `docs/slice.md` 30.3 says the block keeps its owner.
- `crates/ignite-llvm/src/own.rs:232`: `arm_moves_fresh` resolves a name in an arm's block against the outer block.

The Rust target is not affected. v0.15.0 was not measured.

## Acceptance

- The red cases of the pass (in the unit's test files) are green on LLVM and stay green on Rust.
- `docs/llvm.md` and `docs/slice.md` 30.3 state the rule the code implements.
- Row 1.1 stays 0 for LLVM; the release-count soaks show no leaked and no early-released handle.
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `crates/ignite-llvm/src/own.rs`, `crates/ignite-llvm/src/lower.rs`, `docs/llvm.md`, `docs/slice.md`. Inferred.
