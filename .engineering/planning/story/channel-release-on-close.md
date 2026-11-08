---
format: aep.planning-md/3
id: story:channel-release-on-close
kind: story
status: active
title: A closed channel's memory is freed
summary: 'Case study G30: a task''s closed channel is reported to leak about 370 B'
relations:
- informed_by: story:channels-and-timers
- depends_on: story:leak-census
scope:
- confidence: inferred
  path: crates/ignite-io/src/channel.rs
- confidence: inferred
  path: crates/ignite-llvm/src/glue.rs
- confidence: inferred
  path: crates/ignite-llvm/src/lower.rs
- confidence: inferred
  path: crates/ignite-llvm/src/own.rs
- confidence: inferred
  path: crates/ignite-rt/src/channel.rs
- confidence: inferred
  path: crates/ignite-rt/src/net.rs
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
- confidence: inferred
  path: crates/ignite/tests/main.rs
- confidence: inferred
  path: docs/research/case-study-gaps.md
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/sync.ignite
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T15:00:48Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-08T21:40:02Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":3}}}
---
# Story: channel-release-on-close

## Outcome

A channel end is released when its last owner goes, wherever the end was moved: a second binding, an annotated tuple, a record field, a `Store` and back. A server that opens and closes a channel per request, in any of those forms, stays at a bounded resident size on both targets, and the guard's G30 soak tests (`story:leak-census`) pass.

## Context

