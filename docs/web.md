# A web framework: `x.web`

**Status:** future design, not a slice contract. Nothing here is implemented.
**Date:** 8 November 2026
**Scope:** HTTP services and HTML pages in Ignite: what a program declares, what the compiler
and `x.web` derive from it, the language extensions that needs, and the order to build it in.

Direction (Timo, 2026-11-08): a web framework, "really nice to use, token efficient": a typical
web service in as few source tokens as possible, while the compiler still catches the errors and
the service stays explicit about effects, limits and wire formats. The framework ships with
Ignite under a second root `x` next to `std`, as `x.web` (section 3). Later the same day: the
language may be extended for it ("even jsx, etc."), and the design should be something only
Ignite can do ("or something way cooler").

This document decides. Each section states the chosen design and the alternatives it rejects.
References cite `docs/slice.md` as "slice §n", `docs/serde.md` as "serde §n",
`docs/syntax.md` as "syntax Pn", a file and line otherwise. Every Ignite block in this
document is a sketch of the target language unless it says it was checked; the site check
(`site::`, `crates/ignite/tests/site.rs`) covers `site/book` only, not this file.

## 1 The idea in one page

A web application in Ignite is part of the system declaration. The program declares its
resources and what it exposes; the compiler and `x.web` derive the routes, the JSON, the
OpenAPI document, the HTML forms' checks, the caching, the live updates and the health probe
with its contract. The program writes only the exceptions.

```ignite
package todo

import x.web.{Web, Html, collection, page}

type Todo { id: Int, title: Text, done: Bool = false }

fn board(todos: Store<Todo>) -> Html =
  <main>
    <h1>Todos</h1>
    <ul>{todos.values().map(|t| <li class={t.done.then("done")}>{t.title}</li>)}</ul>
    <form action="/todos" method="post">
      <input name="title" required/>
      <button>Add</button>
    </form>
  </main>

system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource todos: Store<Todo>

  component server: Web {
    listener = ingress
    routes = {
      "/todos" = collection(todos)
      "/" = page(board)
    }
    availability = 99%
    executable = "todod"
  }
}
```

What this program has, with no further line:

| derived | from | why only Ignite can do it |
|---|---|---|
| `GET`, `POST /todos`, `GET`, `PATCH`, `DELETE /todos/{id}`, 404, 405 with `Allow`, 400 naming the member | `collection(todos)` and the type `Todo` | the codec and the shapes `New<Todo>`, `Patch<Todo>` are derived from the type at compile time |
| an `ETag` on every GET and 304 on a match | the read set of each handler | a handler can read only what its parameters hold (no ambient effect, slice §21.3), so its answer is a function of the request and the versions of the stores it reads |
| live updates of `/` in every open browser after any write to `todos` | `board` reads `todos` (its effect set) | the compiler knows the effect set of `board`; no subscription is written |
| a compile error for `<input name="titel">` or `action="/todo"` | the form against the route table | the route table and the page are both values the evaluator sees at check time |
| a health route, a probe member `server.health` and an `Availability` contract `server.reachable` in the system model | `availability = 99%` | the application is a system member: `ignite plan`, `run` and `apply` operate the probe and evaluate the contract |
| an OpenAPI 3.1 document | the route table | `ignite explain TodoService.server.openapi --raw` prints it at check time, with no build |

The same service with every handler written by hand (section 6.4) is 230 tokens, the declared
form 89 (section 2). FastAPI's is 296.

## 2 Measured

### 2.1 Method

- Tokenizer: `cl100k_base` of `tiktoken` 0.14.0. A second column uses the regular-expression
  tokenizer of syntax §2.1 (a literal is one token), extended with Python, TypeScript, Go and
  Elixir string and operator forms. The two agree on every ranking below.
- What is counted: every line of code, imports included. Blank lines, comment lines and
  leading indentation are not counted.
- The reference service is the in-memory part of `examples/todo/todo.ignite`: `GET /health`,
  `GET /todos`, `POST /todos` with `{"title", "done"?}` answering 201, `GET`, `PATCH` (either
  field optional) and `DELETE` (204) on `/todos/{id}`, 404 for an unknown id, 400 for a body
  that does not decode.
