---
format: aep.planning-md/3
id: epic:x-web
kind: epic
status: draft
title: 'x.web: a web framework in the standard library'
summary: 'Web services declared in the system: routes, codecs, pages, live updates, policies, identity and outgoing calls derived by x.web'
revision: 3
---
# Epic: x-web

## Outcome

A web service in Ignite is part of the system declaration: a program imports `x.web` from a second module root `x` beside `std`, declares its resources, routes and policies, and the compiler and `x.web` derive the JSON codecs, the CRUD routes of a collection, the problem+json errors, the OpenAPI document, the checked HTML forms and Markdown pages, the ETags and live page updates, the outgoing-call dependencies and the health probe with its contract. The program of `docs/web.md` section 1 checks, builds and serves on both targets, and `ignite plan`, `run` and `apply` operate it as an ordinary executable member.

## Context

- Direction (Timo, 2026-11-08), three rounds in one day: a web framework "really nice to use, token efficient", allowed to extend the language, "something only Ignite can do"; round 3 added authentication and authorisation, policies instead of a middleware chain, outgoing calls with an HTTP client, and HTML/Markdown templating.
- Design: `docs/web.md` (decides; sections cited below as web §n), reader page `site/book/src/planned/web.md`.
  - Packaging (`x` root, `--x`, layering row): web §3.
  - Language extensions X1 to X15 and their measured savings: web §5.
  - The design: `Web` kind §6.1, routes and checks §6.2, effects as cache key and live view §6.3, handlers by hand §6.4, contracts on routes §6.5, durable data §6.6, errors §6.7, OpenAPI and test client §6.8, identity and access §6.10, policies §6.11, outgoing calls §6.12, views and Markdown §6.13.
  - Fit with the system model: web §7. Delivery order this epic follows: web §8. Open questions with defaults: web §10 (Q1 to Q15; each story takes the default unless Timo answers).
- Reused stories (planned elsewhere, edges, not duplicates): `story:data-codec-combinators`, `story:data-derived-codec` (X1, `epic:data-formats`), `story:http-client-role`, `story:tls-client`, `story:http-app-connection`, `story:std-digest`, `story:channels-and-timers`. `story:data-value-model` is reached through `data-codec-combinators`. `story:std-websocket` is not needed: live pages use an event stream over a streamed body (slice §19.23), not WebSocket.
- Every language extension is generic (layering rule 5): no compiler crate names HTTP or HTML.

## Stories

In delivery order. `depends_on` edges carry two kinds of need: a semantic one (the story uses what the other creates) and an ordering one (the two edit the same files: the frontend crates, `crates/ignite-eval/src`, `x/web.ignite`, `x/web/client.ignite`, the backends and `CODEGEN_REVISION`). Each story's Scope names its paths and its ordering. The result is one chain after two parallel roots (`web-core`, `eval-closures-and-members`); the reused stories outside the epic run beside it.

Phase 1, the smallest useful server on today's language:

| step | story | needs (semantic; ordering) |
|---|---|---|
| 1 | `web-core`: the `x` root and `x.web` serving a hand-built route table | `data-codec-combinators` |
| 2 | `eval-closures-and-members`: X3 closures in the model, X9 derived members | |

Phase 2, the declared form:

| step | story | needs (semantic; ordering) |
|---|---|---|
| 3 | `route-annotations`: X8 annotations and pattern literals, X4 answering handlers | 1, 2 |
| 4 | `given-and-defaults`: X5 `given`, X6 parameter defaults, X7 `Store.insert` and update from another record | `data-derived-codec`; 3 |
| 5 | `durable-store`: X12 `Durable<T>` | 4, `data-derived-codec`; 1 |
| 6 | `web-collection`: X2 `New<T>`/`Patch<T>`, `collection`, the `Web` kind serving routes and annotated functions, `shape`, derived probe and contracts | 1, 2, 3, 5, `data-derived-codec` |
| 7 | `web-policies`: the fixed policy slots on `Web`, task deadlines | 6, `http-app-connection` |
| 8 | `web-openapi-client`: OpenAPI, the in-process test client, `explain --raw`, the todo example in the declared form | 6; 7 |

