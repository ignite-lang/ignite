---
format: aep.planning-md/3
id: review-result:leak-guard-parallel-round-1
kind: review-result
status: active
title: Parallel-safety critic, leak-guard round 1
relations:
- reviews: epic:leak-guard
- reviews: story:leak-census
- reviews: story:leak-check-tests
- reviews: story:runtime-metrics
- reviews: story:channel-release-on-close
revision: 1
---
needs-revision

- story:leak-check-tests — shares `std/runtime.ignite` (`Runtime.leaves_running` is declared in `std.runtime`, design section 5.2, while `story:runtime-metrics` adds `lines` there), `docs/slice.md` and the `std/`, `examples/` and `crates/ignite/tests` trees with `story:runtime-metrics`. Neither body mentions the other, and both depend only on `story:leak-census`, so the epic's order table (2 and 3, both "needs 1") reads as parallel. `std/runtime.ignite` is not in the tree yet, because `story:leak-census` creates it. Remedies, not chosen: an ordering edge recording the shared file, or split the surface (for example, put `leaves_running` in a file the other story does not touch). Cited — .engineering/planning/story/leak-check-tests.md:61-65, .engineering/planning/story/runtime-metrics.md:41-43, docs/leak-guard.md:212-226
- story:leak-check-tests — its acceptance "`task test-ignite` runs every test of `std/` and `examples/` under the check" (line 58) reads tests that `story:runtime-metrics` adds: the inline `lines` test in `std/runtime.ignite` and the todo example's route. No `depends_on` edge runs from the reader to the producer, so the sweep and its opt-out count can be settled before those tests exist. The missing edge is `story:leak-check-tests` depends_on `story:runtime-metrics`, or the two merge. Cited — .engineering/planning/story/leak-check-tests.md:58, .engineering/planning/story/runtime-metrics.md:37
- story:leak-check-tests — its scope names whole trees (`std`, `examples`, `crates/ignite-llvm/src`, `crates/ignite-types/src`) although the body needs only the builtin tables and the `ignite test` flag. By the directory-covers-files reading in the wave plan, it collides with every story under those trees. Two members of the set are already inside it: `story:channel-release-on-close` (`crates/ignite-llvm/src/lower.rs` and `own.rs`, no edge) and `story:runtime-metrics` (`std/runtime.ignite`, `examples/todo/todo.ignite`). Narrow the scope to the files that change, and say the sweep's opt-out files are found by the sweep. Cited — .engineering/planning/story/leak-check-tests.md:61-65, .engineering/planning/story/channel-release-on-close.md:48-50, .engineering/waves/plan-2026-11-07.md:46
- story:leak-census — its scope lists `crates/ignite-io/src`, `crates/ignite-rt/src` and `crates/ignite-types/src` as whole directories, while its Scope section names the files for the first two. `ignite-types/src` has no file named at all ("lowering and builtins"). In wave 8 this reads as a collision with every story that touches those crates, so the entries should be the named files. Cited — .engineering/planning/story/leak-census.md:67-70

- story:leak-census — its G30 acceptance ("fails under the guard before the fix … and passes … with the fix; the wave record shows both runs") reads state that `story:channel-release-on-close` produces. The only edge runs the other way (the fix `depends_on` the census), so it cannot be reversed without a cycle. The census story therefore cannot reach done on its own, and the "fails before the fix" half may never occur: the fix story records G30 as not reproduced on `main`. Move the before-and-after-fix half into `story:channel-release-on-close`, which already depends on the census, or merge the two. Cited — .engineering/planning/story/leak-census.md:63, .engineering/planning/story/channel-release-on-close.md:42-46

What I read: 5 artifacts, using `aep plan artifact show` on each, `aep plan artifact graph` and `aep plan artifact waves`. I also read `docs/leak-guard.md` in full, the wave plan `.engineering/waves/plan-2026-11-07.md`, and ran `ls`/`git grep` checks on the cited files.

Surfaces: 5 items placed from file paths in their Scope sections, 0 by my own inference, 0 unplaceable. Those Scope sections label every path "Inferred", so the claims are only as strong as the scoper's guess. I checked that most paths exist. `std/runtime.ignite` and `crates/ignite-io/src/census.rs` do not exist yet. Acceptance reads traced to a producer in the set: 6, and 4 of them have a `depends_on` edge. The two without an edge are the second and fifth findings.
- `story:leak-check-tests` reads `IGNITE_DEBUG_LEAK` and the counters from `story:leak-census` (edge).
- `story:runtime-metrics` reads `Runtime.census()` from `story:leak-census` (edge).
- `story:channel-release-on-close` depends on `story:leak-census` (edge), and the two share `channel.rs`, `net.rs`, `docs/slice.md` and `crates/ignite/tests/main.rs`. I did not make that a finding, since the edge covers the shared files.
- `story:leak-check-tests` reads tests that `story:runtime-metrics` adds (no edge; second finding).
- `story:leak-census` reads the fix that `story:channel-release-on-close` produces (reversed, no edge; fifth finding).
- `story:leak-check-tests` and `story:runtime-metrics` read the `std/runtime.ignite` that `story:leak-census` creates (edge).

What I could not establish:
- Whether `story:leak-check-tests` has to change `crates/ignite/src/apply/plan.rs` (`CODEGEN_REVISION`). If `Runtime.leaves_running` changes LLVM generation, it would, and so would `story:channel-release-on-close` if its lowering change does. Neither scope lists the file, and I did not make it a finding.
- Collisions with other wave-8 stories outside this set (`drain-keeps-bound-listeners`, `infer-channel-ends` and the rest). Those belong to a separate pass.
- Out of my lane (acceptance or design): the epic and wave plan say the guard's G30 test proves the fix, but `story:channel-release-on-close`'s own acceptance is an RSS test and never names the soak test.

```findings
- file: .engineering/planning/story/leak-check-tests.md
  line: 61
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "this story and story:runtime-metrics both change std/runtime.ignite (Runtime.leaves_running is declared in std.runtime, design 5.2, while runtime-metrics adds lines there), docs/slice.md and the std, examples and crates/ignite/tests trees, neither body mentions the other, and both depend only on story:leak-census; std/runtime.ignite is created by leak-census and not yet in the tree; remedies: an ordering edge recording the shared file, or split the surface"
- file: .engineering/planning/story/leak-check-tests.md
  line: 58
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance sweeping every test of std/ and examples/ reads the inline lines test in std/runtime.ignite and the todo example's route that story:runtime-metrics adds, and no depends_on edge runs from this story to story:runtime-metrics; add the edge or merge the two"
- file: .engineering/planning/story/leak-check-tests.md
  line: 61
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the scope names whole trees (std, examples, crates/ignite-llvm/src, crates/ignite-types/src) for a builtin table entry and a flag, so by the directory-covers-files reading it collides with story:channel-release-on-close (ignite-llvm/src/lower.rs, own.rs) and story:runtime-metrics with no edge; narrow it to the files that change"
- file: .engineering/planning/story/leak-census.md
  line: 67
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the scope lists crates/ignite-io/src, crates/ignite-rt/src and crates/ignite-types/src as whole directories while the Scope section names files for the first two and none for ignite-types/src, so in wave 8 it reads as colliding with every story touching those crates; list the named files"
- file: .engineering/planning/story/leak-census.md
  line: 63
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the G30 acceptance (fails before the fix, passes with it, wave record shows both runs) reads the fix that story:channel-release-on-close produces, the only edge runs the other way so the census story cannot close on its own and the fails-before half may never occur; move that half into channel-release-on-close, which already depends_on leak-census, or merge the two"
```
