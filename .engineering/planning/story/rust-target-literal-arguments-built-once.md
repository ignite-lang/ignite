---
format: aep.planning-md/3
id: story:rust-target-literal-arguments-built-once
kind: story
status: active
title: A literal list or enum value passed to a read-only parameter is built once on the Rust target
relations:
- informed_by: story:rust-target-constants-built-once
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T20:27:00Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
- {from: "proposed", to: "active", at: "2026-11-12T20:27:00Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-12"}
---
# Story: a literal list or enum value passed to a read-only parameter is built once on the Rust target

## Outcome

On the Rust target a list, tuple, record or enum literal whose every part is a literal, passed where it is only read, is built once per process, as a text literal in that position already is.

## Why

Reported on 2026-11-12 by the first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS, Rust target, release, v0.16.2): its generated projections call a selector with a literal path for every key of every row, and each call builds the path again.

A synthetic program of 90 lines, `ignite emit` at `1febe156` (v0.16.2 plus documents):

- `find(fields, "x")` with `key: Text` read-only is emitted as `find(.., { static TEXT: ::std::sync::LazyLock<String> = ::std::sync::LazyLock::new(|| String::from("x")); &*TEXT })`: built once, inside a closure too.
- `sel(r, [F("col_a")])` with `path: List<P>` read-only and `enum P { F(Text)  I(Int) }` is emitted as `sel(&r, &vec![P::F(String::from("col_a"))])`: two allocations per call for a value that never differs.

`story:rust-target-constants-built-once` builds a named constant once (`OnceLock`, read by reference); this story gives the same form to a literal without a name.

## Acceptance

- A count test, red first: a function that calls `sel(r, [F("col_a")])` in a loop allocates 0 times per round after the first on the Rust target (today two); the same for a list of two paths, a tuple of literals, a record literal of literals and a nested enum payload.
- A literal with a part that is not a literal (`[F(name)]`, `[x, "a"]`) is built per call as before; a literal in a value position (a by-value argument, a `let` that is changed, a `return`) is built per use as before or copied once from the one built once, whichever counts less: both counts are stated.
- The evaluator and the LLVM target give the same values; the LLVM target's count for the same calls is written down.
- The size of the generated code is stated: stripped size of `todod` and `terminald` before and after (row 3.9's method), and what 100 such literals cost a program.
- No new `unsafe` site; the form is stated in `docs/slice.md` section 6; `CODEGEN_REVISION` is bumped; `task gate` green; row 3.10 holds; a dedicated-host run shows no row worse.

## Scope

- `crates/ignite-rustgen/src/lib.rs` (literal arguments of reference parameters), `crates/ignite-rustgen/tests/golden/`, `crates/ignite/tests/` (count test), `docs/slice.md` section 6. Inferred.

## Not in this story

- A text literal in a value position and a text copied into a field (`story:rust-target-text-literals-and-field-copies`).
