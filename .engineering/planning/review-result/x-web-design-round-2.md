---
format: aep.planning-md/3
id: review-result:x-web-design-round-2
kind: review-result
status: active
title: Design critic, x-web round 2
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
approve

What I read: 14 stories and `epic:x-web`, in full, plus `review-result:x-web-design-round-1`, the `scope` and `Web` sections of `docs/web.md`, and the outgoing edges of the 7 outside stories. Commands: `aep plan artifact show` on each, `relations`, `graph`, `validate` (reports `valid` for this set), `waves`.

The three round-1 findings are closed:
- **`explain … routes` twice:** `web-live` no longer mentions it. `web-identity` alone introduces it and states the line grammar (`web-identity.md:61`).
- **`Web` serving `@get` functions:** `web-collection` now states that `Web.routes` serves the module's annotated functions and has `depends_on story:route-annotations` (`web-collection.md`, acceptance line 7).
- **Policy slots on `Web`:** `web-policies` now has `depends_on story:web-collection` and names the `Web` component in its acceptance (`web-policies.md:60`).

What I could not establish, none of it set the verdict:
- **Edges walked:** about 55, 41 `depends_on` and 14 `decomposes`, and I walked outside the set. The 7 outside stories point at nothing in the epic, and the graph has no cycle.
- **Acceptance reads traced:** about 35 reads of state a sibling produces. Each has a direct edge or a transitive one through the chain; none is unrecorded.
  - `Shape` from `data-derived-codec`.
  - `fake` and `client` from `web-openapi-client`.
  - `page` from `markup-views`.
  - Read sets from `web-live`.
  - `per_identity` from `web-policies`.
  - `Durable` from `durable-store`.
- **Chain shape:** after the two roots (`web-core`, `eval-closures-and-members`) the set is one 13-story order. Every edge in it has a written reason: a semantic need, or a shared file named in the `Ordering:` line of the body and the epic's Stories section. By the rubric that is a trade-off the drafter has already made, not a finding. The alternative is splitting the shared surfaces (`crates/ignite-eval/src`, `x/web.ignite`, `x/web/client.ignite`, the backends, `CODEGEN_REVISION`) so that more stories can run at once.
- **Out of my lane, parallel safety:** `web-live` and `given-and-defaults` both declare a collision with `story:store-api-gaps` on `crates/ignite-io/src/store.rs`, with no edge to it.
- **Out of my lane, scope:** `scope(...)` is a route constructor in `docs/web.md:366`. `web-policies` tests it (`web-policies.md:65`), but no body says which story builds it. `docs/web.md:801` gives it to `web-policies`.

```findings
[]
```