Phase 3, pages:

| step | story | needs (semantic; ordering) |
|---|---|---|
| 9 | `markup-views`: X10 `view` blocks, `x.web.html`, checked forms and routes, `page` | 4, 6; 8 |
| 10 | `web-live`: X11 effect sets as values, `Store.version`, ETag/304 (also for `Durable`), live pages | 9, `http-app-connection`, `channels-and-timers` |
| 11 | `web-markdown`: X15 file literals, `x.web.markdown`, typed front matter, checked links | 9; 10 |

Phase 4, calls and identity:

| step | story | needs (semantic; ordering) |
|---|---|---|
| 12 | `web-service-calls`: the `Service` resource, `CallError`, attempts, breaker, `with`, fakes, dependency edges | 3, 7, 8, 10, `http-client-role`, `tls-client`; 11 |
| 13 | `web-identity`: `Secret`, the clock resource, X13 parameters by type, `jwt` (HS256), `api_key`, roles, `explain … routes` | 3, 7, 8, `std-digest`; 12 |
| 14 | `web-sessions`: `session`, `oidc`, `any`, CSRF | 5, 9, 12, 13 |

Shared surfaces outside the epic: steps 4, 7 and 10 change substrate builtins and `CODEGEN_REVISION` and collide with `story:store-api-gaps` (`crates/ignite-io/src/store.rs`) and `story:mid-level-ir` (the backends); every story edits `docs/slice.md` (its own section).

## Acceptance

- Every story implemented.
- The program of web §1 (with `page(board)` and `availability = 99%`) checks silently, builds on both targets and answers its routes; `ignite plan` lists `server.health` and `server.reachable` as derived (`story:web-collection`, `story:markup-views`, `story:web-live`).
- `examples/todo` is in the declared form, durable, with its tests on `client` (`story:web-openapi-client`).
- `ignite check` refuses each check of web §6.2, §6.10 and §6.11 with E120 naming the route, except a `GET` handler that writes, which is E017 as the probe of web appendix B gives (fixtures in the stories).

## Not covered

- X14 derived modules and `Client<X>` (web §8 step 20, `derived-modules` and `web-peer-client`): route-table digests in saved plans and callee-first rollout. Filed after phase 4.
- `es256(public_key)` and `jwks(service)` keys for `jwt` (web §6.10): need ECDSA P-256 in std; HS256 only here.
- Client components on WebAssembly (concept B), session actors (concept C), hot reload: web §9.
- SQL or a database resource (web §5, not taken).
- Q6's second half, no ETag for a `Durable` store during a rollout's drain: a process cannot tell a second instance runs; recorded as a limit by `story:web-live`.

## Plan review

Two rounds of the four plan critics (`review-result:x-web-*-round-1`, `-round-2`). Round 2: acceptance and design approve; parallel-safety (1 finding) and scope (5 findings) asked for revision. All six were revised into the story bodies after round 2 and were not re-reviewed (no third round): `eval-closures-and-members` names the shared `crates/ignite/tests/main.rs` and gains assembly binding of derived members; `web-live` records Q6's drain half as a limit; `web-identity` gains `require = User` and `@public`; `web-sessions` gains CORS credentials under a cookie identity; `web-service-calls` gains `backoff`, `open_for` and the no-ETag read of a `Service`. Also from the round-2 reports' out-of-lane notes: `web-policies` owns `scope(..)`, `durable-store` names `crates/ignite-types/src`, `markup-views` tests a page through `client`, `web-markdown` tests holes.

Accepted trade-off (design round 2): after the two roots the stories form one chain, ordered by the shared surfaces (`crates/ignite-eval/src`, `x/web.ignite`, `x/web/client.ignite`, the backends, `CODEGEN_REVISION`); splitting those surfaces is what would let more of them run at once.
