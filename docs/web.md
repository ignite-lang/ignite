# A web framework: `x.web`

**Status:** future design, not a slice contract. Nothing here is implemented.
**Date:** 8 November 2026
**Scope:** writing a typical HTTP JSON service in Ignite in as few source tokens as possible,
with no loss of checkability: routes, handlers, bodies, errors, middleware, OpenAPI, tests
without a socket, static files and streaming, on top of `std.protocol.http` and `std.data`.

Direction (Timo, 2026-11-08): a web framework, "really nice to use, token efficient". Token
efficient means that an agent or a person writes a typical web service in as few source tokens
as possible, while the compiler still catches errors and the service stays explicit about
effects, limits and wire formats. The framework ships with Ignite under a second root `x`
next to `std`, as the module `x.web` (Timo, 2026-11-08; section 3).

This document decides. Each section states the chosen design and the alternatives it rejects.
References cite `docs/slice.md` as "slice §n", `docs/serde.md` as "serde §n",
`docs/syntax.md` as "syntax Pn", a file and line otherwise. Every Ignite block in this
document is a sketch: `x.web`, `std.data` and the features of section 8 do not exist, so no
block is checked by the compiler, and the site check (`site::`, `crates/ignite/tests/site.rs`)
covers `site/book` only, not this file. Section 1 names the blocks that were checked.

## 1 The problem, measured

### 1.1 Method

- Tokenizer: `cl100k_base` of `tiktoken` 0.14.0. A second column uses the regular-expression
  tokenizer of syntax §2.1 (a literal is one token), extended with Python, TypeScript and Go
  string and operator forms. The two agree on the ranking of every row below.
- What is counted: every line of code, imports included. Blank lines, comment lines and
  leading indentation are not counted. A line goes to the endpoint it serves; a route line
  that serves several endpoints goes to the first (Axum registers `get(show).patch(patch)
  .delete(delete)` on one line, counted under show). Everything else is "setup": imports,
  types, codecs, store, router and server wiring.
- The reference service is the in-memory part of `examples/todo/todo.ignite`: `GET /health`
  answers `ok`; `GET /todos` lists; `POST /todos` takes `{"title", "done"?}` and answers 201
  with the todo; `GET`, `PATCH` (either field optional) and `DELETE` (204) on `/todos/{id}`,
  404 for an unknown id; 400 for a body that does not decode. Persistence (todo.ignite:76-194)
  is left out: no comparison framework writes it, and it is the same code in every Ignite
  variant (section 9).
- Ignite today: the reference service written with `std.protocol.http` and `std.json` in the
  idiom of `examples/todo/todo.ignite`, checked silently by `ignite check` 0.13.0 against
  this repository's `std/`. It is in appendix A.
- The four comparisons are written from the idioms of their documentation (FastAPI with
  Pydantic models; Hono with `@hono/zod-validator`; Axum 0.8 with serde and `State`; Go 1.22
  `net/http` with method patterns) and are in appendix A. They were not run.
- The Ignite proposal is counted at four levels (section 8): L0 needs no compiler change,
  each later level adds compiler features.

### 1.2 Tokens per endpoint (cl100k)

| endpoint | Ignite today | FastAPI | Hono | Axum | Go `net/http` | median of four | target | L0 | L1 | L2 | L3 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| health | 17 | 12 | 14 | 12 | 26 | 13 | ≤ 13 | 4 | 4 | 4 | 4 |
| list | 37 | 21 | 16 | 42 | 72 | 31.5 | ≤ 32 | 40 | 38 | 34 | 34 |
| create | 110 | 42 | 66 | 86 | 116 | 76 | ≤ 76 | 70 | 70 | 61 | 43 |
| show | 53 | 32 | 43 | 67 | 68 | 55 | ≤ 55 | 37 | 37 | 33 | 33 |
| patch | 110 | 57 | 97 | 101 | 144 | 99 | ≤ 99 | 69 | 69 | 63 | 63 |
| delete | 48 | 41 | 40 | 49 | 69 | 45 | ≤ 45 | 37 | 37 | 33 | 33 |
| setup | 217 | 91 | 72 | 206 | 125 | 108 | ≤ 140 | 382 | 189 | 134 | 134 |
| **total** | **592** | **296** | **348** | **563** | **620** | | **≤ 370** | **639** | **444** | **362** | **344** |
| total, regex tokenizer | 637 | 315 | 394 | 707 | 653 | | | 708 | 499 | 385 | 363 |
| lines | 68 | 44 | 38 | 77 | 102 | | | 37 | 34 | 31 | 27 |

Targets: each endpoint at most the median of the four frameworks; setup at most that median
plus the system declaration, which Ignite writes and the others leave to a command line (the
listener's address and port, the store, the executable's name: 30 to 35 tokens); the total at
most 1.25 times FastAPI's. L2 meets every target but list (34 against 32: the `_: Unit`
parameter and the `Ok(…)`; section 8 names no feature that removes them).

L0 is longer than today: the hand-written combinator codecs of serde §3.3 cost 382 tokens of
setup, three times today's hand-written `to_json`. L0's gain is checkability (typed decoding
with a path in every 400, 405 with `Allow` from the table, a route table checked at
`ignite check`, problem+json, OpenAPI), not tokens. The tokens come with `codec<T>()`
(serde §3.2), L1: 195 fewer.

### 1.3 The real todo service

`examples/todo/todo.ignite` without its tests, per part:

| part | lines | cl100k today | in `x.web` (L2, section 9) |
|---|---|---|---|
| imports, `Todo`, `Todos` | 23 | 131 | 251 (with `Item`, `NewTodo`, `Change`, `item`) |
| `to_json`, `to_record`, `from_record` (todo.ignite:59-75) | 13 | 158 | 0 |
| persistence (todo.ignite:76-194) | 86 | 651 | 656 |
| `stored`, `live` | 8 | 80 | 122 (with the `before` layer) |
| list, create, show, patch, delete handlers | 55 | 464 | 375 |
| `route`, `handle` (todo.ignite:277-298) | 20 | 202 | 4 (`alive`) and the route lines inside the system |
| system and assembly | 32 | 197 | 204 |
| **total without tests** | **237** | **1883** | **1612 (198 lines), −14 %** |
| HTTP part (all but persistence and system) | | 1035 | 752, −27 % |

Where today's tokens go: the JSON shape of `Todo` is written three times (todo.ignite:59, 63,
71); every handler parses its own body and picks fields (`value.text("title")`, todo.ignite:228
to 238); every handler chooses its status and error body (`status_response(404)` appears 7
times); the router matches method and segments by hand and writes each `Allow` list
(todo.ignite:277-293); an unparsable id is handled in the router (todo.ignite:282). The
framework removes each of these; the persistence code stays as it is.

## 2 Goals and non-goals

Goals:

1. Each endpoint of a JSON service in at most the tokens the median of four common frameworks
   needs for it (section 1.2, L2).
