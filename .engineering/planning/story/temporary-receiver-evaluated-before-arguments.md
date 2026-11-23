---
format: aep.planning-md/3
id: story:temporary-receiver-evaluated-before-arguments
kind: story
status: active
title: A temporary receiver is evaluated before the arguments on every target
relations:
- informed_by: story:element-receiver-read-before-arguments
- informed_by: story:eval-receiver-order
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-23T03:51:03Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-23T03:51:03Z", actor: "human:timo", revision: 3}
---
# Story: a temporary receiver is evaluated before the arguments on every target

## Outcome

For a builtin method whose receiver is a temporary (a call result), the receiver runs before the arguments on the evaluator, the Rust target and the LLVM target, for mutating builtins (`push`, `put`) and read-only ones (`get`, `contains`) alike, and the receiver value is kept through the arguments.

## Why

IG-06, severity high, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

Synthetic reproducer: an order counter to which the temporary receiver appends digit 1 and the argument appends digit 2. For a list `push`, the Rust target recorded 12; the evaluator (`ignite plan`) and the LLVM target recorded 21. A map `get` returned the same item on both targets but left the counter at 12 on the Rust target and 21 on the LLVM target.

The evaluator's `mutating` evaluates the arguments (`crates/ignite-eval/src/eval.rs:1623`) before a temporary receiver (`:1641`); the review cites `:1622`, the place lookup just before. The LLVM target lowers the pushed element (`crates/ignite-llvm/src/lower.rs:4862`) before the receiver (`:4866`), and the map key of `get` (`:4681`) before the receiver (`:4682`); `contains` does the same (`:4688`, `:4689`).

`story:eval-receiver-order` (archived) made the evaluator read a builtin receiver first for the text cases; `story:element-receiver-read-before-arguments` is about element receivers. Neither covers a temporary receiver.

## Acceptance

- A differential program, red first: `push`, `put`, `get` and `contains` on a temporary receiver with the counter give 12 on the evaluator and both targets.
- Controls: a receiver that is a place keeps its current order (`differential::the_evaluator_reads_a_receiver_before_its_arguments`); `docs/slice.md` states receiver first for a temporary receiver.
- `task gate` green; `CODEGEN_REVISION` bumped if generated code changes.

## Scope

- `crates/ignite-eval/src/eval.rs:1614` to `:1644` (`mutating`), `crates/ignite-llvm/src/lower.rs:4666` to `:4700` (`builtin`, map arms, cited `:4681`), `:4856` to `:4866` (`ListPush`, cited `:4862`).
- Each cited line read at a1d1407e, as the review states (`:1622` is one line before the argument evaluation).
