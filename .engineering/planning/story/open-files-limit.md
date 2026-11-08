---
format: aep.planning-md/3
id: story:open-files-limit
kind: story
status: active
title: Programs raise the open-files limit at start
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T01:26:00Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T01:26:48Z", actor: "human:timo", revision: 3}
---
# Story: open-files-limit

## Outcome

A compiled program raises its open-files soft limit to the hard limit at start, so a server holds as many connections as the host allows without code.

## Context

- Case study gap G10: no `setrlimit`; the container default caps connections under the benchmark; the reference implementation raises `RLIMIT_NOFILE` at start.

## Acceptance

- `ignite-io::run` raises `RLIMIT_NOFILE` soft to hard before `main` on both targets; a failure is ignored and reported once on stderr only when the soft limit stays under 4,096.
- A test starts a built program under `prlimit --nofile=256:65536` and reads `/proc/<pid>/limits`: soft 65536.
- `Process.open_files_limit() -> Int` reads the current soft limit (test-visible).
- `docs/slice.md` section; `task gate` green.

## Scope

- `crates/ignite-io` (run, a `process` builtin), `crates/ignite-rt`, `crates/ignite-types`/`ignite-check` builtins, both backends, `std/process.ignite`, `docs/slice.md`. Inferred.
