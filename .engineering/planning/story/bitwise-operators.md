---
format: aep.planning-md/3
id: story:bitwise-operators
kind: story
status: implemented
title: Bitwise operators on Int
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T01:25:59Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T01:26:47Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-08T06:51:08Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: bitwise-operators

## Outcome

`Int` has `&`, `|`, `^`, `<<`, `>>` and a bitwise not, and `Bytes` exposes byte get/set cheaply, so SHA-1, base64 and WebSocket masking are written directly in Ignite at native speed.

## Context

- Case study gap G7: `^` is E001; the service does bit-by-bit arithmetic per frame.

## Acceptance

- The operators parse with documented precedence (grammar table in `site/book/src/reference/grammar.md`), type on `Int` only (E040 otherwise), and agree in the evaluator and both targets; shifts by a negative or >= 64 amount are defined (documented, tested).
- A differential test over random operands for every operator on the evaluator and both targets.
- SHA-1 and base64 in Ignite (test vectors of RFC 3174 and RFC 4648) run within 3x of the Rust target's hand-written equivalent on a 1 MiB input (recorded).
- `docs/slice.md` section; `CODEGEN_REVISION` bumped; `task gate` green.

## Scope

- `crates/ignite-syntax` (lexer, parser), `crates/ignite-types`, `crates/ignite-eval`, `crates/ignite-rustgen`, `crates/ignite-llvm`, `site/book/src/reference/grammar.md`, `docs/slice.md`. Inferred.
