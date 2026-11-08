---
format: aep.planning-md/3
id: story:web-core
kind: story
status: draft
title: The x module root and x.web serving a hand-built route table
relations:
- decomposes: epic:x-web
- depends_on: story:data-codec-combinators
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: crates/ignite-check/tests/layering.rs
- confidence: inferred
  path: crates/ignite/src/lib.rs
- confidence: inferred
  path: crates/ignite/src/main.rs
- confidence: inferred
  path: crates/ignite/tests/main.rs
- confidence: inferred
  path: dist/toolchain/Dockerfile
- confidence: inferred
  path: docs/layering.md
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: std/option.ignite
- confidence: inferred
  path: std/protocol/http.ignite
- confidence: inferred
  path: x/web.ignite
revision: 3
---
# Story: web-core

## Outcome

A program imports `x.web` from a second module root `x` beside `std` and serves a route table of hand-built `Route` values on today's language: patterns with parameters, typed bodies through `std.data` codecs, 404, 405 with `Allow`, 400 and 413 as `application/problem+json`, a `before` check, and the route table's errors reported by `ignite check` as E120. This is the smallest useful server slice of `epic:x-web`.

## Context

- Packaging decided in `docs/web.md` §3: `x/web.ignite` is `x.web`, loaded as std is (reachable only, slice §16.22), root from `--x PATH`, default `share/ignite/x` beside the driver, else the repository's `x/`; `task dist` and the toolchain image carry it; a layering row above every std layer (`x` may import std and `x`; no `std.*` imports `x.*`).
- Today an import that is neither `std.*` nor a sibling is E031 (`parse_siblings`, `crates/ignite/src/lib.rs`; `std_root` beside it).
- Routes and checks: web §6.2; errors: web §6.7; the std-only reference service (639 tokens): web §5 last paragraph and appendix A.1.
- `Bool.then` (web §8 `std-bool-then`) is used by every later example; it lands here in `std/option.ignite`.

## Acceptance

- Loader: with `--x DIR`, a program importing `x.demo` from a fixture root checks, builds and runs its inline tests on both targets (`crates/ignite/tests/`); without `--x`, a driver copied to `<prefix>/bin/ignite` with `<prefix>/share/ignite/x` reads that root first, else the repository `x/`; an `x` module whose `package` differs from its path is E031; an unreached `x` module with a type error produces no diagnostic.
- Layering: `layering::` refuses a fixture `std` module importing `x.web`; `docs/layering.md` has the `x` row.
- `task dist` produces an archive containing `share/ignite/x/web.ignite` (test reads the tarball listing); `task test-ignite` runs the inline tests of `x/` on both targets; the toolchain `Dockerfile` copies `share/ignite/x`.
- `x.web` serves, over `Pipe.pair()` through `Http.serve` on both targets, the reference service: `GET`/`POST /todos`, `GET`/`PATCH`/`DELETE /todos/{id}`, `/health`; inline tests assert: unknown path 404; known path and other method 405 with `Allow` listing the table's methods; a body that does not decode 400 `application/problem+json` with `invalid-params` naming `$.title`; a body over `limits.body` 413; every error body has `type`, `title`, `status`, `detail` (RFC 9457).
- Routes `files(directory)` (GET and HEAD under a `Directory`; `..` refused with 404) and `stream(start, next)` each have an inline test; the stream test sends two server-sent events built with `sse_event(name, data)` and asserts `Content-Type: text/event-stream` and the bytes `event: name\ndata: …\n\n` of each.
- A `get` route whose handler writes a store is refused at `ignite check`: `get` takes handlers with `requires [state.read]`, so the write is E017 (the code the probe of web appendix B gives; web §6.2 lists the check), fixture with `.expected` naming the handler.
- `before = f` runs before every handler; an `Err(fail)` from it is the answer and the handler does not run (inline test).
- `ignite check` reports E120 for each of: a pattern declared twice, a pattern an earlier one hides, a malformed pattern, a handler parameter that is no pattern parameter, body or query field (one fixture with `.expected` each), through `HttpServer.validate` reading the application's checks.
- One test of the same service over TCP; `true.then(1) == Some(1)` inline; `docs/slice.md` section; `AGENTS.md` layout rows for `x/` and `x/web.ignite`; `task gate` green.

## Scope

- `crates/ignite/src/lib.rs`, `crates/ignite/src/main.rs` (the `x` root); `Taskfile.yml` (`dist`, `test-ignite`); `dist/toolchain/Dockerfile`.
- `docs/layering.md`, `crates/ignite-check/tests/layering.rs`.
- `x/web.ignite` (new); `std/protocol/http.ignite` (the application's checks for `validate`); `std/option.ignite` (`Bool.then`).
- `crates/ignite/tests/main.rs` and a new test file, fixtures under `crates/ignite/tests/fixtures/`.
- `docs/slice.md`, `AGENTS.md`. All inferred from web §3 and §8 rows 1 and 2.
