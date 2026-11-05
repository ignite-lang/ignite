---
format: aep.planning-md/3
id: story:task-join
kind: story
status: draft
title: Join spawned tasks
summary: spawn returns a handle whose join waits for the task and yields its result, so main does not end its tasks
scope:
- confidence: inferred
  path: crates/ignite-check/src
- confidence: inferred
  path: crates/ignite-io/src
- confidence: inferred
  path: crates/ignite-llvm/src
- confidence: inferred
  path: crates/ignite-rt/src
- confidence: inferred
  path: crates/ignite-rustgen/src
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: std
revision: 2
---
# Story: task-join

## Outcome

A program waits for the tasks it spawned: `spawn` gives a handle whose `join` blocks until the task
ends and yields its result, on both targets, so `main` does not end its own tasks by returning.

## Context

- Row E8 found by the playground session (2026-11-04, compiler `9578530`), `examples/friction.md`
  in the sibling examples repository: the process ends when `main` returns (`ignite-io` `run`
  calls `std::process::exit`), so a load generator that spawned N client tasks from `main` printed
  nothing. Workaround: `main` blocks in `accept` on its own loopback listener and the last client,
  picked by a ticket from `Store.next_id`, connects to it.
- `docs/actors.md` plans typed actors and supervision but names no join of a plain task; this
  story is the small step before that wave, or becomes part of it when actors are contracted.
- `ignite-io::spawn` owns the task threads (`AGENTS.md` lifecycle ownership).

## Acceptance

- An inline test on both targets spawns three tasks that each write a line, joins all three, and
  sees all three lines before `main` returns.
- A joined task that ended by an `assert` or overflow reports that to the joiner instead of
  ending it.

## Out of Scope

- Supervision, restart and cancellation (actors wave).
