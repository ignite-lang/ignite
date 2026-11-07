---
format: aep.planning-md/3
id: story:http-octet-head-and-date
kind: story
status: draft
title: Http parses the head as octets and sends Date
relations:
- informed_by: story:http-rfc9112-9110-gaps
scope:
- confidence: cited
  path: STATUS.md
- confidence: inferred
  path: crates/ignite-check/src/lib.rs
- confidence: inferred
  path: crates/ignite-eval/src/eval.rs
- confidence: inferred
  path: crates/ignite-io/src/clock.rs
- confidence: inferred
  path: crates/ignite-llvm/src/lower.rs
- confidence: inferred
  path: crates/ignite-llvm/src/region.rs
- confidence: inferred
  path: crates/ignite-rt/src/clock.rs
- confidence: inferred
  path: crates/ignite-rt/src/lib.rs
- confidence: inferred
  path: crates/ignite-rustgen/src/lib.rs
- confidence: inferred
  path: crates/ignite-types/src/builtins.rs
- confidence: cited
  path: crates/ignite-types/src/lib.rs
- confidence: inferred
  path: crates/ignite-types/src/typer/effects.rs
- confidence: cited
  path: crates/ignite/src/apply/plan.rs
- confidence: cited
  path: crates/ignite/tests/conformance.rs
- confidence: cited
  path: crates/ignite/tests/fixtures/conformance/adversary_std_http.ignite
- confidence: inferred
  path: crates/ignite/tests/run.rs
- confidence: inferred
  path: crates/ignite/tests/static_link.rs
- confidence: cited
  path: docs/slice.md
- confidence: inferred
  path: examples/todo/health.ignite
- confidence: inferred
  path: site/book/src/reference/std/http.md
- confidence: cited
  path: std/protocol/http.ignite
- confidence: inferred
  path: std/time.ignite
revision: 23
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
