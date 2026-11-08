---
format: aep.planning-md/3
id: review-result:x-web-parallel-safety-round-1
kind: review-result
status: active
title: Parallel-safety critic, x-web round 1
relations:
- reviews: epic:x-web
- reviews: story:web-core
- reviews: story:web-policies
- reviews: story:route-annotations
- reviews: story:given-and-defaults
- reviews: story:eval-closures-and-members
- reviews: story:durable-store
- reviews: story:web-collection
- reviews: story:web-openapi-client
- reviews: story:markup-views
- reviews: story:web-live
- reviews: story:web-markdown
- reviews: story:web-service-calls
- reviews: story:web-identity
- reviews: story:web-sessions
revision: 1
---
needs-revision

- story:route-annotations — its acceptance has `ignite plan` list annotated functions in the model, which needs function values encoded in component properties. `story:eval-closures-and-members` creates that, and no `depends_on` edge runs from this story to it. Both also land on `crates/ignite-eval/src`, so adding the edge fixes both (cited, both bodies, a reader beside its producer) — .engineering/planning/story/route-annotations.md:49, .engineering/planning/story/eval-closures-and-members.md:30
- story:web-identity — its acceptance has `ignite explain … TodoService.server.routes` print `GET /todos  public`. `story:web-live` creates that `routes` explain output with read sets, and no `depends_on` edge runs from identity to live. Add the edge or merge the two (cited, a reader beside its producer) — .engineering/planning/story/web-identity.md:58, .engineering/planning/story/web-live.md:54
- story:durable-store — its acceptance exercises `insert` on a `Store&lt;T&gt;`-shaped interface. Today's `Store` has `next_id` and `put` but no `insert` (`docs/slice.md` 11.3, table row `s.next_id()` … `s.upsert(id, f)`). `story:given-and-defaults` creates `Store.insert`, and no edge runs from durable-store to it (cited, a reader beside its producer) — .engineering/planning/story/durable-store.md:39, .engineering/planning/story/given-and-defaults.md:51
- story:durable-store — both this and `story:web-core` edit the layering table in `docs/layering.md` and `crates/ignite-check/tests/layering.rs`. Both also edit `AGENTS.md`, `crates/ignite/tests/main.rs` and `docs/slice.md`, no edge orders them, and neither body names the other. Either record the shared layering files as the reason for an ordering edge, or split the layering rows into one story (cited, both bodies) — .engineering/planning/story/durable-store.md:48, .engineering/planning/story/web-core.md:64
- story:route-annotations — it is unordered with `story:given-and-defaults` and `story:markup-views`. All three change `crates/ignite-syntax/src`, `crates/ignite-check/src`, `crates/ignite-types/src`, `crates/ignite-eval/src` and `docs/syntax.md`, and none mentions the others. `docs/web.md` §8 says "a wave takes at most two of" steps 6 to 12 for this reason, and the stories do not carry it. Record the shared surface as an ordering edge or split it (cited, bodies) — .engineering/planning/story/route-annotations.md:55, .engineering/planning/story/given-and-defaults.md:56, .engineering/planning/story/markup-views.md:59, docs/web.md:§8 note after the table
- story:web-policies — the scope claims six crates (`ignite-io`, `ignite-rt`, `ignite-llvm`, `ignite-rustgen`, `ignite-check`, `ignite-types`), `plan.rs` and `std/net.ignite` for one deadline builtin plus `x/` code. That collides with `story:given-and-defaults`, `story:web-live` and `story:route-annotations` on rt, llvm, rustgen, types, check, `plan.rs` (`CODEGEN_REVISION`) and `docs/slice.md`. None of them is ordered against policies and none says so. Either split the deadline builtin into its own story, or record the shared surfaces as edges (cited, bodies) — .engineering/planning/story/web-policies.md:71, .engineering/planning/story/given-and-defaults.md:57, .engineering/planning/story/web-live.md:63
- story:web-identity — it is unordered with `story:web-service-calls`. Both edit `x/web/client.ignite` (`api.as`, `fake`), `x/web.ignite`, `crates/ignite-eval/src` and `crates/ignite/tests/golden`, and neither names the other. Only `story:web-sessions` joins them. Order them by an edge naming the shared files, or split the surface (cited, both bodies) — .engineering/planning/story/web-identity.md:64, .engineering/planning/story/web-service-calls.md:53
- story:web-markdown — it is unordered with `story:web-live`, `story:web-identity` and `story:web-service-calls`. All four edit `x/web.ignite` and `crates/ignite-eval/src`. Markdown and live also share `crates/ignite-check/src`, `crates/ignite-types/src` and `plan.rs`. No body names another. Record an ordering edge or split the surface (cited, bodies) — .engineering/planning/story/web-markdown.md:55, .engineering/planning/story/web-live.md:62
- story:eval-closures-and-members — it is unordered with `story:given-and-defaults` on `crates/ignite-eval/src` (model, explain and `value.rs` against the `Store.insert` builtin) and `docs/slice.md`, and neither names the other. Record an ordering edge or split the surface (cited, bodies) — .engineering/planning/story/eval-closures-and-members.md:44, .engineering/planning/story/given-and-defaults.md:56

