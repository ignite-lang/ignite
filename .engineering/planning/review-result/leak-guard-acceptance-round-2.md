---
format: aep.planning-md/3
id: review-result:leak-guard-acceptance-round-2
kind: review-result
status: active
title: Acceptance critic, leak-guard round 2
relations:
- reviews: epic:leak-guard
- reviews: story:leak-census
- reviews: story:leak-check-tests
- reviews: story:runtime-metrics
- reviews: story:channel-release-on-close
revision: 1
---
needs-revision
story:channel-release-on-close — the no-reproducer ending ("the G30 entry of `docs/research/case-study-gaps.md` stating \"not reproduced\" with the shapes tried") already holds on `main`, because G30 already reads "Not reproduced on `main` (05574a7c8 ...), 2026-11-08: ... one at a time and 1,000 at once, and the three lines looped 200,000 times", so that branch reads the same before the work as after it; the acceptance must name what the entry gains, such as the two test names and the shapes added beyond those already listed — .engineering/planning/story/channel-release-on-close.md:50 (against docs/research/case-study-gaps.md:350 on origin/main)
story:leak-census — `arena_bytes` is a compared kind in the soak (`docs/leak-guard.md` 5.1: "compared, 64 KiB slack"), but neither the census-per-kind bullet nor the seeded-leak list (`task`, `channel`, `listener`, `stream`, `pipe`, `child`, `directory`, `capability`, `handle`, `heap_copy`, `bytes`) covers it, so it can stay at 0 and every bullet still passes, although the epic claims "a seeded leak of every compared kind" — .engineering/planning/story/leak-census.md:97

Round 1 findings:
- **Resolved.**
  - **Epic bullets:** the epic's acceptance is now a list of checkable bullets, including a metrics bullet (epic:leak-guard).
  - **Per-kind seeded leaks:** the seeded-leak list now spans every kind except `arena_bytes`, which is the new finding above (story:leak-census, round-1 finding 2).
  - **G30 closing condition:** the story now closes without the reproducer, and the after-fix pass belongs to story:channel-release-on-close alone (story:leak-census, round-1 finding 3).
  - **Opt-out before state:** the opt-out bullets now state the fail-then-pass transition under `IGNITE_DEBUG_LEAK=channel` (story:leak-check-tests).
- **Partly resolved.** The fallback in story:channel-release-on-close now names a place, `docs/research/case-study-gaps.md`, but that statement is already true, which is the finding above.

What I read: 6 artifacts (epic:leak-guard, story:leak-census, story:leak-check-tests, story:runtime-metrics, story:channel-release-on-close, review-result:leak-guard-acceptance-round-1), with `aep plan artifact show`. I also read `aep plan artifact lifecycle story`, `docs/leak-guard.md` from `origin/main` in full, and the G30 entry of `docs/research/case-study-gaps.md` on `origin/main`. I checked E051 in `docs/slice.md` and the count rows in `quality/baseline.json` with `git grep` and `git show`.

What I could not establish:
- The `quality/baseline.json` count-row mechanism for opt-outs is not run. I confirmed only that count rows exist.
- `todo_handles (LLVM)` in story:runtime-metrics now reads as "asserted on LLVM only", since `Census.handles` is an `Int` that is 0 on Rust. I left it unraised.
- Out of my lane (design or scope), not setting the verdict:
  - The epic's G30 bullet repeats the "not reproduced" fallback; fixing the story's wording fixes it too.
  - The 200,000-task RSS test in story:channel-release-on-close against the AGENTS.md "no local load tests" constraint.
  - story:leak-check-tests `depends_on` story:runtime-metrics as well as story:leak-census.

```findings
- file: .engineering/planning/story/channel-release-on-close.md
  line: 50
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the no-reproducer ending (the G30 entry of docs/research/case-study-gaps.md stating \"not reproduced\" with the shapes tried) already holds on main, since G30 already says \"Not reproduced on main (05574a7c8 ...), 2026-11-08\", so that branch reads the same before the work as after it; the acceptance must name what the entry gains, such as the two test names and the shapes added beyond those already listed"
- file: .engineering/planning/story/leak-census.md
  line: 97
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "arena_bytes is a compared kind in the soak (docs/leak-guard.md 5.1) but neither the census-per-kind bullet nor the seeded-leak list covers it, so it can stay at 0 and every bullet still passes, although the epic claims a seeded leak of every compared kind"
```
