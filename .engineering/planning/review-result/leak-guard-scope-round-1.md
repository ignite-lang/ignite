---
format: aep.planning-md/3
id: review-result:leak-guard-scope-round-1
kind: review-result
status: active
title: Scope critic, leak-guard round 1
relations:
- reviews: epic:leak-guard
- reviews: story:leak-census
- reviews: story:leak-check-tests
- reviews: story:runtime-metrics
- reviews: story:channel-release-on-close
revision: 1
---
needs-revision

- story:channel-release-on-close — the epic says "the fix is proven by the guard's test" and design section 7 requires G30's soak to pass with the fix on both targets. The acceptance names only an RSS test (line 44) and never the soak test "a closed channel is freed (G30)". Meanwhile `story:leak-census` claims the post-fix half of that outcome (line 63), and that half cannot be met until this story's fix lands. The body should say the fix is done when that soak test, with the case study's reproducer in place of the three lines, passes on both targets. `leak-census` should then claim only the red-before result and the seeded cases.

What I read: the parent's promises, extracted before opening any item, were these 8.
1. A leak fails under `ignite test` and the gate on both targets.
2. Live objects are counted per kind.
3. Heap bytes are counted on both targets.
4. A test can soak a body.
5. Every test is checked for leftovers.
6. A server exports the counts as metrics.
7. The G30 reproducer and fix.
8. Design section 7 holds on both targets and `task gate` is green.

I traced 8 of 8. Items 2, 3 and 4 go to `leak-census`, items 1 and 5 to `leak-check-tests`, item 6 to `runtime-metrics`, and item 7 to `leak-census` and `channel-release-on-close`. Item 8 is covered by the three stories' own acceptances. The one overlap is the finding above.

I found no reach beyond the parent. Every scope entry and every acceptance clause traces to a design section. That covers the `CODEGEN_REVISION` bump, `--no-leak-check`, the `quality/baseline.json` count row, `GET /runtime` and exit code 102.

What I read, in order:
- Artifacts: `epic:leak-guard`, the three stories and `story:channel-release-on-close`, plus `docs/leak-guard.md`.
- Commands: `aep plan artifact show` for each of the five, `aep plan artifact graph`, and line lookups in the store files.

What I could not establish:
- Whether `leak-census` can be marked done before the fix lands. That is acceptance and design, not scope, so I left it to `plan-critic-acceptance` and `plan-critic-design`.
- Nothing records the epic's constraint that runtime cost is "measured by the next release's `task quality-remote`" in a story acceptance. Only `leak-census` carries a cost note, in its context. That is acceptance lane, not mine.

```findings
- file: .engineering/planning/story/channel-release-on-close.md
  line: 44
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance proves the fix only by an RSS test and never names the guard's soak test 'a closed channel is freed (G30)', though the epic says the fix is proven by the guard's test and leak-census (line 63) also claims its post-fix pass, so the same outcome is split across two stories"
```
