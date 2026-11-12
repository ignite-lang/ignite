---
format: aep.planning-md/3
id: story:rust-target-shared-list-storage
kind: story
status: draft
title: A list on the Rust target is copied by reference count and copied in full only when it changes
relations:
- informed_by: story:rust-target-read-only-forms-borrow
revision: 4
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

Measured by the same user on 2026-11-12, three steps, on a service that had already rewritten its hot functions to read lists in place (instructions per request for its largest answer, one request at a time):

- The borrow fix (`4bcda2ab`, unit `mysql-cost`): 218 M → 216 M (about 1 %). The fix removes copies the service no longer made.
- Binding the read before a record update, in the service's own source (`let b = read_in(r, "b")?` then `r = set_in(r, "a", b)`, which moves `r`; 842 emitted `r.clone()` sites → 0): 217.9–219.0 M → 216.9–218.0 M (0.5 %). The record is small when those copies happen.
- The profile after both: 74 % of instructions in libc (memcpy, malloc, free); the copy of a record's field list (`<(String, Value)>::to_vec`) 32 % inclusive; the drop of values 36 % inclusive; a lookup that returns a field's value 9 %; a filter that pushes kept elements into a new list 4 % self.

So what is left is the copy of a value taken out of a lent list: `out.push(r)` in a loop over a lent list, and `return Some(e.1)` from a lookup (the reporter's reproducer `top.ignite`, forms 1 and 2; synthetic). Neither is a copy the backend makes by mistake: under value semantics the list keeps its element and the result needs one. A lookup whose result is only compared (`field(r, k) != None`) copies the whole value to answer yes or no.

The design note's first step: count, on a synthetic program that builds an answer of nested records, lists and maps out of rows, the copies by form, and for forms 1 and 2 compare at least three designs by count:

- reference-counted list (and text) storage, copied in full only at the first change after a share;
- a parameter taken by value when its elements leave the function, with the caller moving a list that is dead after the call and copying one that is not (a filter that keeps few rows of a live list then copies more than today);
- a lookup that returns a lent value where every caller only reads it.

## Measured 2026-11-12: the branch on the first user's service

The first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS) built its unchanged source on the Rust target, release, at v0.16.0 and at `3de42165` (branch `wave/14-shared-lists`: the copies fix, unit `call-moves` and the shared lists), and counted one request at a time:

- Largest answer: allocations −32 %, instructions −17 %.
- Small answer: allocations −35 %, instructions −34 %.
- The bodies are byte-identical; the service's own conformance run passes as before; no compile error and no new compiler warning.

The synthetic workload of `docs/value-copies.md` predicted −93 % instructions for the same change. It models a filter over a live captured list whose result is filtered again; the service's largest answer spends most of its allocations elsewhere. Its remaining allocations by site, same build, one request:

- 34 %: `push` onto fresh lists of `Int` in the service's own arbitrary-precision arithmetic (source-level; the language has no decimal or big integer in std).
- 23 %: copies of `Text` (fields and bindings; the forms are not reduced yet).
- 12.5 %: text literals and constants turned into owned texts, of which about 9 % of all allocations are two module constants rebuilt on every use (`story:rust-target-constants-built-once`).
- 6.8 %: copies of byte slices charged to the HTTP head parser; the count does not fit one head per request and is being checked.
- No copy of a whole list is among the top 15 sites.

What this changes: the shared lists remove what they were built to remove, and the next units on this path are the constants and the `Text` copies, in that order, each after its forms are reduced from the service's source to synthetic programs.