- Ignite today: the reference service written with `std.protocol.http` and `std.json` in the
  idiom of `examples/todo/todo.ignite`, checked silently by `ignite check` 0.13.0. The other
  programs are written from their documentation's idioms and were not run; the Ignite target
  forms are sketches of the language section 5 proposes. All are in appendix A.
- Every framework has libraries that generate CRUD routes; the comparison counts each
  framework's documented core idiom.

### 2.2 The JSON service

| | Ignite today | FastAPI | Hono | Axum | Go `net/http` | `x.web`, handlers by hand (6.4) | `x.web`, declared (1) |
|---|---|---|---|---|---|---|---|
| tokens (cl100k) | 592 | 296 | 348 | 563 | 620 | **230** | **89** |
| tokens (regex) | 637 | 315 | 394 | 707 | 653 | 232 | 75 |
| lines | 68 | 44 | 38 | 77 | 102 | 20 | 12 |

Per endpoint (cl100k), the handlers-by-hand form against the median of the four frameworks:
health 12 against 13, list 12 against 31.5, create 25 against 76, show 18 against 55, patch 32
against 99, delete 18 against 45, setup 113 against 108 (the system declaration, which the
others leave to a command line). The declared form has no per-endpoint lines: 12 tokens for the
route, 77 for the module, the type and the system.

Exceptions cost what they say: a validation rule on create (`check = titled`) is 44 tokens and
a custom route `POST /todos/{id}/done` 36 (appendix A.8: 173 in all).

### 2.3 The live page

| | `x.web` (section 1, the API included) | Phoenix LiveView (the LiveView module alone) |
|---|---|---|
| tokens (cl100k) | 176 (page 83) | 214 |
| lines | 24 | 30 |

The LiveView count leaves out the context module, the router line and the PubSub setup it
needs; the Ignite count is the whole program.

### 2.4 The real todo service

`examples/todo/todo.ignite` without its tests is 237 lines and 1883 tokens. In `x.web` with a
durable store (section 6.6), its health probe, contract and assembly: 24 lines, 179 tokens
(appendix A.9). The persistence (todo.ignite:76-194, 651 tokens) does not vanish: it moves into
std as `Durable<T>`, written once for every service.

## 3 Packaging and naming

Decided by Timo (2026-11-08): the framework is a layer above std, ships and is versioned with
the compiler, and lives in this repository under a second root `x` beside `std`, with the
prefix `x.web`.

| question | decision |
|---|---|
| where the files live | `x/web.ignite` is the module `x.web`; `x/web/html.ignite` (markup, vocabulary), `x/web/live.ignite` (the live runtime and its fixed browser script), `x/web/openapi.ignite`, `x/web/client.ignite`. The root `x/` sits beside `std/` |
| how a program imports it | `import x.web.{Web, collection, page}`, as a std module. Imports are not re-exported (slice §4.1), so each submodule is imported by its own path |
| how the loader finds it | as std: module `x.a.b` is `<x root>/a/b.ignite`, its `package` equal to its module path (E031 otherwise), loaded only when reached (slice §16.22). The x root is `--x PATH`, default `share/ignite/x` beside the driver's `bin/`, else the repository's `x/` next to `std/`, mirroring `--std` (`crates/ignite/src/main.rs:24`, `std_root` in `crates/ignite/src/lib.rs:639`). Today an import of a module that is neither `std.*` nor a sibling is E031 (slice §4.1, §25.6; `parse_siblings`, `crates/ignite/src/lib.rs:1035`) |
| release archive and toolchain image | `task dist` copies `x` to `share/ignite/x` next to `share/ignite/std` (`Taskfile.yml:102-104`); the toolchain image carries both |
| layering | a new row above every std layer in `docs/layering.md` and `crates/ignite-check/tests/layering.rs`: `x` (every `x.*`) may import every std layer and `x`; no `std.*` module imports `x.*`. Rule 3 (no protocol names outside `std.protocol.*`) does not apply to `x`. Rule 5 stays: the compiler crates name no protocol, so every language extension of section 5 is generic (markup literals call the provider in scope, annotations are values, the evaluator reads effect sets of any fn) |
| rules `x` follows | std's: inline `test` blocks on both targets in `task test-ignite`, which gains `x/`; every module checks silently; no hand-written Rust, except the fixed browser script of `x.web.live`, which is a static asset, not compiled program code |
| versions | `x.*` has the compiler's version |

