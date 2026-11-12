---
format: aep.planning-md/3
id: epic:leak-guard
kind: epic
status: active
title: 'Leak guard: a leak fails a test, not a server'
summary: Per-kind runtime census, counted heap bytes, soak, a leak check in ignite test and runtime metrics, both targets
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-08T15:04:35Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-12T20:59:59Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":1}}, executor: "agent:claude-coordinator", correlation: "upkeep-2026-11-12"}
---
# Epic: leak-guard

## Outcome

The next memory leak in an Ignite program or its runtime fails a test under `ignite test` and the gate, on both targets, instead of showing as a production server that grows. Live runtime objects are counted per kind in the substrate, heap bytes are counted on both targets, a test can soak a body and assert that nothing grew, every test is checked for objects left behind, and a server can export the same counts as metrics.

## Context

- Request (Timo through the case-study session, 2026-11-08): beyond fixing G30, a programmatic guard so the next leak is caught by a test. G30 (`docs/research/case-study-gaps.md`): a closed channel's memory reported never freed, about 370 B per task; not reproduced on `main` from the three reported lines, a reproducer is requested (`story:channel-release-on-close`).
- Design: `docs/leak-guard.md` (PR 129). Kinds and where they live: section 2; options (a) to (d) weighed: section 4; the recommendation: section 5; target form and failure output: section 6; acceptance: section 7; open questions with defaults: section 9 (each story takes the default unless Timo answers).
- Not part of the guard: compile-time ownership of resources (option d, section 4); the handle table's high-water segments are reported, not judged (section 2, Q11).
- Constraint (Timo, 2026-11-08, AGENTS.md): no local load tests or benchmark batches. The guard is a test inside `ignite test` and the gate; its runtime cost is recorded as not measured locally and measured by the next release's `task quality-remote`.

## Stories

In order:

| order | story | needs |
|---|---|---|
| 1 | `leak-census`: counters, heap bytes, `std.runtime`, `soak`; G30's `alias` and `field` forms red on `main` by the live handle count; seeded leaks red | none |
| 2 | `runtime-metrics`: the census as metric lines, served by the todo example | 1 |
| 3 | `leak-check-tests`: `ignite test` fails a test that leaves a counted object behind; its sweep covers the tests of 2 and of the fix | 1, 2 (both edit `std/runtime.ignite`), `channel-release-on-close` (both edit `crates/ignite-llvm/src/lower.rs` and `std/sync.ignite`) |
| 4 | `unowned-handle-ratchet`: `own.rs`'s give-ups listed and held to a shrinking baseline | `channel-release-on-close` |

`story:channel-release-on-close` depends on `leak-census`: the guard's G30 soak tests are red before the fix and green after it, and that is the fix story's acceptance. The two land in one wave (wave 8), the guard first.

## Acceptance

- Census: every kind of `docs/leak-guard.md` 5.1 rises by one per object and returns on release, on each target where it exists, and a seeded leak of every compared kind (`IGNITE_DEBUG_LEAK`) fails a soak naming it (`story:leak-census`).
- G30: the soak tests of `docs/leak-guard.md` 6.1 are red on `main` on LLVM (recorded on `story:leak-census`) and green on both targets after `story:channel-release-on-close`.
- Check: a test that leaves a channel behind fails under `ignite test` on both targets as `FAILED (leaked 1 object)`, and every test of `std/` and `examples/` passes the check or opts out with a reason counted in `quality/baseline.json` (`story:leak-check-tests`).
- Metrics: `todod` on each target answers `GET /runtime` with census lines whose `todo_streams` is back at its earlier value after 100 requests (`story:runtime-metrics`).
- Static: a new unowned handle in `std/` or `examples/` fails the gate (`story:unowned-handle-ratchet`).
- `task gate` green at each story's merge.
