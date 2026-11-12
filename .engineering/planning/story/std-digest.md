---
format: aep.planning-md/3
id: story:std-digest
kind: story
status: implemented
title: SHA-1, SHA-256 and base64 in std
relations:
- depends_on: story:bitwise-operators
- depends_on: story:u32-words
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: cited
  path: crates/ignite-check/tests/layering.rs
- confidence: inferred
  path: crates/ignite/tests/bitwise.rs
- confidence: inferred
  path: crates/ignite/tests/fixtures/bitwise/codecs.ignite
- confidence: inferred
  path: crates/ignite/tests/fixtures/bitwise/codecs.rs
- confidence: cited
  path: docs/layering.md
- confidence: inferred
  path: docs/research/case-study-gaps.md
- confidence: inferred
  path: docs/slice.md
- confidence: cited
  path: std/data.ignite
- confidence: cited
  path: std/digest.ignite
- confidence: inferred
  path: std/encoding.ignite
- confidence: cited
  path: std/json.ignite
revision: 16
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:04:57Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T21:40:02Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-12T12:37:09Z", actor: "human:timo", revision: 16, decided_on: {"recorded":{"test_result":1}}, executor: "agent:claude-coordinator", correlation: "audit-2026-11-12"}
---
# Story: std-digest

## Outcome

std has SHA-1, SHA-256, base64 and base64url as Ignite modules, so a WebSocket handshake, a token check or a content hash needs no hand-written bit arithmetic.

## Context

- Case study gap G22: hand-written with bit arithmetic in a 288-line module.
- Depends on `story:bitwise-operators`; `std.data` already has a `base64` helper (`story:data-value-model`), which moves here or calls this.

## Acceptance

- `std.digest.sha1(Bytes) -> Bytes`, `sha256`, and an incremental form for large inputs; `std.encoding.base64` encode and decode, standard and url-safe, padding rules of RFC 4648.
- Test vectors of RFC 3174, FIPS 180-4 (the NIST examples) and RFC 4648 on both targets; a 1 MiB input within 3x of the Rust target's hand-written equivalent (recorded).
- Layering rule kept; `task gate` green.

## Scope

- `std/digest.ignite`, `std/encoding.ignite` (new), `std/data.ignite`, `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `docs/slice.md`. Inferred.
