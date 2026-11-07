---
format: aep.planning-md/3
id: story:map-put-handle-roots
kind: story
status: draft
title: A handle stored in a Map keeps its roots
relations:
- informed_by: review-result:adversary-ownership-pass-1
revision: 1
---
# Story: map-put-handle-roots

## Outcome

A handle stored in a `Map` (or any container through a value-changing builtin) keeps its roots, so reading it back and using it needs the same effects as through a `List`.

## Context

- Found by the adversary of `story:shared-ownership-analysis` (`review-result:adversary-ownership-pass-1`, 2026-11-07), pre-existing on base `6fa8da18`.
- Site: `crates/ignite-check/src/lib.rs:2709` (at `fa52517b`): root extension covers `push`, `set` and `append` only, a set kept apart from `ignite_check::IN_PLACE`.
- Red case: `ownership_adversary::map_put_of_a_handle_keeps_its_roots`: a `TcpStream` put into a `Map`, read back with `get` and used with `read`, needs no `requires [s.read]` (no E016), while the `List.push` version gets E016.
- What reaches it: any program that stores a stream in a `Map`; an effect-soundness gap.

## Acceptance

- `map_put_of_a_handle_keeps_its_roots` passes: E016 for the `Map` program as for the `List` one.
- Root extension reads the value-changing rows of `IN_PLACE` that store an argument into the receiver; a test lists which rows extend roots.
- `task gate` green.
