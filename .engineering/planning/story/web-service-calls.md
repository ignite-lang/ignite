---
format: aep.planning-md/3
id: story:web-service-calls
kind: story
status: draft
title: 'The Service resource: typed outgoing calls, fakes and dependency edges'
relations:
- decomposes: epic:x-web
- depends_on: story:web-policies
- depends_on: story:route-annotations
- depends_on: story:web-openapi-client
- depends_on: story:web-live
- depends_on: story:http-client-role
- depends_on: story:tls-client
- depends_on: story:web-markdown
scope:
- confidence: inferred
  path: crates/ignite-eval/src
- confidence: inferred
  path: crates/ignite/tests/golden
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: x/web.ignite
- confidence: inferred
  path: x/web/client.ignite
- confidence: inferred
  path: x/web/service.ignite
revision: 4
---
# Story: web-service-calls

## Outcome

A program declares `resource weather_api: Service { base = "https://api.weather.example/v1", timeout = 2 seconds }`; a route function given it calls `weather_api.get<Weather>("/current?city={city}")` and receives `Result<Weather, CallError>`, with attempts only for idempotent methods, a breaker, the request's deadline and id carried, `?` answering 502, 503 or 504; `ignite plan` shows the dependency edge and a derived probe and contract for the service; a test replaces the service with a fake served over a pipe (`docs/web.md` §6.12 layer 2).

## Context

- web §6.12: the `Service` kind and its properties; the path is a pattern literal (`story:route-annotations`) used as an RFC 6570 level 1 template, each `{name}` a local, percent-encoded; `CallError` is `Timeout`, `Refused`, `Status(code, body)`, `Decode(problem)`, `Open`; attempts only for RFC 9110 §9.2.2 idempotent methods, `POST` with `attempts > 1` needs `idempotency_key = true` (E120 otherwise); the breaker counts timeouts, refusals and 5xx; deadline the smaller of `timeout` and the request's remaining time (`story:web-policies` slot 10).
- Layer 1, the client role, is `story:http-client-role`; `https` bases go through `Tls<T>` of `story:tls-client`.
- Dependency edges come from route read sets (X11, `story:web-live`); the derived probe and contract from X9 (`story:eval-closures-and-members`); the test client `client(..)` is `story:web-openapi-client`'s. This story owns `Sys.server.with(resource = …)` (web §6.8, §6.12) and `fake(f)` with its recorded `calls()`.
- Q13 default: a target above what a dependency promises is reported by `explain`, not refused. Q12 default: no switch turns TLS verification off.

## Acceptance

- Inline tests on both targets with a fake (`TodoService.server.with(weather_api = fake(f))`, served by `Http.serve` over `Pipe.pair()`): a route returns the fake's `Weather`; `{city}` with a space and a `/` is sent percent-encoded (the fake's recorded request, `weather_api.calls()`); the fake answering 500 makes the route answer 502; a fake that never answers makes the route answer 504 at the service `timeout`; `attempts = 3` retries a `GET` twice and a `POST` never; after `breaker = 2` consecutive failures the third call is `Open` and the route answers 503 without a request; with `open_for = 30 seconds` on the manual clock, a call after 30 s reaches the fake again and a success closes the breaker; with `backoff = exponential(100 ms, 2 seconds)` the recorded attempt times of a retried `GET` are at least 100 ms and 200 ms apart and never more than 2 s; a route that reads a `Service` answers no `ETag` (web §6.3); the request id reaches the fake as `X-Request-Id`; with the component `timeout = 1 seconds` and the service `timeout = 10 seconds`, a fake that never answers makes the route answer 504 within 1.5 s (the call's deadline is the smaller of the two), and with the component at 10 s and the service at 1 s it is again 504 within 1.5 s.
- `with(nope = …)` naming a resource the routes do not reach is a check error; `POST` with `attempts = 2` and no `idempotency_key` is E120 (fixtures).
- `ignite plan` prints `TodoService.server depends on weather_api` with its base (golden); `ignite explain TodoService.weather_api` lists the routes that call it; `availability = 99.5%` on the service derives `weather_api.health` and `weather_api.reachable`; a server target of 99.9 % over it is reported by `explain` (golden).
- One end-to-end test calls a local TLS server with the test CA of `story:tls-client` through an `https` base; a wrong host name fails as `Refused`.
- `docs/slice.md` section; `task gate` green.

## Scope

- `x/web/service.ignite` (new), `x/web.ignite` (answers of `CallError`, `with`), `x/web/client.ignite` (`fake`).
- `crates/ignite-eval/src` (dependency edges, the service's derived members).
- `crates/ignite/tests/`, `crates/ignite/tests/golden/`, `crates/ignite-check/tests/` fixtures, `docs/slice.md`. Inferred from web §8 row 17.
- Ordering: after `story:web-markdown`, before `story:web-identity`; this story and identity both edit `x/web/client.ignite` (`fake` here, `api.as` there), `x/web.ignite`, `crates/ignite-eval/src` and the goldens.
