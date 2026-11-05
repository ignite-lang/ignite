---
format: aep.planning-md/3
id: story:store-hosted
kind: story
status: draft
title: A Store resource is hosted by the process that creates it
relations:
- decomposes: epic:slice-18-one-service-operated
- depends_on: story:slice-18-contract
scope:
- confidence: cited
  path: crates/ignite-eval/src/resolve.rs
- confidence: cited
  path: crates/ignite/tests/plan.rs
revision: 3
---
# Story: store-hosted

## Outcome

A `Store` resource needs no binding: it is hosted implicitly by its server, as a capability (D6).

## Context

- Unit U2 of `docs/slice.md` 22.13 (branch `feat/slice18-one-service`, contract at `19c70564`): owned files, wave and shared-file rules are there; decisions in 22.12.
- Depends on: story:slice-18-contract.

## Acceptance

- The first failing test `plan::store_is_hosted_by_its_server` is red before and green after, on both targets where it has a target.
- Rows 10 of 22.0 pass with the exact results the table names; rows 8 and 31 hold; `task gate` green.

## Out of Scope

- Files 22.13 gives to another unit.
