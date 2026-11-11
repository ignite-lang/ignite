---
format: aep.planning-md/3
id: story:tls-chain-only-verification
kind: story
status: draft
title: std.tls verifies a server's chain without its host name when the program asks for that
relations:
- informed_by: story:std-mysql-client
revision: 1
---
# Story: std.tls verifies a server's chain without its host name when the program asks for that

## Outcome

A client can ask `std.tls` to verify the server's certificate chain against its trust roots without checking the host name, as MySQL's `VERIFY_CA` mode does, and gets a refusal for a chain that does not lead to a root.

## Why

Reported on 2026-11-11 by the first user of `std.protocol.mysql` (client at `a4e1a6d7`, Rust target, MySQL 8.0.42, TLSv1.3). `std.tls` verifies chain and identity together, so a connection mode that means "verify the CA only" behaves as identity verification: a server reached under a name its certificate does not carry cannot be connected to with a verified chain. Pre-existing: the TLS client had this shape before the MySQL client used it.

## Acceptance

- A TLS client option verifies the chain and the validity dates against the given roots and skips the name check; the default stays chain and identity. Held by tests on both targets: a server whose certificate carries another name connects under the option and is refused without it; a chain from an unknown root is refused under the option.
- `std.protocol.mysql` maps its "verify the CA" mode to the option; its section of `docs/slice.md` says which mode checks what.
- `task gate` green; no row of `docs/quality.md` worse.

## Scope

- `std/tls` and the TLS engine crates, `std/protocol/mysql.ignite`, `docs/slice.md`. Inferred.
