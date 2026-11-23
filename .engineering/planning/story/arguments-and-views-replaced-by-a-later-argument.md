---
format: aep.planning-md/3
id: story:arguments-and-views-replaced-by-a-later-argument
kind: story
status: draft
title: Arguments and views that a later argument replaces
relations:
- informed_by: story:place-receiver-read-before-index-and-prefix-argument
- informed_by: story:temporary-receiver-evaluated-before-arguments
revision: 2
---
# Story: arguments and views that a later argument replaces

## Outcome

A call whose later argument replaces the variable an earlier argument or a view receiver was read from gives the evaluator's value on both native targets, and the Rust target builds it: an earlier argument is read before a later one runs, and a receiver that is a view of a local (`t.to_bytes()`, `b.slice(i, j)`) or a changing builtin's place that an argument assigns keeps the value it had.

## Why

Found by the adversary pass on the unit of `story:place-receiver-read-before-index-and-prefix-argument` and `story:temporary-receiver-evaluated-before-arguments` (wave 16, 2026-11-23); the unit fixed the receiver itself, these cases remain. Observed, the evaluator giving the reference value in each:

- `b.index_of(n, {n = b"h"; 0})`: LLVM gives 0 where 2 is right; `m.put(k, {k = ..; 1})`: LLVM gives -1 where 1 is right. LLVM reads the earlier argument through a pointer after the later one ran (`crates/ignite-llvm/src/lower.rs:4870`, `:4753`). The Rust target refuses the needle form (E0506).
- `t.to_bytes().index_of(..)` and `b.slice(0, 5).index_of(..)` whose argument assigns `t` or `b`: the Rust target refuses the program (E0506); LLVM gives the right value. The Rust generator copies only a place (`crates/ignite-rustgen/src/lib.rs:2783`, `:5795`).
- `l.push(if c { l = [4, 5]; 6 } else { 6 })`: the Rust target refuses the program (E0506); the evaluator and LLVM give 306. Recorded as open in `docs/slice.md` section 10.1.

## Acceptance

- The three adversary cases `adversary_push_whose_argument_assigns_its_place`, `adversary_a_view_receiver_whose_argument_replaces_its_local` and `adversary_an_argument_replaced_by_a_later_argument`, in commit `bc907c82` of branch `wave/16-receiver-order`, file `crates/ignite/tests/receiver_order_adversary.rs` (removed from the tree afterwards so the count of ignored tests stays at its bar), come back as tests and pass on the evaluator and both targets.
- A call whose arguments cannot change each other emits no new copy on either target (a count test).

## Scope

- `crates/ignite-llvm/src/lower.rs` (argument reads of `index_of` and `put`), `crates/ignite-rustgen/src/lib.rs` (`replaced_by`, view receivers, changing builtins), `docs/slice.md` section 10.1. Inferred.
