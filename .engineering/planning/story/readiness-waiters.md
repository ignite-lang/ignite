---
format: aep.planning-md/3
id: story:readiness-waiters
kind: story
status: active
title: Every substrate wait goes through one waiter that parks a thread or a task
relations:
- decomposes: epic:native-parity
- decomposes: story:readiness-io
scope:
- confidence: inferred
  path: crates/ignite-io/Cargo.toml
- confidence: cited
  path: crates/ignite-io/src/channel.rs
- confidence: cited
  path: crates/ignite-io/src/child.rs
- confidence: inferred
  path: crates/ignite-io/src/clock.rs
- confidence: inferred
  path: crates/ignite-io/src/drain.rs
- confidence: inferred
  path: crates/ignite-io/src/net.rs
- confidence: cited
  path: crates/ignite-io/src/pipe.rs
- confidence: inferred
  path: crates/ignite-io/src/reactor.rs
- confidence: cited
  path: crates/ignite-io/src/task.rs
- confidence: cited
  path: crates/ignite-io/src/wait.rs
- confidence: inferred
  path: crates/ignite-io/tests
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T08:45:33Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-08T08:45:33Z", actor: "human:timo", revision: 5}
---
# Story: readiness-waiters

## Outcome

Every place `crates/ignite-io` waits (the `Condvar`s of channels and pipes, child `wait`, the task handshake, `Clock.sleep`, a blocking socket call) goes through one `Waiter` with `park` and `wake`, and readiness comes from one reactor interface with the hand-written epoll implementation behind it, so a later reactor (tokio-driven) or a later waiter kind (a task on a worker) replaces one implementation and touches neither backend. Phase R0 of `docs/readiness-io.md`: threads only, no behaviour change.

## Context

- Design: `docs/readiness-io.md` sections 4.2 (one waiter for every wait), 3.4 (keeping both possible), 7 (R0), 8 (draft stories).
- Decision: Timo, 2026-11-08, "A now - then make both possible" (`docs/vision.md` T8): stackful tasks over epoll now, the reactor swappable.
- Umbrella `story:readiness-io`; epic `epic:native-parity`.

## Acceptance

- `crates/ignite-io/src/wait.rs` holds `Waiter`, `park(waiter, deadline) -> Woken`, `wake(waiter)` and `WaitList`; no `Condvar` wait remains in `crates/ignite-io/src` (`grep -n Condvar crates/ignite-io/src` lists none outside `wait.rs`).
- A reactor trait in `crates/ignite-io` (register a descriptor with its interest, deregister, wait for events until a deadline returning the waiters to wake, wake from any thread) with two implementations: `EpollReactor` (epoll plus an `eventfd`, `nix` feature `event`) and the test double `ScriptedReactor`, which delivers readiness events a test scripts without a kernel descriptor. One contract test suite in `ignite-io` runs against both and passes; it covers readiness wakes a parked waiter, a deadline expires, a cross-thread wake ends a wait, and deregistration stops events. The double is the proof that the seam carries no epoll type.
- The thread-per-task implementation stays one implementation of the waiter seam, beside the task waiter R1 adds, not a fallback to be removed: a `park` on a plain thread blocks it (a futex), and the contract suite runs over it. This keeps the IO mode a build or executable setting (`story:io-mode-setting`, Timo 2026-11-08).
- A thread off a worker waits for a descriptor through the reactor interface (section 4.2's `poll` on the descriptor and the thread's `eventfd`), so `main` accepts on a listener the drain wakes.
- No behaviour change: every existing test green on both targets, `task gate` green, counts and exit in the PR body; `terminal::idle_terminald_runs_one_thread` unchanged.
- No generated-code change on either target; `apply::plan::CODEGEN_REVISION` unchanged.

## Scope

- `crates/ignite-io/src/wait.rs` (new), `crates/ignite-io/src/reactor.rs` (new: the trait, `EpollReactor`), `crates/ignite-io/src/{channel,pipe,child,task,clock,net,drain}.rs`, `crates/ignite-io/Cargo.toml` (`nix` feature `event`), `crates/ignite-io/tests/` (the contract suite and `ScriptedReactor`). Cited from `docs/readiness-io.md` section 8; `reactor.rs`, `clock.rs`, `drain.rs` inferred from sections 3.4 and 4.2.
