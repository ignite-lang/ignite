---
format: aep.planning-md/3
id: review-result:leak-guard-parallel-round-2
kind: review-result
status: active
title: Parallel-Safety critic, leak-guard round 2
relations:
- reviews: epic:leak-guard
- reviews: story:leak-census
- reviews: story:leak-check-tests
- reviews: story:runtime-metrics
- reviews: story:channel-release-on-close
revision: 1
---
needs-revision

- story:leak-check-tests — it and `story:channel-release-on-close` both change `crates/ignite-llvm/src/lower.rs` (cited, both Scope sections) and `docs/slice.md` (12.7 against 30.3), neither body names the other, and no `depends_on` edge or sentence records an order. The ordering exists only in the wave plan ("later wave") and in the CLI's scope-derived waves. Remedies, not chosen: an edge from this story to `story:channel-release-on-close` recording the shared file as its reason, or splitting the surface so `Runtime.leaves_running` lowering sits outside `lower.rs`. The sweep reads the `std/sync.ignite` G30 test body the fix story rewrites, so the same edge would cover that read. Repeats the narrowing part of round 1's third finding, which the revision only partly fixed. — .engineering/planning/story/leak-check-tests.md:65, .engineering/planning/story/channel-release-on-close.md:55

Round 1 status:
- Finding 1 (shared `std/runtime.ignite` between leak-check-tests and runtime-metrics) is resolved. `leak-check-tests` now has `depends_on story:runtime-metrics`, and its context names the shared file.
- Finding 2 (the sweep reads runtime-metrics' tests with no edge) is resolved by the same edge.
- Finding 3 (whole-tree scopes) is resolved except for the lower.rs pair above. `leak-check-tests` and `leak-census` now list named files only.
- Finding 4 (`leak-census` scope as whole directories) is resolved. Named files only, including `crates/ignite-types/src/{lib,typer,builtins}.rs`.
- Finding 5 (the G30 before/after-fix half in `leak-census`) is resolved. Its acceptance now closes without the reproducer (`leak-census.md` G30 bullet), and the fail-before and pass-after runs sit in `channel-release-on-close`, which depends on the census.

What I read: 5 stories and the epic plus `review-result:leak-guard-parallel-round-1`. Commands: `aep plan artifact show` on each, `aep plan artifact graph` and `aep plan artifact waves` (the in-set collision lines), `git show origin/main:docs/leak-guard.md` grepped for `leaves_running`, `CODEGEN` and `lower.rs`, the wave plan lines 16 to 24, and `ls` of the cited existing files.

Surfaces: 4 stories placed cited (named paths in each body's Scope section), 0 inferred, 0 unplaceable. The epic names no files and is not a work item. The Scope sections label every path "Inferred", so the claims are only as strong as the scoper's guess. `std/runtime.ignite`, `crates/ignite-io/src/census.rs` and `crates/ignite-rt/src/census.rs` do not exist yet. Acceptance reads traced to a producer in the set: 6, with 5 recorded by `depends_on`:
- `leak-check-tests` reads the leak fixture and `IGNITE_DEBUG_LEAK` from `leak-census` (edge).
- `leak-check-tests` reads runtime-metrics' tests in the sweep (edge).
- `runtime-metrics` reads `Runtime.census()` from `leak-census` (edge).
- `channel-release-on-close` reads the G30 soak test from `leak-census` (edge).
- `leak-check-tests` reads the `std/runtime.ignite` that `leak-census` creates (edge).
- `leak-check-tests` reads the `std/sync.ignite` test body that `channel-release-on-close` changes (no edge, covered by the finding above).

What I could not establish:
- Whether the test-only `Runtime.leaves_running` changes generated code. If it does, `leak-check-tests` has to move `crates/ignite/src/apply/plan.rs` `CODEGEN_REVISION`. Design line 163 says the revision moves once, in story 1, and the `leak-check-tests` scope does not list `plan.rs`.
- `story:channel-release-on-close` and `story:runtime-metrics` share only `docs/slice.md` (different sections) and the `crates/ignite/tests/` directory. I judged an append-only collision of that kind not worth a finding. A person may want it recorded.
- Collisions with stories outside the set. The waves output lists many, for example `drain-keeps-bound-listeners` on `lower.rs` and `plan.rs` with `leak-census`. I was not asked to judge them.
- Out of my lane: the epic's order table puts `runtime-metrics` before `leak-check-tests`, while the wave plan line 22 says both stay for a later wave and the epic says only `leak-census` and the fix share wave 8. That is consistent, but the epic does not say where `leak-check-tests` lands relative to the fix.

```findings
- file: .engineering/planning/story/leak-check-tests.md
  line: 65
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "this story and story:channel-release-on-close both change crates/ignite-llvm/src/lower.rs and docs/slice.md (cited, both Scope sections), neither body names the other and no depends_on edge records an order (it exists only in the wave plan); add an edge from this story to story:channel-release-on-close recording the shared file, or split the surface so Runtime.leaves_running lowering leaves lower.rs; the same edge would cover the sweep reading the std/sync.ignite G30 test body the fix rewrites"
```
