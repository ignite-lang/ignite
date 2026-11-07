---
format: aep.planning-md/3
id: story:http-inline-no-content
kind: story
status: implemented
title: Inline Http responses carry no body where RFC 9110 forbids one
scope:
- confidence: cited
  path: docs/slice.md
- confidence: cited
  path: std/protocol/http.ignite
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-07T03:00:11Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-07T03:00:11Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-07T05:13:50Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: http-inline-no-content

## Outcome

An inline or binary Http response carries no body or `Content-Length` where RFC 9110 forbids one (304, 1xx, a 2xx answer to CONNECT), as the streamed path already does, and the streamed path's branches have tests.

## Context

- Adversary review of PR 84 (2026-11-07); branch `test/adv-http-responses` at `f09fbbd6` adds 8 inline tests to `std/protocol/http.ignite`, 3 red on both targets:
  - F1: `render_message` (`:493`) gives every status but 204 a `Content-Length` and the body, so `status_response(304)` sends content a client does not read and a pipelined response after it is misread (RFC 9110 15.4.5);
  - F2: an inline or Binary 1xx carries `Content-Length` (RFC 9110 8.6);
  - F3: an inline 2xx answer to CONNECT carries `Content-Length` and keep-alive (RFC 9110 9.3.6).
  F1-F3 are pre-existing (v0.11.0 has the same rule).
- F4: 5 mutants of `send_streamed`, `has_no_content`, `streamed_framing` and `hex` survived PR 84's tests; the branch's 5 green cases kill them.
- F5: `docs/slice.md` 19.23 (`:6567`) says an `Err` of `piece` leaves the body visibly incomplete; on a close-delimited HTTP/1.0 or CONNECT stream it looks complete.

## Acceptance

- The 8 tests of `test/adv-http-responses` are on main and pass on both targets (`render_message` uses `has_no_content` and the CONNECT rule).
- 19.23 states what a client sees after an `Err` of `piece` for chunked and close-delimited bodies.
- `ignite conformance std/protocol/http.ignite` unchanged or better on both targets; `task gate` green.
