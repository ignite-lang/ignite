---
format: aep.planning-md/3
id: review-result:x-web-acceptance-round-1
kind: review-result
status: active
title: Acceptance critic, x-web round 1
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

- story:web-policies — the Outcome promises "the one fixed order of `docs/web.md` §6.11", but the bullets test each slot alone, so no test observes an order between two slots (for example a 429 or 503 problem carrying the request id as `instance`, a rate-limited request never reaching `before`, or a 429 being compressed); the acceptance should add such cases — .engineering/planning/story/web-policies.md:44 (acceptance bullets at :55-:63)
- story:web-policies — the access-log bullet sits under "Inline tests over `Pipe.pair()`" yet says "test reads the captured standard error", and the acceptance names no mechanism by which an inline test (or any named test) captures the server's stderr — .engineering/planning/story/web-policies.md:57
- story:web-markdown — "passes the CommonMark 0.31.2 spec examples of the sections it claims … the passed and skipped counts are printed" names no rule that a skipped example lies outside the claimed sections or tables, so any failing example can be skipped and the test still passes — .engineering/planning/story/web-markdown.md:47
- story:markup-views — "`site/book/theme/highlight.js` highlights `view` blocks" names no test or command, and `git grep highlight` finds no test of that file in `crates/` or `ux/`, so the check is a vote — .engineering/planning/story/markup-views.md:55
- story:web-service-calls — the Outcome says "the request's deadline and id carried", but only the id is tested (`X-Request-Id`); no bullet shows the call deadline is the smaller of `timeout` and the request's remaining time — .engineering/planning/story/web-service-calls.md:45 (rule stated at :38)
- story:web-sessions — the Outcome and Context say `oidc` derives `/login`, `/callback` and `/logout` and checks the ID token, but the OIDC bullet tests no `/logout` and no refused ID token (bad signature, wrong `nonce`) — .engineering/planning/story/web-sessions.md:45 (Context at :35)
- story:route-annotations — "`CODEGEN_REVISION` bumped if generated code changes" is vacuously true whenever someone judges nothing changed; the acceptance should state bumped or unchanged, with a check such as a golden of the generated code — .engineering/planning/story/route-annotations.md:51
- story:web-collection — the same conditional: "`CODEGEN_REVISION` bumped if generated code changes" cannot be told apart from not checking — .engineering/planning/story/web-collection.md:50

What I read: 14 of 14 stories in full with `aep plan artifact show`, plus `epic:x-web`, `aep plan artifact kinds` and `lifecycle story|epic`, `aep plan artifact validate` (valid), `docs/web.md` §6.11, `crates/ignite/tests/todo.rs`, `examples/todo/todo.ignite`, and the earlier `data-formats` acceptance rounds for the store's convention.

What I could not establish:
- Multi-bullet acceptance and bundled bullets such as "slice section; `task gate` green" follow the store's convention, as in `review-result:data-formats-critic-acceptance-1`, so I did not report them.
- `story:web-openapi-client` requires the existing todo tests to pass after a rewrite on `collection`, and `todo.rs` asserts exact statuses, `Allow` order and `content-type`. I could not tell whether `collection` reproduces them without changing that suite.
- Out of my lane (design or parallel-safety): `story:web-live` and `story:web-identity` both define the output of `ignite explain … routes` with different goldens, and neither depends on the other. The stories that touch `crates/ignite-io/src/store.rs` and `crates/ignite-types/src` overlap `store-api-gaps` and `mid-level-ir`.

```findings
- file: .engineering/planning/story/web-policies.md
  line: 44
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome promises the one fixed order of docs/web.md 6.11 but the bullets test each slot alone, so no test observes an order between two slots (a 429 or 503 problem carrying the request id, a rate-limited request never reaching before, a 429 compressed)"
- file: .engineering/planning/story/web-policies.md
  line: 57
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the access-log bullet sits under inline tests over Pipe.pair() yet says 'test reads the captured standard error' and names no mechanism by which a test captures the server's stderr"
- file: .engineering/planning/story/web-markdown.md
  line: 47
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the CommonMark acceptance prints passed and skipped counts but names no rule that a skipped example lies outside the claimed sections and tables, so any failing example can be skipped and the test still passes"
- file: .engineering/planning/story/markup-views.md
  line: 55
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'site/book/theme/highlight.js highlights view blocks' names no test or command and no test of highlight.js exists in crates/ or ux/, so the check is a vote"
- file: .engineering/planning/story/web-service-calls.md
  line: 45
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome says the request's deadline and id are carried but only the id is tested (X-Request-Id); no bullet shows the call deadline is the smaller of timeout and the request's remaining time"
- file: .engineering/planning/story/web-sessions.md
  line: 45
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Outcome and Context say oidc derives /login, /callback and /logout and checks the ID token, but the OIDC bullet tests no /logout and no refused ID token (bad signature, wrong nonce)"
- file: .engineering/planning/story/route-annotations.md
  line: 51
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'CODEGEN_REVISION bumped if generated code changes' is vacuously true whenever someone judges nothing changed; the acceptance names no check that decides bumped or unchanged"
- file: .engineering/planning/story/web-collection.md
  line: 50
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "'CODEGEN_REVISION bumped if generated code changes' is vacuously true whenever someone judges nothing changed; the acceptance names no check that decides bumped or unchanged"
```
