---
format: aep.planning-md/3
id: story:deployment-resolution-indexes-members-by-id
kind: story
status: draft
title: Deployment resolution looks members up through an index
revision: 1
---
# Story: deployment resolution looks members up through an index

## Outcome

Resolving an assembly builds one index by member id and uses it for every key and every member, so the lookups grow linearly with members and keys.

## Why

IG-34, severity medium, class performance; an operation count derived from source (the maintainer review of 2026-11-23 against main at a1d1407e). No wall-clock time was measured.

Each key finds its member by a linear scan (`crates/ignite-eval/src/resolve.rs:189`, `SystemInfo::member` at `crates/ignite-types/src/lib.rs:1464` to `:1466`). The `host` closure scans the key list twice (`resolve.rs:291`, `:296` to `:299`) and runs for every resource and component (`:304`). For N members with one direct binding each, that is N(N+1) key comparisons.

## Acceptance

- A counting test on a one-to-one binding fixture, red first: lookups linear in N.
- Controls: E111 and E112 and the resolution output unchanged; `task gate` green.

## Scope

- `crates/ignite-eval/src/resolve.rs:189`, `:291`, `crates/ignite-types/src/lib.rs:1464` (cited). Each read at a1d1407e, as the review states.
