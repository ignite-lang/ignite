---
format: aep.planning-md/3
id: story:g30-reproducer-build-dir-guarded-across-processes
kind: story
status: draft
title: The G30 reproducer's build directory is guarded across processes
summary: leak-repro is shared under an in-process Mutex only; the ETXTBSY of the leak tests can happen there
revision: 1
---
# Story: the G30 reproducer's build directory is guarded across processes

## Outcome

`leak::g30_reproducer_stays_flat_in_every_mode` builds into its shared `leak-repro` directory under a guard that holds across test processes, as the other leak tests do since 2ec34bec.

## Why

The leak tests shared `CARGO_TARGET_TMPDIR/leak/<target>` under a `Mutex` of one test process; a second test process of the same tree running the shared executable made the copy over it fail with ETXTBSY ("Text file busy", wave/17 gate, 2026-11-24). 2ec34bec replaced that `Mutex` with a file lock held for the whole `ignite test`. `g30_reproducer_stays_flat_in_every_mode` builds into `leak-repro` under the same in-process-only protection, so the same failure can happen there.

## Acceptance

- A test starts a second process that holds the reproducer's executable running; a run of `g30_reproducer_stays_flat_in_every_mode` started meanwhile waits and passes (it fails with ETXTBSY before the change).

## Scope

- `crates/ignite/tests/leak.rs`. Inferred.
