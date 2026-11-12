---
format: aep.planning-md/3
id: story:json-duplicate-keys
kind: story
status: implemented
title: std.json get returns the last duplicated member
scope:
- confidence: inferred
  path: crates/ignite/tests/data_json.rs
- confidence: inferred
  path: crates/ignite/tests/fixtures/data_json.ignite
- confidence: cited
  path: docs/slice.md
- confidence: cited
  path: site/book/src/reference/std/json.md
- confidence: cited
  path: std/json.ignite
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T03:32:56Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-08T21:40:02Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-12T12:37:08Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1}}, executor: "agent:claude-coordinator", correlation: "audit-2026-11-12"}
---
# Story: json-duplicate-keys

## Outcome

`std.json` `get` returns the last of duplicated object members, as ECMAScript's `JSON.parse` does, and `get_all` returns every member of a name, so a server agrees with JavaScript clients.

## Context

- Case study gap G29: `{"channel":"/a","channel":"/meta/handshake"}` gives `/a` in std and `/meta/handshake` in JavaScript; RFC 8259 section 4 leaves it open.
- `std.json.read` (`story:data-value-model`) refuses a repeated member; `parse` keeps every member. This story keeps that split and documents it.

## Acceptance

- `get` returns the last member of a name; `get_all(name) -> List<Json>` in document order; `encode` writes every member as parsed.
- Tests with the G29 document on both targets and in the evaluator; `docs/slice.md` 19.20 states the rule and the reason; the book's `std.json` page too.
- `task gate` green.

## Scope

- `std/json.ignite`, `docs/slice.md`, `site/book/src/reference/std/json.md`. Inferred.
