---
format: aep.planning-md/3
id: story:llvm-release-handle-temporaries
kind: story
status: active
title: The LLVM target releases a value read from a store that no binding holds
scope:
- confidence: cited
  path: crates/ignite-llvm/src/lower.rs
- confidence: cited
  path: crates/ignite-llvm/src/own.rs
- confidence: cited
  path: crates/ignite/tests/fixtures/leak
- confidence: cited
  path: crates/ignite/tests/leak.rs
- confidence: cited
  path: docs/llvm.md
- confidence: cited
  path: docs/slice.md
- confidence: inferred
  path: quality/baseline.json
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T00:24:55Z", actor: "human:timo", revision: 4, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T00:24:55Z", actor: "human:timo", revision: 5, executor: "agent:claude-coordinator", correlation: "wave-11"}
---
# Story: the LLVM target releases a value read from a store that no binding holds

## Outcome

Row 1.1 `llvm` is 0 again (now 4): the four cases of
`crates/ignite/tests/fixtures/leak/attack2.ignite` listed in `known_differences` pass on the LLVM
target as they do on the Rust target.

## Why

`quality/baseline.json` names this story for the four differences; it was never written. The
fixture came after v0.15.0 (`97b8c3c9`); the leaks are older than the row.

Diagnosis (`.engineering/waves/2026-11-11-wave-11-diagnosis/counts.md`): `Lower::builtin`, arm
`StoreGet | StoreRemove` (`crates/ignite-llvm/src/lower.rs`), copies the value out of the store
through the glue `copy_from_heap.<T>`, which retains every handle in the copy. Only a block that
registered the value releases it at exit, and `crates/ignite-llvm/src/own.rs` decides
registration syntactically (`owned`, `owned_match`, `owned_iter`). Four forms register nothing:

- a nested `match` over a store read that is an unbraced arm value;
- a record literal with a handle field passed to `put`;
- a store read that is the receiver of a call (`get(..).is_some()`);
- a bare `remove` statement.

A patch of +119 -2 lines in `own.rs` and `lower.rs` was tried in a copy: `attack2.ignite` 16
passed on LLVM, `cargo test -p ignite-llvm` 41 passed. It is a starting point.

## Acceptance

- `leak::stored_ends_read_in_every_form_are_released` asserts "16 passed; 0 failed" on both
  targets (today it asserts the four failures on LLVM).
- Further cases, each red or green for a stated reason: a temporary returned from the fn, passed
  to `spawn`, in a loop condition, under `?`, and in a task that fails.
- The four entries leave `known_differences`; bar 1.1 `llvm` is 0 and its decision is withdrawn.
- `docs/slice.md` 30.3 no longer lists the gap; `docs/llvm.md` states the owner rules as built.
- The Rust target's output is unchanged (its goldens do not move).
- `CODEGEN_REVISION` is bumped by the coordinator at the merge, not by the unit.

## Scope

- `crates/ignite-llvm/src/own.rs`, `lower.rs`. Cited.
- `crates/ignite/tests/leak.rs`, `crates/ignite/tests/fixtures/leak/`. Cited.
- `docs/slice.md` 30.3, `docs/llvm.md`. Cited.
- `quality/baseline.json` (`known_differences`, bar 1.1): written by the coordinator at the
  merge.
