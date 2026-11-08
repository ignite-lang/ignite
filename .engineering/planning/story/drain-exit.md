---
format: aep.planning-md/3
id: story:drain-exit
kind: story
status: active
title: Process.exit during the drain exits with its code, without waiting for tasks
scope:
- confidence: inferred
  path: crates/ignite-io/src
- confidence: inferred
  path: crates/ignite-llvm/src
- confidence: inferred
  path: crates/ignite-rt/src/process.rs
- confidence: inferred
  path: docs/slice.md
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T22:06:09Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-07T22:06:09Z", actor: "human:timo", revision: 3}
---
# Story: drain-exit

## Outcome

`Process.exit(code)` during the SIGTERM drain ends the process with `code` without waiting for live tasks, on both targets, as `ignite_io::exit` documents; writes made before it are flushed.

## Context

- Found by the case-study case study (session `a case-study session`, GitLab `the case study's Ignite candidate`, G16 and the G13 update in `docs/research/case-study-gaps.md`), ignite v0.13.0 (`33b59123`), LLVM target; the Rust target not checked.
- Site: `Process.exit` lowers to `ignite_rt_exit` (`crates/ignite-llvm/src/lower.rs:4607`); while draining it calls `drained()`, which runs `wait_for_tasks()` unbounded and then `exit(0)` (`crates/ignite-rt/src/process.rs:307-348`). `ignite_io::exit` (`crates/ignite-io/src/lib.rs:171-182`) says other tasks are not waited for.
- Reproducer: `docs/research/case-study-gaps/G16.ignite` with its log `g16.log`: after SIGTERM the log shows `drain began`, two `released`, `calling Process.exit(3)`, and the process stays up until SIGKILL.
- G13 update: `close()` sends only its own stream (`crates/ignite-io/src/net.rs:438`); a write just before `main` returns while draining is never sent because `ignite_rt_exit` waits without flushing.
- What reaches it: any server that calls `Process.exit` after SIGTERM while a task waits in `accept`, a read or a sleep; the case-study's `task candidate:run` hung.

## Acceptance

- G16 as a test on both targets: SIGTERM, then `Process.exit(3)` with tasks blocked in `accept` and an idle read; the process exits 3 within 1 s.
- A write followed by `Process.exit` during the drain reaches the peer (test on both targets).
- `docs/slice.md` states what exit does during the drain; `task gate` green.
