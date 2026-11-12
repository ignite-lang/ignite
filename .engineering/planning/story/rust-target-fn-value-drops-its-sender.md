---
format: aep.planning-md/3
id: story:rust-target-fn-value-drops-its-sender
kind: story
status: draft
title: A fn value that captured the only sender is dropped where the evaluator drops it, on the Rust target
revision: 1
---
# Story: a fn value that captured the only sender is dropped where the evaluator drops it, on the Rust target

## Outcome

On the Rust target a fn value that captured the only sender of a channel is dropped at the point the evaluator drops it, so a receiver waiting for the channel to close sees it close at the same point on every target.

## Why

Found on 2026-11-12 by the second adversary pass on unit `call-moves`. A fn value that captured the only sender, passed to a parameter that does not consume it or used directly as an item of a tuple scrutinee at its last use, stays alive on the Rust target until the end of its scope: a program that counts what it receives until the channel closes gives 51 on the evaluator and 50 on the Rust target. The same programs give 50 at `21bc8eda`, before the unit, so the unit did not introduce it. The cause is not diagnosed and no test is committed; the adversary's probe programs are described in its report (group C and the pre-existing finding).

The unit's own case, a fn value held by a record, a list, an option or a variant that is moved into a tuple match, is fixed in the unit (`holds_handles` treats a fn value as possibly holding a handle). This story is the bare fn value.

The LLVM target has a related, older gap of its own: a place that holds a handle and is moved into a tuple scrutinee is never released (`docs/llvm.md` section 4).

## Acceptance

- A test with the two forms (a fn value passed to a non-consuming parameter at its last use; a fn value as a tuple scrutinee item at its last use), red first on the Rust target: the evaluator's value on the Rust target.
- The cause is written down in `docs/slice.md`: which rule keeps the fn value (a borrow of the closure, a copy of an `Rc`, a binding kept to the end of the scope).
- A receiver loop that waits for the close does not hang on the Rust target in either form.
- The LLVM target's value for the same two forms is written down; a fix there is a separate story if it differs.
- `CODEGEN_REVISION` is bumped if emitted code changes; `task gate` green.

## Scope

- `crates/ignite-rustgen/src/lib.rs` (fn values as arguments and as tuple items, last uses), `crates/ignite-types/src/moves.rs` if the last use of a fn value is not marked as a move, `crates/ignite/tests/`. Inferred.
