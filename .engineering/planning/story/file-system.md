---
format: aep.planning-md/3
id: story:file-system
kind: story
status: implemented
title: Files under a root directory
summary: Read, write, list, create directories and remove files under a root, never outside it
relations:
- decomposes: epic:playground-substrate
- depends_on: story:slice-17-frontend
scope:
- confidence: cited
  path: crates/ignite-io/src/fs.rs
- confidence: cited
  path: crates/ignite-rt/src/fs.rs
- confidence: cited
  path: std/fs.ignite
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T05:11:07Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-11-05T05:11:07Z", actor: "human:timo", revision: 8}
- {from: "active", to: "implemented", at: "2026-11-05T06:27:17Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1}}}
---
# Story: file-system

## Outcome

An Ignite program reads, writes, lists, creates directories and removes files under a root
directory it is given, on both targets, and cannot reach a path outside that root.

## Context

- Gap P2 of the playground session (2026-11-04): its server holds the user's `.ignite` files.
- Substrate in `crates/ignite-io`, a std module over it, a new effect the checker tracks; the root
  is a capability value, as a listener is.
- Constraint P3 (playground session, 2026-11-04): an `HttpApp` handler may use effects only on its state (`std/protocol/http.ignite:50`; `Clock.monotonic()` in a handler is E017, also in a task it spawns), so this capability arrives as a handle passed in the state, as `Store` does, not as an ambient static.
- Contract: `docs/slice.md` section 21 on `feat/slice17-processes-files`, unit U2 of 21.12, rows 1 to 7 of 21.0; first failing test `fs::symlinks_out_are_refused`; after `story:slice-17-frontend`.

## Acceptance

- An inline test on both targets writes a file, reads it back byte for byte, lists the directory,
  creates a subdirectory and removes both.
- A path that leaves the root (`..`, an absolute path, a symlink out) is refused with an error
  value, not an IO.
- A function that touches files without declaring the effect is rejected by `ignite check`.

## Out of Scope

- File watching, permissions, memory-mapped files.
