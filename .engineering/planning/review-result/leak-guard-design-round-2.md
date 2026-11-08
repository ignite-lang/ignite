---
format: aep.planning-md/3
id: review-result:leak-guard-design-round-2
kind: review-result
status: active
title: Design critic, leak-guard round 2
relations:
- reviews: epic:leak-guard
- reviews: story:leak-census
- reviews: story:leak-check-tests
- reviews: story:runtime-metrics
- reviews: story:channel-release-on-close
revision: 1
---
approve

I found no defects in the set's shape. Round 1's blocker is resolved and the revision introduced no new problem.

Round 1 finding, resolved:
- `story:leak-census` now closes without the case study's reproducer. Its G30 acceptance reads: "this story closes without the reproducer".
- The red-before, green-after leg now sits in `story:channel-release-on-close`'s acceptance. That story names the soak test "a closed channel is freed (G30)" in `std/sync.ignite` as the place the reproducer runs. Its `depends_on story:leak-census` edge already records the order.
- The unclear owner of the plain test is also fixed. `story:leak-census`'s acceptance now says the fixture under `crates/ignite/tests/fixtures/leak/` holds one soak test per kind plus one plain test, and `story:leak-check-tests` reads that plain test. Its edge to `leak-census` covers the read.

Revision checks:
- **Added edge:** `story:leak-check-tests` now has `depends_on story:runtime-metrics`. The reason is written beside it in the epic table: both edit `std/runtime.ignite`. The sweep also covers the tests `runtime-metrics` adds. This is the shared-file ordering trade-off, not an unexplained chain. The set stays a fan-out from `leak-census`, with `channel-release-on-close` off to one side. It is not a queue.
- **No cycle:** the edges are `runtime-metrics -> leak-census`, `leak-check-tests -> {leak-census, runtime-metrics}` and `channel-release-on-close -> leak-census`. All of them mean "needs first". `channel-release-on-close -> story:channels-and-timers` is `informed_by`, so it is not a blocker.
- **No split abstraction:** `Runtime.leaves_running` is declared in `leak-check-tests` and only documented in `std/runtime.ignite`. `lines()` lives only in `runtime-metrics`. The census is produced in one story.

What I read: 5 artifacts, plus `review-result:leak-guard-design-round-1` and `docs/leak-guard.md` on `origin/main`. I ran `aep plan artifact show` on each, then `relations`, `graph` and `validate`. `validate` printed `valid`.

I walked 8 edges (5 `depends_on`, 3 `decomposes`), including the `informed_by` edge to `story:channels-and-timers` outside the set. I traced 8 acceptance reads of state another story in the set produces, and a `depends_on` edge records all 8:
- `IGNITE_DEBUG_LEAK`
- the leak fixture
- `Runtime.census()` and its fields (`todo_handles`, `todo_resident_bytes`, `todo_streams`)
- `std/runtime.ignite`
- the G30 soak test in `std/sync.ignite`
- the metrics tests the sweep covers

What I could not establish:
- The design doc's story table still says `leak-census` includes "G30's reproducer red before the fix" (`docs/leak-guard.md`, near line 367). The artifacts diverge from it. Which one wins is a design-document matter outside the shape of this set. It does not affect my verdict.
- Whether the shared-file edits (`std/runtime.ignite`, `std/sync.ignite`, `docs/slice.md`, `crates/ignite-io/src/channel.rs`) are safe at wave time is the parallel-safety critic's lane. The edges already order them.

```findings
[]
```
