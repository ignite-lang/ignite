---
format: aep.planning-md/3
id: story:http-rfc9112-9110-gaps
kind: story
status: draft
title: Http closes its RFC 9112 and 9110 MUST gaps
summary: Check the application's response headers and parse the head as octets to close Http's MUST gaps
relations:
- derived_from: epic:slice-15-protocols-p2
scope:
- confidence: cited
  path: docs/slice.md
- confidence: cited
  path: std/protocol/http.ignite
revision: 4
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

- The gap counts drop; the list of closed requirement ids with their tests is in the commit and
  in `docs/slice.md` 19.14; no other row of the matrix changes verdict for the worse.

## Out of Scope

- TLS, HTTP/2.
