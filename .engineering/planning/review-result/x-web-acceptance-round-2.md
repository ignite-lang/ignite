---
format: aep.planning-md/3
id: review-result:x-web-acceptance-round-2
kind: review-result
status: active
title: Acceptance critic, x-web round 2
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

Each of the eight round-1 findings is fixed in the current bodies:

- **web-policies, order:** an "Order, inline tests" bullet now covers a 429 and a 503 carrying the request id as `instance`, a rate-limited request never reaching `before` (counter stays 0), a preflight answered without `before`, and a 2 KiB problem body compressed.
- **web-policies, access log:** the bullet is now an end-to-end test over TCP that reads the process's standard error.
- **web-markdown:** the claimed sections are enumerated exactly, every example of a claimed section must pass, unclaimed sections are skipped "by section name only", and the counts are recorded in `docs/slice.md`.
- **markup-views:** the `highlight.js` clause is gone from the acceptance.
- **web-service-calls:** the deadline is tested both ways (component 1 s with service 10 s, and the reverse), each answering 504 within 1.5 s.
- **web-sessions:** the OIDC bullet now covers `/logout` (cookie cleared, old id refused, 303 to `/`) and an ID token refused for a bad signature or wrong `nonce`.
- **route-annotations, web-collection:** `CODEGEN_REVISION` is "bumped" unconditionally, which is checkable in `crates/ignite/src/apply/plan.rs:18`.

I found nothing new in the other stories.

What I read: 14 of 14 stories in full (`aep plan artifact show` on each), `review-result:x-web-acceptance-round-1`, `aep plan artifact kinds` and `aep plan artifact lifecycle story`, `aep plan artifact validate` (valid), and `git grep CODEGEN_REVISION` in `plan.rs`.

What I could not establish:
- Whether `collection` lets the existing `crates/ignite/tests/todo.rs` pass unchanged after the `web-openapi-client` rewrite. The acceptance says the tests pass, and that holds either way.
- Out of my lane, carried from round 1 and not judged here: `story:web-live` and `story:web-identity` may both define `ignite explain … routes`. Several stories overlap `store.rs` (`store-api-gaps`) and the type crates.

```findings
[]
```
