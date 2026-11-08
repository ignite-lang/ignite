---
format: aep.planning-md/3
id: story:web-sessions
kind: story
status: draft
title: 'Browser identities: sessions, OpenID Connect, any, CSRF'
relations:
- decomposes: epic:x-web
- depends_on: story:web-identity
- depends_on: story:durable-store
- depends_on: story:markup-views
- depends_on: story:web-service-calls
scope:
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: x/web.ignite
- confidence: inferred
  path: x/web/csrf.ignite
- confidence: inferred
  path: x/web/html.ignite
- confidence: inferred
  path: x/web/identity.ignite
revision: 4
---
# Story: web-sessions

## Outcome

Browser identities for `x.web`: `session<T>(sessions)` signs a user in with a `__Host-session` cookie over a `Durable<Session<T>>`, `oidc<T>(issuer, client, claims)` signs in through OpenID Connect and then a session, `any([p, q])` takes the first provider that accepts, and every unsafe route under a cookie identity is protected against cross-site requests: a hidden, masked `_csrf` input in every form of a view and a Fetch Metadata and `Origin` check for JSON routes (`docs/web.md` §6.10 providers and CSRF).

## Context

- web §6.10 table: `session` cookie `HttpOnly`, `Secure`, `SameSite=Lax`, ids of 128 bits from `Entropy`, a handler returns `sign_in(user)` or `sign_out()`, a new id at each sign-in; a page under a session identity without one answers 303 to `/login`.
- `oidc`: derives `/login`, `/callback`, `/logout`; authorization code with PKCE (RFC 7636), `state` and `nonce`; the ID token checked against the issuer's keys fetched through `issuer: Service` (`story:web-service-calls`); `claims` admits an account or answers 403. The ID token's signature algorithm is limited to what `story:web-identity` verifies (HS256 for a test issuer; RS256/ES256 issuers need the key work the epic does not cover).
- CSRF (web §6.10; Q10 default): forms get the token with no program line because the view knows the form's route (`story:markup-views`); the token is per session and masked with fresh random bytes per response, so compression stays on (BREACH); JSON routes refuse unsafe cross-site requests by `Sec-Fetch-Site` and `Origin` against the listener's origin and `cors`.
- web §6.11 check: `cors` holding `"*"` with a cookie identity is E120.
- Sessions persist in `Durable<Session<T>>` (`story:durable-store`); the providers build on `story:web-identity`.

## Acceptance

- Inline tests on both targets over `Pipe.pair()`: a `POST /login` handler returning `sign_in(user)` sets `__Host-session` with `HttpOnly; Secure; SameSite=Lax; Path=/`; the cookie admits `/me`; a second sign-in issues a different id; `sign_out()` clears it and the old id is refused; a page route without a session is 303 to `/login`; a restart keeps sessions (end-to-end, `Durable`).
- `any([jwt(..), session(..)])` admits a bearer caller and a cookie caller on the same route; providers yielding different types are a check error (fixture).
- CSRF: a view form posting to an unsafe route contains a hidden `_csrf` input; two responses carry different masked values that both verify; a `POST` without it or with another session's token is 403; a JSON `POST` with `Sec-Fetch-Site: cross-site` from an origin not in `cors` is 403, same-origin admitted; a `jwt` identity needs no token (inline tests).
- OIDC against a fake issuer (`fake` of `story:web-service-calls`) with an HS256 test key: `/login` redirects with `code_challenge`, `state` and `nonce`; `/callback` with a wrong `state` is 400; an ID token with a bad signature or a `nonce` other than the one sent is refused with 401 and no session; a valid code yields a session; `claims` returning `None` is 403; `/logout` clears the cookie, the old session id is refused afterwards, and the answer is a 303 to `/`.
- `cors = ["*"]` with `session` is E120 (fixture).
- CORS credentials (web §6.11 slot 4): under a `session` identity a preflight and a response to an allowed origin carry `Access-Control-Allow-Credentials: true`; under a `jwt` identity they do not (inline tests).
- `docs/slice.md` section; `task gate` green.

## Scope

- `x/web/identity.ignite` (`session`, `oidc`, `any`), `x/web/csrf.ignite` (new), `x/web/html.ignite` (hidden input in forms), `x/web.ignite` (slot 8, the cookie answers).
- `crates/ignite/tests/`, `crates/ignite-check/tests/` fixtures, `docs/slice.md`. Inferred from web §8 row 18.
