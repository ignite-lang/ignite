---
format: aep.planning-md/3
id: story:jsonrpc-protocol
kind: story
status: active
title: 'std.protocol.jsonrpc: Content-Length framing and JSON-RPC 2.0 messages'
relations:
- decomposes: epic:ignite-lsp
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T23:55:26Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-04T23:55:26Z", actor: "human:timo", revision: 3}
---
# std.protocol.jsonrpc: Content-Length framing and JSON-RPC 2.0 messages in Ignite

Step 3a of `design:ignite-lsp` (§3.4, §4 row 3): the transport layer the Ignite-written LSP
server runs on. The catalogue of JSON-RPC 2.0 and LSP base-protocol requirements (step 3b) waits
for decision D5 (`std.rfc` gains an add-only `id: Text`) and is not part of this story.

## Outcome

`std/protocol/jsonrpc.ignite` declares `protocol JsonRpc` with:

- framing: read one message from a `ByteStream` (headers `Content-Length: N` and optional
  `Content-Type`, `\r\n\r\n`, then exactly N body bytes, split across reads) and write one
  message with the header computed from the body's byte length;
- message types: `Id` (`Int` or `Text`), `Request {id, method, params}`, `Notification {method,
  params}`, `Response {id, result | error}`, `ErrorObject {code, message, data}`, the standard
  error codes (-32700, -32600, -32601, -32602, -32603), each encoded and decoded through
  `std.json` with `"jsonrpc": "2.0"`;
- bounds: a `JsonRpcLimits` record (maximum header bytes, maximum body bytes) whose violation is a
  typed error rather than an unbounded read, following `HttpLimits`.

## Acceptance (inline `test` blocks, both targets, over `Pipe.pair()`)

1. A message written by one pipe end is read back byte-identical by the other, including a body
   with non-ASCII text (Content-Length counts bytes, not chars).
2. Two messages written back to back are read as two messages; a message split across many small
   writes is read as one.
3. A missing or non-numeric `Content-Length`, a body larger than the limit and a header larger
   than the limit each give their typed error.
4. Request, notification and response (result and error) round-trip through encode and decode;
   an object without `"jsonrpc": "2.0"` or with neither `id` nor `method` decodes to the
   invalid-request error.
5. `task test-ignite` and the layering test (`crates/ignite-check/tests/layering.rs`) pass;
   `docs/layering.md` lists the new protocol name.

## Out of scope

Requirement catalogues (step 3b), batch requests (JSON-RPC arrays), the LSP lifecycle, decimals
in JSON.

## Adversary findings

Adversary pass over 159ac77, 2026-11-05: framing held under every attack (case and whitespace
of headers, bare `\n`, length forms, limits at and one over, a three-message stream split at
every byte, `rest` carry-over). Three defects, failing tests in 80f3f905:

- medium, fixed in 0b28ae4e: a body that is not UTF-8 was decoded lossily and answered
  normally; now `is_utf8` (RFC 3629) and -32700 with a null id.
- low, fixed in 0b28ae4e: `serve` could reply to a notification or leave a request unanswered;
  `JsonRpcApp<S>` now has `call` (always answered under the request's id) and `notify` (never
  answered), and a peer's response is dropped.
- medium, deferred to `story:json-decimals`: a fraction or exponent in params is answered with
  -32700 because `std.json` reads integers only; the test is kept in that story.
