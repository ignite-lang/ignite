---
format: aep.planning-md/3
id: story:local-inference-stops-substituting-once-resolved
kind: story
status: draft
title: Local inference stops substituting a type once it is resolved
revision: 1
---
# Story: local inference stops substituting a type once it is resolved

## Outcome

Local inference applies the solved holes to a binding hint until the hint has no hole or stops changing, so N independent inferred locals cost a number of substitutions linear in N.

## Why

IG-31, severity medium, class performance; an operation count derived from source (the maintainer review of 2026-11-23 against main at a1d1407e). No wall-clock time was measured.

`with_local_inference` applies the solved map to each hinted binding `solved.len() + 1` times (`crates/ignite-types/src/typer.rs:576` to `:584`, the loop at `:578`), and each `apply` rebuilds the type (`:235` to `:237`). For N independent empty-list locals each fixed by a later `push(1)`, that is N(N+1) type walks.

## Acceptance

- A counting test, red first: N independent locals cost at most 2N substitutions.
- Controls: a chained constraint (a hole resolved through another hole) still resolves; `task gate` green.

## Scope

- `crates/ignite-types/src/typer.rs:576`, `:235` (cited). Each read at a1d1407e, as the review states.
