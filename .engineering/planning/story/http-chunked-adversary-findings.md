---
format: aep.planning-md/3
id: story:http-chunked-adversary-findings
kind: story
status: draft
title: 'Chunked decoder: tests that kill its mutants, memory bound and R6_2_5 claim'
scope:
- confidence: cited
  path: STATUS.md
- confidence: cited
  path: crates/ignite/tests/conformance.rs
- confidence: inferred
  path: crates/ignite/tests/golden/studio.model.json
- confidence: inferred
  path: crates/ignite/tests/golden/todo.model.json
- confidence: cited
  path: docs/slice.md
- confidence: cited
  path: std/protocol/http.ignite
revision: 7
---
# Story: http-chunked-adversary-findings

## Outcome

The chunked decoder of PR 79 has tests that kill its two surviving mutants, and its documented memory bound and one claim are true.

## Context

- Adversary review of PR 79 (2026-11-07); branch `test/adv-http-chunked` at `9dd5e5bb` adds two inline tests to `std/protocol/http.ignite`, green on main, each red against a mutant that passes the 60 earlier http tests:
  - F1: drop the LF half of the CR LF check after chunk data (`:1160`); test "a chunk whose data ends in CR and another byte instead of CR LF is 400".
  - F2: remove `read_line`'s length guard (`:1103`), so an unterminated chunk-size or trailer line buffers until the idle timeout; test "an unterminated chunk-size line or trailer over the head limit is 431, not read until the peer closes".
- F3 (inferred, no test): `docs/slice.md` 19.22 says the decoder holds at most the body and one read; a single large chunk is held in the read buffer and in `body` until compaction (`:1134`), about twice the body.
- F4: `render_message` (`:403`) claims RFC9110.R6_2_5 while Http declares the RFC 9112 gap R2_2_1.

## Acceptance

- The two tests of `test/adv-http-chunked` are on main and pass on both targets.
- 19.22 states the bound the decoder keeps (or the decoder compacts per chunk and the text holds), checked by a test that decodes one chunk of `HttpLimits.body` bytes.
- R6_2_5's claim is `unsupported` until R2_2_1 is closed, or the PR states why it holds; conformance counts updated.
- `task gate` green.
