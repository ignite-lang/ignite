---
format: aep.planning-md/3
id: story:tls-client
kind: story
status: proposed
title: TLS client sessions as a ByteStream
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:04:57Z", actor: "human:timo", revision: 2}
---
# Story: tls-client

## Outcome

A program opens a TLS client session over a `TcpStream` with certificate verification against the system roots, and uses it as a `ByteStream`, so an https backend or a broker behind TLS is reachable.

## Context

- Case study gap G25: the service refuses https backend URLs at start.
- AGENTS.md (Timo, 2026-11-01): the TLS engine (rustls) may live in `crates/ignite-io`; the `Tls<T>` layer is Ignite.

## Acceptance

- `Tls.client(stream, server_name, options) -> IoResult<TlsStream>` implements `ByteStream`; verification on by default (system roots), SNI, ALPN list, a handshake deadline; an invalid certificate fails with a typed `IoError`.
- Tests against a local TLS server built in the test with a generated CA (no network); a wrong host name and an expired certificate are refused; both targets.
- `story:http-client-role` accepts `https` URLs through it.
- The dependency line in the commit names what it replaces; `docs/slice.md` section; `CODEGEN_REVISION` bumped if generation changes; `task gate` green.

## Scope

- `crates/ignite-io` (rustls session, handle), `crates/ignite-rt`, builtins in `ignite-check`/`ignite-types`, both backends, `std/net.ignite` or a new `std/tls.ignite`, `docs/slice.md`. Inferred.
