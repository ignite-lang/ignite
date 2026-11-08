---
format: aep.planning-md/3
id: review-result:leak-guard-acceptance-round-1
kind: review-result
status: active
title: Acceptance critic, leak-guard round 1
relations:
- reviews: epic:leak-guard
- reviews: story:leak-census
- reviews: story:leak-check-tests
- reviews: story:runtime-metrics
- reviews: story:channel-release-on-close
revision: 1
---
needs-revision

epic:leak-guard — the acceptance's "The three stories are implemented" is not a check, and `docs/leak-guard.md` section 7 has no clause for the metrics outcome, so the metrics outcome has no check at epic level — .engineering/planning/epic/leak-guard.md:39
story:leak-census — the first bullet names failing cases only for `channel`, `handle`, `heap_copy` and `bytes`, so `tasks`, `listeners`, `streams`, `pipes`, `children`, `directories`, `capabilities`, `stores` and `workers` can stay at 0 and every bullet still passes; the outcome says "count the live runtime objects of its process per kind" — .engineering/planning/story/leak-census.md:62
story:leak-census — the G30 bullet is satisfied by a passing test on `main` while the reproducer has not arrived ("Until the reproducer arrives the test holds the three reported lines and passes"), and its "passes ... with the fix" half is an outcome `story:channel-release-on-close` produces and also claims at its acceptance line 45, so it needs a stated closing condition for the no-reproducer case and a single owner of the after-fix pass — .engineering/planning/story/leak-census.md:63
story:leak-check-tests — the bullet "`Runtime.leaves_running(reason)` skips the check; ... `--no-leak-check` turns the check off for one run" names no before state, so nothing says the same leaking test fails without them and passes with them under `IGNITE_DEBUG_LEAK=channel` — .engineering/planning/story/leak-check-tests.md:57
story:channel-release-on-close — the second bullet branches on the unknown reproducer ("on the target(s) that leak. If no shape leaks on `main`, the story ends with the test on `main` green and G30 recorded as not reproduced"), and it names no place where "not reproduced" is recorded, so the done-state is not observable — .engineering/planning/story/channel-release-on-close.md:45

What I read: 5 artifacts (epic:leak-guard, story:leak-census, story:leak-check-tests, story:runtime-metrics, story:channel-release-on-close) with `aep plan artifact show` for each, `aep plan artifact kinds`, `docs/leak-guard.md` in full, and a `git grep` of the tree for `quality/baseline.json` (count rows exist), `E051`, `quality-remote` and `IGNITE_DEBUG_HOST_FAULT`. `std/runtime.ignite` does not exist yet, which is expected.

What I could not establish:
- I did not run `aep plan artifact lifecycle <kind>`, so I did not check status names or terminal states.
- story:runtime-metrics has no finding. The `todo_handles (LLVM)` clause does not say whether the Rust target must omit the line or report 0, a gap I did not raise as a finding.
- Out of my lane, for `plan-critic-design` and `plan-critic-parallel-safety`:
  - The `leak-census` G30 pass-after-fix cannot close before `channel-release-on-close` lands, although that story depends on `leak-census`.
  - The 200,000-task RSS test in `channel-release-on-close` sits uneasily with the AGENTS.md constraint of no local load tests.
  - `leak-census` and `channel-release-on-close` both touch `crates/ignite-rt/src` and `docs/slice.md`.

```findings
- file: .engineering/planning/epic/leak-guard.md
  line: 39
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance 'The three stories are implemented' is not a check, and docs/leak-guard.md section 7 has no clause for the metrics outcome, so the metrics outcome has no check at epic level"
- file: .engineering/planning/story/leak-census.md
  line: 62
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the first bullet names failing cases only for channel, handle, heap_copy and bytes, so tasks, listeners, streams, pipes, children, directories, capabilities, stores and workers can stay at 0 and every bullet still passes; the outcome says 'count the live runtime objects of its process per kind'"
- file: .engineering/planning/story/leak-census.md
  line: 63
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the G30 bullet is satisfied by a passing test on main while the reproducer has not arrived ('Until the reproducer arrives the test holds the three reported lines and passes'), and its 'passes with the fix' half is an outcome story:channel-release-on-close produces and also claims, so it needs a stated closing condition for the no-reproducer case and a single owner of the after-fix pass"
- file: .engineering/planning/story/leak-check-tests.md
  line: 57
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the bullet 'Runtime.leaves_running(reason) skips the check; ... --no-leak-check turns the check off for one run' names no before state, so nothing says the same leaking test fails without them and passes with them under IGNITE_DEBUG_LEAK=channel"
- file: .engineering/planning/story/channel-release-on-close.md
  line: 45
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the second bullet branches on the unknown reproducer ('on the target(s) that leak. If no shape leaks on main, the story ends with the test on main green and G30 recorded as not reproduced') and names no place where 'not reproduced' is recorded, so the done-state is not observable"
```
