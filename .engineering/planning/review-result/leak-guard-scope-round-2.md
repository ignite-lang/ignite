---
format: aep.planning-md/3
id: review-result:leak-guard-scope-round-2
kind: review-result
status: active
title: Scope critic, leak-guard round 2
relations:
- reviews: epic:leak-guard
- reviews: story:leak-census
- reviews: story:leak-check-tests
- reviews: story:runtime-metrics
- reviews: story:channel-release-on-close
revision: 1
---
approve

Round 1's one finding is resolved, and the revision introduced no new scope problems.

- **Round 1 finding, resolved.** `story:channel-release-on-close` now names the soak test "a closed channel is freed (G30)" in its first acceptance bullet (the case study's reproducer as its body, red before the fix on the target(s) that leak, green after on both targets, both runs in the wave record). `story:leak-census` now says it "closes without the reproducer", so only the channel story claims red-before and green-after.
- **No double claim of G30.** `leak-census` owns the instrument: the soak test with the three lines, passing on `main`. The channel story owns the reproducer swap and the fix. The epic's G30 acceptance, including the "or records G30 as not reproduced" branch, matches the channel story's third bullet.
- **No new gaps.** I extracted 9 promises from the epic, each traced to one item: per-kind counts, heap bytes on both targets, soak, every test checked, metrics, G30, the seeded leaks, the `baseline.json` count row, and gate green. The mapping:
  - census, heap bytes, soak and the seeded leaks: `leak-census`
  - the per-test check, the opt-out sweep and the `baseline.json` row: `leak-check-tests`
  - metrics: `runtime-metrics`
  - G30: `channel-release-on-close`
  - gate: each story's own acceptance
- **No reach beyond the parent.**
  - `leak-census` seeds more kinds than design section 7 lists (task, listener, stream, pipe, child, directory, capability, and the four design ones), but the epic's own acceptance says "a seeded leak of every compared kind". It is traceable to the parent.
  - `--no-leak-check`, exit code 102, the E051 fixture and `GET /runtime` are all traceable to design sections 5.3, 5.4 and 7.
- **Epic ordering differs from the design.** The epic makes `leak-check-tests` depend on `runtime-metrics`, where the design lists it as depending on story 1 only. The epic states the reason (both edit `std/runtime.ignite`), so it is a recorded decision, not a silent narrowing.

What I read: 6 store artifacts and the design doc. The commands were `aep plan artifact show` for the epic, the four stories and `review-result:leak-guard-scope-round-1`, `aep plan artifact graph`, and `git show origin/main:docs/leak-guard.md`.

What I could not establish:
- **Out of my lane (acceptance):** the census acceptance does not assert that `arena_bytes` or `pooled_bytes` are exercised, although design 5.1 lists them.
- **Out of my lane (design):** the epic reorders the design's story 2 and 3 dependency; I judged it a recorded decision, not a coverage defect.

```findings
[]
```
