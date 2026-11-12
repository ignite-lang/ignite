---
format: aep.planning-md/3
id: story:option-try-in-fns
kind: story
status: active
title: '? on an Option in a fn that returns an Option'
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T20:26:59Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
- {from: "proposed", to: "active", at: "2026-11-12T20:26:59Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
---
# Story: `?` on an Option in a fn that returns an Option

## Outcome

In a fn or closure that returns `Option<U>`, `e?` with `e: Option<T>` has type `T` and returns `None` when `e` is `None`, on the evaluator and both targets.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite: `?` works on a `Result` only (E040, `crates/ignite-types/src/typer.rs` `try_`), so each optional step is a `match … None => return None`. A test body already accepts `?` on an `Option` (`docs/slice.md` section 12.3). `docs/syntax.md` P3 proposes the rule and counts the lines it saves in std.

## Acceptance

- `fn f(..) -> Option<U>` with `e?` on an `Option<T>` checks, and gives `None` for `None` and the payload otherwise (one differential program: evaluator, Rust target, LLVM target).
- In a closure `?` returns from the closure.
- `?` on an `Option` in a fn that returns a `Result`, and the reverse, stays E040; the diagnostic names both types and says there is no conversion.
- The std fns `docs/syntax.md` P3 lists are rewritten with `?` and their tests pass unchanged.
- `docs/slice.md` states the rule; `docs/syntax.md` marks P3 as done; `task gate` green.

## Scope

- `crates/ignite-types/src/typer.rs` (`try_`), `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-llvm/`, the evaluator, `std/json.ignite`, `std/text.ignite`, `docs/slice.md`, `docs/syntax.md`. Inferred.

## Not in this story

- A conversion between `Option` and `Result` inside `?`.
- `?` on `None` answering 404 in a route fn (`story:route-annotations`).
