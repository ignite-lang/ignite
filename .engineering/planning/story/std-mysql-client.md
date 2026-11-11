---
format: aep.planning-md/3
id: story:std-mysql-client
kind: story
status: draft
title: 'std.protocol.mysql: MySQL wire client with caching_sha2_password'
revision: 2
---
# Story: std-mysql-client

## Outcome

`std.protocol.mysql` is a MySQL wire-protocol client over any `ByteStream`: handshake with `caching_sha2_password`, text queries and prepared statements with typed rows, so a read-only service answers a request from a SQL query without FFI.

## Context

- Case study gap G11 (`docs/research/case-study-gaps.md`): no MySQL client in std and no FFI.
- Asked for on 2026-11-11 by a consumer that generates read-only HTTP servers (request, SQL, JSON projection); its server is MySQL 8.0 with TLS 1.2 and 1.3 offered and not required, and `mysql_native_password` as the default plugin while single accounts use `caching_sha2_password`.
- `std.digest` has `sha256` (the scramble of `caching_sha2_password`); `std.tls` wraps a `TcpStream`; both ship in v0.16.0. `std.protocol.resp` and `std.protocol.nats` are the pattern: a codec with limits, a client generic over `T: ByteStream`, tests over `Pipe.pair()`.
- `caching_sha2_password` has two paths. Fast authentication needs only the SHA-256 scramble and works when the server has the account cached. Full authentication sends the password itself and is allowed only over TLS or encrypted with the server's RSA key; std has no RSA.

## Acceptance

- Packet framing with sequence numbers and the 16 MiB continuation rule; limits on packet and row size (`MysqlLimits`); a server error packet is a typed `MysqlError` with code, SQL state and message.
- Handshake v10 with both plugins a MySQL 8.0 server uses, chosen by the server's handshake and by an auth-switch request in either direction: `mysql_native_password` (SHA-1 scramble, `std.digest.sha1`) and `caching_sha2_password` (fast authentication on a plain stream; full authentication over a `TlsStream`; on a plain stream a server that asks for full authentication gets a typed error that names TLS, not a hang). Any other plugin is refused with a typed error that names it.
- A connection can be upgraded to TLS before authentication (the `SSLRequest` packet, then `std.tls`), so one client serves a server that offers TLS without requiring it.
- `query` (text protocol) and `prepare`/`execute`/`close` (binary protocol) return column definitions and rows; values decode to `Int`, `Float`, `Text`, `Bytes`, `Bool` and null as `Option`; a statement parameter is never spliced into SQL text.
- `ping`, `quit`, and a documented way to hold several connections for concurrent requests (a pool or a helper), with a connection that failed taken out.
- Tests over `Pipe.pair()` with a scripted server for every packet kind and every error; one test against a real `mysqld` when present (ignored elsewhere, like `image::`).
- No new `unsafe`, no new dependency of the substrate; layering rule kept; `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `std/protocol/mysql.ignite` (new), `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `docs/slice.md`, `AGENTS.md` layout table. Inferred.
- Out of scope: RSA key exchange for full authentication on a plain stream (a story of its own if a consumer needs it), compression, replication, `LOAD DATA LOCAL`, multi-statement and multi-result.