Rejected: a repository of its own like `ignite-lang/ignite-lsp` (an application built against a
released `ignite`; a library there needs a package mechanism that does not exist, vision §5 wave
H); `std.web` (the framework is above std); `web.*` (takes a short name a program may want);
`ignite.web.*` (two more tokens per import for nothing); vendored sibling modules (a sibling
needs the program's package as prefix, `docs/friction.md` row 59).

## 4 Concepts considered

| concept | what it is | verdict |
|---|---|---|
| **A, chosen: the web app is a system declaration** | resources and intent declared; routes, JSON, OpenAPI, forms, caching, live updates, probe and contract derived; exceptions written | it uses what only Ignite has: a system model the evaluator builds at check time, effect sets the compiler computes, contracts that `ignite run` evaluates, handlers with no ambient authority. It is also the shortest (89 tokens) |
| B: one program for server and browser | components compiled to native for the server and to WebAssembly for the browser (`docs/wasm.md`), server state streamed as typed diffs, no API between them | needs the wasm target (wave E, not implemented) and a browser host for it. A keeps the path open: a `page` view is a pure function of the capabilities it reads, which is what a client component needs; B is a later phase of A (section 9) |
| C: typed actors per session or socket | a session is an actor (`docs/actors.md`) holding its view state, messages from the browser are typed | needs wave C. A's live views push from the server over a stream (slice §19.23) and need no per-session state; bidirectional live components become actors when wave C lands |
| D: routes as annotated functions only | `@get("/todos/{id}") fn show(id: Int) = todos.get(id)?`, the conventional framework with better syntax | kept as A's exception form (section 6.4): 230 tokens, below every framework measured, but it derives nothing and checks only what the handler says |

Effects as the cache key and contracts on routes, two further seeds, are not separate concepts:
both are consequences of A (sections 6.3 and 6.5).

## 5 Language extensions

Each extension is generic: none names HTTP or HTML in a compiler crate (layering rule 5). The
saving is measured on the reference service (appendix A) unless marked; a cumulative step is
measured, a split inside a step is by ablation.

| # | extension | what it is | saving | compiler change | proposed before |
|---|---|---|---|---|---|
| X1 | derived codecs | `codec<T>()` written by the frontend; at an HTTP boundary a record is JSON with no codec named | −195 (639 → 444) | one frontend pass (`ignite-types` or the mid-level IR) | serde §3.2, step 5 |
| X2 | mapped record types | `New<T>` (T without the fields a store assigns and with defaulted fields optional) and `Patch<T>` (every field optional), `t.with(p)` applies a patch | −32 by ablation; −141 with X9 (230 → 89) | types: a record type computed from another; derived codec and schema | here |
| X3 | values the model can hold | closures and fn values in component properties encoded as the creating fn and its captures, so a route table is a property | −51 (444 → 393) | `crates/ignite-eval` model encoding (`Value::Closure`, `crates/ignite-eval/src/value.rs:188`; today E118) | here |
| X4 | handlers that answer | a route function's result is inferred, its tail is the success value, `?` on `None` is 404 and on `Err(f)` is the `Fail` `f` | −41 by ablation | types: inferred results for fns a route names; `?` on `Option` through the error type's declared `none()` (no implicit conversion: the conversion is a named associated fn) | syntax P3 (`?` on `Option`) |
| X5 | `given` | `given todos: Store<Todo>` makes `todos` a leading parameter of every fn of the module that uses it, as Lean's `variable` | −33 (263 → 230) | check, types: one desugaring before effect inference, so effects stay on a parameter | here |
| X6 | parameter defaults | syntax P10's defaults, a default whose type mentions a type parameter typed at the call site (`codec: Codec<P> = codec()`) | −31 (393 → 362) | syntax, check, types | syntax P10 (defaults unscheduled; slice §14.2 took named arguments) |
| X7 | `Store.insert` and update from another record | `todos.insert(\|id\| Todo { id, ..new })`, a record update whose base is another record type with the same field names | −18 (362 → 344) | a `Store` builtin; types | here |
| X8 | pattern literals and route annotations | a text literal where a `Pattern` is expected keeps `{id}` as a parameter, not an interpolation (today `"/todos/{id}"` is E001, appendix B); `@get("/todos/{id}") fn show(id: Int)` binds parameters by name, a name the function does not take is an error | −8; the gain is checked names | syntax (annotations: `@expr` before a fn, a value the evaluator reads), types (pattern literals) | here |
| X9 | derived members | a kind's hook returns members the evaluator adds to the model (`server.health`, `server.reachable`), shown by `plan` and `explain` as derived | with X2 −141; for the durable todo the health component and contract, 99 tokens, become `availability = 99%` | `crates/ignite-eval` (as `std.control.companions` does for machines, slice §16.12) | here |
| X10 | markup literals | `<li class={c}>{t.title}</li>` calls the markup provider in scope (`x.web.html`); text is escaped; a capitalised tag is a function (`<Card title="x"/>` calls `Card`); element and attribute names checked against the provider's vocabulary at check time; facts such as a form's `action`, `method` and input names recorded for the evaluator | the live page is 83 tokens against LiveView's 214 for its module; checked forms | syntax (a markup expression), check (the provider's pure check run by the evaluator per literal), types | here |
| X11 | effect sets as values | the evaluator reads a fn's inferred read and write sets, mapped to the resources its parameters are bound to | the subscription code a live page needs elsewhere (LiveView: subscribe, broadcast, `handle_info`, about 70 tokens); automatic `ETag` | `crates/ignite-eval`, `ignite-check`'s effect table exposed (`Symbols.effects`) | here |
| X12 | durable store | `Durable<T>`, a `Store<T>` over a `Directory`, written in std from todo.ignite's persistence | −1300 in the durable todo (moved, not removed) | none in the compiler; `std` and a resource kind | here |

