---
format: aep.planning-md/3
id: story:http-client-role
kind: story
status: proposed
title: HTTP client role and Url in std.protocol.http
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:04:57Z", actor: "human:timo", revision: 2}
---
# Story: http-client-role

## Outcome

`std.protocol.http` has a client role over any `ByteStream`: a `Url` type, requests with headers and bodies, responses with `Content-Length`, chunked or close-delimited bodies, keep-alive reuse and deadlines, so a service calls another HTTP service without hand-written framing.

## Context

- Case study gap G19: 738 hand-written lines, among them a client for an auth backend call.
- The server role exists (`Http.serve`); this adds the client side of RFC 9112 and RFC 9110 and a URL parser (RFC 3986 subset: scheme, host, port, path, query).

## Acceptance

- `Url.parse` with tests for the RFC 3986 examples that apply; `Http.request(stream, request, limits) -> IoResult<Response>` and a `Client` that keeps one connection alive per origin.
- Response bodies: length, chunked (trailers checked as the server does), until close; limits (`HttpClientLimits`); a deadline per request.
- The protocol's `conforms` claims cover the client parties it implements; `ignite conformance` reports them; gaps are listed.
- Tests over `Pipe.pair()` against `Http.serve`, one over TCP; `https` URLs refused with a clear error until `story:tls-client` lands.
- `docs/slice.md` section; `task gate` green.

## Scope

- `std/protocol/http.ignite`, `std/net.ignite` (Url) or a new `std/url.ignite`, `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `docs/slice.md`. Inferred.