**What I read:**
- 14 stories and `epic:x-web`.
- Commands: `aep plan artifact show` for each story, `aep plan artifact waves --kind story --status draft`, `aep plan artifact graph`, `aep plan artifact relations`, and `docs/web.md` §8 and `docs/slice.md` 11.3.
- Surfaces: all 14 are placed from the `## Scope` sections, which cite paths, so I count 14 cited, 0 inferred and 0 unplaced. The store tags every scope entry "inferred".
- Acceptance reads: I traced about 27 to a producer in the set. 24 are recorded by a `depends_on` edge, directly or transitively. The 3 missing are the first three findings.

**What I could not establish:**
- `aep plan artifact waves` puts every web story in its own wave (6 to 18) on inferred collisions, so scheduled work may already be serial. No `depends_on` edge records that, and the other findings stand against the plan as recorded.
- `docs/slice.md` is in all 14 scopes. I treated it as append-only sections and did not raise it, but a person should decide.
- Out of my lane (design or acceptance): `with(...)` is named as `story:web-openapi-client`'s product in `story:web-service-calls` context (line 36 region) but absent from the openapi-client acceptance, while the service-calls scope also claims it. `story:web-policies` bundles the substrate deadline with the `x/` policy work. `story:web-policies` adds a `flate2` dev-dependency, so `Cargo.toml` and `Cargo.lock` are in no scope.

```findings
- file: .engineering/planning/story/route-annotations.md
  line: 49
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance has ignite plan list annotated functions as model properties, which needs the function-value encoding story:eval-closures-and-members creates (also on crates/ignite-eval/src), and no depends_on edge runs from this story to it; add the edge"
- file: .engineering/planning/story/web-identity.md
  line: 58
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance reads the `explain ... server.routes` output that story:web-live creates, and no depends_on edge runs from identity to live; add the edge or merge the two"
- file: .engineering/planning/story/durable-store.md
  line: 39
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance exercises `insert`, which today's Store lacks (slice 11.3 lists next_id and put) and story:given-and-defaults creates, and no depends_on edge runs from this story to it; add the edge"
- file: .engineering/planning/story/durable-store.md
  line: 48
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "this and story:web-core both edit docs/layering.md, crates/ignite-check/tests/layering.rs, AGENTS.md, crates/ignite/tests/main.rs and docs/slice.md, are unordered, and neither body says so; record an ordering edge naming the files or split the layering rows into one story"
- file: .engineering/planning/story/route-annotations.md
  line: 55
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "unordered with story:given-and-defaults and story:markup-views, all three change ignite-syntax, ignite-check, ignite-types, ignite-eval and docs/syntax.md, and none says so; record an ordering edge naming the shared surface or split it"
- file: .engineering/planning/story/web-policies.md
  line: 71
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the deadline builtin claims ignite-io, rt, llvm, rustgen, check, types, plan.rs and std/net.ignite, colliding with story:given-and-defaults, story:web-live and story:route-annotations on rt, llvm, rustgen, types, check, plan.rs and docs/slice.md with no edge or mention; split the deadline substrate into its own story or record ordering edges"
- file: .engineering/planning/story/web-identity.md
  line: 64
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "unordered with story:web-service-calls, both edit x/web/client.ignite, x/web.ignite, crates/ignite-eval/src and tests/golden, and neither names the other; record an ordering edge naming the files or split the surface"
- file: .engineering/planning/story/web-markdown.md
  line: 55
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "unordered with story:web-live, story:web-identity and story:web-service-calls, all edit x/web.ignite and crates/ignite-eval/src (and live shares check, types and plan.rs); no body names another; record an ordering edge or split the surface"
- file: .engineering/planning/story/eval-closures-and-members.md
  line: 44
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "unordered with story:given-and-defaults on crates/ignite-eval/src and docs/slice.md and neither names the other; record an ordering edge or split the surface"
```
