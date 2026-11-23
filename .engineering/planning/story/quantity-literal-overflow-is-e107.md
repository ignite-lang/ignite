---
format: aep.planning-md/3
id: story:quantity-literal-overflow-is-e107
kind: story
status: draft
title: A quantity literal that overflows while scaled is E107
revision: 1
---
# Story: a quantity literal that overflows while scaled is E107

## Outcome

A quantity literal whose value overflows while it is scaled to base units is reported as E107, never a panic of the checker and never a wrapped value.

## Why

IG-24, severity medium, class logic; traced in source only (the maintainer review of 2026-11-23 against main at a1d1407e). No literal was run.

The lexer keeps a decimal mantissa before a unit as its digits (`crates/ignite-syntax/src/lexer.rs:491` to `:495`, `decimal_before_unit`). `quantity` parses whole and fraction digits as `i128` and multiplies by the unit factor with plain `*` (`crates/ignite-types/src/typer/system.rs:236` to `:244`, the product at `:241`) before `i64::try_from`. A mantissa that fits `i128` can overflow it once scaled; the largest factor is `day` at 86,400,000,000,000 ns (`crates/ignite-syntax/src/ast.rs`, `quantity_unit`). A shape such as a 26-digit mantissa with `.0 days` (synthetic, not run) overflows: with overflow checks the checker panics, without them it wraps before the range test.

## Acceptance

- A test, red first in a build with overflow checks: such a literal gives E107 with the existing message and no panic.
- Controls: the existing precision (more than 4 decimals) and range tests of E107; the largest valid literal of each kind still checks; `task gate` green.

## Scope

- `crates/ignite-syntax/src/lexer.rs:495` (cited), `crates/ignite-types/src/typer/system.rs:226` to `:273` (`quantity`, cited `:241`). Each read at a1d1407e, as the review states.
