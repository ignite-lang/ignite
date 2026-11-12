---
format: aep.planning-md/3
id: story:rust-target-constants-built-once
kind: story
status: active
title: A module constant is built once on the Rust target
relations:
- informed_by: story:rust-target-shared-list-storage
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T12:37:03Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "after-v0.16.2"}
- {from: "proposed", to: "active", at: "2026-11-12T12:37:03Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "after-v0.16.2"}
---
# Story: a module constant is built once on the Rust target

## Outcome

A module constant whose type is not `Copy` (`Text`, `Bytes`, a list, a record, a map) is built once per process on the Rust target, and a use that only reads it allocates nothing.

## Why

Reported on 2026-11-12 from an allocation profile of the first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS, Rust target, release): two module constants were among the largest allocating sites of one request, a 16-byte text used as a digit table and a list of nine pairs of texts used as a lookup table.

The generator emits such a constant as a function that builds the value on every use, and the use copies the result again:

- `std/protocol/http.ignite:686` `const HEX_DIGITS: Text = "0123456789abcdef"` is emitted as `pub fn HEX_DIGITS() -> String`, and its use as `HEX_DIGITS().clone().into_bytes()`: two allocations for a read of 16 bytes.
- `std/json.ignite:59` `const PARSE_BOUNDS: Bounds = Bounds { .. }` is emitted as `pub fn PARSE_BOUNDS() -> Bounds` and passed as `&PARSE_BOUNDS()`.
- A list constant is a `vec![..]` with one `String` per text element on every use.

Constants of `Int`, `Bool`, `Byte` are not affected.

## Acceptance

- A count test: a function that reads a `Text` constant, a `Bytes` constant, a list-of-pairs constant and a record constant in a loop allocates 0 times per round after the first on the Rust target (today: at least one allocation per use, two for the text). The evaluator and the LLVM target give the same values.
- A use that needs an owned value (a by-value argument, a `var` initialised from the constant and changed) copies once, and a later change is not seen by other uses of the constant.
- The emitted form is stated in `docs/slice.md` section 6 (for example a `static` built on first use, read through a reference); no new `unsafe` site; the order of effects of a constant whose initialiser calls a function is stated (the checker's rule for constant initialisers decides whether one exists).
- The LLVM target's cost for the same four constants is counted and written down; a fix there is a separate story if the count is not 0.
- `CODEGEN_REVISION` is bumped; `task gate` green; a dedicated-host run shows no row worse.

## Scope

- `crates/ignite-rustgen/src/lib.rs` (the constant's item and every use site), `crates/ignite-rustgen/tests/golden/`, `crates/ignite/tests/` (count test), `docs/slice.md` section 6. Inferred.
