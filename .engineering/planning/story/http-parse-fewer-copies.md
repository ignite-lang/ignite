---
format: aep.planning-md/3
id: story:http-parse-fewer-copies
kind: story
status: proposed
title: HTTP request parsing without std-level copies
relations:
- decomposes: epic:native-parity
- depends_on: story:text-fast-search
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T05:10:09Z", actor: "human:timo", revision: 2}
---
# Story: http-parse-fewer-copies

## Outcome

`std.protocol.http` and `std.text` parse a request without the copies written in the std source (Request clones, split pieces, quoting, head parsing, lower-casing of header names), so a todo GET allocates and executes about what the plain-Rust reference does on both targets.

## Context

- Epic epic:native-parity. After story:rustgen-fewer-allocations the Rust-target todo GET still makes 98 allocations per reply (from 191); the rest are copies in std code: Request clones (about 12 per reply), split pieces (17), quote (8), parse_head (8), to_lower (6). Instructions per reply: get-64 3.21x of the reference.

## Acceptance

- Behaviour and the conformance claims of `std.protocol.http` unchanged (`ignite conformance`, inline tests on both targets).
- Allocations per reply (dhat, Rust target, todo get, 1 connection) at most 30; LLVM and Rust-target instructions per reply recorded before and after (method of the epic); `task gate` green.

## Scope

- `std/protocol/http.ignite`, `std/text.ignite`, `examples/todo/todo.ignite`. Inferred.
