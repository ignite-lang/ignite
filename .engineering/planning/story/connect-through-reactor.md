---
format: aep.planning-md/3
id: story:connect-through-reactor
kind: story
status: draft
title: TcpStream.connect waits through the reactor
revision: 1
---
# Story: TcpStream.connect waits through the reactor

## Outcome

A task that opens a TCP connection does not block its worker thread in `connect(2)`: the connect
is started non-blocking and the task waits for it through the reactor, as reads and accepts do.

## Why

Found while diagnosing the inline test `std.protocol.http/a thousand requests held over TCP are
each answered by their message` (wave 10). `TcpStream.connect` is a blocking `connect(2)` on the
task's worker (`crates/ignite-io/src/net.rs`), the standard library listens with a backlog of
128, and a task never moves to another worker. A program whose tasks open many connections at
once to a listener in the same process overflows the listen queue; the kernel drops the SYNs,
each dropped SYN keeps its worker blocked until the retransmit, and once every worker is blocked
that way, the accepting task's worker among them, nothing is accepted until the retries give up.
Measured: the listener's queue at 129 of 128, 6 to 12 clients in SYN-SENT, every worker waiting
in the kernel. The test now starts its clients 100 at a time; the runtime is unchanged.

## Acceptance

- A test starts 1,000 clients at once against an acceptor in the same process and all are
  accepted within 10 seconds, on both targets, with the default worker count.
- A connect to an address that does not answer blocks no other task on the same worker: a test
  shows a second task on that worker making progress while the connect waits.
- The connect timeout and the error a refused connection gives are unchanged; their tests pass.

## Scope

- Own: `crates/ignite-io/src/net.rs` (`connect`), the reactor's registration of a connecting
  socket, the LLVM runtime's counterpart in `crates/ignite-rt`. Inferred.
