---
format: aep.planning-md/3
id: story:place-receiver-read-before-index-and-prefix-argument
kind: story
status: implemented
title: An index or a prefix check reads its receiver before an argument that replaces it
relations:
- informed_by: story:element-receiver-read-before-arguments
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:51:02Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-23T03:51:02Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-23T21:50:06Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: an index or a prefix check reads its receiver before an argument that replaces it

## Outcome

`l[f()]` where `f` replaces the local list `l`, and `t.starts_with(g())` where `g` replaces the local text `t`, give the value of the receiver as it was before the argument ran, on the evaluator and on both targets, and the Rust target compiles the index form.

## Why

IG-05, severity high, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

Synthetic reproducer: a list holding `[10]` whose index expression replaced the list with `[20]` and returned zero. The LLVM target returned 20 instead of 10; the Rust target rejected the generated borrow with E0506. A text `"cat"` whose `starts_with` argument replaced it with `"dog"` gave false on the LLVM target.

`list_at` lowers the receiver as a borrowed pointer (`crates/ignite-llvm/src/lower.rs:4115`), then the index (`:4120`), and reads the element through that pointer. `TextStartsWith` lowers the receiver (`:4771`) before the prefix argument (`:4773`). The Rust target lends the receiver in `checked_at_call` (`crates/ignite-rustgen/src/lib.rs:4982` to `:4986`) and then evaluates the index (`:4987`). `list_slice` already copies a borrowed receiver when a bound changes a variable (`lower.rs:4262` to `:4281`).

`story:element-receiver-read-before-arguments` owns the element forms (`l[0].starts_with(f())`, `l[0][f()]`, `l[0].items.slice(..)`). This story is the receiver that is a local itself. The LLVM `starts_with` change is likely shared: whichever story lands first makes it, and the other keeps its own tests.

## Acceptance

- Differential tests, red first: the index form gives 10 and the prefix form gives true on the evaluator, the Rust target and the LLVM target; the Rust target compiles both.
- Controls: an index or prefix argument that changes no variable keeps the borrowed path (no copy of the receiver; the path-cost cells of `docs/quality.md` row 3.10 not worse); `strip_prefix` with the same shape gives the old receiver too.
- `task gate` green; `CODEGEN_REVISION` bumped if generated code changes.

## Scope

- `crates/ignite-llvm/src/lower.rs:4109` to `:4135` (`list_at`, cited `:4115`), `:4770` to `:4778` (`TextStartsWith`, cited `:4771`), `:4258` to `:4281` (`list_slice`, the model, cited `:4264`).
- `crates/ignite-rustgen/src/lib.rs:4981` to `:4993` (`checked_at_call`, cited `:4982`).
- Each cited line read at a1d1407e, as the review states.
