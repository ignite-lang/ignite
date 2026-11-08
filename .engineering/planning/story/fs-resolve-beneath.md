---
format: aep.planning-md/3
id: story:fs-resolve-beneath
kind: story
status: active
title: Directory operations resolve a path in one syscall
relations:
- decomposes: epic:native-parity
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:59:05Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T03:59:07Z", actor: "human:timo", revision: 3}
---
# Story: fs-resolve-beneath

## Outcome

A `Directory` operation resolves its path with one `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)` (the openat walk only where openat2 is missing) and shares the parent directory handle, so the todo patch path makes about as many syscalls as plain Rust.

## Context

- Epic epic:native-parity (diagnosis and evidence in its body).

## Acceptance

- The symlink refusal and every `fs` test of `docs/slice.md` 21.4 still hold; a test forces the fallback walk.
- todo patch-16 syscalls per reply within 1.3x of the reference (`strace -c`), CPU per reply recorded.
- Measured before and after locally with `perf stat` cost per reply against plain Rust (the method in the epic); the numbers go in the commit body; `task gate` green.

## Scope

- `crates/ignite-io/src/fs.rs`, `docs/slice.md`. Inferred.
