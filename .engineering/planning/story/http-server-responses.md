---
format: aep.planning-md/3
id: story:http-server-responses
kind: story
status: implemented
title: Streamed and binary HTTP responses with full reason phrases
summary: 'Close the playground''s HTTP gaps P4 to P6: chunked responses, byte bodies, every RFC 9110 reason phrase'
relations:
- depends_on: story:http-rfc9112-9110-gaps
scope:
- confidence: cited
  path: crates/ignite/tests/conformance.rs
- confidence: cited
  path: site/book/src/reference/std/http.md
- confidence: cited
  path: std/protocol/http.ignite
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T23:50:48Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-11-06T23:50:48Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-11-07T02:53:19Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1}}}
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

## Acceptance

Scoped 2026-11-07 on main df7be28c: `Response.body` is `Text` (`std/protocol/http.ignite:60`), `render_message` always writes `Content-Length` (`:409`), `reason` names 11 codes (`:179-192`). The shape of a streamed body (a producer on `Response`, a writer, a sum type for `body`) is the implementor's choice; `Response.plain` and `Response.json` keep their output byte for byte.

- Streamed response: an inline test over `Pipe.pair()`: the handler's body comes in pieces, the producer waits on a second pipe the test controls; the client reads the head with `Transfer-Encoding: chunked` and the first chunk before the test releases the second piece, then the rest and `0\r\n\r\n`; a following pipelined request on the same connection is answered.
- Where Http must not chunk: an HTTP/1.0 request gets no `Transfer-Encoding` (close-delimited or buffered `Content-Length`); a 204 and a HEAD get no chunked body; the test verifies RFC9112.R6_1_7 and R6_1_11.
- Binary body: a body of the bytes 0..=255 is read back equal, with `Content-Length: 256`.
- Reason phrases: an inline test asserts the phrase for 100, 101, 200-206, 300-305, 307, 308, 400-417, 421, 422, 426, 500-505; one round trip shows `HTTP/1.1 403 Forbidden` on the wire.
- `ignite test std/protocol/http.ignite --target=all` passes; `ignite conformance std/protocol/http.ignite` on llvm and rust: no row that held on the base turns into a gap or failure, gap counts not higher; `conformance::` and `site::` pass with `site/book/src/reference/std/http.md` updated; `task gate` green.

## Out of Scope

- Server-sent events framing, WebSockets, HTTP/2.
