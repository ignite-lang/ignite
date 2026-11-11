---
format: aep.planning-md/3
id: story:http-reply-cost-back
kind: story
status: active
title: An HTTP reply allocates no more than at v0.15.0, and a server without held requests starts no task for them
scope:
- confidence: inferred
  path: crates/ignite-io/src/net.rs
- confidence: inferred
  path: crates/ignite-rustgen/src/lib.rs
- confidence: cited
  path: std/protocol/http.ignite
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T00:24:55Z", actor: "human:timo", revision: 4, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T00:24:55Z", actor: "human:timo", revision: 5, executor: "agent:claude-coordinator", correlation: "wave-11"}
---
# Story: an HTTP reply allocates no more than at v0.15.0, and a server without held requests starts no task for them

## Outcome

Row 3.3 `rust/todo/get-1` is at or better than v0.15.0 on the dedicated host, and the todo server
holds one task and one worker fewer at rest (row 3.8 `todo/get-1`).

## Why

Dedicated-host run `iq-20261110t211023z-863a` (HEAD `4520b8cf`, BASE v0.15.0): 3.3
rust/todo/get-1 0.971x -> 0.941x; 3.8 llvm/todo/get-1 +8.5 %, rust/todo/get-1 +7.4 %.

Diagnosis (`.engineering/waves/2026-11-11-wave-11-diagnosis/rss-rr.md`):

- System calls per reply are equal at v0.15.0 and now; allocations per reply on the Rust target
  went 37 -> 53. Inferred, not shown: the `connection` field `Request` gained with
  `story:http-app-connection`.
- `serve_listener` in `std/protocol/http.ignite` starts the `Holds` task (`keep_holds`) for every
  server; the todo example holds no request. It costs one live task, one worker and 48 to 84 KiB
  of anonymous memory. A patch that starts it only when the application has a `reply` was tried
  on the LLVM target: 28 to 56 KiB and one thread fewer.

## Acceptance

- A test counts the allocations of one `GET` reply of the todo example on the Rust target (the
  census's counters, or the counting allocator in a test) and fails above 37. Red on the wave
  base at 53.
- A server whose application has no `reply` starts no `Holds` task: a test reads the census (or
  the task count) of such a server at rest. A server whose application holds requests behaves as
  before; the inline tests of `std/protocol/http.ignite` pass with unchanged names.
- `Request` still gives a handler its `Connection` (peer, local, tls).
- The dedicated-host run of the release candidate shows 3.3 `rust/todo/get-1` and 3.8
  `todo/get-1` at or better than BASE. The coordinator reads this.

## Scope

- `std/protocol/http.ignite` (`serve_listener`, `Request`, `Connection`). Cited.
- `crates/ignite-rustgen/src` or `crates/ignite-io/src` where a `Request` is built, if the
  allocations are there. Inferred.
- Runs before `story:ignored-tests-and-long-files-back` in the same slot: both change
  `std/protocol/http.ignite`.