The numbers in parentheses are the reference service with the extensions added one at a time:
639 tokens for the std-only first form (the last paragraph of this section), 444 with X1, 393
with X3, 362 with X6, 344 with X7, 263 in the annotated form of section 6.4 without `given`,
230 with X5, 89 declared (X2 with X9). The intermediate programs are not reproduced here.

Considered and not taken:

| candidate | why not |
|---|---|
| SQL-ish query literals | Ignite has no database substrate; `Store` and `Durable` are the data layer. Revisit with a database resource |
| text templates (`html"…"`) | markup literals give the same brevity with names checked and escaping by construction |
| URL literals as a type of their own | pattern literals (X8) cover the routes; outbound URLs are not in scope |
| route blocks as a keyword (`api { get … }`) | annotations and the route map do the same without a protocol-specific keyword in the compiler |

What ships first without them: `x.web` on today's language, a route table of hand-built
`Route` values with combinator codecs and a named handler (639 tokens for the reference
service, every check of section 6.2 included), then X1.

## 6 The design

### 6.1 The `Web` kind

```ignite
type Web {
  listener: TcpListener
  routes: Map<Pattern, Route>
  executable: Text
  availability: Option<Ratio> = None     // derives server.health and server.reachable
  latency: Map<Pattern, Duration> = {}   // derives one latency probe and contract per route
  limits: HttpLimits = HttpLimits {}
  shape: Shape = Shape.default()         // wire names (serde §5.1)
  recovery: Restart = Restart {}
  rollout: DrainRollout = DrainRollout {}
}
```

`Web.main` runs `Http.serve` of `std.protocol.http` with an `HttpApp` built from the routes, so
everything below Http (framing, limits, RFC 9112 claims) is unchanged. Its `machine` and
`validate` hooks are `HttpServer`'s (slice §16.16); its `members` hook (X9) adds the probe and
contracts. `ignite plan`, `run` and `apply` see an ordinary executable member and its derived
members.

### 6.2 Routes and their checks

