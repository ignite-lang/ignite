---
format: aep.planning-md/3
id: story:http-octet-head-and-date
kind: story
status: draft
title: Http parses the head as octets and sends Date
relations:
- informed_by: story:http-rfc9112-9110-gaps
revision: 1
---
# Story: http-octet-head-and-date

## Outcome

Http closes its last two MUST gaps: RFC 9112 R2_2_1 (a request head is parsed as octets, so a non-UTF-8 byte in a header value is not a parse failure of the whole head) and RFC 9110 R6_6_1_2 (a server with a clock sends `Date` in IMF-fixdate).

## Context

- Split from `story:http-rfc9112-9110-gaps` on 2026-11-06 by its scoping pass.
- R2_2_1: `Request.headers` is `List<(Text, Text)>` (`std/protocol/http.ignite`); `Text` holds UTF-8 only, so a header byte 0xE9 cannot be kept. Options: a `Bytes` header value, or reject such a value with 400 (if the RFC allows it for the field).
- R6_6_1_2: no wall-clock builtin exists (`Clock.monotonic` only, `crates/ignite-types/src/lib.rs:684`); a `Date` in every response changes tests that compare exact response bytes.

## Acceptance

- Both rows `satisfies` with a test that names them; `ignite conformance std/protocol/http.ignite` on both targets: rfc9112 and rfc9110 0 gaps, 0 failures; `CODEGEN_REVISION` bumped if a builtin is added; `task gate` green.

## Out of Scope

- SHOULD-level rows.
