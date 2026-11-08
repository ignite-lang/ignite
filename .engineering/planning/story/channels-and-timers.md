---
format: aep.planning-md/3
id: story:channels-and-timers
kind: story
status: implemented
title: Bounded channels with a deadline replace loopback bells
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T01:25:58Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T01:26:48Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-08T06:51:08Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: channels-and-timers

## Outcome

A task can wait on a bounded `Channel<T>` with a deadline and wake when another task sends, so a long-poll, a heartbeat, a ping interval or a drain window needs no loopback socket pair.

## Context

- Case study gap G3 (`docs/research/case-study-gaps.md`): the service's "bells" cost two fds and a loopback connect per waiter; `Clock.sleep` landed in wave 6 (`docs/slice.md` 21.20), the waitable signal did not.
- `docs/actors.md` plans channels inside actors; this story is the substrate primitive they can build on.

## Acceptance

- `Channel.bounded<T>(capacity)` gives a sender and a receiver; `send` blocks when full, `try_send` returns false; `receive_within(duration) -> Option<T>` returns `None` at the deadline; a closed channel ends every waiter.
- Senders and receivers move into spawned tasks and are checked by the moves analysis; values are moved, not shared.
- G3's reproducer rewritten without bells passes on both targets; a test with 1,000 waiters woken by one sender holds no extra fd (`/proc/self/fd` count).
- `docs/slice.md` section; `CODEGEN_REVISION` bumped; `task gate` green.

## Scope

- `crates/ignite-io` (channel over a mutex and condvar), `crates/ignite-rt` (C ABI), `crates/ignite-types` and `ignite-check` (builtins), both backends, `std/process.ignite` or a new `std/sync.ignite`, `docs/slice.md`. Inferred.
