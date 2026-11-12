---
format: aep.planning-md/3
id: story:rust-target-shared-list-storage
kind: story
status: draft
title: A list on the Rust target is copied by reference count and copied in full only when it changes
relations:
- informed_by: story:rust-target-read-only-forms-borrow
revision: 2
---
# Story: a list on the Rust target is copied by reference count and copied in full only when it changes

## Outcome

Where the ownership analysis cannot lend or move a `List`, `Map`, `Text` or `Bytes` on the Rust target, the copy costs a reference count, and the elements are copied only by the first change after it.

## Why

`story:rust-target-read-only-forms-borrow` removes copies the backend makes where a borrow is enough. A value that is stored, returned or captured is still copied element by element (`Vec::clone`). The first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS, Rust target, release, v0.16.0) measured 55 ms of CPU per request for its largest answer after reading its lists in place, against 5.3 ms for the same service in Rust, whose values are shared by reference count; malloc and free were 62 % of the samples before that change. The reporter asks for shared storage as the general remedy.

## Acceptance

- A design note in `docs/` first: the representation (for example a reference-counted vector that is unique when changed in place), what the ownership analysis already proves unique, the cost of the count on the paths of rows 3.3 and 3.7, and how the LLVM target's arenas compare. Options with their counts, then a decision.
- A count test per kind: storing, returning and capturing a list of M elements allocates nothing and copies no element; the first change after a share copies once.
- The dedicated host's rows 3.3, 3.7 and 3.8 are not worse than at the last release; instructions per request round of the frozen scenarios do not rise.
- `task gate` green.

## Not established

- How many copies remain after `story:rust-target-read-only-forms-borrow`, by form.
- Whether an atomic or a plain count is needed (values that cross a `spawn`).

## Scope

- `crates/ignite-rustgen/src/`, the Rust target's runtime types in `crates/ignite-rt/` and `crates/ignite-io/src/checked.rs`, `docs/slice.md` section 6. Inferred.

## After the borrow fix

Measured by the same user on 2026-11-12 with the borrow fix (`4bcda2ab`, unit `mysql-cost`) on a service that had already rewritten its hot functions to read lists in place:

- Instructions per request for its largest answer: 218 M before the fix, 216 M with it (about 1 %). The fix removes copies the service no longer made.
- The profile with the fix: dropping values 11.1 % of samples, copying record fields 9.9 %, the allocator 14.6 % in its two top frames and about 61 % of all samples in libc.
- What is left is the copy of a value that is stored, returned or read out of a record field or a `Map`: the forms this story is about, and the `Map` lookup and `let` forms the borrow fix left (`docs/slice.md`, "Still copied").

So the count of copies that remain after the borrow fix, by form, is the first thing the design note establishes, from a synthetic program that builds an answer of nested records, lists and maps out of rows.
