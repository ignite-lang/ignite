---
format: aep.planning-md/3
id: review-result:x-web-scope-round-1
kind: review-result
status: active
title: Scope critic, x-web round 1
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

epic:x-web — web §6.2 promises "a `GET` handler that writes" as an E120 check, and epic acceptance requires every §6.2 check to be refused; no story's acceptance claims it (web-core lists only the pattern and parameter checks, markup-views only the form checks), so web-core should take it — `docs/web.md:375`, `.engineering/planning/epic/x-web.md:70`
story:web-live — Q6's default (the ETag of a `Durable` store across processes is the version of the newest file) is disclaimed by durable-store as "web-live's, not here", but web-live's context and acceptance never mention a `Durable` ETag, so nothing claims it — `.engineering/planning/story/durable-store.md:35`, `docs/web.md:836`
story:web-live — the golden that `explain …routes` prints each route's read set (`GET /  reads todos`) traces to no sentence in web §6.3 or §6.10, and it is the same command whose output web-identity defines as needs (`DELETE /todos/{id}  Admin (jwt, role admin)`); two stories claim one output with different content — `.engineering/planning/story/web-live.md:54`, `.engineering/planning/story/web-identity.md:58`, `docs/web.md:546`
story:web-service-calls — its Context says fakes use "`client` and `with` of `story:web-openapi-client`", but web-openapi-client's outcome and acceptance never claim `with`, while web-service-calls's own Scope and Acceptance do, so the owner of `with` (web §6.8) is stated two ways — `.engineering/planning/story/web-service-calls.md:40`, `.engineering/planning/story/web-service-calls.md:53`, `docs/web.md:487`
story:web-policies — slot 5's rule that the client is the peer address unless `trusted_proxy` names the proxy, and then only from that proxy's address, is covered only by an E120 fixture, with no behaviour test. The E120 timeout fixture is also component-level only, though the parent says "of the component or of a route" — `.engineering/planning/story/web-policies.md:63`, `docs/web.md:599-602`
story:web-core — `stream(start, next)` is promised as "server-sent events with `sse_event`", but the acceptance tests only "a streamed body", and no later story claims `sse_event` (web-live only says "event stream") — `.engineering/planning/story/web-core.md:56`, `docs/web.md:365`
story:web-collection — the `Web` kind's `shape: Shape` property (wire names, serde §5.1) is in web §6.1 but no outcome or acceptance applies it to the routes' codecs, and no story records it as uncovered — `docs/web.md:337`
story:markup-views — the `site/book/theme/highlight.js` change and its acceptance line have no sentence in the parent. The epic's source says nothing about site highlighting, and `planned/web.md` marks its blocks `fragment`, so it is not needed to keep the site check green — `.engineering/planning/story/markup-views.md:55`, `site/book/src/planned/web.md:15`

**What you read:** 15 artifacts (the epic and its 14 stories) via `aep plan artifact show` and `aep plan artifact graph`, plus `docs/web.md` §1, §3, §5, §6 (6.1 to 6.13), §8 and §10 and the store files for line numbers. I extracted about 38 promises from the epic and §6 to §8: 31 trace cleanly to a story, 7 are the findings above.

**What you could not establish:**
- Whether the unclaimed "GET handler that writes" check needs a new type rule or only `validate` reading existing effects. That is design and acceptance lane, not scope.
- Several stories edit the same files (`x/web/client.ignite`, `x/web/identity.ignite`, `crates/ignite-types/src`). That is parallel-safety lane and does not affect this verdict.
- Whether the `flate2` dev-dependency in web-policies's acceptance needs a dependency line. That is outside scope.

```findings
- file: .engineering/planning/epic/x-web.md
  line: 70
  category: scope
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "web §6.2 promises the E120 check 'a GET handler that writes' and the epic acceptance requires every §6.2 check, but no story claims it; web-core would most naturally take it"
- file: .engineering/planning/story/web-live.md
  line: 54
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "Q6's Durable-store ETag default is disclaimed by durable-store as web-live's, but web-live's context and acceptance never claim it, so nothing covers the ETag of a Durable store"
- file: .engineering/planning/story/web-live.md
  line: 54
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the golden that 'explain ...routes' prints each route's read set traces to no sentence of web 6.3 or 6.10 and collides with web-identity's golden for the same command (needs, not read sets); two stories claim one output"
- file: .engineering/planning/story/web-service-calls.md
  line: 40
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the context attributes 'with' to story:web-openapi-client, which does not claim it, while this story's own scope and acceptance claim it; the body must say this story owns 'with' and 'fake'"
- file: .engineering/planning/story/web-policies.md
  line: 63
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "trusted_proxy's behaviour (peer address unless the proxy is named, then only from that proxy) has only an E120 fixture and no behaviour test, and the timeout E120 fixture omits the per-route case the parent names"
- file: .engineering/planning/story/web-core.md
  line: 56
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "stream(start, next) is promised with sse_event but the acceptance covers only 'a streamed body', and no later story claims sse_event"
- file: docs/web.md
  line: 337
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Web kind's shape: Shape property is not claimed by story:web-collection or any other story and is not recorded as uncovered; web-collection would most naturally take it"
- file: .engineering/planning/story/markup-views.md
  line: 55
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the highlight.js acceptance and scope entry trace to no sentence in the parent; the planned page marks its blocks fragment, so drop it or record why the epic needs it"
```
