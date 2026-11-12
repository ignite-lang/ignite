---
format: aep.planning-md/3
id: story:rust-target-read-only-forms-borrow
kind: story
status: proposed
title: Read-only forms over a list borrow on the Rust target
relations:
- informed_by: story:std-mysql-client
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T03:05:43Z", actor: "human:timo", revision: 2, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: read-only forms over a list borrow on the Rust target

## Outcome

On the Rust target a `for` with a tuple, record or enum pattern over a list that is only read, and `l[i]` passed to a lent parameter, copy nothing: the generated Rust borrows, as it does for a single binding and for `l.at(i)`.

## Why

Reported on 2026-11-11 by the first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS, Rust target, release, v0.16.0): about 24 times the CPU per request of the same service in Rust. Its profile: 95 % of samples in the service's own code walking lists of values, `Vec::clone` 15 %, the drop of those copies 12 %, malloc and free 62 % in total; the MySQL client under 1 %, TLS about 1.5 %. Reading lists in place on the service's side took its largest answer from 319 ms to 55 ms of CPU per request (the Rust service: 5.3 ms).

The emitted Rust at v0.16.0, from the reporter's reproducer `copies.ignite` (synthetic):

- `for (name, v) in (l.clone())` for a tuple pattern, while `for e in l` gives `for e in (l).iter()`.
- `size(&::ignite_io::checked::at(l, i, ..).clone())` for `size(l[i])`, while `size(l.at(i))` gives no clone. `docs/slice.md` section 6 says both forms are lent.
- `return e.1.clone()` for a tuple field returned from a loop over a field of a lent record.

## Acceptance

- For each of the three forms a test over the emitted Rust that fails on the clone, and a count test: N lookups over a list of M elements allocate nothing per lookup on the Rust target.
- A sweep of the other read-only forms that clone a `List`, `Map`, `Text` or `Bytes` where a borrow is enough, each listed with its emitted Rust before and after; what is not fixed is filed.
- The LLVM target is checked for the same forms by allocation count.
- The differential corpus and the std inline tests give the same values on both targets.
- `CODEGEN_REVISION` is bumped; `task gate` green.

## Scope

- `crates/ignite-rustgen/src/` (patterns in `for`, index expressions in argument position), the shared ownership analysis it reads, `crates/ignite/tests/`. Inferred.