2. Routes are data: one table per application, checked at `ignite check` (duplicates, shadowed
   routes, malformed patterns, path parameters the handler's input does not take).
3. Handlers are plain functions `fn(state: S, params: P[, body: B]) -> Reply<O>`; bodies,
   parameters and outputs go through `std.data` codecs (serde §3.3); a body that does not
   decode is a 400 naming the path (`$.title`) before the handler runs.
4. One error shape: RFC 9457 problem details (`application/problem+json`).
5. Effects stay the existing rules: a handler touches only what its state holds; a safe method
   (GET, HEAD) cannot write, by type.
6. The application is an `HttpApp<S>` in an `HttpServer` component: `ignite plan`, `explain`,
   `run` and `apply` work unchanged.
7. OpenAPI generated from the table at check time, a test client with no socket, static files
   through `Directory`, streamed bodies through slice §19.23.

Non-goals for the first version: HTML templates, sessions and cookies beyond reading a
header, WebSockets, HTTP/2, TLS (Http has none, `std/protocol/http.ignite:113`), content
negotiation beyond JSON and plain text, a client for outbound requests.

## 3 Packaging and naming

Decided by Timo (2026-11-08): the framework is a layer above std, ships and is versioned with
the compiler, and lives in this repository under a second root `x` beside `std`, with the
prefix `x.web`.

| question | decision |
|---|---|
| where the files live | `x/web.ignite` is the module `x.web`; `x/web/openapi.ignite`, `x/web/client.ignite`, `x/web/files.ignite` are `x.web.openapi`, `x.web.client`, `x.web.files`. The root `x/` sits beside `std/` |
| how a program imports it | `import x.web.{web, get, post, Reply}`, as a std module; `import x.web.client.{client}` for the test client. Imports are not re-exported (slice §4.1), so each submodule is imported by its own path |
| how the loader finds it | as std: module `x.a.b` is `<x root>/a/b.ignite`, its `package` equal to its module path (E031 otherwise), loaded only when reached (slice §16.22). The x root is `--x PATH`, default `share/ignite/x` beside the driver's `bin/` when it exists, else the repository's `x/` next to `std/`, mirroring `--std` (`crates/ignite/src/main.rs:24`, `std_root` in `crates/ignite/src/lib.rs:639`). Today an import of a module that is neither `std.*` nor a sibling of the program is E031 (slice §4.1, §25.6; `parse_siblings`, `crates/ignite/src/lib.rs:1035`) |
| release archive and toolchain image | `task dist` copies `x` to `share/ignite/x` next to `share/ignite/std` (`Taskfile.yml:102-104`); the toolchain image carries both |
| layering | a new row above every std layer in `docs/layering.md` and `crates/ignite-check/tests/layering.rs`: `x` (every `x.*`) may import every std layer and `x`; no `std.*` module imports `x.*` (a failure of the test, as rule 1). Rule 3 (no protocol names outside `std.protocol.*`) does not apply to `x`: `x.web` is HTTP by name |
| rules `x` follows | std's: inline `test` blocks run on both targets by `task test-ignite`, which gains `x/`; every module checks silently; no hand-written Rust; each diagnostic it relies on keeps its fixture |
| versions | `x.*` has the compiler's version; a program that imports it builds against the `x` of its `ignite` |

The loader change is its own story (`x-root`, section 11) before any framework story.

Rejected:

| option | rejected because |
|---|---|
| a repository of its own, as `ignite-lang/ignite-lsp` | that repository is an application built against a released `ignite` (its `AGENTS.md`, "Source of truth"); a library there would need a package mechanism that does not exist (vision §5, "non-std imports, packages", wave H), and std and framework changes could not land in one PR |
| `std.web` | the framework is a layer above std, not part of it (Timo); std stays the base a framework builds on |
| `web.*` | takes a short name a program package may want; the `x` root marks everything outside std that ships with Ignite |
| `ignite.web.*` | two more tokens per import for no information |
| vendored files as sibling modules | a sibling needs the program's package as its prefix (`docs/friction.md` row 59), so every program would hold its own renamed copy |

## 4 The design in one page

The reference service at L2 (section 8), 362 tokens:

```ignite
package bench

import x.web.{web, get, post, patch, delete, alive, found, existed, Reply}

type Todo { id: Int, title: Text, done: Bool = false }
type NewTodo { title: Text, done: Bool = false }
type Change { title: Option<Text> = None, done: Option<Bool> = None }

fn list_todos(todos: Store<Todo>, _: Unit) -> Reply<List<Todo>> = Ok(todos.values())

fn create(todos: Store<Todo>, _: Unit, new: NewTodo) -> Reply<Todo> {
  let todo = Todo { id = todos.next_id(), title = new.title, done = new.done }
  todos.put(todo.id, todo)
  Ok(todo)
}

fn show(todos: Store<Todo>, id: Int) -> Reply<Todo> = todos.get(id).found()

fn change(todos: Store<Todo>, id: Int, c: Change) -> Reply<Todo> =
  todos.update(id, |t| Todo { title = c.title.unwrap_or(t.title), done = c.done.unwrap_or(t.done), ..t }).found()

fn remove(todos: Store<Todo>, id: Int) -> Reply<Unit> = todos.remove(id).existed()

system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource todos: Store<Todo>

  component server: HttpServer {
    listener = ingress
    application = web(todos, [
      alive("/health"),
      get("/todos", list_todos),
      post("/todos", create),
      get("/todos/:id", show),
      patch("/todos/:id", change),
      delete("/todos/:id", remove)
    ])
    executable = "todod"
  }
}
```

What the compiler and the framework check here, with no line written for it: a body that is
not a `NewTodo` is a 400 problem naming the member; `/todos/abc` matches no route (404) because
`id` decodes as `Int`; `PUT /todos` is 405 with `Allow: GET, POST`; two routes for
`GET /todos/:id` are an E120 at `ignite check`; `show` writing to the store is an E017 because
`get` takes a read-only handler (section 6); `create` reading the clock is an E017 (no ambient
effect in a handler, slice §21.3).

### 4.1 The layers

| layer | where | compiler change |
|---|---|---|
| HTTP protocol, `HttpApp<S>`, `HttpServer<S>` | `std.protocol.http` (exists) | none; two add-only fields (section 5.6) |
| value model, codecs, formats | `std.data`, `std.json` (serde §3, steps 1 and 2) | none |
| the framework | `x.web`, `x.web.openapi`, `x.web.client`, `x.web.files` | none (L0) |
| derived codecs | `codec<T>()` (serde step 5) | one frontend pass |
| shorter forms | section 8, F2 to F4 | per feature |

## 5 The surface

### 5.1 Routes as data

```ignite
type Route<S> {
  method: Text
  pattern: Text                    // "/todos/:id"
  status: Int
  input: Schema                    // of P, from its codec (section 5.6)
  body: Option<Schema>
  output: Schema
  run: fn(state: S, request: Request, params: List<(Text, Text)>) -> Response requires [state.*]
}

type Routes<S> = List<Route<S>>

fn get<S, P, O>(path: Text, handler: fn(state: S, params: P) -> Reply<O> requires [state.read], headers: List<Text> = [],
                params: Codec<P> = codec(), output: Codec<O> = codec(), status: Int = 200) -> Route<S>
fn post<S, P, B, O>(path: Text, handler: fn(state: S, params: P, body: B) -> Reply<O> requires [state.*],
                    params: Codec<P> = codec(), body: Codec<B> = codec(), output: Codec<O> = codec(),
                    status: Int = 201) -> Route<S>
// put, patch (200) as post; delete (status 200, 204 when O is Unit) as get with requires [state.*]
fn alive<S>(path: Text) -> Route<S>                         // 200 "ok\n", text/plain
fn raw<S>(method: Text, path: Text, handler: fn(state: S, request: Request) -> Response requires [state.*]) -> Route<S>
fn scope<S>(prefix: Text, layers: List<Layer<S>>, routes: Routes<S>) -> Routes<S>
fn web<S>(state: S, routes: Routes<S>, layers: List<Layer<S>> = [], shape: Shape = Shape.default(),
          limits: WebLimits = WebLimits {}) -> HttpApp<S>
```

The defaults `= codec()` are L2 (features F1 and F3); at L0 every codec is an argument
(`get("/todos/:id", show, int(), TODO)`).

- **Erasure.** A route's input and output types differ per route, and Ignite has no
  existential types and no user interfaces (E099, slice §18.7). Each constructor captures its
  codecs and the handler in the closure `run`, whose type no longer mentions `P`, `B` or `O`.
  Probe (appendix B, `ignite check` and `ignite test` on the LLVM target, 0.13.0): a generic
  constructor returning `Route<S>` whose `run` closure calls the captured effectful handler with
  its own `state` parameter fits `requires [state.*]` (slice §11.2, §13.3) and serves typed
  handlers through `HttpApp`.
- **Patterns.** `:name` marks a parameter, as in Hono and Express. `{name}` is not usable: a
  `{` in a text literal opens an interpolation (probe: `"/todos/{id}"` is E001 "unclosed `{`
  in interpolation"), so it would need `{{id}}` or a raw string. A pattern is matched on the
  path's non-empty segments, as `segments` does (`std/protocol/http.ignite:753`): `/todos/` is
  `/todos`.
- **Parameters.** `P` decodes from a `Value.Map` of the path parameters, then the query
  parameters, by name. A pattern with one parameter and a `P` that is not a record decodes that
  parameter alone (`show(todos, id: Int)`). No parameters: `P` is `Unit` (`_: Unit`). A segment
  that does not decode means the route does not match, so `/todos/abc` is 404 as today
  (todo.ignite:282), not 400. A query parameter never fills a field a path parameter filled.
- **Headers.** A route lists the headers it reads: `get("/todos", list, headers =
  ["if-none-match"])`; each listed header fills the field of `P` of the same name with `-` as
  `_` (`if_none_match`). A listed header is never read from the query, and an unlisted one never
  reaches `P`, so a query parameter cannot stand in for a header. A field of `P` that is neither
  a pattern parameter nor a listed header comes from the query only.
- **Order.** The first route that matches method and pattern runs; a literal segment beats a
  parameter at the same position only by being listed first. The checks of 5.1.1 make the order
  visible.

#### 5.1.1 Checks at `ignite check`

`web` computes, purely, a list of reasons over the table and puts it in the add-only field
`HttpApp.checks` (5.6). `HttpServer.validate` (`std/protocol/http.ignite:168`) returns it with
its limit reasons, so each is an E120 at `ignite check` (slice §16.17), through the evaluator
that already runs `validate`; no compiler change.

| check | example reason |
|---|---|
| duplicate | `route GET /todos/:id is declared twice (routes 4 and 7)` |
| shadowed | `route GET /todos/new (route 5) is never reached: GET /todos/:id (route 4) matches first` |
| malformed pattern | `route GET /todos/:: an empty parameter name` |
| parameter not taken | `route GET /todos/:id: the handler's input Int is filled from the path, but the pattern has 2 parameters` (from the input `Schema`) |
| header not taken | `route GET /todos: header if-none-match has no field in ListQuery` |
| body on a safe method | `route GET /search takes a body; GET content has no defined semantics (RFC 9110 9.3.1)` |

A missing handler cannot be written: a route is built from its handler. A handler whose types
do not fit its route (a `get` handler that takes a body, a body type with no codec) is a type
error at the constructor call (E040, E042), not a run-time 500.

Rejected: a router written by hand with `match (method, segments)` (today, todo.ignite:277);
it cannot be checked and every `Allow` list is hand-written. A route table as a compile-time
literal checked by the type checker (F5, section 8): it adds the same checks with better
positions, but needs the compiler, and the evaluator gives them now.

### 5.2 Handlers and status mapping

```ignite
type Reply<T> = Result<T, Fail>

fn found<T>(value: Option<T>) -> Reply<T>        // None is 404
fn existed<T>(value: Option<T>) -> Reply<Unit>   // Some is Ok(()), None is 404
```

- A handler is a plain function; nothing marks it as one. It takes the state first, then
  `P`, then the body for `post`, `put` and `patch`. It can be tested by calling it.
- Success: the route's `status` (GET, PUT, PATCH 200; POST 201; DELETE 200), and 204 with no
  body when the output encodes to `Null` (a `Unit` output). `status = 202` on the constructor
  overrides it (named arguments, slice §14.2).
- Output: `encode(output, value)` written by `std.json.write`, `Content-Type:
  application/json`. A handler that needs another representation or its own headers returns
  through `raw`.
- Rejected: a `Response` builder returned by every handler (`Ok(created(todo))`): 3 to 5
  tokens per handler and a status decided in code where the table could say it. Rejected:
  handlers that take the `Request` (Express, Go): every handler would parse its own input,
  which is today's cost.

### 5.3 Bodies

- `B` decodes with `decode(body, json.read(request.body)?)` (serde §3.3, §6.1). A body that is
  not JSON, or does not decode, is 400 with the `DecodeError` (serde §5.2) as an
  `invalid-params` member of the problem: `{"name": "$.title", "reason": "expected Text, found
  Int"}`. A body whose `Content-Type` is present and not `application/json` is 415.
- Sizes: `HttpLimits.body` (1 MiB default, `std/protocol/http.ignite:88`) bounds every body
  before the framework sees it (413). `WebLimits` adds `DataLimits` (serde §2, goal 5: depth,
  entries) for decoding. A `max_body(16 KiB)` layer narrows one scope.
- A field with a default is optional on decode (`done: Bool = false` accepts `{"title": "a"}`).
  serde §5 does not say this yet; it is a requirement on `data-derived-codec` (the frontend
  knows each default) and on the combinators (`field_or`). `Option` fields are absent or
  `null`.
- Wire names: `web(…, shape = Shape { keys = Case.Camel, ..Shape.default() })` sets the
  `Shape` (serde §5.1) for every codec of the table.

### 5.4 Errors

```ignite
type Fail {
  status: Int
  title: Text = ""                 // the reason phrase when empty
  detail: Text = ""
  kind: Text = "about:blank"       // the problem's "type"
  invalid: List<(Text, Text)> = [] // invalid-params: (name, reason)
}

fn fail(status: Int, detail: Text) -> Fail
fn not_found() -> Fail
fn bad_request(detail: Text) -> Fail
fn conflict(detail: Text) -> Fail
```

Every error the framework writes or a handler returns is `application/problem+json` (RFC 9457):
`{"type", "title", "status", "detail", "invalid-params"?}`. The framework's own: 404 (no route),
405 with `Allow` listing the table's methods for that path, 400 (decoding), 413, 415, 500 (a
handler's `Err` with status 500, or an encoding failure). A handler failure that ends the task
(slice §14.8) stays what Http does today: the connection ends.

Rejected: free-form error bodies per handler (today's `"Not Found\n"`): a client must know each
handler. Rejected: an error enum per application: needs user interfaces to map it to statuses.

### 5.5 Middleware as values

```ignite
type Layer<S> {
  before: fn(state: S, request: Request) -> Reply<Request> requires [state.*] = pass
  after: fn(state: S, request: Request, response: Response) -> Response requires [state.*] = keep
}

fn before<S>(check: fn(state: S, request: Request) -> Reply<Unit> requires [state.*]) -> Layer<S>
fn bearer<S>(valid: fn(state: S, token: Text) -> Bool requires [state.*]) -> Layer<S>   // 401 with WWW-Authenticate
fn max_body<S>(size: ByteSize) -> Layer<S>
fn headers<S>(set: List<(Text, Text)>) -> Layer<S>                                      // e.g. CORS, cache control
```

- A layer is a record of two functions with the state, so it has the handler's authority and no
  more (slice §19.23 states why: a closure can carry an effect only on its own parameters,
  §20.4.3). Layers apply in list order, outermost first: `web`'s layers around every route,
  `scope`'s around its routes. A `before` that returns `Err` answers with the problem and runs
  no handler.
- Access logging is not a layer: writing to standard error is an ambient effect (`stderr`,
  slice §25.3), and an ambient effect in a handler is E017 (slice §21.3, fixture
  `e017_ambient_in_handler`). It is a property of the server instead:
  `HttpServer { access_log = true }` (story `http-access-log`), written by
  `std.protocol.http`, whose role may declare the effect.
- Rejected: middleware as functions wrapping a handler (`fn(Handler) -> Handler`, Axum's tower,
  Express's `next`): a wrapped handler is a closure over a closure, and control flow moves into
  closures (syntax §5 rejects the same for Gleam's `use`). The record form keeps every layer
  inspectable data the model can show.



### 5.6 What `std.protocol.http` gains

Two add-only fields of `HttpApp<S>`, which knows nothing of `x.web`:

```ignite
type HttpApp<S> {
  state: S
  handle: fn(state: S, request: Request) -> Response requires [state.*]
  piece: fn(state: S, request: Request) -> IoResult<Option<Bytes>> requires [state.*] = no_piece
  checks: List<Text> = []          // reasons HttpServer.validate reports (E120)
  description: Text = ""           // a machine-readable description of the interface
}
```

`std.data` gains `Schema`, a description of a type's `Value` shape (record members and their
schemas, list items, options, enum tags), carried by every `Codec<T>` as a third field and
written by the combinators and by `codec<T>()`. It serves 5.1.1's checks and OpenAPI
(story `data-schema`).

### 5.7 OpenAPI

`x.web.openapi(routes, info) -> Text` writes an OpenAPI 3.1 document: one path item per
pattern (`:id` as `{id}`), each operation's parameters, request body and responses from the
route's schemas, every error as `application/problem+json`. `web` stores it in
`HttpApp.description`. It is pure, so the evaluator computes it at check time, and the generic
command that prints a `Text` property verbatim prints it with no build:

```text
ignite explain examples/todo/todo.ignite TodoService.server.application.description --raw > openapi.json
```

`--raw` is new (story `explain-raw`): today `explain` prints a property as debug text (slice
§15.8). It names no `x` or std item, so layering rule 6 holds.

Rejected: an `ignite openapi` command (the CLI would know `x.web`); a test that writes the
document (a test is an effect root, but an artifact written by a test run is not a build
output); runtime serving only (`GET /openapi.json`): also offered, as the route
`openapi_route("/openapi.json")`, but the document must exist without a running server.

### 5.8 A test client with no socket

```ignite
import x.web.client.{client}

test "create and read" {
  let api = client(web(Store.new(), routes()), Pipe.pair())
  assert api.post("/todos", r#"{"title":"milk"}"#).status == 201
  assert api.get("/todos/1").body == r#"{"id":1,"title":"milk","done":false}"#
  assert api.get("/todos/x").status == 404
}
```

`client` spawns `Http.serve` (slice §18.6) on one end of the pipe and speaks HTTP/1.1 on the
other, so a test passes through the same parser, framing and limits as a socket, as the session
tests of `std/protocol/http.ignite` do. `Pipe.pair()` is test-only (E051, slice §18.7), so the
test passes it in; `client` itself is an ordinary function. `call(app, request)` runs the
handler directly when framing is not under test. An `HttpApp` held in a `let` needs its type
written to call `handle` on it in a test (probe: E042 "the type of its receiver is not
declared"); `client` takes it as a parameter, so a test does not meet that.

### 5.9 Static files

`files("/assets", |s| s.assets)` serves `GET` and `HEAD` of every path under the prefix from
the `Directory` the selector returns, as `Content::Binary` (slice §19.23) with a content type
by extension and 404 for a missing file. Path safety is `Directory`'s: an absolute path, a `..`
component and every symbolic link are refused inside the substrate (slice §21.4, rows 2 to 4 of
21.0), so the route needs no checks of its own. The selector is a pure function of the state;
the read needs `state.read`, which `get`'s type allows. Bounded by `Directory.read_file`'s size
limit; a file too large to hold is a streamed route.

### 5.10 Streaming

```ignite
fn stream<S, P>(path: Text, start: fn(state: S, params: P) -> Reply<List<(Text, Text)>> requires [state.*],
                next: fn(state: S, params: P) -> IoResult<Option<Bytes>> requires [state.*]) -> Route<S>
```

`start` answers the head (status 200 and headers, or a problem); then Http calls
`HttpApp.piece` until `Ok(None)` (slice §19.23), which `web` routes to the matching route's
`next` with the same decoded `P`. A stream's position lives in the state (a `Store` keyed by
something in `P`), because `piece` is called with state and request only and a closure cannot
carry a handle (slice §19.23). Server-sent events are a `stream` whose pieces are `data:` lines
(`sse_event(text) -> Bytes`).

## 6 Effects and capabilities

No new rule. A handler states what it touches by what its state holds and by what it calls:

- The handler has no `requires`; its set is inferred (slice §5), on its parameter `state`:
  `show` above needs `todos.read`, `create` `todos.write`. Every capability a service has is a
  resource held in the state record (slice §21.3: a capability comes from a resource or a test
  constructor only).
- Route constructors take `fn(state: S, …) -> Reply<O> requires [state.*]` (slice §11.2), so
  any effect on the state fits. A handler that reaches anything else (the clock, standard
  output) is E017 at the constructor call, as `e017_ambient_in_handler` is today.
- `get`, `head` and `files` take `requires [state.read]`: a safe method (RFC 9110 9.2.1) cannot
  write. Probe (appendix B): with `get` so typed, a `show` that calls `todos.put` is "E017:
  effectful function `show` does not fit `fn(state: Store<Todo>, params: Int) -> Result<Todo,
  Fail> requires [state.read]`: its effect `todos.write` is not covered". The ops differ per
  capability kind (slice §21.3: `read`, `write`, `launch`, `draw`, `argument`), so a GET handler
  can read a `Store` and a `Directory` but cannot launch a process or draw random bytes; one that
  must uses `raw("GET", …)`, which is visible in the table.
- A handler may still write `requires [state.read]` to document itself; E016 then holds it to
  that.
- Clock: there is no capability for wall-clock time. `Clock.monotonic` is ambient (`clock.read`,
  slice §16.13), so no handler can read it. A `Clock` resource held in the state is a std story
  outside this design (`std-clock-capability`, section 11).

Rejected: per-route `requires` lists on the table (`get("/todos", list, requires = …)`): the
handler's inferred set already says it, and the method-level read-only rule catches the error
that matters. Rejected: an effect-polymorphic route type: no effect variables in the language.

## 7 Fit with the system model

- The application is an `HttpApp<S>`: `web(state, routes)` returns one, and the component stays
  `HttpServer` (`std/protocol/http.ignite:138`). Its `machine` and `validate` hooks, its
  `nativeProcess` binding, `ignite run`'s supervisor and rollout and `ignite apply` see no
  difference.
- Today a closure in a component property stops the model: probe, `application = app(todos,
  routes())` is "E118: evaluating Probe.server.application: a closure cannot be encoded in the
  model" (`Value::Closure` in `crates/ignite-eval/src/value.rs:188`). Until that changes (F2,
  story `eval-closure-values`), a program names its handler and calls `dispatch` (L0, L1):

  ```ignite
  fn api(todos: Store<Todo>, request: Request) -> Response = dispatch(routes(), todos, request)

  component server: HttpServer {
    listener = ingress
    application = HttpApp { state = todos, handle = api, checks = checks(routes()) }
    executable = "todod"
  }
  ```

  The model then shows `handle` as `{"fn": "bench.api", "type": "Fn"}` (probe, `ignite plan`).
  With F2 the model encodes a closure as the function that created it and its captured values,
  so the route table is in the model, `ignite explain TodoService.server.application` lists the
  routes, and a saved plan's identity (slice §17.3) changes when a route changes.
- No backend change, so `CODEGEN_REVISION` does not move for the framework itself.

## 8 What needs the compiler

Std-only first, as serde §3 does: every story up to L0 changes no compiler crate except the
loader (section 3), and the framework is usable at L0 the day `x-root`, the two `std.data`
steps and `http-app-checks` land.

| level | adds | kind | tokens (reference service) | saving | proposed where |
|---|---|---|---|---|---|
| L0 | `x.web` with combinator codecs, the named-handler form of section 7 | std and `x` only; loader | 639 | (+47 against today) | this document |
| L1 | F1 `codec<T>()` derivation | one frontend pass | 444 | −195 | serde §3.2, step 5 |
| L1 + F2 | F2 closures encoded in the model: `web(…)` straight in the property, no `api`, `routes()` or `checks` lines | `crates/ignite-eval` | 393 | −51 | here |
| L2 | F3 parameter defaults, with a default whose type mentions a type parameter typed at the call site (`params: Codec<P> = codec()`), and `status` defaults | syntax, check, types | 362 | −31 | syntax P10 (defaults unscheduled; slice §14.2 took named arguments only); the call-site typing is new |
| L3 | F4 `Store.insert(f: fn(Int) -> T) -> T` and record update from a record of another type with the same field names (`Todo { id, ..new }`) | builtin and types | 344 | −18 | here |
| — | F5 route literals checked by the type checker | syntax, types | 0 | better positions for 5.1.1's checks | here; not planned |

Other proposals of `docs/syntax.md` touch the todo rewrite but not the reference service: P3
(`?` on `Option`) shortens `read_record` by 3 lines; P1 (`is`) shortens `live`. P11 (pair
literals) is not needed once codecs replace hand-written `Object([...])` lists.

F3's call-site typing is the one real language question: syntax P10 types a default in the
declaring module like a constant (slice §4.7). A default `codec()` of type `Codec<P>` can only
be typed once `P` is known, at the call. The rule proposed: a default whose type mentions a
type parameter of the fn is typed and filled at each call site, after inference (open question
Q6).

## 9 The todo service in `x.web`

`examples/todo/todo.ignite` rewritten at L2, without its tests: 198 lines and 1612 tokens
against 237 lines and 1883 tokens (section 1.3). The persistence functions (todo.ignite:76-194)
stay as they are except `save`, which writes `write(encode(codec(), todo))` in place of
`encode(to_record(todo))`, and `read_record`:

```ignite
fn read_record(state: Todos, path: Text) -> Option<Todo> = match read_text_file(state.data, path) {
  Ok(text) => match read(text) {
    Ok(value) => decode(codec(), value).ok()
    Err(_) => None
  }
  Err(_) => None
}
```

The rest:

```ignite
package examples.todo

import std.data.{codec, encode, decode}
import std.json.{read, write}
import std.option.unwrap_or
import std.fs.{read_text_file, write_text_file, join_path}
import std.text.{ends_with, strip_suffix}
import std.contract.Availability
import std.sli.rolling
import std.deploy.{Deployment, localSocket, nativeProcess, scheduledProbeHost}
import std.result.{map, map_err, ok}
import x.web.{web, get, post, patch, delete, alive, before, not_found, fail, Reply}
import examples.todo.health.{HttpHealth, healthy}

type Todo {
  id: Int
  title: Text
  done: Bool = false
  version: Int = 1
  deleted: Bool = false
  deleted_by: Int = 0
}

type Item { id: Int, title: Text, done: Bool }
type NewTodo { title: Text, done: Bool = false }
type Change { title: Option<Text> = None, done: Option<Bool> = None }

type Todos {
  todos: Store<Todo>
  loaded: Store<Bool>
  data: Directory
}

fn item(todo: Todo) -> Item = Item { id = todo.id, title = todo.title, done = todo.done }

// ... persistence, todo.ignite:76-194, with save and read_record as above ...

fn stored(state: Todos, todo: Todo) -> Reply<Item> = match save(state, todo) {
  Ok(_) => Ok(item(todo))
  Err(_) => Err(fail(500, "not stored"))
}

fn live(todo: Option<Todo>) -> Reply<Todo> = match todo {
  Some(found) => if found.deleted { Err(not_found()) } else { Ok(found) }
  None => Err(not_found())
}

fn ensure_loaded(state: Todos, _: Request) -> Reply<Unit> = load(state).map_err(|e| fail(503, "loading failed: {e.message}"))

fn list(state: Todos, _: Unit) -> Reply<List<Item>> {
  var shown = []
  for todo in state.todos.values() {
    if !todo.deleted {
      shown.push(item(todo))
    }
  }
  Ok(shown)
}

fn free_id(todos: Store<Todo>) -> Int {
  var id = todos.next_id()
  while todos.get(id) != None {
    id = todos.next_id()
  }
  id
}

fn create(state: Todos, _: Unit, new: NewTodo) -> Reply<Item> {
  let todo = Todo { id = free_id(state.todos), title = new.title, done = new.done }
  state.todos.put(todo.id, todo)
  stored(state, todo)
}

fn show(state: Todos, id: Int) -> Reply<Item> = live(state.todos.get(id)).map(item)

fn change(state: Todos, id: Int, c: Change) -> Reply<Item> {
  let changed = state.todos.update(id, |todo| if todo.deleted { todo } else {
    Todo { title = c.title.unwrap_or(todo.title), done = c.done.unwrap_or(todo.done), version = todo.version + 1, ..todo }
  })
  stored(state, live(changed)?)
}

fn remove(state: Todos, id: Int) -> Reply<Unit> {
  let token = state.loaded.next_id()
  match state.todos.update(id, |todo| if todo.deleted { todo } else { Todo { deleted = true, version = todo.version + 1, deleted_by = token, ..todo } }) {
    Some(todo) => if todo.deleted_by == token { stored(state, todo).map(|_| ()) } else { Err(not_found()) }
    None => Err(not_found())
  }
}

system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource todos: Store<Todo>
  resource loaded: Store<Bool>
  resource data: Directory { path = "todo-data", create = true }

  component server: HttpServer {
    listener = ingress
    application = web(Todos { todos = todos, loaded = loaded, data = data }, layers = [before(ensure_loaded)], routes = [
      alive("/health"),
      get("/todos", list),
      post("/todos", create),
      get("/todos/:id", show),
      patch("/todos/:id", change),
      delete("/todos/:id", remove)
    ])
    executable = "todod"
  }

  // health, reachable and the assembly `local` as todo.ignite:312-335
}
```

Behaviour that changes against today's tests (todo.ignite:337-428), each deliberate:

- Error bodies are problem+json, not `Not Found\n`.
- `PATCH /todos/1` with a body that is not JSON is 400 even when todo 1 does not exist (today
  404, todo.ignite:262): the body is decoded before the handler runs.
- The file of a todo also holds `deleted_by`, written by the derived codec; an older file
  without it decodes with the default `0`. `version` and `deleted` are written as today.
- `GET /health` passes through `ensure_loaded`, as today (todo.ignite:295): a failed load is
  503.

The tests become `client` calls (5.8): `answer(todos, "GET", "/todos", "")` is
`api.get("/todos")`.

## 10 Rejected designs

| design | rejected because |
|---|---|
| handlers registered by decorators or attributes (FastAPI, Rails) | Ignite has no attributes (serde §5.1) and no macros (design §3.5) |
| extractors chosen by parameter type (Axum's `FromRequest`) | needs user interfaces and `impl` (E099; vision §5, P4) |
| a context object passed to handlers (Hono's `c`, Go's `w, r`) | every handler parses its own input and picks its own status: today's cost |
| a router built by mutation (`app.get(…)` statements) | a table built by statements is not a value the evaluator can check or the model can show |
| a framework-specific component kind (`WebServer`) | duplicates `HttpServer`'s hooks; goal 6 |
| route patterns `{id}` | text interpolation (5.1) |
| runtime reflection for codecs | serde §3.2 option C |

## 11 Delivery

Std and loader stories (no framework code) come first; the framework stories need them. The
serde stories `data-value-model`, `data-codec-combinators` and `data-derived-codec` are
serde §8's and are not repeated here.

| step | story (draft) | contents | needs | shared files it touches |
|---|---|---|---|---|
| 1 | `x-root` | the `x` root: `--x`, the default beside `std`, reachable loading, `task dist` packs `share/ignite/x`, `task test-ignite` runs `x/`, the layering row and the rule "no `std.*` imports `x.*`", slice §4.1 | | `crates/ignite/src/lib.rs`, `crates/ignite/src/main.rs`, `Taskfile.yml`, `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `docs/slice.md`, `AGENTS.md` |
| 2 | `http-app-checks` | `HttpApp.checks` and `description`, add-only; `HttpServer.validate` reports `checks` as E120 | | `std/protocol/http.ignite`, `docs/slice.md` |
| 3 | `data-schema` | `Schema` and the codec's third field, written by every combinator | `data-codec-combinators` | `std/data.ignite`, `docs/slice.md` |
| 4 | `explain-raw` | `ignite explain … --raw` prints a `Text` property verbatim | | `crates/ignite/src/main.rs`, `crates/ignite-eval/src/explain.rs`, `docs/slice.md` |
| 5 | `http-access-log` | `HttpServer.access_log`, one line per request on standard error | | `std/protocol/http.ignite`, `docs/slice.md` |
| 6 | `web-core` | `x/web.ignite`: `Route`, the constructors, patterns, parameters, headers, `dispatch`, `web`, `checks`, `Reply`, `Fail` and problem+json, 404/405/400/413/415, `Layer` with `before`, `bearer`, `max_body`, `headers`, `scope`, `alive`, `raw`; inline tests on both targets | 1, 2, `data-codec-combinators` | `x/web.ignite` (new), `docs/slice.md`, `AGENTS.md` |
| 7 | `web-client` | `x/web/client.ignite`: `client` over `Pipe.pair()`, `call` | 6 | `x/web/client.ignite` (new) |
| 8 | `web-openapi` | `x/web/openapi.ignite`, `web` fills `description`, `openapi_route` | 3, 4, 6 | `x/web/openapi.ignite` (new), `x/web.ignite` |
| 9 | `web-files-stream` | `x/web/files.ignite` (`files`), `stream`, `sse_event` | 6 | `x/web/files.ignite` (new), `x/web.ignite` |
| 10 | `eval-closure-values` | F2: the model encodes a closure (slice §15.4 gains a row), explain prints it, plan identity covers it | | `crates/ignite-eval/src/model.rs`, `crates/ignite-eval/src/value.rs`, `docs/slice.md` |
| 11 | `param-defaults` | F3: syntax P10's defaults, with call-site typing of generic defaults | `data-derived-codec` | `crates/ignite-syntax`, `crates/ignite-check`, `crates/ignite-types`, `docs/slice.md`, `docs/syntax.md` |
| 12 | `todo-on-web` | `examples/todo/todo.ignite` on `x.web`, its tests on `client`, the token count of section 1.3 re-measured | 6, 7, `data-derived-codec`; 10 and 11 for the L2 form | `examples/todo/todo.ignite`, `crates/ignite/tests/` (todo suite), `docs/slice.md` |
| — | `std-clock-capability` | a `Clock` resource with wall-clock time for handlers | | `std/`, `crates/ignite-io`, `docs/slice.md`; independent |

Steps 1, 2, 4, 5 and 10 are independent and can run in one wave; 2 and 5 both edit
`std/protocol/http.ignite` and merge in order. Steps 7, 8 and 9 can run in one wave after 6;
8 and 9 each add one export line to `x/web.ignite`. Every step edits `docs/slice.md`, which the
wave's integration branch merges. F4 is not scheduled.

## 12 Open questions

| id | question | default if not answered |
|---|---|---|
| Q1 | POST answers 201 by default | yes; `status =` overrides |
| Q2 | a body that does not decode: 400 or 422 | 400, as todo today (todo.ignite:235, 237) and RFC 9110 15.5.1 |
| Q3 | typed headers beyond the `headers = […]` name list | the list only |
| Q4 | where a stream's position lives between pieces (5.10) | in the state, keyed by a field of `P` |
| Q5 | content negotiation (`Accept`, YAML through `std.yaml`) | JSON only; `Accept` not read |
| Q6 | F3: a default whose type mentions a type parameter, typed at the call site | as proposed in section 8; until then L1's explicit `codec()` arguments |
| Q7 | `codec()` as an argument before the handler fixes its `T`: the order inference uses (serde O1) | inference over all arguments; otherwise constructors take the handler first, as section 5.1 already does |
| Q8 | `x.web` versioned apart from the compiler | no, one version |
| Q9 | the read-only rule for GET (section 6) is too strict for counters and caches | keep it; `raw("GET", …)` is the visible exception |

## Appendix A: the measured programs

Each is counted by section 1.1's rules. The Ignite one is checked; the others are not run.

### A.1 Ignite today (checked, `ignite check` 0.13.0)

```ignite
package bench

import std.json.*
import std.protocol.http.*
import std.option.unwrap_or

type Todo {
  id: Int
  title: Text
  done: Bool = false
}

fn to_json(todo: Todo) -> Json =
  Object([("id", Number(todo.id)), ("title", Str(todo.title)), ("done", Bool(todo.done))])

fn list(todos: Store<Todo>) -> Response =
  Response.json(200, Array(todos.values().map(to_json)))

fn create(todos: Store<Todo>, body: Text) -> Response = match parse(body) {
  Ok(value) => match value.text("title") {
    Some(title) => {
      let todo = Todo { id = todos.next_id(), title, done = value.bool("done").unwrap_or(false) }
      todos.put(todo.id, todo)
      Response.json(201, to_json(todo))
    }
    None => status_response(400)
  }
  Err(_) => status_response(400)
}

fn show(todos: Store<Todo>, id: Int) -> Response = match todos.get(id) {
  Some(todo) => Response.json(200, to_json(todo))
  None => status_response(404)
}

fn patch(todos: Store<Todo>, id: Int, body: Text) -> Response = match parse(body) {
  Ok(value) => match todos.update(id, |todo| Todo {
    title = value.text("title").unwrap_or(todo.title)
    done = value.bool("done").unwrap_or(todo.done)
    ..todo
  }) {
    Some(todo) => Response.json(200, to_json(todo))
    None => status_response(404)
  }
  Err(_) => status_response(400)
}

fn delete(todos: Store<Todo>, id: Int) -> Response = match todos.remove(id) {
  Some(_) => status_response(204)
  None => status_response(404)
}

fn route(todos: Store<Todo>, request: Request) -> Response = match (request.method, request.segments()) {
  ("GET", ["health"]) => Response.plain(200, "ok\n")
  ("GET", ["todos"]) => list(todos)
  ("POST", ["todos"]) => create(todos, request.body)
  (_, ["todos"]) => method_not_allowed("GET, POST")
  (method, ["todos", id]) => match Int.parse(id) {
    None => status_response(404)
    Some(n) => match method {
      "GET" => show(todos, n)
      "PATCH" => patch(todos, n, request.body)
      "DELETE" => delete(todos, n)
      _ => method_not_allowed("GET, PATCH, DELETE")
    }
  }
  _ => status_response(404)
}

system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource todos: Store<Todo>

  component server: HttpServer {
    listener = ingress
    application = HttpApp { state = todos, handle = route }
    executable = "todod"
  }
}
```

### A.2 FastAPI (https://fastapi.tiangolo.com/tutorial/body/, /tutorial/body-updates/)

```python
from itertools import count

from fastapi import FastAPI, HTTPException, Response
from pydantic import BaseModel

app = FastAPI()


class Todo(BaseModel):
    id: int
    title: str
    done: bool = False


class NewTodo(BaseModel):
    title: str
    done: bool = False


class TodoPatch(BaseModel):
    title: str | None = None
    done: bool | None = None


todos: dict[int, Todo] = {}
ids = count(1)


@app.get("/health")
def health():
    return "ok"


@app.get("/todos")
def list_todos() -> list[Todo]:
    return list(todos.values())


@app.post("/todos", status_code=201)
def create(new: NewTodo) -> Todo:
    todo = Todo(id=next(ids), **new.model_dump())
    todos[todo.id] = todo
    return todo


@app.get("/todos/{id}")
def show(id: int) -> Todo:
    if id not in todos:
        raise HTTPException(404)
    return todos[id]


@app.patch("/todos/{id}")
def patch(id: int, change: TodoPatch) -> Todo:
    if id not in todos:
        raise HTTPException(404)
    todo = todos[id].model_copy(update=change.model_dump(exclude_unset=True))
    todos[id] = todo
    return todo


@app.delete("/todos/{id}", status_code=204)
def delete(id: int):
    if todos.pop(id, None) is None:
        raise HTTPException(404)
    return Response(status_code=204)
```

### A.3 Hono (https://hono.dev/docs/api/routing, https://hono.dev/docs/guides/validation)

```typescript
import { Hono } from 'hono'
import { zValidator } from '@hono/zod-validator'
import { z } from 'zod'

type Todo = { id: number; title: string; done: boolean }

const todos = new Map<number, Todo>()
let next = 1
const app = new Hono()

app.get('/health', (c) => c.text('ok'))

app.get('/todos', (c) => c.json([...todos.values()]))

app.post(
  '/todos',
  zValidator('json', z.object({ title: z.string(), done: z.boolean().default(false) })),
  (c) => {
    const todo = { id: next++, ...c.req.valid('json') }
    todos.set(todo.id, todo)
    return c.json(todo, 201)
  }
)

app.get('/todos/:id{[0-9]+}', (c) => {
  const todo = todos.get(Number(c.req.param('id')))
  return todo ? c.json(todo) : c.notFound()
})

app.patch(
  '/todos/:id{[0-9]+}',
  zValidator('json', z.object({ title: z.string().optional(), done: z.boolean().optional() })),
  (c) => {
    const id = Number(c.req.param('id'))
    const todo = todos.get(id)
    if (!todo) return c.notFound()
    const changed = { ...todo, ...c.req.valid('json') }
    todos.set(id, changed)
    return c.json(changed)
  }
)

app.delete('/todos/:id{[0-9]+}', (c) =>
  todos.delete(Number(c.req.param('id'))) ? c.body(null, 204) : c.notFound()
)

export default app
```

### A.4 Axum 0.8 (https://docs.rs/axum/0.8/axum/extract/index.html, examples/todos in the axum repository)

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Serialize)]
struct Todo {
    id: u64,
    title: String,
    done: bool,
}

#[derive(Deserialize)]
struct NewTodo {
    title: String,
    #[serde(default)]
    done: bool,
}

#[derive(Deserialize)]
struct TodoPatch {
    title: Option<String>,
    done: Option<bool>,
}

#[derive(Clone, Default)]
struct Db {
    todos: Arc<Mutex<BTreeMap<u64, Todo>>>,
    next: Arc<Mutex<u64>>,
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/todos", get(list).post(create))
        .route("/todos/{id}", get(show).patch(patch).delete(delete))
        .with_state(Db::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn list(State(db): State<Db>) -> Json<Vec<Todo>> {
    Json(db.todos.lock().unwrap().values().cloned().collect())
}

async fn create(State(db): State<Db>, Json(new): Json<NewTodo>) -> (StatusCode, Json<Todo>) {
    let mut next = db.next.lock().unwrap();
    *next += 1;
    let todo = Todo { id: *next, title: new.title, done: new.done };
    db.todos.lock().unwrap().insert(todo.id, todo.clone());
    (StatusCode::CREATED, Json(todo))
}

async fn show(State(db): State<Db>, Path(id): Path<u64>) -> Result<Json<Todo>, StatusCode> {
    db.todos.lock().unwrap().get(&id).cloned().map(Json).ok_or(StatusCode::NOT_FOUND)
}

async fn patch(
    State(db): State<Db>,
    Path(id): Path<u64>,
    Json(change): Json<TodoPatch>,
) -> Result<Json<Todo>, StatusCode> {
    let mut todos = db.todos.lock().unwrap();
    let todo = todos.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    if let Some(title) = change.title {
        todo.title = title;
    }
    if let Some(done) = change.done {
        todo.done = done;
    }
    Ok(Json(todo.clone()))
}

async fn delete(State(db): State<Db>, Path(id): Path<u64>) -> StatusCode {
    match db.todos.lock().unwrap().remove(&id) {
        Some(_) => StatusCode::NO_CONTENT,
        None => StatusCode::NOT_FOUND,
    }
}
```

### A.5 Go 1.22 `net/http` (https://pkg.go.dev/net/http#hdr-Patterns-ServeMux)

```go
package main

import (
	"encoding/json"
	"net/http"
	"strconv"
	"sync"
)

type Todo struct {
	ID    int    `json:"id"`
	Title string `json:"title"`
	Done  bool   `json:"done"`
}

var (
	mu    sync.Mutex
	todos = map[int]*Todo{}
	next  = 0
)

func writeJSON(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(v)
}

func main() {
	http.HandleFunc("GET /health", func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("ok"))
	})
	http.HandleFunc("GET /todos", func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		defer mu.Unlock()
		list := []*Todo{}
		for id := 1; id <= next; id++ {
			if t, ok := todos[id]; ok {
				list = append(list, t)
			}
		}
		writeJSON(w, 200, list)
	})
	http.HandleFunc("POST /todos", func(w http.ResponseWriter, r *http.Request) {
		var in struct {
			Title *string `json:"title"`
			Done  bool    `json:"done"`
		}
		if json.NewDecoder(r.Body).Decode(&in) != nil || in.Title == nil {
			http.Error(w, "bad request", 400)
			return
		}
		mu.Lock()
		defer mu.Unlock()
		next++
		t := &Todo{ID: next, Title: *in.Title, Done: in.Done}
		todos[next] = t
		writeJSON(w, 201, t)
	})
	http.HandleFunc("GET /todos/{id}", func(w http.ResponseWriter, r *http.Request) {
		id, _ := strconv.Atoi(r.PathValue("id"))
		mu.Lock()
		defer mu.Unlock()
		t, ok := todos[id]
		if !ok {
			http.NotFound(w, r)
			return
		}
		writeJSON(w, 200, t)
	})
	http.HandleFunc("PATCH /todos/{id}", func(w http.ResponseWriter, r *http.Request) {
		id, _ := strconv.Atoi(r.PathValue("id"))
		var in struct {
			Title *string `json:"title"`
			Done  *bool   `json:"done"`
		}
		if json.NewDecoder(r.Body).Decode(&in) != nil {
			http.Error(w, "bad request", 400)
			return
		}
		mu.Lock()
		defer mu.Unlock()
		t, ok := todos[id]
		if !ok {
			http.NotFound(w, r)
			return
		}
		if in.Title != nil {
			t.Title = *in.Title
		}
		if in.Done != nil {
			t.Done = *in.Done
		}
		writeJSON(w, 200, t)
	})
	http.HandleFunc("DELETE /todos/{id}", func(w http.ResponseWriter, r *http.Request) {
		id, _ := strconv.Atoi(r.PathValue("id"))
		mu.Lock()
		defer mu.Unlock()
		if _, ok := todos[id]; !ok {
			http.NotFound(w, r)
			return
		}
		delete(todos, id)
		w.WriteHeader(204)
	})
	http.ListenAndServe("127.0.0.1:8080", nil)
}
```

### A.6 L0 and L1 (sketches)

L0 differs from section 4 in its imports, three codec constants and the named-handler form of
section 7:

```ignite
import std.data.{Codec, record3, record2, field, field_or, int, text, bool, option, list, unit}
import x.web.{get, post, patch, delete, alive, dispatch, checks, found, existed, Reply, Routes}

const TODO: Codec<Todo> = record3("Todo", field("id", int(), |t| t.id), field("title", text(), |t| t.title), field("done", bool(), |t| t.done), |id, title, done| Todo { id, title, done })
const NEW: Codec<NewTodo> = record2("NewTodo", field("title", text(), |n| n.title), field_or("done", bool(), false, |n| n.done), |title, done| NewTodo { title, done })
const CHANGE: Codec<Change> = record2("Change", field("title", option(text()), |c| c.title), field("done", option(bool()), |c| c.done), |title, done| Change { title, done })

fn routes() -> Routes<Store<Todo>> = [
  alive("/health"),
  get("/todos", list_todos, unit(), list(TODO)),
  post("/todos", create, unit(), NEW, TODO, 201),
  get("/todos/:id", show, int(), TODO),
  patch("/todos/:id", change, int(), CHANGE, TODO),
  delete("/todos/:id", remove, int(), unit())
]
```

L1 replaces the three constants and every codec argument with `codec()`
(`get("/todos/:id", show, codec(), codec())`).

## Appendix B: the probe

A stand-in for the L0 core over `std.json` (no `std.data` yet), checked silently by
`ignite check` 0.13.0 and its test passed by `ignite test` on the LLVM target (the Rust target
was not run: the installed driver's substrate checkout was missing). It establishes three
facts used above: the erasure of section 5.1 type-checks and runs; a closure in a component
property is E118 (section 7); a `get` typed `requires [state.read]` refuses a writing handler
with E017 (section 6). Further findings: a record field named like a builtin method (`read`)
is shadowed by it (E041 "`Codec<P>` has no method `read`"), so `Codec`'s fields must avoid
builtin method names; a module fn named like a fn-typed field (`handle`) is chosen over the
field for `app.handle(…)` (E003, wrong number of arguments).

```ignite
type Codec<T> {
  enc: fn(value: T) -> Json
  dec: fn(value: Json) -> Option<T>
}

type Route<S> {
  method: Text
  pattern: List<Text>
  status: Int
  run: fn(state: S, params: List<Text>, body: Text) -> Response requires [state.*]
}

fn get<S, P, O>(path: Text, p: Codec<P>, out: Codec<O>, handler: fn(state: S, params: P) -> Result<O, Fail> requires [state.*]) -> Route<S> =
  Route {
    method = "GET"
    pattern = path.split("/")
    status = 200
    run = |state: S, params: List<Text>, body: Text| match param(p, params) {
      Some(value) => answer(200, out, handler(state, value))
      None => problem(not_found())
    }
  }

fn dispatch<S>(routes: List<Route<S>>, state: S, request: Request) -> Response {
  for route in routes {
    if route.method == request.method {
      match bind(route.pattern, request.path.split("/")) {
        Some(params) => {
          return route.run(state, params, request.body)
        }
        None => {}
      }
    }
  }
  problem(not_found())
}

fn show(todos: Store<Todo>, id: Int) -> Result<Todo, Fail> = todos.get(id).found()

fn todo_api(todos: Store<Todo>, request: Request) -> Response = dispatch(routes(), todos, request)
```

`post`, `answer` (204 for a `Null` output), `param`, `bind`, the codecs and the test (list,
create 201, show, 404 for an unknown and for a non-numeric id, 400 for a body without `title`)
complete it; 170 lines in all.
