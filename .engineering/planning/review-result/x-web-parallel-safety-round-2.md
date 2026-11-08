---
format: aep.planning-md/3
id: review-result:x-web-parallel-safety-round-2
kind: review-result
status: active
title: Parallel-safety critic, x-web round 2
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

- story:eval-closures-and-members — its Ordering line says it and `story:web-core` "share only `docs/slice.md`", but its scope claims `crates/ignite/tests/` and its acceptance needs an end-to-end test and a fixture kind. `story:web-core` cites `crates/ignite/tests/main.rs` and a fixtures directory. A new test file needs a `mod` line in `main.rs` (AGENTS.md § Layout rules). The two are unordered, so they collide on `crates/ignite/tests/main.rs`, which the body does not say. Name that file beside `docs/slice.md` in the Ordering line, or add an ordering edge that records both files as its reason (cited for the directory and the `main.rs` claim; the `mod` line itself is inferred) — .engineering/planning/story/eval-closures-and-members.md:48, .engineering/planning/story/web-core.md:67

**What I read:**
- 14 stories and the round-1 record, with `aep plan artifact show` for each and `aep plan artifact waves --kind story --status draft`.
- I also read `aep plan artifact graph`, `docs/web.md` §6.2 and §8, and `crates/ignite-types/src/builtins.rs`.
- Surfaces: all 14 placed. 14 cited, 0 inferred, 0 unplaced (the store tags every scope entry "inferred"; the `## Scope` sections cite paths).
- Acceptance reads: I traced about 30 to a producer in the set, and every one is recorded by a `depends_on` edge, direct or transitive.
- The 14 stories are one total order. `eval-closures-and-members` and `web-core` come first and are unordered with each other. After them: `route-annotations`, `given-and-defaults`, `durable-store`, `web-collection`, `web-policies`, `web-openapi-client`, `markup-views`, `web-live`, `web-markdown`, `web-service-calls`, `web-identity`, `web-sessions`. Each step has an edge to the one before it.
- All nine round-1 findings are fixed: the edges exist and the Ordering lines name the shared files. `web-identity` now creates the `routes` explain output itself, so it no longer reads `web-live`'s. `web-service-calls` owns `with`.

**What I could not establish:**
- Out of my lane (scope): `story:web-policies` acceptance (line 65) uses a `scope` override, but no story in the set creates `scope(layers, routes)`. It appears in `docs/web.md` §6.2 and in no story's outcome or scope. This is not a reader-beside-producer finding because there is no producer in the set to link to.
- Out of my lane (scope): `story:durable-store` acceptance (line 45) wants a fn generic over an interface that both `Store&lt;T&gt;` and `Durable&lt;T&gt;` satisfy. `Store` is a compiler builtin (`Ty::Store`, `crates/ignite-types/src/builtins.rs:74`), yet the scope has no `crates/ignite-types/src`. The real surface is probably wider than claimed. The total order means no collision results.
- `docs/slice.md` is in all 14 scopes. I treated it as append-only sections, as in round 1, and did not raise it.
- Out of my lane: `story:web-policies` adds a `flate2` dev-dependency only if "no new dependency" is dropped. Its acceptance says no new dependency, so I found no `Cargo.toml` surface.
- Stories outside the set (`data-derived-codec`, `http-app-connection` and others) were not assessed.

```findings
- file: .engineering/planning/story/eval-closures-and-members.md
  line: 48
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Ordering line says this story and story:web-core share only docs/slice.md, but its scope claims crates/ignite/tests/ (end-to-end test, fixture kind) and web-core cites crates/ignite/tests/main.rs; the two are unordered and both add a mod line there (the mod line is inferred from AGENTS.md); name the file in the Ordering line or add an ordering edge recording it"
```
