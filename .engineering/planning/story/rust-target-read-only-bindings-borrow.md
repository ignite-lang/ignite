---
format: aep.planning-md/3
id: story:rust-target-read-only-bindings-borrow
kind: story
status: draft
title: A binding that is only read, and a Map lookup, borrow on the Rust target
revision: 2
---
# Story: a binding that is only read, and a `Map` lookup, borrow on the Rust target

## Outcome

On the Rust target `let v = l[i]` and `let x = r.f` whose binding is only read, `m.get(k)` and a walk over `m.keys()` copy nothing: the generated Rust binds a reference, as it does for an element read in place since unit `mysql-cost`.

## Why

Unit `mysql-cost` (`4bcda2ab`) lends an element or a `for` pattern that is read in place. It left, and `docs/slice.md` lists under "Still copied":

- `let v = l[i]`, then only read: `let v = checked::at(..).clone();`
- `let names = r.names`, then only read: `let names = r.names.clone();`
- `match m.get(k)`: `m.get(&k.clone()).cloned()`, a copy of the key and of the value.
- `for k in m.keys()`: `m.keys().cloned().collect::<Vec<_>>()`.

The first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS) reported on 2026-11-12 that with `4bcda2ab` its profile still has samples in copies of record fields and in drops of values, and names `m.get` and record field reads.

## Acceptance

- For each of the four forms a test over the emitted Rust that fails on the copy, and a count test: N reads allocate nothing per read on the Rust target.
- A single-name `let` may be moved at its last use today; the unit states how a borrowed `let` and a last-use move are told apart, and a test holds a binding that is moved after reads (the copy or the move stays).
- `m.get(k)` binds `Option<&V>` where every arm only reads; a key that is a place is lent, not copied.
- The LLVM target is checked for the same forms by allocation count.
- The differential corpus and the std inline tests give the same values on both targets; `CODEGEN_REVISION` is bumped; `task gate` green.

## Not established

- How much of the reporter's remaining cost these forms are, against values that are stored or returned (`story:rust-target-shared-list-storage`).

## Scope

- `crates/ignite-rustgen/src/lib.rs` (`let` lowering, last-use moves, `Map` methods), `crates/ignite-llvm/src/lower.rs`, `crates/ignite/tests/`, `docs/slice.md` section 6. Inferred.
