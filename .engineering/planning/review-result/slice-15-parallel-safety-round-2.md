---
format: aep.planning-md/3
id: review-result:slice-15-parallel-safety-round-2
kind: review-result
status: active
title: Parallel-safety critic, slice 15 round 2
relations:
- reviews: epic:slice-15-protocols-p2
- reviews: story:slice-15-contract
- reviews: story:slice-15-frontend
- reviews: story:slice-15-rfc-importer
- reviews: story:slice-15-claim-model
- reviews: story:slice-15-catalogues
- reviews: story:slice-15-conformance-command
- reviews: story:slice-15-protocol-claims
- reviews: story:rustgen-type-parameter-shadowing
revision: 1
---
needs-revision

- story:slice-15-claim-model — its scope says "No change in `crates/ignite/`", but the acceptance runs `ignite check` against `.expected` fixtures, and no harness for that exists in `crates/ignite-eval`. The only runner of eval-owned `.expected` fixtures is `crates/ignite/tests/plan.rs::fixtures_match_expected`. It spawns `ignite check` and filters fixture names on `e11` and `e12`. `ignite-eval` has no dependency on `ignite`, so its own tests cannot do this. The story therefore lands in `crates/ignite/tests/`, in `plan.rs`, a new test file with a `mod` line in `main.rs`, or `fixtures/`. The `main.rs` option is the file `story:slice-15-rfc-importer` edits in the same wave 3, with no edge between them, and `story:slice-15-conformance-command` edits it too. This is an inferred surface. The body must name the harness file. It must then either order itself after the importer, or put the E090/E091/E092 fixtures in `plan.rs` with no new `mod` line (surface split) — `.engineering/planning/story/slice-15-claim-model.md:42`, `:48`, `crates/ignite/tests/plan.rs:337-348`, `crates/ignite-eval/Cargo.toml` (`[dependencies]` lists only `ignite-syntax` and `ignite-types`)

**What I read.** Eight artifacts: the seven slice-15 stories and `story:rustgen-type-parameter-shadowing`, plus the epic. I ran `aep plan artifact show` on each, `aep plan artifact graph` and `aep plan artifact waves --status draft`. I checked the cited code with `git grep` and by reading the files. That covered `crates/ignite/src/lib.rs` (module list at lines 15 to 18, `Checked::check` at line 1115), `crates/ignite/tests/main.rs`, `plan.rs`, the AST literals across the crates, `crates/ignite-check/tests/layering.rs` and `docs/layering.md`. I did not read the round-1 review results.

**Surfaces.**
- Of 8 items, 8 were placed by what their bodies cite, 0 by inference alone and 0 were unplaceable. The one finding rests on an inferred landing site.
- I traced 16 acceptance reads to a producer inside the set. 10 are recorded by a direct `depends_on` edge. 6 are reached only through a chain of edges: the contract's D3, D4/D5 and D8, the section-19 table, and E090 read by protocol-claims.
- The ordered pairs are all recorded:
  - frontend, shadowing and contract, on `units.rs` and `golden.rs`;
  - importer and conformance-command, on `lib.rs`, `main.rs` and `tests/main.rs`;
  - conformance-command and protocol-claims, on `tests/conformance.rs`;
  - frontend and catalogues, on `layering.rs` and `docs/layering.md`.
- The wave tool's 6 collisions are all on edge-ordered pairs, so none is open.
- The only AST struct literals outside `ignite-syntax` are the two in `crates/ignite-check/src/lib.rs` (test code under the frontend's scope) and `crates/ignite-rustgen/tests/units.rs`. The frontend's scope covers all of them.

**What I could not establish.**
- Conditional on the contract's D2, D5 and D7 outcomes, which don't exist yet:
  - If D2 makes catalogue constants compile on both targets, or never emits them, `ignite-rustgen/src` and `ignite-llvm/src` may need edits that the frontend's scope omits. No other item touches `ignite-llvm`, and the shadowing story is already ordered before the frontend, so this would be a scope question, not a collision.
  - D5's vendored-source path could land inside the importer's `tests/fixtures/rfc/`. The catalogues story is already ordered after the importer.
- Out of my lane, not counted in the verdict:
  - The frontend's scope omits `crates/ignite-syntax/tests/` and `crates/ignite-check/tests/fixtures/std`, although its parser tests and E091 fixture presumably live there. `fixtures.rs` asserts the fixture std module list exactly. This is a scope and acceptance matter.
  - The D2 runtime-use diagnostic has no code allocated in the contract's scope.

```findings
- file: .engineering/planning/story/slice-15-claim-model.md
  line: 42
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "scope says no change in crates/ignite/, but the acceptance runs `ignite check` on .expected fixtures and the only runner of eval-owned fixtures is crates/ignite/tests/plan.rs (ignite-eval has no dependency on ignite), so the story lands unnamed in crates/ignite/tests (plan.rs, a new file with a mod line in tests/main.rs, or fixtures/), which the importer and conformance-command also edit; inferred surface, name the harness file and either order after the importer or keep the fixtures in plan.rs"
```
