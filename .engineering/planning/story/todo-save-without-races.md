---
format: aep.planning-md/3
id: story:todo-save-without-races
kind: story
status: active
title: The todo example removes superseded versions once
relations:
- decomposes: epic:native-parity
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T04:24:53Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T04:24:53Z", actor: "human:timo", revision: 3}
---
# Story: todo-save-without-races

## Outcome

The todo example's `save` removes only the version files it superseded, once, so parallel saves of one todo do not try to delete the same old files and the patch path makes no more syscalls per reply than the plain-Rust reference does for the same work.

## Context

- Epic epic:native-parity. After story:fs-resolve-beneath the patch path makes 45.8 syscalls per reply against 16.0; 8.1 of them are removes that fail because 16 server threads save the same todo at once and each tries to delete old versions another save already removed (3 syscalls each); the reference fails 0.3 per reply. Without them: 21.5 against 15.6 (1.38x).

## Acceptance

- The durable layout (`docs/slice.md` 22.4: `<shard>/<id>/<version>.json`, tombstones, newest version wins on load) and every todo test unchanged.
- Failed removes per reply under 1 in the patch-16 workload (`strace -c`); syscalls per reply within 1.3x of the reference; CPU per reply recorded before and after (method of the epic); `task gate` green.

## Scope

- `examples/todo/todo.ignite`. Inferred.