| route | answers |
|---|---|
| `collection(store)` | `GET` list, `POST` create from `New<T>` (201), `GET`, `PATCH` with `Patch<New<T>>`, `DELETE` (204) on `/{id}`; `check = f` runs `f` on every create and patch |
| `page(view)` | `GET` HTML from `view`, live (6.3) |
| `get(f)`, `post(f)`, `put(f)`, `patch(f)`, `delete(f)` | the function `f` (6.4) |
| `files(directory)` | `GET` and `HEAD` of the files under a `Directory`; `..`, absolute paths and symbolic links refused by the directory itself (slice §21.4) |
| `stream(start, next)` | a streamed body (slice §19.23), server-sent events with `sse_event` |
| `scope(layers, routes)` | the routes under the pattern, with layers (6.7) |

Checked at `ignite check`, each an E120 through `validate`:

- a pattern declared twice, a pattern an earlier one hides, a malformed pattern;
- a pattern parameter the handler does not take, or a handler parameter that is neither a
  pattern parameter, a body, a query field nor a declared header;
- a form in a page whose `action` and `method` name no route, or whose input names are not
  fields of that route's input type (from X10's facts);
- a `GET` handler that writes (its type: `requires [state.read]`; probe, appendix B, gives E017
  for a write).

A body that does not decode is 400 with the member (`$.title`) and what was expected, before the
handler runs; an unknown route 404; a known path with another method 405 with `Allow` from the
table; a body over `limits.body` 413.

### 6.3 Effects as the cache key and the live view

A handler and a view can read only what their parameters hold: an ambient effect in a handler is
E017 today (slice §21.3, fixture `e017_ambient_in_handler`). So the answer to a GET is a
function of the request and the state of the capabilities it reads. X11 gives the read set of
every route function to `x.web`, mapped to resources: `board` reads `todos`.

- **ETag.** Every `Store` keeps a version that each write raises (`Store.version()`, a substrate
  addition). A GET whose read set holds only stores answers `ETag: "<hash of the versions>"` and
  304 for a matching `If-None-Match`. A read set holding a `Directory` or a `Launcher` gets no
  ETag, since their state has no version.
- **Live.** `page(view)` sends the HTML and a fixed script of `x.web.live` (a static asset,
  under 2 KB, no build step), which opens one event stream per page. When a write to a store in
  the view's read set completes, from any route or task, the server renders the view again and
  sends it on the stream; the script replaces the changed elements. No subscription and no
  broadcast is written, and none can be forgotten: the read set is computed, not declared.
- **Cost.** One render per open page per write to a store it reads; a `live = false` on `page`
  turns it off. Writes in one request are coalesced to one render.

Rejected: declared dependencies (`page(board, watches = [todos])`): a declared list can be wrong;
the inferred one cannot. Rejected: client-side state and a JSON API for the page (the React
shape): two programs and an API between them; concept B covers the client side later.

### 6.4 Handlers by hand

The exception form, and the whole form for services that are not collections:

```ignite
package bench

import x.web.{get, post, patch, delete, serve}

type Todo { id: Int, title: Text, done: Bool = false }
type NewTodo { title: Text, done: Bool = false }

given todos: Store<Todo>

@get("/health") fn health() = "ok"
@get("/todos") fn list() = todos.values()
@post("/todos") fn create(new: NewTodo) = todos.insert(|id| Todo { id, ..new })
@get("/todos/{id}") fn show(id: Int) = todos.get(id)?
@patch("/todos/{id}") fn change(id: Int, c: Patch<NewTodo>) = todos.update(id, |t| t.with(c))?
@delete("/todos/{id}") fn remove(id: Int) = todos.remove(id)?

system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource todos: Store<Todo>

  component server: HttpServer {
    listener = ingress
    application = serve(todos, module())
    executable = "todod"
  }
}
```

- Parameters bind by name: a pattern parameter, then the body (one parameter of a record type
  on `post`, `put`, `patch`), then query fields; a header is read only when the annotation lists
  it (`@get("/todos", headers = ["if-none-match"])`), so a query parameter can never stand in
  for a header.
- Results (X4): the tail is the success value with the method's status (POST 201, a `Unit`
  result 204, else 200); `status = 202` on the annotation overrides it; `?` on `None` answers
  404, on `Err(fail)` the problem `fail`.
- `serve(state, module())` collects the module's annotated functions; `module()` is the
  existing module record of slice §14.4 extended to annotations (X8).
