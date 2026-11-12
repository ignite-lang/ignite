---
format: aep.planning-md/3
id: story:list-last-pop-reverse
kind: story
status: draft
title: A List has last, pop and reverse
relations:
- informed_by: story:list-contains-and-filter
revision: 1
---
# Story: a List has last, pop and reverse

## Outcome

A `List<T>` answers `last()`, `pop()` and `reverse()` as builtin methods, on the evaluator and both targets, with the same result everywhere.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: the last element is read as `xs[xs.len() - 1]`, a pop is written `xs = xs.slice(0, xs.len() - 1)`, and a reverse is a `while` loop that counts down. The builtin methods of `List` (`crates/ignite-types/src/builtins.rs` `list_method`) are `len`, `is_empty`, `at`, `slice`, `push`, `set`, `append`, `clear`, `map`, `join`, `to_text` and `index_of`.

## Acceptance

- `xs.last() -> Option<T>`: the last element, `None` on an empty list.
- `xs.pop() -> Option<T>` removes and returns the last element of a `var` list, `None` on an empty list; on a list that is not a place it is the diagnostic `push` gives there.
- `xs.reverse() -> List<T>` returns the elements in reverse order; the receiver is unchanged.
- A property test on generated lists: `xs.reverse().reverse() == xs`; `push(x)` then `pop()` gives `Some(x)` and the list as before.
- Allocations on the Rust target: `last` at most the copy of the element, `pop` 0, `reverse` one list and one copy per element; counted at two sizes; the LLVM target's counts are written down.
- `docs/slice.md` and `site/book` list the methods; `task gate` green.

## Scope

- `crates/ignite-types/src/builtins.rs`, `crates/ignite-check/src/lib.rs` (the name list and `IN_PLACE` for `pop`), `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, `docs/slice.md`. Inferred.

## Not in this story

- `fold`, `zip`, `enumerate` (`story:list-contains-and-filter` excludes them too).
- A dedup that keeps the first occurrence in order: membership is `story:std-set`; a `unique()` on a list gets its own story when a second requester names it.