- Case study gap G30 (`docs/research/case-study-gaps.md`), v0.15.0, LLVM target: a server holding one channel per waiting request grew from 1 MB to 88 MB over 200,000 requests; with closed channels reused it stays at 2 MB. Design and guard: `docs/leak-guard.md` 1.1, 1.2, 4 (option d against `own.rs`).
- The three lines first reported (`let (r, b) = Channel.bounded(1); r.close(); b.receive_within(0 ms)`) do not leak: both ends stay plain bindings and `own.rs` releases them.
- Cause (the case study's reproducer, 2026-11-08): a channel end's handle-table entry is released only while the end stays in a plain `let (tx, rx) = Channel.bounded(n)` binding. `crates/ignite-llvm/src/own.rs` makes the block the owner only while every later mention is a builtin method receiver, a `spawn` argument or a plain call argument; any other mention "leaves the binding unowned, as before: never released". An end taken out of a `Store` (`let tx: Sender<Int> = found`) is never owned either. The entry holds a clone of the `ignite-io` end, so the channel stays allocated; `close()` closes the channel but releases no handle. The Rust target drops by Rust ownership and does not leak.
- G30 and G32 interact: `alias` is the annotation E042 asks for when no use fixes the element type (`story:infer-channel-ends`), so the G32 workaround triggers G30.
- Why `own.rs` gives up: an LLVM value has no destructor; a record or list lives in an arena region freed without visiting its contents. The fix is drop glue for values whose type can hold a handle, run where the region or binding that holds them ends (as the `Store` glue's `free_heap` for heap copies), so ownership moves with the value.

Forms and peak RSS (MB) at 0 / 50,000 / 100,000 / 200,000 iterations, `ignite build --static --release`, LLVM, toolchain image 0.15.0:

| mode | the ends | RSS | leaks |
|---|---|---|---|
| `none` | no channel | 8.4 / 8.4 / 8.4 / 8.4 | no |
| `local` | `let (tx, rx) = Channel.bounded(1)`, used only through methods | 8.3 / 8.3 / 8.3 / 8.3 | no |
| `alias` | as `local`, then `let r: Receiver<Int> = rx` | 8.4 / 26 / 52 / 104 | yes, about 490 B per iteration |
| `typed` | `let ends: (Sender<Int>, Receiver<Int>) = Channel.bounded(1)` | 8.4 / 34 / 68 / 137 | yes, both ends, about 660 B |
| `field` | as `local`, the receiver in a record field | 8.4 / 26 / 52 / 104 | yes, about 490 B |
| `server` | the sender in a `Store<Sender<Int>>`, the receiver returned in a record, the sender removed and closed | 8.2 / 22 / 43 / 87 | yes, about 400 B |

The reproducer (run by `MODE` and `N`; copied here because its scratch copy expires):

```ignite
package repro

type Waiter {
  id: Int
  wake: Receiver<Int>
}

fn local(done: Sender<Int>) -> Result<Unit, IoError> {
  let (tx, rx) = Channel.bounded(1)
  let rung = tx.try_send(1)
  tx.close()
  let got = rx.receive_within(0 ms)
  let sent = done.send(1)
  Ok(())
}

fn alias(done: Sender<Int>) -> Result<Unit, IoError> {
  let (tx, rx) = Channel.bounded(1)
  let r: Receiver<Int> = rx
  let rung = tx.try_send(1)
  tx.close()
  let got = r.receive_within(0 ms)
  let sent = done.send(1)
  Ok(())
}

fn typed(done: Sender<Int>) -> Result<Unit, IoError> {
  let ends: (Sender<Int>, Receiver<Int>) = Channel.bounded(1)
  let rung = ends.0.try_send(1)
  ends.0.close()
  let got = ends.1.receive_within(0 ms)
  let sent = done.send(1)
  Ok(())
}

fn field(done: Sender<Int>) -> Result<Unit, IoError> {
  let (tx, rx) = Channel.bounded(1)
  let w = Waiter { id = 1, wake = rx }
  let rung = tx.try_send(1)
  tx.close()
  let got = w.wake.receive_within(0 ms)
  let sent = done.send(1)
  Ok(())
}

fn open(senders: Store<Sender<Int>>) -> Waiter {
  let (tx, rx) = Channel.bounded(1)
  let id = senders.next_id()
  senders.put(id, tx)
  Waiter { id, wake = rx }
}

fn close(senders: Store<Sender<Int>>, w: Waiter) -> Unit {
  match senders.remove(w.id) {
    Some(found) => {
      let tx: Sender<Int> = found
      tx.close()
    }
    None => {}
  }
}

fn server(senders: Store<Sender<Int>>, done: Sender<Int>) -> Result<Unit, IoError> {
  let w: Waiter = open(senders)
  let got = w.wake.receive_within(0 ms)
  close(senders, w)
  let sent = done.send(1)
  Ok(())
}

fn none(done: Sender<Int>) -> Result<Unit, IoError> {
  var l: List<Int> = [1, 2, 3]
  l.push(4)
  let sent = done.send(1)
  Ok(())
}

type Repro {
  senders: Store<Sender<Int>>
  environment: Environment
  executable: Text
}

fn Repro.main(self: Repro) -> Result<Unit, IoError> {
  let n = match self.environment.get("N") {
    Some(t) => match Int.parse(t) {
      Some(v) => v
      None => 1000
    }
    None => 1000
  }
  let mode = match self.environment.get("MODE") {
    Some(m) => m
    None => "local"
  }
  let (done, finished) = Channel.bounded(1)
  var i = 0
  while i < n {
    if mode == "alias" {
      spawn alias(done)
    } else if mode == "typed" {
      spawn typed(done)
    } else if mode == "field" {
      spawn field(done)
    } else if mode == "server" {
      spawn server(self.senders, done)
    } else if mode == "none" {
      spawn none(done)
    } else {
      spawn local(done)
    }
    let got = finished.receive()
    i = i + 1
  }
  log("{mode}: {n} done")
  Ok(())
}

system ReproSystem {
  resource senders: Store<Sender<Int>>
  resource environment: Environment
  component repro: Repro {
    senders = senders
    environment = environment
    executable = "repro"
  }
}
```

## Acceptance

- The guard proves the fix: `story:leak-census`'s soak tests "a channel end bound a second time is released (G30)" and "a channel end kept in a record field is released (G30)" in `std/sync.ignite` (2,000 runs each, `docs/leak-guard.md` 6.1), red on `main` on LLVM naming `handles` and `channels`, pass on both targets with the fix; the wave record shows both runs.
- The other two forms get soak tests beside them, red on LLVM before the fix and green after on both targets: "an annotated pair of channel ends is released (G30)" (`typed`) and "a channel end put in a store and taken out is released (G30)" (`server`: the sender in a `Store`, the receiver in a returned record, the sender removed and closed).
- `crates/ignite/tests/` builds the reproducer above on each target and runs every mode at `N=20000`; each mode's peak resident size is within 2 MB of `none`'s (the leaking modes grew by about 10 MB at that count on v0.15.0). Measured inside the test, not a load test.
- The G30 entry of `docs/research/case-study-gaps.md` names the cause, the four forms and the six test names; `docs/slice.md` 30.3 states that an end moved into a binding, a tuple, a record field or a store is released with its last owner; `task gate` green.

## Scope

- `crates/ignite-llvm/src/own.rs`, `crates/ignite-llvm/src/lower.rs`, `crates/ignite-llvm/src/glue.rs`: ownership that moves with the value, drop glue for values holding handles. Inferred.
- `crates/ignite-rt/src/channel.rs`, `crates/ignite-rt/src/net.rs`, `crates/ignite-io/src/channel.rs`: release entries. Inferred.
- `std/sync.ignite` (the two further soak tests), `crates/ignite/tests/` and its `main.rs` (the reproducer as a fixture), `crates/ignite/src/apply/plan.rs` (`CODEGEN_REVISION`), `docs/research/case-study-gaps.md`, `docs/slice.md`. Inferred.
