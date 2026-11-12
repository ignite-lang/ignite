---
format: aep.planning-md/3
id: story:element-receiver-read-before-arguments
kind: story
status: draft
title: An element receiver is read before its arguments on every target
revision: 1
---
# Story: an element receiver is read before its arguments on every target

## Outcome

A method call or an index whose receiver is an element or a field of an element (`l[0].starts_with(f())`, `l[0].items.slice(0, f())`, `l[0][f()]`) gives the same value on the evaluator, the Rust target and the LLVM target when an argument or the index changes the list.

## Why

The adversary pass on unit `mysql-cost` (2026-11-12) wrote three programs that were wrong before that unit. Its tests are on branch `wave/13-mysql-cost-adv` (`edb6f127`), file `crates/ignite/tests/borrowed_reads_adversary.rs`:

- `adversary_a_read_only_method_on_an_element_whose_argument_changes_it`: `l[0].starts_with(..)` and `l[0].strip_prefix(..)` where the argument changes `l[0]`. The evaluator and the Rust target give 1 and 2; the LLVM target gives 2 and 100: it reads the receiver after the argument has run (`crates/ignite-llvm/src/lower.rs:4707`).
- `adversary_a_field_receiver_of_a_builtin_whose_argument_changes_the_list`: `l[0].items.slice(..)` with a bound that replaces the list does not compile on the Rust target (`error[E0506]`), at `1e16dfba` too (`crates/ignite-rustgen/src/lib.rs:2683`).
- `adversary_an_index_of_an_element_that_replaces_the_list`: `size(l[0][j])` where `j` assigns `l`. The evaluator reads the index first and gives 2; both targets read the receiver first and give 3.

`differential::the_evaluator_reads_a_receiver_before_its_arguments` pins the order for a receiver that is a place. No program in std or the examples uses the element forms.

## Acceptance

- `docs/slice.md` states the order for an element receiver and for an index chain (receiver first, as for a place, unless the note decides otherwise with its reason).
- The three tests are taken from the branch and are green on the evaluator and both targets.
- `task gate` green; `CODEGEN_REVISION` is bumped if generated code changes.

## Scope

- `crates/ignite-llvm/src/lower.rs` (`starts_with`, `strip_prefix` receivers), `crates/ignite-rustgen/src/lib.rs` (field-of-element receivers), the evaluator's index evaluation, `crates/ignite/tests/`. Inferred.
