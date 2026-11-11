---
format: aep.planning-md/3
id: story:llvm-client-record-closes-its-connection
kind: story
status: draft
title: A client on the LLVM target closes its connection
relations:
- informed_by: story:std-mysql-client
revision: 1
---
# Story: a client on the LLVM target closes its connection

## Outcome

On the LLVM target, a program that connects a std client (`std.protocol.mysql`, `std.protocol.nats`, `std.protocol.resp`) and quits or drops it holds no descriptor of that connection afterwards, as on the Rust target.

## Why

Adversary pass 1 on unit mysql (`review-result:adversary-mysql-pass-1`, finding 3), `crates/ignite/tests/mysql_adversary.rs` `connections_leave_no_descriptor_on_both_targets`, pre-existing: on LLVM 7 descriptors before and 47 after 40 connections that quit, were refused or were refused TLS, 9 to 129 for 40 pools of 3, 9 to 449 against a real server in one process; the Rust target returns to its start. A probe that imports nothing of the unit grows the same way (5 before, 45 after 40 streams returned or held in a record).

The mechanism is the owner rule of `docs/llvm.md` section 4: a binding that is returned or stored in a field is owned by no block and is not released before the process exits, and `TcpStream.close()` shuts only the write side. `connect` returns `Client { stream }`, a generic record over `ByteStream`, from another module.

A service that keeps one pool for its lifetime loses one descriptor per reconnect; a program that connects per request loses one per request.

v0.15.0 was not measured; it has the same owner rule.

## Acceptance

- `mysql_adversary::connections_leave_no_descriptor_on_both_targets` asserts no growth on LLVM for its four cases (40 quit, 40 refused, 40 refused TLS, 40 pools of 3); it pins the growth until this story.
- The same count for a NATS client and a RESP client connected and closed 40 times against a scripted peer, both targets.
- A handle returned inside a record by a fn of another module, generic or not, is owned by the caller's block when the caller keeps it there; one inline test per shape in the differential corpus; row 1.1 stays 0.
- Instructions per connection of the todo and terminal servers on LLVM are reported against the story's base; rows 3.3, 3.7 and 3.8 on the dedicated host are not worse.
- `task gate` green; no row of `docs/quality.md` worse; `CODEGEN_REVISION` bumped.

## Scope

- `crates/ignite-llvm/src/own.rs`, `crates/ignite-llvm/src/lower.rs`, `docs/llvm.md` section 4, `crates/ignite/tests/mysql_adversary.rs`, `crates/ignite/tests/stream_ownership.rs`. Inferred.
- Not in scope: the five store reads of `story:llvm-temporaries-without-owner-rule`, the aliases of `story:llvm-task-move-aliases`.
