---
format: aep.planning-md/3
id: review-result:x-web-design-round-1
kind: review-result
status: active
title: Design critic, x-web round 1
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

web-live — `explain …routes` is specified twice with two different line formats, and no edge orders the two stories — .engineering/planning/story/web-live.md:54, .engineering/planning/story/web-identity.md:58

web-identity — its acceptance needs the Web component to serve `@get` functions, and the `Web` kind comes from web-collection while annotations come from route-annotations. Neither body says who joins them, and web-collection has no edge to route-annotations — .engineering/planning/story/web-identity.md:53, .engineering/planning/story/web-collection.md:34, .engineering/planning/story/route-annotations.md:35

web-policies — its tests set `cors`, `limit` and `timeout` as properties of "the server". docs/web.md §6.1 puts them on the `Web` kind, which web-collection creates, and web-policies has no edge to web-collection and names no other component — .engineering/planning/story/web-policies.md:44, docs/web.md:340-346

How each would close:
- **web-live and web-identity:** add `depends_on story:web-live` to web-identity, or have web-live's golden state the combined line grammar. Today whichever lands second rewrites the other's golden. One of the two bodies must say what a `routes` line holds, for example `GET /todos  reads todos  public`.
- **web-identity:** either web-collection's body states that `Web` serves the module's annotated functions, with `depends_on story:route-annotations`, or web-identity's outcome owns that join explicitly.
- **web-policies:** either add `depends_on story:web-collection` and name `Web` in its acceptance, or state that the slots are arguments of `x.web.serve`. In that second case web-collection's body must own mapping them onto the `Web` fields (web-collection lists only `availability` and `latency`).

What I read: 14 stories, `epic:x-web` and `docs/web.md` §1, §5, §6.1 to §6.13 and §8. Commands run:
- `aep plan artifact show` on each story and the epic
- `aep plan artifact relations`
- `aep plan artifact graph`
- `aep plan artifact validate`

What I could not establish:
- **Edges walked:** 45 declared edges of the set, 31 `depends_on` and 14 `decomposes`. I also walked the full-store graph for edges from the seven outside stories back into the set and found none. I found no cycle, and the set is not a chain (the fan-in at web-collection, markup-views and web-sessions leaves several parallel branches).
- **Acceptance reads:** I traced about 25 reads to a producer in the set. All are recorded by a direct or transitive edge except the three findings above.
- **Validate:** it reported nothing on this set. Its only output was missing-scope notes on other stories.
- **Out of my lane, did not set the verdict:**
  - Parallel safety: web-identity and web-service-calls both edit `x/web/client.ignite` and `x/web.ignite` with no edge between them. web-given-and-defaults and web-live both edit `store.rs` and `CODEGEN_REVISION`.
  - Possible two-things-in-one: web-policies carries the task-deadline substrate across five crates and the deflate implementation beside the policy slots. I did not judge the seam strong enough to report.
  - Scope: docs/web.md §8 step 15 lists web-live as a need of todo-on-web, but appendix A.9 has no page. I treated that as correctly dropped.

```findings
- file: .engineering/planning/story/web-identity.md
  line: 58
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the golden for `ignite explain FILE TodoService.server.routes` prints `Admin (jwt, role admin)` and `public` lines, while web-live.md:54 specifies the same command as `GET /  reads todos` lines, and no depends_on edge runs between the two stories, so the body must state one combined line grammar and add `depends_on story:web-live` (or web-live's golden must carry the identity column)"
- file: .engineering/planning/story/web-identity.md
  line: 53
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its acceptance serves `@get(\"/me\") fn me(user: User)` through a `Web` component, but the `Web` kind is created in web-collection and annotations in route-annotations, web-collection has no edge to route-annotations and neither body says that `Web.routes` serves the module's annotated functions, so the join is unowned; web-collection's body should state it and depend_on story:route-annotations, or web-identity's outcome should own it"
- file: .engineering/planning/story/web-policies.md
  line: 44
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its acceptance sets `cors`, `limit` and `timeout` as properties of an unnamed server, docs/web.md §6.1 declares them as fields of the `Web` kind that web-collection creates, and web-policies has no depends_on edge to web-collection, so either add the edge and name `Web`, or state the slots are `x.web.serve` arguments and have web-collection own mapping them onto the `Web` fields"
```
