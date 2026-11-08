---
format: aep.planning-md/3
id: story:drain-keeps-bound-listeners
kind: story
status: proposed
title: A listener bound at run time can be kept through the drain
summary: 'Case study G31: keep_through_drain for TcpListener.bind'
relations:
- informed_by: story:drain-keeps-listeners
scope:
- confidence: inferred
  path: crates/ignite-check/src/lib.rs
- confidence: inferred
  path: crates/ignite-llvm/src/lower.rs
- confidence: inferred
  path: crates/ignite-rt/src/net.rs
- confidence: inferred
  path: crates/ignite-rustgen/src/lib.rs
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
- confidence: inferred
  path: crates/ignite/tests/drain.rs
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: site/book/src/reference/std/net.md
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T15:00:48Z", actor: "human:timo", revision: 3}
---
# Story: drain-keeps-bound-listeners

## Outcome

A listener bound at run time, with an address and port the program reads from its environment or configuration, can be kept through the SIGTERM drain on both targets, as a `TcpListener` resource with `keep_through_drain = true` is.

## Context

- Case study gap G31 (`docs/research/case-study-gaps.md`), on v0.15.0: `keep_through_drain` (story `drain-keeps-listeners`, G6, `docs/slice.md` 16.13) exists only as a property of a `TcpListener` resource whose port is fixed in the system declaration (the Rust target needs it as a literal, E020). `TcpListener.bind(address, port)` always registers with the drain, so a service whose admin or internal port comes from an environment variable re-binds it after SIGTERM, the G6 workaround.
- Where it lives: `crates/ignite-io/src/net.rs` (`TcpListener::bind`, `bind_kept`, `bind_with`; `bind_kept` already exists), `crates/ignite-io/src/drain.rs` (a kept listener is never registered); LLVM runtime `crates/ignite-rt/src/net.rs` (`ignite_rt_tcp_bind`, `ignite_rt_tcp_bind_kept`); the builtin `TcpListenerBind` in `crates/ignite-check/src/lib.rs` (tables of static functions and effects), `crates/ignite-types`, `crates/ignite-llvm/src/lower.rs` and `crates/ignite-rustgen/src/lib.rs`; the resource path in `crates/ignite-llvm/src/lib.rs` and `crates/ignite-rustgen/src/lib.rs` (`keep_through_drain`).
- Proposed surface: `TcpListener.bind_kept(address: Text, port: Int) -> Result<TcpListener, IoError>`, effect `net.listen` as `bind`; the contract may choose another spelling in `docs/slice.md` 16.13.

## Acceptance

- `crates/ignite/tests/drain.rs` (or its successor) builds a program on each target that binds one listener with `TcpListener.bind` and one with the new form, both on ports read at run time, and sends SIGTERM: the first is `released`, the second prints no `released` line, keeps accepting and answering, and a task blocked in its `accept` does not hold up the exit, as 16.13 states for a kept resource.
- `docs/slice.md` 16.13 and the book's `std.net` page state the form; `task gate` green.

## Scope

- `crates/ignite-check/src/lib.rs`, `crates/ignite-types/src`, `crates/ignite-llvm/src/lower.rs`, `crates/ignite-rustgen/src/lib.rs`, `crates/ignite-rt/src/net.rs`, `crates/ignite/tests/drain.rs`, `docs/slice.md`, `site/book/src/reference/std/net.md`, `crates/ignite/src/apply/plan.rs` (`CODEGEN_REVISION`). Inferred.
