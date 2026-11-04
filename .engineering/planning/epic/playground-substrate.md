---
format: aep.planning-md/3
id: epic:playground-substrate
kind: epic
status: active
title: Processes and files for Ignite programs
summary: Start child processes and read and write files under a root, so an Ignite server can host the playground
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T01:57:17Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-04T01:57:17Z", actor: "human:timo", revision: 3}
---
# Epic: playground-substrate

## Outcome

An Ignite program can start child processes and read and write files under a root directory, so
a server written in Ignite can host the Ignite playground: a browser page with a file tree, an
editor, a run button and an output pane, whose run button calls `ignite check|test|build` and then
the built program.

## Context

- Requested by Timo through the playground session (2026-11-04); the playground lives outside this
  repository and resumes once both stories land.
- At `9578530` the substrate (`crates/ignite-io/src`) offers TCP, the pipe, stdio, tasks, the
  store, the clock, arguments and exit; it has no file system and no process module.
- The HTTP side exists: `std.protocol.http` `HttpServer` serves a page and a JSON API.
- Layering (`AGENTS.md`, `docs/layering.md`): the substrate is implemented once in
  `crates/ignite-io` and both targets call it; protocols and policy stay Ignite in `std/`. Each new
  capability is an effect the checker tracks, as `stdin`, `stdout` and the socket effects are.

## Acceptance

- `story:process-spawn` and `story:file-system` are implemented on both targets.
- A slice contract in `docs/slice.md` names the effects, the std modules and the substrate
  functions before implementation starts.

## Out of Scope

- The playground itself; sandboxing beyond a root directory and a timeout.
