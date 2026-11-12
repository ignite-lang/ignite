---
format: aep.planning-md/3
id: story:rust-target-handle-method-keeps-parameter-lent
kind: story
status: active
title: A record used only through its handles is lent on the Rust target
relations:
- informed_by: story:rust-target-read-only-forms-borrow
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T20:27:00Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
- {from: "proposed", to: "active", at: "2026-11-12T20:27:00Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
---
# Story: a record used only through its handles is lent on the Rust target

## Outcome

On the Rust target a record parameter whose only "change" is a method of a handle it holds (`p.counts.put(..)`, `p.counts.upsert(..)`, a `send` on a field) is lent, so its caller does not copy the record, and the state of an `HttpApp` is not copied for each request.

## Why

Reported on 2026-11-12 by the first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS, Rust target, release, v0.16.2): a record that holds the connection settings, among them a certificate bundle as one large `Text`, was copied once per statement and twice more per request. The bytes copied per request were several times the size of the answer; each copy is a block above the allocator's mmap threshold.

A synthetic program of 90 lines, `ignite emit` at `1febe156` (v0.16.2 plus documents):

1. A handle's method on a field. `fn bump(p: Pool) -> Int { p.counts.put(1, 2)  p.max_open }` with `counts: Store<Int>` is emitted as `pub fn bump(mut p: Pool) -> i64` and its call as `bump(p.clone())`: the whole record is copied for a call that changes no field of it. `Store::put`, `upsert`, `update` and their `_read` forms take `&self` in the substrate (`crates/ignite-io/src/store.rs`). The same function without the `put` is emitted as `pub fn peek(p: &Pool)` and called as `peek(p)`.
2. The same with a closure that reads the parameter: `p.counts.upsert(1, |old| .. p.max_open ..)` is emitted as `pub fn run(mut p: Pool, ..)`, called as `run(p.clone(), sql)`.
3. The state of an HTTP application, as the first user reports the emitted line: `std/protocol/http.ignite:1873` `application.handle(application.state, question)` is emitted as `(application.handle)(application.state.clone(), request)`: one copy of the state per request, because a fn value's parameters are owned. Not reproduced here; the unit confirms the form first.

`examples/todo` has form 1 (`state.todos.put(todo.id, todo)`, `todo.ignite:239`); its state holds two stores and no data, so the copy is two counts.

## Acceptance

- A count test with the synthetic program's functions (`run`, `bump`, `query`), red first: bytes and allocations per call with a record that holds a `Text` of 100,000 bytes, Rust target. `bump` and `run`: 0 allocations for the record (today one copy of every field that is not a handle).
- The rule is written down in `docs/slice.md` section 6: which methods of which handle types (`Store`, `Sender`, `Receiver`, a stream, a listener) leave the parameter lent, and that an assignment to a field or a change in place of a field that is a value still makes it owned.
- Form 3: the emitted call of an application's handler is quoted in the story's evidence; if it copies the state, either the handler's state parameter is lent, or the story states why a fn value cannot lend and what a program does instead, with a count for a state record that holds a list of 1,000 elements.
- A closure passed to the handle's method that changes a field of the parameter (`p.counts.upsert(1, |old| { p.max_open = 3  old })`, if the checker accepts it) gives the evaluator's value on all three.
- The evaluator and the LLVM target give the same values for the count program; the LLVM target's count is written down.
- Row 3.10 (`task path-cost`): the cells of `todo` are not above their counts at the last release; `CODEGEN_REVISION` is bumped; `task gate` green; a dedicated-host run shows no row worse.

## Scope

- `crates/ignite-rustgen/src/lib.rs` (the parameter modes: what counts as a change of a parameter), `crates/ignite-rustgen/tests/golden/`, `crates/ignite/tests/` (count test), `std/protocol/http.ignite` (form 3, if the handler's signature is what copies), `docs/slice.md` section 6. Inferred.

## Not in this story

- Sharing the storage of `Text` (`story:rust-target-text-literals-and-field-copies`).
- By-value parameters that every caller could move into (`story:rust-target-text-reads-borrow`, forms 3 and 4).
