---
format: aep.planning-md/3
id: review-result:leak-guard-design-round-1
kind: review-result
status: active
title: Design critic, leak-guard round 1
relations:
- reviews: epic:leak-guard
- reviews: story:leak-census
- reviews: story:leak-check-tests
- reviews: story:runtime-metrics
- reviews: story:channel-release-on-close
revision: 1
---
needs-revision

story:leak-census — its G30 acceptance (soak test fails before the fix, passes after it, wave record shows both runs) reads the fix that story:channel-release-on-close produces, while that story already depends_on leak-census, so leak-census cannot finish. Both stories also claim the same G30 reproducer test, in two places with two forms: leak-census in a soak in `std/sync.ignite`, channel-release in an RSS test under `crates/ignite/tests/`. The fix is to cut leak-census's G30 line to "the soak test exists in `std/sync.ignite` and passes on the three reported lines". The red-before, green-after leg moves into channel-release-on-close's acceptance, which should name that soak test. No new edge is needed, since the existing depends_on from channel-release to leak-census already records the order. — `.engineering/planning/story/leak-census.md:63`, `.engineering/planning/story/channel-release-on-close.md:44`

What I read: 5 artifacts (epic:leak-guard, story:leak-census, story:leak-check-tests, story:runtime-metrics, story:channel-release-on-close) and `docs/leak-guard.md`, using `aep plan artifact show`, `relations`, `graph` and `validate`. I walked 7 edges: 4 depends_on or informed_by edges (all on leak-census except one to `story:channels-and-timers`, outside the set) and 4 decomposes edges (the three stories to the epic, plus leak-census to the epic). The graph has no cycle, and it is not a chain: three stories fan out from leak-census. `validate` reports `valid`; its warnings concern other artifacts. Of 6 acceptance reads that cross a story boundary, 5 trace to leak-census, and each of those has a depends_on edge. The sixth is the G30 read above, which runs the wrong way. The reads were:
- `IGNITE_DEBUG_LEAK` and the leak fixture (leak-check-tests)
- `Runtime.census()` and the `Census` record (runtime-metrics)
- `std/runtime.ignite` (runtime-metrics)
- the census counters (leak-check-tests)
- the soak fix (leak-census, the sixth)

What I could not establish:
- Whether the "plain test" that leak-check-tests runs under `IGNITE_DEBUG_LEAK=channel` is added by leak-census or by leak-check-tests. The design says the fixture holds one soak test and one plain test, but leak-census's acceptance names only the soak test. An edge already orders the two, so I did not count this as a finding.
- Out of my lane, so it does not set the verdict: leak-census, leak-check-tests and runtime-metrics all list `std/runtime.ignite` or `docs/slice.md` in scope. depends_on edges already order them, so this is for the parallel-safety critic.

```findings
- file: .engineering/planning/story/leak-census.md
  line: 63
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its G30 acceptance (soak red before the fix, green after, wave record shows both runs) reads the fix that story:channel-release-on-close produces while that story depends_on leak-census, and both stories claim the G30 reproducer test (soak in std/sync.ignite versus RSS test in crates/ignite/tests), so the red-before and green-after leg belongs in channel-release-on-close's acceptance and leak-census keeps only the soak test passing on the three reported lines"
```
