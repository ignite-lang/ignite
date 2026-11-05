---
format: aep.planning-md/3
id: story:durable-todo-service
kind: story
status: active
title: The todo service keeps its data in a Directory and is probed over HTTP
relations:
- decomposes: epic:slice-18-one-service-operated
- depends_on: story:slice-18-contract
- depends_on: story:typed-probe-outcomes
- depends_on: story:store-hosted
- depends_on: story:slice-17-records
scope:
- confidence: cited
  path: crates/ignite/tests/golden
- confidence: cited
  path: crates/ignite/tests/todo.rs
- confidence: cited
  path: examples/todo
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T12:43:29Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-11-05T12:43:30Z", actor: "human:timo", revision: 6}
---
# Story: durable-todo-service

## Outcome

`examples/todo` stores one file per todo version in a `Directory` and a todo survives SIGKILL; an HTTP probe kind and its outcome type in `examples/todo/health.ignite` feed an `Availability` contract (D4, D7, D13).

## Context

- Unit U4 of `docs/slice.md` 22.13 (branch `feat/slice18-one-service`, contract at `19c70564`): owned files, wave and shared-file rules are there; decisions in 22.12.
- Depends on: story:typed-probe-outcomes, story:store-hosted, slice 17.

## Acceptance

- The first failing test `todo::rust::todos_survive_sigkill` is red before and green after, on both targets where it has a target.
- Rows 9 (check), 11 to 15 of 22.0 pass with the exact results the table names; rows 8 and 31 hold; `task gate` green.

## Out of Scope

- Files 22.13 gives to another unit.
