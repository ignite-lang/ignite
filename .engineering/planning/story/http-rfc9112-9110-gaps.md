---
format: aep.planning-md/3
id: story:http-rfc9112-9110-gaps
kind: story
status: implemented
title: Http closes its RFC 9112 and 9110 MUST gaps
summary: Check the application's response headers and parse the head as octets to close Http's MUST gaps
relations:
- derived_from: epic:slice-15-protocols-p2
scope:
- confidence: cited
  path: crates/ignite/tests/conformance.rs
- confidence: cited
  path: crates/ignite/tests/fixtures/conformance
- confidence: cited
  path: crates/ignite/tests/todo.rs
- confidence: cited
  path: docs/slice.md
- confidence: cited
  path: std/protocol/http.ignite
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T15:07:21Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-11-06T15:07:22Z", actor: "human:timo", revision: 8}
- {from: "active", to: "implemented", at: "2026-11-06T22:40:30Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1}}}
---

# Story: http-rfc9112-9110-gaps

## Outcome

`ignite conformance std/protocol/http.ignite` reports fewer MUST gaps for RFC 9112 and RFC 9110
than at the slice-15 close, each closed gap with a passing verifies test on both targets.

## Context

At the slice-15 close Http claimed `rfc9112: no (12 gaps …)` and `rfc9110: no (9 gaps …)` before
the U6 adversary fixes (`story:slice-15-protocol-claims`), and `rfc9112: no (15 gaps, 0 failures,
0 inconclusive)`, `rfc9110: no (9 gaps, …)` in the close gate (`AGENTS.md`, slice 15). The 15
and 9 are the baseline. The U6 implementor estimated about 8 gaps close by checking the
application's response headers. Restated as gaps in slice 15: R6_1_2 and R6_1_4 (an application
`Transfer-Encoding` sent on a kept connection), R2_2_1 (the head is parsed as lossy UTF-8 rather
than octets). R7_1_2 (chunked decoding) and R3_2_6 (400 without Host) are the P2 gaps of 19.0
row 2.

Sequencing: slice 17's U6 changes `std/protocol/http.ignite` (`HttpServer` hooks), so this story
starts after slice 17 is on `main`.

## Acceptance

## Acceptance

Scoped 2026-11-06 from main 5d24ae8: all 24 MUST gaps open (`std/protocol/http.ignite:81-123`; counts pinned at `crates/ignite/tests/conformance.rs:938` 15 and `:943` 9). Landed as three PRs in this order, each with its own green `task gate`; each PR moves its rows from `unsupported` to `satisfies` with a test that names the row, and lowers the pinned counts to match.

- PR A, response headers in `checked` (500 on violation): R2_2_8, R6_1_2, R6_1_4, R6_1_7, R6_1_8, R6_1_11, R6_2_1 (RFC 9112); R5_3_3, R5_6_1_1_1, R5_6_3_4, R5_6_7_2, R5_6_7_3 (RFC 9110). The R6_1_4 adversary test (`conformance.rs:1477` or `:1491`) is no longer ignored. A row that a generic check cannot close stays `unsupported` with the reason in the PR body.
- PR B, request head: R3_2_6 (400 on a missing or repeated Host), R3_3 (414 on a request-target over the limit), R3_2_2_5 and R3_2_2_6 (absolute-form accepted), R9_1_1 and R9_3_2_1 (HEAD answered as GET without a body). Inline tests and fixtures that send no Host gain one.
- PR C, chunked request bodies decoded in `next_request` instead of 501: R6_1_1, R6_3_3, R7_1_2 (RFC 9112), R6_2_5 (RFC 9110); the rows that become applicable (R7_1_3, R7_1_1_1, R7_1_2_2, R7_1_2_3, RFC 9110 R6_5_1_2) each get `satisfies` with a test or stay a counted gap; `crates/ignite/tests/todo.rs:560` expects the decoded behaviour.
- After C: `ignite conformance std/protocol/http.ignite` on both targets prints at most 2 gaps for RFC 9112 (R2_2_1) and RFC 9110 (R6_6_1_2) together, 0 failures, 0 inconclusive; `docs/slice.md` 19.14 item 17 and 19.0 row 2 state the counts as landed; no row that held before turns into a failure.

## Out of Scope

- R2_2_1 (parse the head as octets: needs a `Request.headers` type decision) and R6_6_1_2 (the Date header: needs a wall-clock builtin): story `http-octet-head-and-date`.
- Streamed and binary responses (`story:http-server-responses`).

## Out of Scope

- TLS, HTTP/2.
