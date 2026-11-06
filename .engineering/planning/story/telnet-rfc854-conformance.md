---
format: aep.planning-md/3
id: story:telnet-rfc854-conformance
kind: story
status: implemented
title: Telnet conforms to RFC 854
summary: GA after the prompt, Synch over TCP urgent data and a partial-line key, so the rfc854 claim holds
relations:
- derived_from: epic:slice-15-protocols-p2
scope:
- confidence: inferred
  path: crates/ignite-io/src
- confidence: inferred
  path: crates/ignite-rt/src
- confidence: inferred
  path: crates/ignite/tests/terminal.rs
- confidence: inferred
  path: examples/terminal/commands.ignite
- confidence: inferred
  path: std/protocol/telnet.ignite
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T07:25:52Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-06T07:26:03Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-11-06T10:36:21Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---

# Story: telnet-rfc854-conformance

## Outcome

`ignite conformance std/protocol/telnet.ignite` prints
`claim conformance of Telnet to rfc854: yes (0 gaps, 0 failures, 0 inconclusive)` on both targets.

## Context

Slice 15 U6 (`story:slice-15-protocol-claims`) states four RFC 854 MUSTs as `unsupported`; row 3
of `docs/slice.md` 19.0 was amended to expect `no (4 gaps …)`, and 19.14 item 16 records the cost:

- RP5_4: no IAC GA after the prompt (GA changes every server transcript: inline tests,
  `examples/terminal/commands.ignite`, `crates/ignite/tests/terminal.rs`).
- RP10_2 and RP12_4: Synch needs TCP urgent send and receive in `ignite-io` and `ignite-rt` on
  both targets, and a `CODEGEN_REVISION` bump.
- RP5_3: `tn`'s line editor has no key that sends a partial line (its Must comes from the word
  table limit of 19.14 item 15).

## Acceptance

- The claim line above on both targets; each of the four rows `satisfied` with a passing verifies
  test; the slice-14 transcripts updated in the same change and still passing on both targets.

## Out of Scope

- RFC 1143 conformance.