- Effects (X5): `given` is a leading parameter, so `show` needs `todos.read` on it exactly as a
  written parameter would; `get` takes `requires [state.read]` handlers.

### 6.5 Contracts on routes

`availability = 99%` derives the member `server.health` (an HTTP probe of `x.web`, the shape of
`examples/todo/health.ignite`), the route `/health`, and the contract `server.reachable:
Availability` over the probe's checks, rolling 5 minutes. `latency = { "/todos" = 200 ms }`
derives one probe per route and a contract that the route's p99 stays under the bound. They are
members of the model like written ones: `ignite plan` lists them with `derived by
TodoService.server`, `ignite run` schedules the probes and reports standings, an assembly binds
them (`TodoService.server.health = scheduledProbeHost()`). A contract declared by hand replaces
the derived one of the same name.

### 6.6 Durable data

`resource todos: Durable<Todo> { directory = data }` is a `Store<Todo>` whose every change is
written as todo.ignite writes it today (one file per version, tombstones, sharded directories,
load on first use; slice §22.4) by `std` code generalised from todo.ignite:76-194 with derived
codecs. `collection` takes either. The known limit of slice §22.2 (two processes over one
directory during a drain) stays the same limit.

### 6.7 Errors and layers

- Every error is `application/problem+json` (RFC 9457): `type`, `title`, `status`, `detail`, and
  `invalid-params` for a body that does not decode. `fail(status, detail)`, `not_found()`,
  `bad_request(detail)`, `conflict(detail)` build one.
- A layer is a record of two functions with the state, `before` and `after`, so it has the
  handlers' authority and no more: `bearer(valid)`, `max_body(16 KiB)`, `headers([...])`,
  `before(f)`. Access logging is a property of the server (`access_log = true`), because
  standard error is an ambient effect no handler holds.

### 6.8 OpenAPI, the typed client, tests without a socket

- `ignite explain FILE TodoService.server.openapi --raw` prints the OpenAPI 3.1 document of the
  route table at check time (`--raw`: a `Text` property verbatim, a generic CLI addition).
- `client(TodoService.server, Pipe.pair())` in a test serves the component's application over an
  in-memory pipe through the real `Http.serve`; `let made: Todo = api.post("/todos", New<Todo> {
  title = "milk" })?` sends a typed body and decodes a typed answer.
- An HTML page is tested the same way: `api.get("/").body` is the rendered page.

### 6.9 Effects and capabilities, in short

No new effect rule. A route function's effects are inferred on its parameters (a `given`
included); capabilities come only from resources (slice §21.3); a `GET` route cannot write; a
handler cannot reach the clock or standard output (E017). There is no wall-clock capability
yet; `std-clock-capability` (section 8) adds one as a resource.

## 7 Fit with the system model

- `Web` is a component kind like `HttpServer`: an executable member with `machine`, `validate`
  and (X9) `members`. `ignite plan` shows the routes, the derived members and the OpenAPI
  document; `ignite run` supervises, rolls out and probes it; `ignite apply` replaces it.
