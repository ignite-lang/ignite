---
format: aep.planning-md/3
id: story:http-server-responses
kind: story
status: draft
title: Streamed and binary HTTP responses with full reason phrases
summary: 'Close the playground''s HTTP gaps P4 to P6: chunked responses, byte bodies, every RFC 9110 reason phrase'
relations:
- depends_on: story:http-rfc9112-9110-gaps
scope:
- confidence: inferred
  path: std/protocol/http.ignite
revision: 2
---
# Story: http-server-responses

## Outcome

`std.protocol.http`'s server sends streamed responses, binary bodies and the standard reason
phrases.

## Context

- Gaps P4 to P6 of the playground session (2026-11-04, compiler `9578530`):
  - P4 responses are Content-Length bodies only (`std/protocol/http.ignite:13-14`, `40-44`), so a
    run's output must be polled.
  - P5 response bodies are `Text` only (`http.ignite:43`), so binary assets cannot be served.
  - P6 the reason-phrase table is short (`http.ignite:101-113`): a 403 goes out as
    `HTTP/1.1 403 Unknown` (seen with curl on an LLVM build).
- Touches the same file as `story:http-rfc9112-9110-gaps`; the two run one after the other.

## Acceptance

- Inline tests over a pipe on both targets: a chunked response the client reads in pieces; a body
  of bytes 0 to 255 read back byte for byte; every status code of RFC 9110 section 15 sent with
  its reason phrase.
- `ignite conformance std/protocol/http.ignite` reports no new gap.

## Out of Scope

- Server-sent events framing, WebSockets, HTTP/2.
