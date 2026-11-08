---
format: aep.planning-md/3
id: review-result:x-web-scope-round-2
kind: review-result
status: active
title: Scope critic, x-web round 2
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

artifact — reason — citation

story:web-live — the Q6 default is claimed only as "the version of the newest file"; the second half, "none during a rollout's drain", is in no body, and no body says it was dropped — `docs/web.md:836`, `.engineering/planning/story/web-live.md:55`

story:web-identity — the §6.10 promise that "`require = User` on the component makes every route need a `User` except the ones annotated `@public`" is claimed by no story and named as uncovered nowhere; web-identity's context takes only the Q9 default (public unless a route takes an identity), and the `Web` kind of §6.1 has no `require` property — `docs/web.md:548`, `.engineering/planning/story/web-identity.md:47`

story:web-sessions — the slot-4 promise "credentials when the identity is a cookie identity" (`Access-Control-Allow-Credentials` derived from the identity) is claimed by neither web-policies, which tests only origins and methods, nor web-sessions, which has only the `"*"` E120 — `docs/web.md:576`, `.engineering/planning/story/web-sessions.md:40`

story:web-service-calls — the `Service` type's `backoff` and `open_for` (when an open breaker closes again) are promised, but the outcome and acceptance cover only `attempts` and `breaker`, so nothing claims them and nothing says they are dropped — `docs/web.md:630`, `.engineering/planning/story/web-service-calls.md:46`

story:eval-closures-and-members — "an assembly binds them (`TodoService.server.health = scheduledProbeHost()`)" for derived members is claimed by no acceptance. The story tests only that `ignite run` schedules a derived member and that a hand-declared member replaces it, not that an assembly can bind one — `docs/web.md:457`, `.engineering/planning/story/eval-closures-and-members.md:36`

**What you read:** 15 artifacts (the epic and its 14 stories) via `aep plan artifact show`, plus `aep plan artifact graph` and `review-result:x-web-scope-round-1`. From `docs/web.md` I read §3, §5, §6.1 to §6.13, §7, §8, §10 and appendix B.
- **Promise count:** I extracted 41 promises from the epic and §6 to §8. I traced 36 to a story, named 1 as uncovered by the epic (X14, `es256`/`jwks`), and the 5 findings above remain.
- **Round 1:** all 8 findings landed.
  - E017 for a `GET` that writes is claimed in web-core and carved out in the epic acceptance, matching appendix B.
  - The `Durable` ETag is now in web-live's acceptance.
  - The `explain …routes` read-set golden is gone.
  - `with` is stated as web-service-calls'.
  - `trusted_proxy` has a behaviour test, and the timeout E120 fixtures cover component and route.
  - `sse_event` is in web-core's acceptance.
  - `shape` is claimed and tested in web-collection.
  - The `highlight.js` item is gone.

**What you could not establish:**
- Whether web-markdown's explicit skipping of link reference definitions and tabs from CommonMark 0.31.2 is the intended narrowing. The story names it, so I did not count it as a finding.
- Out of your lane (acceptance): web-live's context says a `Service` read set gets no ETag, but no acceptance tests it. The `api.get("/").body` page test of §6.8 and the Markdown holes of §6.13 also have no acceptance line.

```findings
- file: .engineering/planning/story/web-live.md
  line: 55
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "Q6's default is claimed only as 'the version of the newest file'; 'none during a rollout's drain' (docs/web.md:836) is in no story body and no body says it was dropped"
- file: .engineering/planning/story/web-identity.md
  line: 47
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "web 6.10 promises 'require = User on the component makes every route need a User except the ones annotated @public' (docs/web.md:548) and no story claims it or names it uncovered; web-identity takes only the Q9 default"
- file: .engineering/planning/story/web-sessions.md
  line: 40
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "web 6.11 slot 4 promises CORS 'credentials when the identity is a cookie identity' (docs/web.md:576); web-policies tests only origins and methods and web-sessions only the '*' E120, so nothing claims the credentials behaviour"
- file: .engineering/planning/story/web-service-calls.md
  line: 46
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Service type's backoff and open_for (docs/web.md:630-632) are promised but the outcome and acceptance cover only attempts and breaker, so nothing claims them or records them as dropped"
- file: .engineering/planning/story/eval-closures-and-members.md
  line: 36
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "web 6.5 promises 'an assembly binds them (TodoService.server.health = scheduledProbeHost())' for derived members (docs/web.md:457); no acceptance claims that an assembly can bind a derived member"
```
