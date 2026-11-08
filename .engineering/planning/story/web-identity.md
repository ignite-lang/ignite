---
format: aep.planning-md/3
id: story:web-identity
kind: story
status: draft
title: 'Identity as a type: Secret, jwt and api_key providers, roles, parameters by type'
relations:
- decomposes: epic:x-web
- depends_on: story:web-policies
- depends_on: story:route-annotations
- depends_on: story:web-openapi-client
- depends_on: story:std-digest
- depends_on: story:web-service-calls
scope:
- confidence: inferred
  path: crates/ignite-eval/src
- confidence: inferred
  path: crates/ignite-eval/src/explain.rs
- confidence: inferred
  path: crates/ignite/src/apply
- confidence: inferred
  path: crates/ignite/src/run/journal.rs
- confidence: inferred
  path: crates/ignite/tests/golden
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/process.ignite
- confidence: inferred
  path: x/web.ignite
- confidence: inferred
  path: x/web/client.ignite
- confidence: inferred
  path: x/web/hmac.ignite
- confidence: inferred
  path: x/web/identity.ignite
revision: 5
---
# Story: web-identity

## Outcome

Who may call a route is a type: a route function that takes `user: User` runs only with a `User`, whose only source is the identity provider the `Web` component declares, and a role function `fn(User) -> Option<Admin>` yields `Admin`; the evaluator resolves each route parameter by type and reports a parameter with no source as E120 (X13). This story ships the `Secret` resource kind, a clock resource, the providers `jwt<T>` (HS256) and `api_key<T>`, roles, 401 and 403, a limit per identity, `explain … routes` and `api.as` in tests (`docs/web.md` §6.10).

## Context

- web §6.10: resolution order (pattern parameter, body, query, then by type from providers); a type two providers yield or a cycle of roles is E120; 401 with `WWW-Authenticate: Bearer` (RFC 6750 §3) for a token identity; 403 when a role answers `None`; Q9 default: routes are public unless they take an identity.
- web §5 row X13: `Signature` values in the evaluator, shared with the binding by name of `story:route-annotations`.
- `jwt<T>(secret)`: a JWS (RFC 7515) with HS256, which needs HMAC-SHA-256 over `std.digest` and base64url (`story:std-digest`); `exp` and `nbf` checked against a clock resource (web §8 `std-clock-capability`, built on `Clock.now`, slice §21.20); `aud` when declared. `api_key<T>(keys)`: header `x-api-key`, `keys: Secret` holds `name key` lines, compared in constant time.
- `Secret` (web §8 `std-secret`): a capability like `Setting` (slice §21.5 to 21.7) that only a provider reads; bound by the assembly from a file; its value appears in no `ignite plan` output, explanation, journal or saved plan.
- Slot 7 of web §6.11 (`per_identity<T>`) belongs here; slots 1 to 5 are `story:web-policies`.
- `api.as(user)` is on the test client of `story:web-openapi-client`. Sessions, OIDC and CSRF are `story:web-sessions`; `es256` and `jwks` are not covered (epic).

## Acceptance

- Inline tests on both targets over `Pipe.pair()`: `GET /me` without a token is 401 with `WWW-Authenticate: Bearer`; with a valid HS256 token it answers the decoded `User`; a token with a bad signature, an `exp` in the past (manual clock) or a wrong `aud` is 401; `DELETE /todos/{id}` with a `User` lacking role `admin` is 403, with it 204; `api_key` with a valid key admits, with a wrong key of equal length 401.
- HMAC-SHA-256 passes the RFC 4231 test vectors; the JWS example of RFC 7515 appendix A.1 verifies (inline tests).
- `api.as(User { sub = 1, name = "a" })` in a test reaches `/me` with no token; roles still run (inline test).
- Fixtures with `.expected` (E120 naming route and parameter): a parameter of a type no provider yields; a type two providers yield; a cycle of two role functions; `per_identity<T>` where no provider yields `T`.
- `require = User` on the component (web §6.10 "Visible"): every route needs a `User` except those annotated `@public`; a route taking no identity under it answers 401 without a token, an `@public` route answers 200 (inline tests); `explain … routes` shows the `@public` routes as `public`.
- `limit = per_identity<User>(2, 1 minute)`: the third request of one user is 429, another user is admitted (inline test).
- This story introduces `ignite explain FILE Sys.server.routes` (web §6.10 "Visible"): one line per route, `METHOD PATTERN  NEEDS`, where NEEDS is `public` or the type with its provider and roles; the golden holds `DELETE /todos/{id}  Admin (jwt, role admin)` and `GET /todos  public`.
- Secret redaction: a run and a saved plan of a program binding a `Secret` from a file contain no byte of its value (test greps `ignite plan` output, `ignite explain`, the run journal and the saved plan).
- `docs/slice.md` section; `task gate` green.

## Scope

- `x/web/identity.ignite` (new: providers, roles, resolution), `x/web/hmac.ignite` or `std/digest.ignite` (HMAC), `x/web.ignite` (slot 7, 401/403).
- `std/process.ignite` (`Secret`, the clock resource), `crates/ignite-eval/src` (X13 `Signature` values, redaction), `crates/ignite/src/apply/` (redaction in saved plans), `crates/ignite/src/run/journal.rs` (redaction).
- `x/web/client.ignite` (`api.as`), `crates/ignite-eval/src/explain.rs` (`routes`), `crates/ignite/tests/`, `crates/ignite/tests/golden/`, `crates/ignite-check/tests/` fixtures, `docs/slice.md`. Inferred from web §8 rows 18 and 21, `std-secret`, `std-clock-capability`.
- Ordering: after `story:web-service-calls` (shared `x/web/client.ignite`, `x/web.ignite`, `crates/ignite-eval/src`, goldens); `story:web-sessions` builds on it.
