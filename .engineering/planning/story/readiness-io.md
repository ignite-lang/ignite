---
format: aep.planning-md/3
id: story:readiness-io
kind: story
status: active
title: Connections served by readiness IO instead of a blocked thread each
relations:
- decomposes: epic:native-parity
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:59:06Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T06:03:41Z", actor: "human:timo", revision: 3}
---
# Story: readiness-io

## Outcome

The substrate serves connections from a small pool driven by epoll (or the planned async substrate of AGENTS.md) instead of one blocked OS thread per connection, so context switches per reply approach tokio's.

## Context

- Epic epic:native-parity (diagnosis and evidence in its body).

## Acceptance

- Design first in docs (`docs/vision.md` decision, both targets); then todo get-64 within 1.1x.
- Measured before and after locally with `perf stat` cost per reply against plain Rust (the method in the epic); the numbers go in the commit body; `task gate` green.

## Scope

- `crates/ignite-io`, `crates/ignite-rt`, both backends' task lowering, `docs/vision.md`. Inferred.
