---
format: aep.planning-md/3
id: epic:slice-16-language-gaps
kind: epic
status: active
title: 'Slice 16, wave A language gaps: closures, last-use moves and the small gaps'
summary: 'Wave A of docs/vision.md 4.1 before actors: closures, last-use moves, and the remaining language gaps on both targets'
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-03T22:40:55Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-03T22:40:55Z", actor: "human:timo", revision: 3}
---

# Epic: slice-16-language-gaps

## Outcome

Wave A of `docs/vision.md` section 4.1 (language gaps) lands as slice 16, so that wave C (actors
P1 to P3) can start: a fn value can capture locals (closures), a `var` passed and reassigned moves
at its last use, and the smaller gaps of the wave row are closed or explicitly deferred, on both
targets.

## Context

- `docs/vision.md` 4.1, row "A language gaps": closures (`docs/friction.md` 9, 13; actors P4),
  `Option.unwrap_or` and `!=` on Option (7), rest patterns `..` (protocols §5.2), ranges in
  patterns (8), raw strings (2), tuple field access (10), `break` as expression (4), recursive
  enums by boxing (15), `Map<K, V>` (design §3.3), last-use moves (actors R1), T1 failure rule.
- Row "C actors P1 to P3" needs "A (last-use moves), T1"; row "15+" of the slice table lists
  "B (P2 on), C, D, E, G, H", which skips A although C needs it. This epic takes A first.
- T1 landed in slice 10. Which other items of the row already hold is to be established by the
  contract from the code and `docs/friction.md`, not assumed.

## Acceptance

- `docs/slice.md` section 20 is the slice contract: an acceptance table, units with file scopes,
  waves, decisions, reviewed against `docs/vision.md` section 6 before implementation.
- Every item of the wave-A row is either delivered with a test on both targets or listed in 20.x
  as deferred with a reason.
- `examples/todo` `patch` uses a closure (friction 9 closed).
- `task gate` passes on the slice branch; release `v0.5.0` after the slice merges.

## Out of Scope

- Actors (wave C), async (wave D), wasm (wave E).