- Today a closure in a component property stops the model (probe: E118, "a closure cannot be
  encoded in the model"); X3 lifts that, and until then the route table is built by a named
  function the property calls.
- No backend change for the framework itself; the language extensions each say whether they
  touch a backend (X2, X4, X7, X10 lower to ordinary code; only `Store.insert` and
  `Store.version` are substrate builtins and move `CODEGEN_REVISION`).

## 8 Delivery

Ordered by saving; each language story is its own slice contract section. `x-root` comes first
because nothing in `x/` loads without it.

| step | story (draft) | contents | needs | shared files it touches |
|---|---|---|---|---|
| 1 | `x-root` | the `x` root: `--x`, the default beside `std`, reachable loading, `task dist`, `task test-ignite` runs `x/`, the layering row | | `crates/ignite/src/lib.rs`, `crates/ignite/src/main.rs`, `Taskfile.yml`, `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `docs/slice.md`, `AGENTS.md` |
| 2 | `web-core` | `x/web.ignite` on today's language: routes, patterns, problem+json, 404/405/400/413, layers, `HttpApp.checks` reported by `validate` | 1, `data-codec-combinators` | `x/web.ignite`, `std/protocol/http.ignite`, `docs/slice.md` |
| 3 | `data-derived-codec` (X1, −195) | serde §8 step 5 | `data-codec-combinators` | `crates/ignite-types`, `std/data.ignite`, `docs/slice.md` |
| 4 | `durable-store` (X12, −1300 moved) | `Durable<T>` in std | 3 | `std/data.ignite` or `std/durable.ignite`, `docs/slice.md` |
| 5 | `mapped-record-types` (X2) and `derived-members` (X9), −141 together | `New<T>`, `Patch<T>`, `with`; the `members` hook; `collection`, the `Web` kind, derived probe and contracts | 2, 3 | `crates/ignite-types`, `crates/ignite-eval`, `x/web.ignite`, `docs/slice.md` |
| 6 | `markup-literals` (X10) | the markup expression, the provider check, `x/web/html.ignite` with its vocabulary, form facts | 1 | `crates/ignite-syntax`, `crates/ignite-check`, `crates/ignite-types`, `crates/ignite-eval`, `x/web/html.ignite`, `docs/slice.md` |
| 7 | `eval-closure-values` (X3, −51) | closures in the model | | `crates/ignite-eval`, `docs/slice.md` |
| 8 | `answering-handlers` (X4, −41) | inferred results for route functions, `?` on `Option` through `none()` | | `crates/ignite-types`, `docs/slice.md`, `docs/syntax.md` |
| 9 | `given` (X5, −33) | module-level capability parameters | | `crates/ignite-syntax`, `crates/ignite-check`, `crates/ignite-types`, `docs/slice.md` |
| 10 | `param-defaults` (X6, −31) | syntax P10 with call-site typing | 3 | `crates/ignite-syntax`, `crates/ignite-check`, `crates/ignite-types`, `docs/slice.md`, `docs/syntax.md` |
| 11 | `store-insert-and-update-from` (X7, −18) | `Store.insert`, update from another record | | substrate, both backends, `crates/ignite-types`, `docs/slice.md` |
| 12 | `annotations-and-pattern-literals` (X8, −8, checked names) | `@expr` on fns, `Pattern` literals, `module()` with annotations | 1 | `crates/ignite-syntax`, `crates/ignite-types`, `crates/ignite-eval`, `docs/slice.md` |
| 13 | `effect-sets-in-the-evaluator` (X11) and `web-live` | read sets as values, `Store.version`, ETag, `page` live updates, the browser script | 5, 6 | `crates/ignite-eval`, `crates/ignite-check`, substrate, `x/web/live.ignite`, `docs/slice.md` |
| 14 | `web-openapi`, `web-client`, `explain-raw` | OpenAPI, the test client, `--raw` | 5 | `x/web/openapi.ignite`, `x/web/client.ignite`, `crates/ignite/src/main.rs`, `crates/ignite-eval/src/explain.rs` |
| 15 | `todo-on-web` | `examples/todo` in the declared form, its tests on `client` | 4, 5, 13, 14 | `examples/todo/`, `crates/ignite/tests/` |
| — | `std-clock-capability`, `http-access-log` | a wall-clock resource; access logging on the server | | `std/`, substrate; `std/protocol/http.ignite` |

Steps 6 to 12 touch disjoint parts of the frontend but share `crates/ignite-types` and
`docs/slice.md`; a wave takes at most two of them. Concept B (wasm client components) and C
(session actors) follow waves E and C and are not in this table.

## 9 Later

- Client components (concept B): a `page` view compiled to WebAssembly runs in the browser and
  receives typed diffs of the stores it reads instead of HTML; the server side stays.
- Live components with input (concept C): a session actor per open page once actors land.
- Hot reload: `ignite run --watch` (`docs/wasm.md`) replaces the `Web` member on a change, live
  pages re-render.

## 10 Open questions

| id | question | default if not answered |
|---|---|---|
| Q1 | which field a store assigns, for `New<T>` | a field `id: Int`; else `New<T>` is `T` |
| Q2 | a body that does not decode: 400 or 422 | 400, as todo today (todo.ignite:235, 237) |
| Q3 | inferred results (X4) only for fns a route names, or every fn with an `=` body | only route fns: a fn's type is API (syntax §5 rejects inferred const types for that reason) |
| Q4 | the markup vocabulary: the whole HTML Living Standard or a subset | elements and global attributes of the standard, data from a vendored source as `docs/rfc/` |
| Q5 | live update granularity: whole view or changed elements | the whole view rendered, changed elements replaced in the browser |
| Q6 | ETag for a `Durable` store across processes | the version of the newest file; none during a rollout's drain |
| Q7 | `given` across modules | a `given` is local to its module; another module passes it as a parameter |
| Q8 | `x.web` versioned apart from the compiler | no |

## Appendix A: the measured programs

Each is counted by section 2.1's rules. A.1 is checked; the comparisons are not run; A.7 to A.9
are sketches of the target language.

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

### A.6 Phoenix LiveView (https://hexdocs.pm/phoenix_live_view/Phoenix.LiveView.html, the PubSub idiom of the Phoenix guides)

```elixir
defmodule TodoWeb.BoardLive do
  use TodoWeb, :live_view
  alias Todo.Todos

  def mount(_params, _session, socket) do
    if connected?(socket), do: Phoenix.PubSub.subscribe(Todo.PubSub, "todos")
    {:ok, assign(socket, todos: Todos.list())}
  end

  def handle_event("add", %{"title" => title}, socket) do
    {:ok, _} = Todos.create(%{title: title})
    Phoenix.PubSub.broadcast(Todo.PubSub, "todos", :changed)
    {:noreply, socket}
  end

  def handle_info(:changed, socket) do
    {:noreply, assign(socket, todos: Todos.list())}
  end

  def render(assigns) do
    ~H"""
    <main>
      <h1>Todos</h1>
      <ul>
        <li :for={t <- @todos} class={t.done && "done"}>{t.title}</li>
      </ul>
      <form phx-submit="add">
        <input name="title" required />
        <button>Add</button>
      </form>
    </main>
    """
  end
end
```

### A.7 Handlers by hand

Section 6.4, 230 tokens.

### A.8 Declared, with two exceptions

```ignite
package bench

import x.web.{Web, collection, post, bad_request}

type Todo { id: Int, title: Text, done: Bool = false }

given todos: Store<Todo>

fn titled(new: New<Todo>) = if new.title.is_empty() { Err(bad_request("title is empty")) } else { Ok(()) }
fn complete(id: Int) = todos.update(id, |t| Todo { done = true, ..t })?

system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource todos: Store<Todo>

  component server: Web {
    listener = ingress
    routes = {
      "/todos" = collection(todos, check = titled)
      "/todos/{id}/done" = post(complete)
    }
    executable = "todod"
  }
}
```

### A.9 The durable todo service, declared

```ignite
package examples.todo

