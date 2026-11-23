---
format: aep.planning-md/3
id: story:range-pattern-bounds-resolve-any-constant-chain
kind: story
status: draft
title: A range pattern bound resolves through a constant chain of any length and order
revision: 1
---
# Story: a range pattern bound resolves through a constant chain of any length and order

## Outcome

`0..=EDGE` with typed constants `EDGE = ALIAS`, `ALIAS = BASE`, `BASE = 10` checks in every declaration order and for any chain length; a cycle among constants stays E015.

## Why

IG-16, severity medium, class logic; observed with a released command (the maintainer review of 2026-11-23 against main at a1d1407e, v0.16.4 archive).

With the three constants declared in the order above, a match on `0..=EDGE` failed with E045; with the declarations reversed it passed.

`bodies` runs `const_eval` over the integer constants exactly twice (`crates/ignite-types/src/typer.rs:1694` to `:1705`). `const_eval` (`:4787` to `:4807`) records a constant whose value is another constant only when that one is already known, so a chain of three declared from the far end needs three passes. `docs/slice.md:305` and `:306`: a constant may use other constants. The checker reports constant cycles as E015 (`crates/ignite-check/src/lib.rs:1752`).

## Acceptance

- A test, red first: chains of length one to five, declared in ascending and descending order, used as a range bound, all check.
- Controls: a bound that is no constant stays E045; a constant cycle stays E015 and the typer does not loop; `task gate` green.

## Scope

- `crates/ignite-types/src/typer.rs:1693` to `:1705` (`bodies`, cited `:1694`), `:4787` (`const_eval`, cited), `docs/slice.md:303` to `:311` (section 4.3, cited `:303`). Each read at a1d1407e, as the review states.
