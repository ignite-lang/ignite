---
format: aep.planning-md/3
id: story:rust-target-shared-list-storage
kind: story
status: implemented
title: A list on the Rust target is copied by reference count and copied in full only when it changes
relations:
- informed_by: story:rust-target-read-only-forms-borrow
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T20:29:57Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":3,"review_outcome":1}}, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
- {from: "proposed", to: "active", at: "2026-11-12T20:29:57Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":3,"review_outcome":1}}, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
- {from: "active", to: "implemented", at: "2026-11-12T20:29:57Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":3,"review_outcome":1}}, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
---
# Story: a list on the Rust target is copied by reference count and copied in full only when it changes

## Outcome

Where the ownership analysis cannot lend or move a `List`, `Map`, `Text` or `Bytes` on the Rust target, the copy costs a reference count, and the elements are copied only by the first change after it.

## Why

`story:rust-target-read-only-forms-borrow` removes copies the backend makes where a borrow is enough. A value that is stored, returned or captured is still copied element by element (`Vec::clone`). The first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS, Rust target, release, v0.16.0) measured, after reading its lists in place, several times the CPU per request of the same service in Rust for its largest answer; the Rust service's values are shared by reference count. malloc and free were the largest group of samples before that change. The reporter asks for shared storage as the general remedy.

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

- The borrow fix (`4bcda2ab`, unit `mysql-cost`): next to no change. The fix removes copies the service no longer made.
- Binding the read before a record update, in the service's own source (`let b = read_in(r, "b")?` then `r = set_in(r, "a", b)`, which moves `r`; every emitted `r.clone()` site of that form gone): next to no change. The record is small when those copies happen.
- The profile after both: most instructions in libc (memcpy, malloc, free). By inclusive share: the drop of values first, the copy of a record's field list (`<(String, Value)>::to_vec`) second, a lookup that returns a field's value third, a filter that pushes kept elements into a new list fourth.

So what is left is the copy of a value taken out of a lent list: `out.push(r)` in a loop over a lent list, and `return Some(e.1)` from a lookup (the reporter's reproducer `top.ignite`, forms 1 and 2; synthetic). Neither is a copy the backend makes by mistake: under value semantics the list keeps its element and the result needs one. A lookup whose result is only compared (`field(r, k) != None`) copies the whole value to answer yes or no.

The design note's first step: count, on a synthetic program that builds an answer of nested records, lists and maps out of rows, the copies by form, and for forms 1 and 2 compare at least three designs by count:

- reference-counted list (and text) storage, copied in full only at the first change after a share;
- a parameter taken by value when its elements leave the function, with the caller moving a list that is dead after the call and copying one that is not (a filter that keeps few rows of a live list then copies more than today);
- a lookup that returns a lent value where every caller only reads it.

## Measured 2026-11-12: the branch on the first user's service

The first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS) built its unchanged source on the Rust target, release, at v0.16.0 and at `3de42165` (branch `wave/14-shared-lists`: the copies fix, unit `call-moves` and the shared lists), and counted one request at a time:

- Largest answer: fewer allocations and fewer instructions; the allocations fall further than the instructions.
- Small answer: fewer of both, and a larger gain in instructions than the largest answer shows.
- The bodies are byte-identical; the service's own conformance run passes as before; no compile error and no new compiler warning.

The synthetic workload of `docs/value-copies.md` predicted −93 % instructions for the same change; the service gains a fraction of that. The workload models a filter over a live captured list whose result is filtered again; the service's largest answer spends most of its allocations elsewhere. Its remaining allocations by site, same build, one request, largest first:

1. `push` onto fresh lists of `Int` in the service's own arbitrary-precision arithmetic (source-level; the language has no decimal or big integer in std).
2. Copies of `Text` (fields and bindings; the forms are not reduced yet).
3. Text literals and constants turned into owned texts, most of it two module constants rebuilt on every use (`story:rust-target-constants-built-once`).
4. Copies of byte slices charged to the HTTP head parser; the count does not fit one head per request and is being checked.

No copy of a whole list is among the largest sites.

What this changes: the shared lists remove what they were built to remove, and the next units on this path are the constants and the `Text` copies, in that order, each after its forms are reduced from the service's source to synthetic programs.

## Measured 2026-11-12: the corrected branch on the first user's service

The same service, with its own source changed since the first measurement (digits from byte arithmetic, an integer fast path in its arithmetic), built at v0.16.0 and at `3673bee8` (branch `wave/14-shared-lists` after the adversary pass: a matched payload that is only read is read in place, `clear` of a shared list copies nothing). One request at a time, the second request of each answer:

- Largest answer: fewer allocations and fewer instructions, as in the first measurement.
- Small answer: fewer of both, and again the larger gain.
- The bodies are byte-identical on every request; the service's own conformance run passes as before; no compile error and the same compiler warnings as at v0.16.0.

By site at `3673bee8`, one request of the largest answer (frame names, an estimate): somewhat fewer than half of the allocations are copies, the rest build values. The ten largest sites hold most of them; eight are the service's own functions (row decoding first, a generated route function second, the cells of the result rows third). Two are std's: `take_bytes` and `string_value` in `std.protocol.mysql`, the second of which is one avoidable copy per text cell (`story:rust-target-text-reads-borrow`).

What this changes: nothing in the order of the next units. The copies that remain in the service's functions are the text forms of `story:rust-target-text-reads-borrow`, reduced by the user to a synthetic program.