import std.data.Durable
import std.deploy.{Deployment, localSocket, nativeProcess, scheduledProbeHost}
import x.web.{Web, collection}

type Todo { id: Int, title: Text, done: Bool = false }

system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource data: Directory { path = "todo-data", create = true }
  resource todos: Durable<Todo> { directory = data }

  component server: Web {
    listener = ingress
    routes = { "/todos" = collection(todos) }
    availability = 99%
    executable = "todod"
  }
}

assembly local: Deployment {
  system = TodoService
  bindings = {
    TodoService.ingress = localSocket()
    TodoService.server = nativeProcess(includes = [])
    TodoService.server.health = scheduledProbeHost()
  }
}
```

## Appendix B: the probe

A stand-in for the first, std-only `x.web` (section 5, last paragraph) over `std.json` (no `std.data` yet), checked silently by
`ignite check` 0.13.0 and its test passed by `ignite test` on the LLVM target (the Rust target
was not run: the installed driver's substrate checkout was missing). It establishes four
facts used above: a route table whose routes erase their types behind closures type-checks and runs; a closure in a component
property is E118 (section 7); a `get` typed `requires [state.read]` refuses a writing handler
with E017 (section 6.2); `"/todos/{id}"` in a text literal is E001 "unclosed `{` in interpolation" (X8). Further findings: a record field named like a builtin method (`read`)
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
