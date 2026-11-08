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
Ignite can do ("or something way cooler"). Third round, the same day: authentication and
authorisation, what replaces a middleware chain, outgoing calls with an HTTP client and the
dependencies they make, and a markup form cheaper than JSX with Markdown as first-class
content (sections 6.10 to 6.13, measured in 2.5 to 2.8).

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

import x.web.{Web, collection, page}

type Todo { id: Int, title: Text, done: Bool = false }

view board(todos: Store<Todo>) =
  main
    h1 Todos
    ul
      for t in todos.values()
        li(class = t.done.then("done")) {t.title}
    form("/todos", post)
      input("title", required)
      button Add

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
| a compile error for `input("titel")` or `form("/todo", post)` | the form against the route table | the route table and the page are both values the evaluator sees at check time |
| a health route, a probe member `server.health` and an `Availability` contract `server.reachable` in the system model | `availability = 99%` | the application is a system member: `ignite plan`, `run` and `apply` operate the probe and evaluate the contract |
| an OpenAPI 3.1 document | the route table | `ignite explain TodoService.server.openapi --raw` prints it at check time, with no build |

The same service with every handler written by hand (section 6.4) is 230 tokens, the declared
form 89 (section 2). FastAPI's is 296.

Who may call a route, the policies around every request and the services a handler calls are
declared the same way, as properties and resources the checker sees (sections 6.10 to 6.12):

```ignite
type User { sub: Int, name: Text, roles: List<Text> = [] }

@get("/me") fn me(user: User) = user

system TodoService {
  resource jwt_secret: Secret
  resource weather_api: Service { base = "https://api.weather.example/v1", timeout = 2 seconds }

  component server: Web {
    identity = jwt<User>(jwt_secret)
    cors = ["https://app.example.org"]
    limit = per_client(100, 1 minute)
    timeout = 5 seconds
    // listener, routes, executable as above
  }
}
```

`me` cannot be reached without a valid token: its parameter `user: User` has one source, the
identity provider, and a route whose parameter has no source is a check error.
`ignite plan` shows `weather_api` as a dependency of `server`, because a route's effect set
names it.

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
| tokens (cl100k) | 150 (page 57) | 214 |
| lines | 25 | 30 |

The second round wrote the page in JSX-style markup: 176 tokens, the page 83. The view form of
section 6.13 replaced it (2.8).

The LiveView count leaves out the context module, the router line and the PubSub setup it
needs; the Ignite count is the whole program.

### 2.4 The real todo service

`examples/todo/todo.ignite` without its tests is 237 lines and 1883 tokens. In `x.web` with a
durable store (section 6.6), its health probe, contract and assembly: 24 lines, 179 tokens
(appendix A.9). The persistence (todo.ignite:76-194, 651 tokens) does not vanish: it moves into
std as `Durable<T>`, written once for every service.

The third round measures four more topics the same way (method 2.1), each against FastAPI,
Hono, Axum and Go and the closest specialised tool for the topic. The programs are in appendix
A.10 to A.13.

### 2.5 Authentication and roles

The task: bearer tokens signed with HS256, a user type from the claims, `GET /me` for any
signed-in user, `DELETE /todos/{id}` for users with the role `admin` (401 without a valid
token, 403 without the role, 404 for an unknown id). The specialised tool is Hono's
`hono/jwt` middleware.

| | `x.web` (6.10) | `x.web` without X13 | Hono + `hono/jwt` | FastAPI + PyJWT | Go + `golang-jwt` | Axum + `jsonwebtoken` |
|---|---|---|---|---|---|---|
| tokens (cl100k) | **122** | 142 | 217 | 237 | 369 | 398 |
| tokens (regex) | 135 | 160 | 215 | 245 | 403 | 445 |
| lines | 11 | 8 | 16 | 30 | 54 | 52 |

Without X13 each handler takes the request and calls the check itself, so a handler that
forgets the call is public. With X13 a handler that uses a `User` cannot run without one, and a handler that takes none is listed as public by `ignite explain` (6.10).

### 2.6 Policies around every request

The task: a request id, an access log, CORS for one origin with credentials, a 5-second
timeout, 100 requests per minute per client, compression, every error as problem+json. The
specialised tool is Hono's built-in middleware (`hono-rate-limiter` for the limit); for Go the
`chi` middleware set, for Axum `tower-http` and `tower_governor`, for FastAPI Starlette's
middleware and `slowapi`.

| | `x.web` (6.11) | Hono | Go + `chi` | Axum + `tower-http` | FastAPI + `slowapi` |
|---|---|---|---|---|---|
| tokens (cl100k) | **40** | 231 | 259 | 316 | 351 |
| lines | 6 | 24 | 32 | 37 | 38 |

`x.web` writes three of the seven: the request id, the access log, compression and problem+json
are defaults of the `Web` kind. The order is not written in any `x.web` program; in the four
others it is the order of the `use` or `layer` lines, and a wrong order compiles (6.11).

### 2.7 Outgoing calls

The task: `GET /weather/{city}` answers the decoded result of a third-party JSON API, with a
2-second timeout, three attempts with exponential backoff, a circuit breaker that opens after
five failures for 30 seconds, 502 when the call fails, and a test with the API replaced by a
fake. The specialised tools: `httpx` with `tenacity` and `pybreaker` (Python), `fetch` with
`zod`, `p-retry` and `opossum` (TypeScript), `gobreaker` and `backoff` (Go),
`reqwest-middleware`, `reqwest-retry` and `failsafe` (Rust).

| | `x.web` (6.12) | FastAPI + httpx | Hono + fetch | Go | Axum + reqwest |
|---|---|---|---|---|---|
| tokens (cl100k), with the test | **154** | 276 | 319 | not written | not written |
| tokens (cl100k), without the test | **88** | 205 | 238 | 324 | 349 |
| lines, without the test | 9 | 23 | 24 | 46 | 41 |

Calling another Ignite service (6.12): one call with a typed body, in `x.web` with a client
derived from the callee's routes 32 tokens, with Hono's RPC client `hc<AppType>` 72. Hono's
client is typed from the server's TypeScript type; the `x.web` client is also a dependency
edge in the system model and checked against the callee at `ignite plan --for-apply` (6.12).

### 2.8 Markup

Four candidate forms in Ignite, measured on the page of section 1 (the view alone, its
signature included) and on a prose page of the same service (a help page with a title and
a date as data, three sections, three links to routes, a list, emphasis), against JSX, Phoenix
HEEx, Go templ, Pug and Markdown:

| form | live page | prose page |
|---|---|---|
| JSX-style markup literals (round 2, X10 then) | 77 | 197 |
| indentation views, positional attributes (**chosen**, 6.13) | **48** | 146 |
| indentation views, named attributes only | 55 | |
| indentation views with conditional classes (`li.done(t.done)`) | 44 | |
| elements as positional functions (`li(class = c, t.title)`) | 58 | 168 |
| Markdown with typed holes (`md"""…"""`) | 60 | **116** |
| Phoenix HEEx | 77 | 196 |
| Go templ | 78 | |
| Pug (a template file, no signature, untyped) | 40 | 139 |

The view form is the cheapest form that keeps every name checked; Markdown is cheapest for
prose and poor for forms. Both are taken: views for pages, Markdown files for content
(6.13).

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
| X10 | views (revised in round 3) | `view board(todos: Store<Todo>) =` followed by an indented block of element lines (`li(class = c) {t.title}`), `for`, `if`, `match` and calls of other views; an element is a function of the markup provider in scope (`x.web.html`: `fn form(action: Route, method: Method = get, ...)`), so its attributes are typed parameters, positional or named; text is escaped; facts such as a form's route and input names recorded for the evaluator. Replaces round 2's JSX-style literals (77 tokens for the page) | the live page 77 → 48 (2.8); with X1 to X9 the whole program 176 → 150, against LiveView's 214 for its module alone; checked forms | syntax (an indentation-sensitive block after `view`, the only one in the language), check (element names resolve as functions of the provider), types (needs X6 for optional attributes) | here |
| X11 | effect sets as values | the evaluator reads a fn's inferred read and write sets, mapped to the resources its parameters are bound to | the subscription code a live page needs elsewhere (LiveView: subscribe, broadcast, `handle_info`, about 70 tokens); automatic `ETag` | `crates/ignite-eval`, `ignite-check`'s effect table exposed (`Symbols.effects`) | here |
| X12 | durable store | `Durable<T>`, a `Store<T>` over a `Directory`, written in std from todo.ignite's persistence | −1300 in the durable todo (moved, not removed) | none in the compiler; `std` and a resource kind | here |
| X13 | parameters filled by type | the evaluator reads a route function's parameter types (`Signature` values, shared with X8's binding by name) and resolves each from the kind's providers: an identity provider yields `User`, a role function `fn(User) -> Option<Admin>` yields `Admin`; a type with no provider, two providers or a cycle is E120 | −20 on two routes (142 → 122, 2.5); a handler that needs a `User` cannot be reached without one | `crates/ignite-eval` (fn signatures as values); no type-system change | here |
| X14 | derived modules | a module whose source the evaluator writes from a model value at check time and the frontend checks like written source; `Client<TodoService.server>` is the module derived from that component's route table, one fn per route; `ignite explain … --raw` prints it | −5 per call against a `Service` call with the path written (6.12); the gain is the check against the callee | `crates/ignite` loader (a module from the evaluator, before typing the module that imports it; a second evaluation stage), `crates/ignite-eval` | here |
| X15 | file literals | `embed("help.md")` is the text of a file beside the module, `embed_all("notes/*.md")` the files a pattern names, known at check time; the file joins the program's source set, so `ignite check` reads it and a saved plan's source digest covers it (slice §17) | the prose page 146 → 116 as a Markdown file (2.8); its links checked against the routes | `crates/ignite` loader and source set, `crates/ignite-eval`, `apply::plan` source identity | here |

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
| JSX-style markup literals (round 2's X10) | 77 tokens for the page against the view form's 48 and 197 against Markdown's 116 for prose; closing tags carry no information the indentation does not (2.8) |
| conditional class shorthand (`li.done(t.done)`) | 4 tokens fewer on the page, but `.name(expr)` reads as a call and collides with attribute parentheses; static `li.done` and `#id` are kept |
| rate literals (`100 per minute`) | `Rate<Event>` is in design §3 but not implemented; `per_client(100, 1 minute)` costs 3 tokens more and needs no syntax |
| middleware as a typed chain (`layers = [cors, auth, …]`) | the order is data the program can get wrong; 6.11 derives it |
| auth as an annotation (`@auth(admin)`) | the guard and the value the handler uses are two things that can disagree; a parameter of type `Admin` is both (6.10) |

What ships first without them: `x.web` on today's language, a route table of hand-built
`Route` values with combinator codecs and a named handler (639 tokens for the reference
service, every check of section 6.2 included), then X1. Policies (6.11), the `Service`
resource and its fakes (6.12) need no extension; identity works on today's language with the
check in the handler (the 142-token form of 2.5) until X13.

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
  identity: Option<Identity> = None      // 6.10
  roles: List<Role> = []                 // 6.10
  cors: List<Text> = []                  // 6.11: origins; methods and credentials derived
  limit: Option<Limit> = None            // 6.11
  timeout: Duration = 30 seconds         // 6.11
  trusted_proxy: Option<Text> = None     // 6.11: the only way a forwarded address is read
  compress: Bool = true                  // 6.11
  access_log: Bool = true                // 6.11
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

### 6.7 Errors

- Every error is `application/problem+json` (RFC 9457): `type`, `title`, `status`, `detail`, and
  `invalid-params` for a body that does not decode. `fail(status, detail)`, `not_found()`,
  `bad_request(detail)`, `conflict(detail)` build one. Every problem carries the request id
  as `instance`.
- Round 2 had layers here, records of a `before` and an `after` function in a written order.
  Round 3 replaces them with declared policies in a derived order (6.11); a program's own
  check is a `before` function in one fixed slot.

### 6.8 OpenAPI, the typed client, tests without a socket

- `ignite explain FILE TodoService.server.openapi --raw` prints the OpenAPI 3.1 document of the
  route table at check time (`--raw`: a `Text` property verbatim, a generic CLI addition).
- `client(TodoService.server)` in a test serves the component's application over an in-memory
  pipe (`Pipe.pair()`) through the real `Http.serve`; `let made: Todo = api.post("/todos",
  New<Todo> { title = "milk" })?` sends a typed body and decodes a typed answer.
- An HTML page is tested the same way: `api.get("/").body` is the rendered page.
- `TodoService.server.with(weather_api = fake(f))` is the component with a resource replaced
  (6.12); `with` takes any resource the component's routes reach, and a name they do not reach
  is a check error.
- An identity in a test: `api.as(User { sub = 1, name = "a" })` sends the requests as that
  user; the provider is skipped and the roles still run, so a role test needs no token.

### 6.9 Effects and capabilities, in short

No new effect rule. A route function's effects are inferred on its parameters (a `given`
included); capabilities come only from resources (slice §21.3); a `GET` route cannot write; a
handler cannot reach the clock or standard output (E017). There is no wall-clock capability
yet; `std-clock-capability` (section 8) adds one as a resource.

### 6.10 Identity and access

Who may call a route is a type. A route function that takes `user: User` can only run with a
`User`, and the only source of a `User` is the identity provider the component declares:

```ignite
type User { sub: Int, name: Text, roles: List<Text> = [] }
type Admin { user: User }

fn admin(user: User) -> Option<Admin> = user.roles.contains("admin").then(Admin { user })

@get("/me") fn me(user: User) = user
@delete("/todos/{id}") fn remove(_: Admin, id: Int) = todos.remove(id)?

system TodoService {
  resource jwt_secret: Secret
  component server: Web {
    identity = jwt<User>(jwt_secret)
    roles = [admin]
    // listener, routes, executable as in section 1
  }
}
```

- **Resolution (X13).** Each parameter of a route function binds, in order, to a pattern
  parameter by name, the body, a query field, then by type to a provider: the identity yields
  its type, each role function `fn(A) -> Option<B>` yields `B` from an `A`. `Admin` here needs a
  `User`, which needs a valid token. A parameter with no source, a type two providers yield, or
  a cycle of roles is E120 at `ignite check`, naming the route and the parameter. There is no
  guard to forget: the value the handler uses and the check that admits the caller are one
  thing.
- **Answers.** No credential or an invalid one: 401 with `WWW-Authenticate: Bearer` (RFC 6750
  §3) for a token identity, a 303 to `/login` for a page under a session identity. A role that
  answers `None`: 403. A check on one object (only the owner may change a todo) is code in the
  handler with the `User` it holds, and the handler chooses 403 or 404.
- **Providers**, all in `x.web.identity`, each generic over the type it yields, decoded with its
  derived codec (X1):

| provider | credential | notes |
|---|---|---|
| `jwt<T>(secret)` | `Authorization: Bearer` (RFC 6750), a JWS (RFC 7515) | HS256 by default, `es256(public_key)` and `jwks(service)` for keys an issuer publishes; `exp` and `nbf` checked against the clock capability, `aud` when declared |
| `api_key<T>(keys)` | a header, `x-api-key` by default | `keys: Secret` holds one `name key` pair per line; compared in constant time; `T` is built from the name |
| `session<T>(sessions)` | a cookie `__Host-session`, `HttpOnly`, `Secure`, `SameSite=Lax` | `sessions: Durable<Session<T>>`; ids of 128 bits from `Entropy`; a handler returns `sign_in(user)` or `sign_out()` and the kind sets the cookie, a new id at each sign-in |
| `oidc<T>(issuer, client, claims)` | an OpenID Connect sign-in, then a session | derives `/login`, `/callback` and `/logout`; authorization code with PKCE (RFC 7636), `state` and `nonce`; the ID token checked against the issuer's keys, fetched through `issuer: Service` (a dependency in the model, 6.12); `claims: fn(OidcClaims) -> Option<T>` admits an account or answers 403 |
| `any([p, q])` | the first that accepts | browser and API callers reach the same routes; every provider in the list yields the same type, checked |

- **Visible.** `ignite explain TodoService.server.routes` prints every route with what it
  needs: `DELETE /todos/{id}  Admin (jwt, role admin)`, `GET /todos  public`. A route that takes
  no identity is public; `require = User` on the component makes every route need a `User` except
  the ones annotated `@public` (Q9).
- **Secrets.** `Secret` is a std resource kind, a capability like `Setting` (slice §21.5 to 21.7) that
  only a provider reads. The assembly binds it from a file; its value appears in no `ignite plan`
  output, explanation, journal or saved plan.
- **CSRF.** An identity the browser attaches by itself (`session`, `oidc`) protects every unsafe
  route. A form in a view whose route is unsafe gets a hidden `_csrf` input with no program line,
  because the view knows the form's route (X10's facts) and markup text cannot hold a raw
  `<form>`; the token is per session and masked with fresh random bytes in each response, so
  compression stays on (BREACH). A route with a JSON body refuses an unsafe cross-site request by
  `Sec-Fetch-Site` and `Origin` against the listener's origin and `cors`. A `jwt` or `api_key`
  identity needs neither: the browser does not attach a bearer token by itself.

Rejected: an `@auth(admin)` annotation (the guard and the value can disagree; a handler can read
a user it was not guarded for); a request-scoped "current user" (an ambient effect, E017 for a
handler today); permissions as a policy language in the declaration (roles are functions,
already typed and tested inline; a policy language is a second language).

### 6.11 Policies instead of a middleware chain

There is no chain to write. Each concern is a property of the `Web` kind (6.1), and `x.web`
applies them in one order that follows from what each needs:

| slot | policy | property, default | why here |
|---|---|---|---|
| 1 | request id | always: `X-Request-Id` kept when it is 1 to 64 characters of `[A-Za-z0-9-]`, else 128 bits from `Entropy` | every later slot logs or answers with it |
| 2 | access log | `access_log = true`: id, method, pattern (not the raw path, so no ids in the log), status, duration | written after the response, by the server: standard error is no handler's capability |
| 3 | limits | `limits`: header and body bounds, 413 (std's `HttpLimits`) | before anything reads a body |
| 4 | CORS | `cors = [origins]`; methods from the route table, credentials when the identity is a cookie identity | a preflight carries no credentials, so it is answered before identity (Fetch standard, CORS protocol) |
| 5 | limit by client | `limit = per_client(n, window)`: a token bucket per peer address, 429 with `Retry-After` | protects slot 6 from guessing |
| 6 | identity and roles | 6.10 | |
| 7 | limit by identity | `limit = per_identity<User>(n, window)` | its key exists only after slot 6 |
| 8 | CSRF | derived from the identity (6.10) | needs the session |
| 9 | the program's check | `before = f`, `f: fn(Request) -> Result<Unit, Fail>` with the handlers' authority | sees the identity, runs before the handler |
| 10 | timeout | `timeout = 30 seconds`: a deadline on the handler and its outgoing calls, 503 when it passes | |
| 11 | handler | | |
| 12 | cache | `ETag`, 304 (6.3) | needs the handler's read set |
| 13 | errors | problem+json with the request id (6.7) | |
| 14 | compression | `compress = true`: gzip for a text body over 1 KiB the request accepts; not for an event stream | last, on the final bytes |

The order is fixed by the data each slot needs, so there is nothing to order and nothing to get
wrong: in Hono, Axum, Go and FastAPI (appendix A.11) the order is the order of the `use` or
`layer` lines, and a logger after the error handler or a rate limit after authentication
compiles. Per route, an annotation overrides a property (`@post("/upload", body = 10 MiB,
timeout = 2 minutes)`); `scope(pattern, routes, …)` overrides it for a subtree.

Checked at `ignite check` (E120 through `validate`):

- `cors` holding `"*"` with a cookie identity (the Fetch standard forbids
  `Access-Control-Allow-Origin: *` with credentials);
- `per_identity<T>` where no provider yields `T`;
- a `timeout`, of the component or of a route, longer than `rollout.deadline`: a request could
  outlive the drain;
- a forwarded address read without `trusted_proxy`: the client of slot 5 is the peer address
  unless the component names the proxy, and then only from that proxy's address.

State: the buckets of slots 5 and 7 and the request ids are the server's, in memory per process;
no handler sees them. Two instances during a rollout count apart (Q11).

Substrate: a timeout needs a deadline per task in `ignite-io`: every blocking call of the task
fails with `TimedOut` after it (story `task-deadlines`). At the deadline `x.web` answers 503; the
handler's thread ends at its next IO call. A handler in a loop with no IO is not stopped: there is
no preemption, a recorded limit.

### 6.12 Outgoing calls and dependencies

Three layers:

1. **A client role for `std.protocol.http`.** `Http.request(stream: T, request: Request,
   limits: HttpLimits) -> Result<Response, HttpError>`, generic over `T: ByteStream`, the
   counterpart of `serve`: Content-Length and chunked bodies, bounded by `limits`, tested over
   `Pipe.pair()` against `serve`. The RFC 9112 catalogue's client-party requirements get claims.
   HTTPS needs `Tls<T>`, which is Ignite over a TLS engine in `ignite-io` (layering rule;
   `docs/protocols.md`); until it lands a `Service` whose base is `https` is E120 and plain
   `http` works.
2. **`Service`, a resource kind of `x.web`.** Something the system uses and does not run:

```ignite
type Service {
  base: Text
  timeout: Duration = 10 seconds
  attempts: Int = 1                          // idempotent methods only, 6.12
  backoff: Backoff = exponential(100 ms, 2 seconds)
  breaker: Int = 0                           // consecutive failures that open it; 0 is none
  open_for: Duration = 30 seconds
  limits: HttpLimits = HttpLimits {}
  availability: Option<Ratio> = None         // derives a probe and a contract
}
```

   A call: `weather_api.get<Weather>("/current?city={city}")`. The text is a pattern literal
   (X8) used as a URI template (RFC 6570 level 1): `{city}` is the local `city`, percent-encoded,
   never interpolated raw. The answer decodes with `Weather`'s derived codec (X1) into
   `Result<Weather, CallError>` (`Timeout`, `Refused`, `Status(code, body)`, `Decode(problem)`,
   `Open`). In a route function `?` answers 502, 504 for `Timeout`, 503 for `Open`, through the
   named conversion of X4. Further attempts happen only for the idempotent methods of RFC 9110
   §9.2.2; a `POST` with `attempts > 1` must pass `idempotency_key = true`, which sends an
   `Idempotency-Key` header (IETF httpapi draft), else E120. The breaker counts timeouts, refused
   connections and 5xx answers. Each call's deadline is the smaller of `timeout` and what is left
   of the request's (6.11 slot 10), and it carries the request id.
3. **A client derived from another Ignite service (X14).** `resource todo_api:
   Client<TodoService.server>` is the module derived from that component's route table, one
   function per route: `todo_api.todos.create(New<Todo> { title = "milk" })?`, `todo_api.me()`.
   It has `Service`'s properties. A changed route in the callee is a check error in the caller
   when both are in one program; across programs, see below.

What only Ignite does with them:

- **Capability.** `weather_api` is a resource; a route function reaches it as a parameter or a
  `given` (X5), and its calls carry the effect `weather_api.call`. A handler cannot call a host
  it was not given: there is no global client.
- **Dependencies in the model.** X11 maps each route's effect set to resources, so `ignite plan`
  prints `TodoService.server  depends on  weather_api  https://api.weather.example/v1` and
  `ignite explain TodoService.weather_api` lists the routes that call it. The assembly binds
  `base` per environment. A route that reads a `Service` gets no `ETag` (6.3): the remote state
  has no version.
- **Probes and contracts.** `availability = 99.5%` on a `Service` derives a probe member
  `weather_api.health` and a contract `weather_api.reachable` (X9), which `ignite run` evaluates
  next to the server's own. `ignite explain TodoService.server.reachable` names the dependency:
  a 99.9 % target over a dependency every request calls that promises 99.5 % is reported as above
  what the dependency allows (reported, not refused: Q13).
- **Across services.** `ignite plan --for-apply` records the digest of each `Client<X>`
  callee's route table the caller was checked against. `ignite apply` refuses a plan whose callee,
  as running on the host, has a different digest, and rolls out a callee before its callers when
  one plan changes both.
- **Fakes without a socket.**

```ignite
test "weather" {
  let api = client(TodoService.server.with(weather_api = fake(|_| Weather { temp_c = 21, wind_kmh = 5 })))
  assert api.get<Weather>("/weather/Berlin")? == Weather { temp_c = 21, wind_kmh = 5 }
}
```

   A fake is a function served by `Http.serve` over a `Pipe.pair()`, so the client role, the
   codec and the status mapping run as in production; it records its requests
   (`weather_api.calls()`). A `Client<X>` takes the callee itself:
   `todo_api = TodoService.server.in_process()`, with fresh stores.

Rejected: a global client in `x.web` (an ambient effect; the model would not see the edge);
retries on every method (a retried `POST` creates twice); a third-party API typed from its
OpenAPI document now (later, as `Client<openapi(embed("weather.json"))>` through X14 and X15).

### 6.13 Views and Markdown

Two forms, each the cheapest for its content (2.8): views for pages that compute, Markdown files
for prose.

**Views (X10).** `view name(parameters) =` starts an indented block, the only indentation-
sensitive syntax in the language; it ends at the first line indented no deeper than `view`.
Each line is one of:

| line | meaning |
|---|---|
| `h1 Todos` | an element and its text; text is literal to the end of the line, `{expr}` holes escaped |
| `li.done#first(class = c, title = t) text` | static classes and an id, then arguments in parentheses |
| `form("/todos", post)` | arguments by position, as the element function declares them |
| `input("title", required)` | a bare name stands for `= true` when the parameter is a `Bool` |
| `\| text` | a text line |
| `{expr}` | a value: `Html` inserted, `Text` escaped, a list spliced |
| `for t in e`, `if c` and `else`, `match e` with `pattern =>` arms, `let x = e` | control, with an indented body |
| `card(t)` | another view; indented lines below are its last parameter `children: Html` |

An element is a function of the markup provider the file imports (`x.web.html`:
`fn form(action: Route, method: Method = get, …) -> Html`), so arguments are type-checked, and a
view lowers to exactly the nested calls of 2.8's "elements as positional functions"; the
indentation is the surface, the calls are the semantics. A `Route` literal (`form`'s action,
`a`'s href) is checked against the route table, and an input's name against the fields of its
form's route's input type (X10's facts, 6.2). A URL attribute takes `Route` or `Url`, so
`javascript:` cannot be written in one.

**Markdown (X15).** A Markdown file beside the module is a typed document:

```markdown
---
title: Using the board
updated: 2026-11-08
---
# Using the board

Every todo you add appears on [the board](/) in every open browser, without a reload.

## Adding

Type a title and press **Add**. The [JSON API](/todos) accepts the same.
```

```ignite
type Help { title: Text, updated: Text }

routes = {
  "/help" = doc<Help>(embed("help.md"), layout = article)
  "/notes" = docs<Note>(embed_all("notes/*.md"), layout = article)
}
```

- The front matter is decoded at `ignite check` with `Help`'s derived codec, from a `key: value`
  subset (one scalar or `[a, b]` list per line); a missing, extra or mistyped key is a check
  error naming the file and line.
- The body is CommonMark (spec 0.31.2), parsed by `x.web.markdown` in the evaluator at check
  time and compiled into the program as `Html`: a document costs nothing per request. Raw HTML
  in a document is escaped.
- Every link whose target starts with `/` is checked against the route table: `[contact](/contact)`
  with no such route is E120 at `help.md:12`. External links are not checked: `ignite check`
  opens no connection.
- Holes: `{title}` and the other front-matter fields, and calls of the module's pure functions
  that take no capability, so a document never reads a store and stays static.
- `docs<T>` serves a directory: `/notes/{name}` from the file names and the index `/notes`
  from `layout`'s list form; `layout` is a view taking a `Doc<T>`.

Considered (2.8): JSX-style literals (round 2), elements as positional functions written out,
Markdown for everything (forms and loops become holes, 60 tokens for the page against 48), Pug
itself (untyped, no signature), conditional class shorthand.

## 7 Fit with the system model

- `Web` is a component kind like `HttpServer`: an executable member with `machine`, `validate`
  and (X9) `members`. `ignite plan` shows the routes, the derived members and the OpenAPI
  document; `ignite run` supervises, rolls out and probes it; `ignite apply` replaces it.
- Today a closure in a component property stops the model (probe: E118, "a closure cannot be
  encoded in the model"); X3 lifts that, and until then the route table is built by a named
  function the property calls.
- No backend change for the framework itself; the language extensions each say whether they
  touch a backend (X2, X4, X7, X10 lower to ordinary code; only `Store.insert` and
  `Store.version` are substrate builtins and move `CODEGEN_REVISION`). Round 3 adds one
  substrate change, the task deadlines of 6.11; X13 to X15 touch the loader and the evaluator
  only.
- `Service` and `Client<X>` are resources: nodes of the model with an edge from every
  component whose routes' effect sets name them (6.12). `Secret` is a resource whose value the
  model never holds.

## 8 Delivery

Ordered by saving; each language story is its own slice contract section. `x-root` comes first
because nothing in `x/` loads without it.

| step | story (draft) | contents | needs | shared files it touches |
|---|---|---|---|---|
| 1 | `x-root` | the `x` root: `--x`, the default beside `std`, reachable loading, `task dist`, `task test-ignite` runs `x/`, the layering row | | `crates/ignite/src/lib.rs`, `crates/ignite/src/main.rs`, `Taskfile.yml`, `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `docs/slice.md`, `AGENTS.md` |
| 2 | `web-core` | `x/web.ignite` on today's language: routes, patterns, problem+json, 404/405/400/413, the `before` slot, `HttpApp.checks` reported by `validate` | 1, `data-codec-combinators` | `x/web.ignite`, `std/protocol/http.ignite`, `docs/slice.md` |
| 3 | `data-derived-codec` (X1, −195) | serde §8 step 5 | `data-codec-combinators` | `crates/ignite-types`, `std/data.ignite`, `docs/slice.md` |
| 4 | `durable-store` (X12, −1300 moved) | `Durable<T>` in std | 3 | `std/data.ignite` or `std/durable.ignite`, `docs/slice.md` |
| 5 | `mapped-record-types` (X2) and `derived-members` (X9), −141 together | `New<T>`, `Patch<T>`, `with`; the `members` hook; `collection`, the `Web` kind, derived probe and contracts | 2, 3 | `crates/ignite-types`, `crates/ignite-eval`, `x/web.ignite`, `docs/slice.md` |
| 6 | `markup-views` (X10, −29 on the page) | `view` blocks, elements as functions of the provider, `x/web/html.ignite` with its vocabulary, `Route` literals, form facts | 1, 10 | `crates/ignite-syntax`, `crates/ignite-check`, `crates/ignite-types`, `crates/ignite-eval`, `x/web/html.ignite`, `docs/slice.md` |
| 7 | `eval-closure-values` (X3, −51) | closures in the model | | `crates/ignite-eval`, `docs/slice.md` |
| 8 | `answering-handlers` (X4, −41) | inferred results for route functions, `?` on `Option` through `none()` | | `crates/ignite-types`, `docs/slice.md`, `docs/syntax.md` |
| 9 | `given` (X5, −33) | module-level capability parameters | | `crates/ignite-syntax`, `crates/ignite-check`, `crates/ignite-types`, `docs/slice.md` |
| 10 | `param-defaults` (X6, −31) | syntax P10 with call-site typing | 3 | `crates/ignite-syntax`, `crates/ignite-check`, `crates/ignite-types`, `docs/slice.md`, `docs/syntax.md` |
| 11 | `store-insert-and-update-from` (X7, −18) | `Store.insert`, update from another record | | substrate, both backends, `crates/ignite-types`, `docs/slice.md` |
| 12 | `annotations-and-pattern-literals` (X8, −8, checked names) | `@expr` on fns, `Pattern` literals, `module()` with annotations | 1 | `crates/ignite-syntax`, `crates/ignite-types`, `crates/ignite-eval`, `docs/slice.md` |
| 13 | `effect-sets-in-the-evaluator` (X11) and `web-live` | read sets as values, `Store.version`, ETag, `page` live updates, the browser script | 5, 6 | `crates/ignite-eval`, `crates/ignite-check`, substrate, `x/web/live.ignite`, `docs/slice.md` |
| 14 | `web-openapi`, `web-client`, `explain-raw` | OpenAPI, the test client, `--raw` | 5 | `x/web/openapi.ignite`, `x/web/client.ignite`, `crates/ignite/src/main.rs`, `crates/ignite-eval/src/explain.rs` |
| 15 | `todo-on-web` | `examples/todo` in the declared form, its tests on `client` | 4, 5, 13, 14 | `examples/todo/`, `crates/ignite/tests/` |
| 16 | `web-policies` (−191 against Hono, 2.6) | the slots of 6.11, their checks, per-route overrides, `scope` | 2, `task-deadlines` for slot 10 | `x/web.ignite`, `x/web/policy.ignite`, `docs/slice.md` |
| 17 | `web-service` (−122 against FastAPI with its test, 2.7) | the `Service` resource, URI templates, `CallError` and its answers, attempts, breaker, fakes, `with`, dependency edges, derived probe and contract of a dependency | 5, 13, `http-client-role` | `x/web/service.ignite`, `crates/ignite-eval` (edges), `docs/slice.md` |
| 18 | `web-identity` (−95 against Hono, 2.5) | `jwt`, `api_key`, `session`, `oidc`, `any`, roles, 401/403, CSRF, `explain … routes`, `api.as` in tests; on today's language with the check in the handler until step 21 | 4, 6, 17 (for `oidc`), `std-secret` | `x/web/identity.ignite`, `docs/slice.md` |
| 19 | `file-literals` (X15) and `web-markdown` (−81 against JSX for prose, −30 against views) | `embed`, `embed_all` in the source set and saved plans; `x.web.markdown`, typed front matter, checked links, `doc`, `docs` | 3, 6 | `crates/ignite/src/lib.rs` (loader), `crates/ignite/src/apply/plan.rs`, `crates/ignite-eval`, `x/web/markdown.ignite`, `docs/slice.md` |
| 20 | `derived-modules` (X14) and `web-peer-client` (−40 against Hono RPC per call site, 2.7) | modules written by the evaluator, `Client<X>`, route-table digests in saved plans, callee-first rollout | 5, 17 | `crates/ignite/src/lib.rs` (loader), `crates/ignite-eval`, `crates/ignite/src/apply/`, `x/web/client.ignite`, `docs/slice.md` |
| 21 | `parameters-by-type` (X13, −20) | `Signature` values in the evaluator, resolution of route parameters from providers | 12, 18 | `crates/ignite-eval`, `x/web/identity.ignite`, `docs/slice.md` |
| — | `std-clock-capability`, `http-access-log` | a wall-clock resource (also `jwt`'s `exp`); access logging on the server | | `std/`, substrate; `std/protocol/http.ignite` |
| — | `http-client-role` | `Http.request` over `ByteStream`, client-party claims of the RFC 9112 catalogue | | `std/protocol/http.ignite`, `std/rfc/rfc9112.ignite` claims, `docs/slice.md` |
| — | `tls-client` | `Tls<T>` for `https` bases (`docs/protocols.md`) | | `crates/ignite-io`, `std/net.ignite`, `docs/slice.md` |
| — | `task-deadlines` | a deadline per task; blocking calls fail with `TimedOut` after it | | `crates/ignite-io`, `crates/ignite-rt`, `std/net.ignite`, `docs/slice.md` |
| — | `std-secret`, `std-bool-then` | the `Secret` resource kind, bound by the assembly and redacted everywhere; `Bool.then` in `std/option.ignite` | | `std/process.ignite`, `crates/ignite-eval` (redaction), `crates/ignite/src/apply/`; `std/option.ignite` |

Steps 6 to 12 touch disjoint parts of the frontend but share `crates/ignite-types` and
`docs/slice.md`; a wave takes at most two of them. Steps 16 to 18 are `x/` code in separate
files and can run together; 19 and 20 share the loader in `crates/ignite/src/lib.rs` and run
one after the other. Concept B (wasm client components) and C
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
| Q9 | routes public unless they take an identity, or `require = User` by default | public unless they take one; `ignite explain … routes` lists the public ones |
| Q10 | CSRF for JSON routes: Fetch Metadata and `Origin`, or a token as for forms | Fetch Metadata and `Origin`; a token only in forms |
| Q11 | rate limits across instances | per process; a shared limit waits for a store resource two processes can share |
| Q12 | a switch that turns TLS verification off | none; `ca = file` for a private authority |
| Q13 | an availability target above what the dependencies promise: report or refuse | reported by `explain`, not an error |
| Q14 | the Markdown dialect | CommonMark 0.31.2 plus tables; no raw HTML |
| Q15 | `Client<X>` for a callee in another repository | not yet: same program only, and across deployments through the route-table digest of saved plans |

## Appendix A: the measured programs

Each is counted by section 2.1's rules. A.1 is checked; the comparisons are not run; A.7 to A.9,
and the Ignite programs of A.10 to A.13, are sketches of the target language.

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

### A.10 Authentication and roles (2.5)
`x.web` (6.10):

```ignite
import x.web.{Web, Secret, jwt, get, delete}

type User { sub: Int, name: Text, roles: List<Text> = [] }
type Admin { user: User }

fn admin(user: User) -> Option<Admin> = user.roles.contains("admin").then(Admin { user })

@get("/me") fn me(user: User) = user
@delete("/todos/{id}") fn remove(_: Admin, id: Int) = todos.remove(id)?

resource jwt_secret: Secret
component server: Web {
  identity = jwt<User>(jwt_secret)
  roles = [admin]
}
```

`x.web` without X13:

```ignite
import x.web.{Web, Request, Secret, verify_jwt, forbidden, get, delete}

type User { sub: Int, name: Text, roles: List<Text> = [] }

given jwt_secret: Secret

fn user(request: Request) = verify_jwt<User>(request, jwt_secret)?
fn admin(request: Request) = { let u = user(request)?; if u.roles.contains("admin") { Ok(u) } else { Err(forbidden()) } }

@get("/me") fn me(request: Request) = user(request)?
@delete("/todos/{id}") fn remove(request: Request, id: Int) = { admin(request)?; todos.remove(id)? }

resource jwt_secret: Secret
```

Hono with `hono/jwt` (https://hono.dev/docs/middleware/builtin/jwt):

```typescript
import { Hono } from 'hono'
import { jwt } from 'hono/jwt'
import { HTTPException } from 'hono/http-exception'
import type { JwtVariables } from 'hono/jwt'

type User = { sub: number; name: string; roles?: string[] }

const app = new Hono<{ Variables: JwtVariables<User> }>()

app.use('/me', jwt({ secret: process.env.JWT_SECRET!, alg: 'HS256' }))
app.use('/todos/*', jwt({ secret: process.env.JWT_SECRET!, alg: 'HS256' }))

const admin = async (c, next) => {
  if (!c.get('jwtPayload').roles?.includes('admin')) throw new HTTPException(403)
  await next()
}

app.get('/me', (c) => c.json(c.get('jwtPayload')))

app.delete('/todos/:id{[0-9]+}', admin, (c) =>
  todos.delete(Number(c.req.param('id'))) ? c.body(null, 204) : c.notFound()
)
```

FastAPI with PyJWT (https://fastapi.tiangolo.com/tutorial/security/oauth2-jwt/):

```python
import os

import jwt
from fastapi import Depends, FastAPI, HTTPException, Response
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer
from pydantic import BaseModel

app = FastAPI()
bearer = HTTPBearer()
SECRET = os.environ["JWT_SECRET"]


class User(BaseModel):
    id: int
    name: str
    roles: list[str] = []


def current_user(creds: HTTPAuthorizationCredentials = Depends(bearer)) -> User:
    try:
        claims = jwt.decode(creds.credentials, SECRET, algorithms=["HS256"])
    except jwt.InvalidTokenError:
        raise HTTPException(401)
    return User(id=claims["sub"], name=claims["name"], roles=claims.get("roles", []))


def admin(user: User = Depends(current_user)) -> User:
    if "admin" not in user.roles:
        raise HTTPException(403)
    return user


@app.get("/me")
def me(user: User = Depends(current_user)) -> User:
    return user


@app.delete("/todos/{id}", status_code=204)
def delete(id: int, user: User = Depends(admin)):
    if todos.pop(id, None) is None:
        raise HTTPException(404)
    return Response(status_code=204)
```

Go 1.22 with `golang-jwt/jwt/v5`:

```go
package main

import (
	"context"
	"encoding/json"
	"net/http"
	"os"
	"slices"
	"strconv"
	"strings"

	"github.com/golang-jwt/jwt/v5"
)

type User struct {
	ID    int      `json:"sub"`
	Name  string   `json:"name"`
	Roles []string `json:"roles"`
	jwt.RegisteredClaims
}

type userKey struct{}

func authed(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		raw, ok := strings.CutPrefix(r.Header.Get("Authorization"), "Bearer ")
		var u User
		_, err := jwt.ParseWithClaims(raw, &u, func(*jwt.Token) (any, error) {
			return []byte(os.Getenv("JWT_SECRET")), nil
		}, jwt.WithValidMethods([]string{"HS256"}))
		if !ok || err != nil {
			http.Error(w, "", http.StatusUnauthorized)
			return
		}
		next(w, r.WithContext(context.WithValue(r.Context(), userKey{}, u)))
	}
}

func admin(next http.HandlerFunc) http.HandlerFunc {
	return authed(func(w http.ResponseWriter, r *http.Request) {
		if !slices.Contains(r.Context().Value(userKey{}).(User).Roles, "admin") {
			http.Error(w, "", http.StatusForbidden)
			return
		}
		next(w, r)
	})
}

func routes(mux *http.ServeMux) {
	mux.HandleFunc("GET /me", authed(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(r.Context().Value(userKey{}).(User))
	}))
	mux.HandleFunc("DELETE /todos/{id}", admin(func(w http.ResponseWriter, r *http.Request) {
		id, err := strconv.Atoi(r.PathValue("id"))
		if err != nil || !todos.Remove(id) {
			http.NotFound(w, r)
			return
		}
		w.WriteHeader(http.StatusNoContent)
	}))
}
```

Axum 0.8 with `axum-extra` and `jsonwebtoken` (examples/jwt in the axum repository):

```rust
use axum::{
    extract::{FromRequestParts, Path, State},
    http::{request::Parts, StatusCode},
    Json,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct User {
    sub: u64,
    name: String,
    #[serde(default)]
    roles: Vec<String>,
}

struct Admin(User);

impl<S: Send + Sync> FromRequestParts<S> for User {
    type Rejection = StatusCode;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(|_| StatusCode::UNAUTHORIZED)?;
        let key = DecodingKey::from_secret(std::env::var("JWT_SECRET").unwrap().as_bytes());
        decode::<User>(bearer.token(), &key, &Validation::default())
            .map(|data| data.claims)
            .map_err(|_| StatusCode::UNAUTHORIZED)
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Admin {
    type Rejection = StatusCode;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = User::from_request_parts(parts, state).await?;
        if user.roles.iter().any(|r| r == "admin") {
            Ok(Admin(user))
        } else {
            Err(StatusCode::FORBIDDEN)
        }
    }
}

async fn me(user: User) -> Json<User> {
    Json(user)
}

async fn delete(_: Admin, State(db): State<Db>, Path(id): Path<u64>) -> StatusCode {
    match db.todos.lock().unwrap().remove(&id) {
        Some(_) => StatusCode::NO_CONTENT,
        None => StatusCode::NOT_FOUND,
    }
}
```

### A.11 Policies (2.6)
`x.web` (6.11); the request id, the access log, compression and problem+json are defaults:

```ignite
import x.web.{Web, per_client}

component server: Web {
  cors = ["https://app.example.org"]
  limit = per_client(100, 1 minute)
  timeout = 5 seconds
}
```

Hono (https://hono.dev/docs/middleware/builtin/cors and the pages beside it; `hono-rate-limiter`):

```typescript
import { Hono } from 'hono'
import { requestId } from 'hono/request-id'
import { logger } from 'hono/logger'
import { cors } from 'hono/cors'
import { timeout } from 'hono/timeout'
import { compress } from 'hono/compress'
import { HTTPException } from 'hono/http-exception'
import { rateLimiter } from 'hono-rate-limiter'

const app = new Hono()

app.use(requestId())
app.use(logger())
app.use(cors({ origin: 'https://app.example.org', credentials: true }))
app.use(timeout(5000))
app.use(rateLimiter({
  windowMs: 60_000,
  limit: 100,
  keyGenerator: (c) => c.req.header('x-forwarded-for') ?? '',
}))
app.use(compress())

app.onError((err, c) => {
  const status = err instanceof HTTPException ? err.status : 500
  return c.json({ title: err.message, status, instance: c.get('requestId') }, status,
    { 'Content-Type': 'application/problem+json' })
})
```

Go with `chi` middleware, `go-chi/cors`, `go-chi/httprate`:

```go
package main

import (
	"context"
	"log/slog"
	"net/http"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/go-chi/cors"
	"github.com/go-chi/httprate"
)

func router() *chi.Mux {
	r := chi.NewRouter()
	r.Use(middleware.RequestID)
	r.Use(middleware.Logger)
	r.Use(cors.Handler(cors.Options{
		AllowedOrigins:   []string{"https://app.example.org"},
		AllowedMethods:   []string{"GET", "POST", "PATCH", "DELETE"},
		AllowCredentials: true,
	}))
	r.Use(middleware.Timeout(5 * time.Second))
	r.Use(httprate.LimitByIP(100, time.Minute))
	r.Use(middleware.Compress(5))
	r.Use(middleware.Recoverer)
	return r
}

func problem(w http.ResponseWriter, r *http.Request, status int, title string) {
	w.Header().Set("Content-Type", "application/problem+json")
	w.WriteHeader(status)
	slog.InfoContext(context.Background(), "problem", "id", middleware.GetReqID(r.Context()))
	w.Write([]byte(`{"title":"` + title + `","status":` + http.StatusText(status) + `}`))
}
```

Axum with `tower-http` and `tower_governor`:

```rust
use std::{sync::Arc, time::Duration};

use axum::{error_handling::HandleErrorLayer, http::{HeaderValue, Method, StatusCode}, BoxError, Router};
use tower::{ServiceBuilder, timeout::TimeoutLayer};
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

fn layers(app: Router) -> Router {
    let governor = Arc::new(
        GovernorConfigBuilder::default().per_millisecond(600).burst_size(100).finish().unwrap(),
    );
    app.layer(
        ServiceBuilder::new()
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
            .layer(PropagateRequestIdLayer::x_request_id())
            .layer(TraceLayer::new_for_http())
            .layer(
                CorsLayer::new()
                    .allow_origin("https://app.example.org".parse::<HeaderValue>().unwrap())
                    .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
                    .allow_credentials(true),
            )
            .layer(HandleErrorLayer::new(|err: BoxError| async move {
                if err.is::<tower::timeout::error::Elapsed>() {
                    (StatusCode::GATEWAY_TIMEOUT, "timeout".to_string())
                } else {
                    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
                }
            }))
            .layer(TimeoutLayer::new(Duration::from_secs(5)))
            .layer(GovernorLayer { config: governor })
            .layer(CompressionLayer::new()),
    )
}
```

FastAPI with Starlette's middleware and `slowapi`:

```python
import asyncio
import logging
import uuid

from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.middleware.gzip import GZipMiddleware
from fastapi.responses import JSONResponse
from slowapi import Limiter, _rate_limit_exceeded_handler
from slowapi.errors import RateLimitExceeded
from slowapi.middleware import SlowAPIMiddleware
from slowapi.util import get_remote_address
from starlette.exceptions import HTTPException

log = logging.getLogger("access")
limiter = Limiter(key_func=get_remote_address, default_limits=["100/minute"])
app = FastAPI()
app.state.limiter = limiter
app.add_exception_handler(RateLimitExceeded, _rate_limit_exceeded_handler)
app.add_middleware(SlowAPIMiddleware)
app.add_middleware(GZipMiddleware)
app.add_middleware(CORSMiddleware, allow_origins=["https://app.example.org"], allow_credentials=True)


@app.middleware("http")
async def request_id_log_timeout(request: Request, call_next):
    rid = request.headers.get("x-request-id", str(uuid.uuid4()))
    request.state.rid = rid
    try:
        response = await asyncio.wait_for(call_next(request), 5)
    except asyncio.TimeoutError:
        response = JSONResponse({"title": "timeout", "status": 504}, 504)
    response.headers["x-request-id"] = rid
    log.info("%s %s %s %s", rid, request.method, request.url.path, response.status_code)
    return response


@app.exception_handler(HTTPException)
async def problem(request: Request, exc: HTTPException):
    return JSONResponse(
        {"title": exc.detail, "status": exc.status_code, "instance": request.state.rid},
        exc.status_code,
        media_type="application/problem+json",
    )
```

### A.12 Outgoing calls (2.7)
`x.web` (6.12), route, resource and test:

```ignite
import x.web.{Web, Service, get, client, fake}

type Weather { temp_c: Decimal, wind_kmh: Decimal }

@get("/weather/{city}") fn weather(city: Text) = weather_api.get<Weather>("/current?city={city}")?

resource weather_api: Service {
  base = "https://api.weather.example/v1"
  timeout = 2 seconds
  attempts = 3
  breaker = 5
}

test "weather" {
  let api = client(TodoService.server.with(weather_api = fake(|_| Weather { temp_c = 21, wind_kmh = 5 })))
  assert api.get<Weather>("/weather/Berlin")? == Weather { temp_c = 21, wind_kmh = 5 }
}
```

FastAPI with `httpx`, `tenacity`, `pybreaker`, and its test:

```python
import httpx
import pybreaker
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from tenacity import retry, stop_after_attempt, wait_exponential

app = FastAPI()
breaker = pybreaker.CircuitBreaker(fail_max=5, reset_timeout=30)
weather_api = httpx.Client(base_url="https://api.weather.example/v1", timeout=2.0)


class Weather(BaseModel):
    temp_c: float
    wind_kmh: float


@breaker
@retry(stop=stop_after_attempt(3), wait=wait_exponential(min=0.1, max=2), reraise=True)
def fetch(city: str) -> Weather:
    response = weather_api.get("/current", params={"city": city})
    response.raise_for_status()
    return Weather.model_validate(response.json())


@app.get("/weather/{city}")
def weather(city: str) -> Weather:
    try:
        return fetch(city)
    except (httpx.HTTPError, pybreaker.CircuitBreakerError):
        raise HTTPException(502)


def test_weather(monkeypatch):
    from fastapi.testclient import TestClient
    monkeypatch.setattr(__name__ + ".fetch", lambda city: Weather(temp_c=21, wind_kmh=5))
    assert TestClient(app).get("/weather/Berlin").json() == {"temp_c": 21, "wind_kmh": 5}
```

Hono with `fetch`, `zod`, `p-retry`, `opossum`, and a `vitest` test:

```typescript
import { Hono } from 'hono'
import { HTTPException } from 'hono/http-exception'
import { z } from 'zod'
import CircuitBreaker from 'opossum'
import pRetry from 'p-retry'

const Weather = z.object({ temp_c: z.number(), wind_kmh: z.number() })

const fetchWeather = async (city: string) => {
  const res = await fetch(
    `https://api.weather.example/v1/current?city=${encodeURIComponent(city)}`,
    { signal: AbortSignal.timeout(2000) })
  if (!res.ok) throw new Error(String(res.status))
  return Weather.parse(await res.json())
}

const breaker = new CircuitBreaker(
  (city: string) => pRetry(() => fetchWeather(city), { retries: 2, minTimeout: 100 }),
  { errorThresholdPercentage: 50, volumeThreshold: 5, resetTimeout: 30_000 })

export const app = new Hono()
app.get('/weather/:city', async (c) => {
  try {
    return c.json(await breaker.fire(c.req.param('city')))
  } catch {
    throw new HTTPException(502)
  }
})

import { test, expect, vi } from 'vitest'
test('weather', async () => {
  vi.spyOn(breaker, 'fire').mockResolvedValue({ temp_c: 21, wind_kmh: 5 })
  const res = await app.request('/weather/Berlin')
  expect(await res.json()).toEqual({ temp_c: 21, wind_kmh: 5 })
})
```

Go with `sony/gobreaker` and `cenkalti/backoff` (no test written):

```go
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"time"

	"github.com/cenkalti/backoff/v4"
	"github.com/sony/gobreaker"
)

type Weather struct {
	TempC   float64 `json:"temp_c"`
	WindKmh float64 `json:"wind_kmh"`
}

var client = &http.Client{Timeout: 2 * time.Second}
var breaker = gobreaker.NewCircuitBreaker(gobreaker.Settings{
	Timeout:     30 * time.Second,
	ReadyToTrip: func(c gobreaker.Counts) bool { return c.ConsecutiveFailures >= 5 },
})

func fetch(ctx context.Context, city string) (Weather, error) {
	var w Weather
	op := func() error {
		resp, err := client.Get("https://api.weather.example/v1/current?city=" + url.QueryEscape(city))
		if err != nil {
			return err
		}
		defer resp.Body.Close()
		if resp.StatusCode != http.StatusOK {
			return fmt.Errorf("status %d", resp.StatusCode)
		}
		return json.NewDecoder(resp.Body).Decode(&w)
	}
	_, err := breaker.Execute(func() (any, error) {
		return nil, backoff.Retry(op, backoff.WithMaxRetries(backoff.NewExponentialBackOff(), 2))
	})
	return w, err
}

func weather(w http.ResponseWriter, r *http.Request) {
	result, err := fetch(r.Context(), r.PathValue("city"))
	if err != nil {
		http.Error(w, "", http.StatusBadGateway)
		return
	}
	json.NewEncoder(w).Encode(result)
}
```

Axum with `reqwest-middleware`, `reqwest-retry`, `failsafe` (no test written):

```rust
use std::time::Duration;

use axum::{extract::{Path, State}, http::StatusCode, Json};
use failsafe::{backoff, failure_policy, futures::CircuitBreaker, Config, StateMachine};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct Weather {
    temp_c: f64,
    wind_kmh: f64,
}

type Breaker = StateMachine<failure_policy::ConsecutiveFailures<backoff::Constant>, ()>;

#[derive(Clone)]
struct Api {
    http: ClientWithMiddleware,
    breaker: Breaker,
}

fn api() -> Api {
    let http = reqwest::Client::builder().timeout(Duration::from_secs(2)).build().unwrap();
    let retry = ExponentialBackoff::builder().build_with_max_retries(2);
    Api {
        http: ClientBuilder::new(http).with(RetryTransientMiddleware::new_with_policy(retry)).build(),
        breaker: Config::new()
            .failure_policy(failure_policy::consecutive_failures(5, backoff::constant(Duration::from_secs(30))))
            .build(),
    }
}

async fn weather(State(api): State<Api>, Path(city): Path<String>) -> Result<Json<Weather>, StatusCode> {
    let call = async {
        api.http
            .get("https://api.weather.example/v1/current")
            .query(&[("city", city)])
            .send()
            .await?
            .error_for_status()?
            .json::<Weather>()
            .await
            .map_err(reqwest_middleware::Error::from)
    };
    api.breaker.call(call).await.map(Json).map_err(|_| StatusCode::BAD_GATEWAY)
}
```

Calling another service, Hono RPC (https://hono.dev/docs/guides/rpc) and `x.web`'s `Client<X>`:

```typescript
import { hc } from 'hono/client'
import type { AppType } from '../todo/server'
const todo = hc<AppType>(process.env.TODO_URL!)
const res = await todo.todos.$post({ json: { title: 'milk' } })
if (!res.ok) throw new Error(String(res.status))
const made = await res.json()
```

```ignite
import todo.{TodoService}
resource todo_api: Client<TodoService.server>
let made = todo_api.todos.create({ title = "milk" })?
```

### A.13 Markup (2.8)
The live page in each Ignite candidate:

indentation view, positional attributes (chosen):

```ignite
view board(todos: Store<Todo>) =
  main
    h1 Todos
    ul
      for t in todos.values()
        li(class = t.done.then("done")) {t.title}
    form("/todos", post)
      input("title", required)
      button Add
```

indentation view, named attributes:

```ignite
view board(todos: Store<Todo>) =
  main
    h1 Todos
    ul
      for t in todos.values()
        li(class = t.done.then("done")) {t.title}
    form(action = "/todos", method = "post")
      input(name = "title", required)
      button Add
```

indentation view, conditional class shorthand:

```ignite
view board(todos: Store<Todo>) =
  main
    h1 Todos
    ul
      for t in todos.values()
        li.done(t.done) {t.title}
    form("/todos", post)
      input("title", required)
      button Add
```

elements as positional functions:

```ignite
fn board(todos: Store<Todo>) -> Html =
  main(
    h1("Todos"),
    ul(todos.values().map(|t| li(class = t.done.then("done"), t.title))),
    form("/todos", post, input("title", required = true), button("Add")))
```

Markdown with typed holes:

```ignite
fn board(todos: Store<Todo>) -> Html = md"""
  # Todos
  {for t in todos.values()}
  - {t.title}{t.done.then(" (done)")}
  {end}
  {form("/todos", post, input("title", required = true), button("Add"))}
  """
```

JSX-style literals (round 2):

```ignite
fn board(todos: Store<Todo>) -> Html =
  <main>
    <h1>Todos</h1>
    <ul>{todos.values().map(|t| <li class={t.done.then("done")}>{t.title}</li>)}</ul>
    <form action="/todos" method="post">
      <input name="title" required/>
      <button>Add</button>
    </form>
  </main>
```

Pug, HEEx (https://hexdocs.pm/phoenix_live_view/Phoenix.Component.html) and templ (https://templ.guide):

```pug
main
  h1 Todos
  ul
    each t in todos
      li(class=t.done ? "done" : undefined)= t.title
  form(action="/todos" method="post")
    input(name="title" required)
    button Add
```

```elixir
def board(assigns) do
  ~H"""
  <main>
    <h1>Todos</h1>
    <ul><li :for={t <- @todos} class={t.done && "done"}>{t.title}</li></ul>
    <form action={~p"/todos"} method="post">
      <input name="title" required />
      <button>Add</button>
    </form>
  </main>
  """
end
```

```go
templ Board(todos []Todo) {
	<main>
		<h1>Todos</h1>
		<ul>
			for _, t := range todos {
				<li class={ templ.KV("done", t.Done) }>{ t.Title }</li>
			}
		</ul>
		<form action="/todos" method="post">
			<input name="title" required/>
			<button>Add</button>
		</form>
	</main>
}
```

The prose page, as a Markdown file (chosen for prose) and as a view:

```markdown
---
title: Using the board
updated: 2026-11-08
---
# Using the board
Every todo you add appears on [the board](/) in every open browser, without a reload.
## Adding
Type a title and press **Add**. A title can be at most 200 characters; the [JSON API](/todos) accepts the same.
## Finishing
- Click a todo to mark it done.
- Click it again to reopen it.
- Delete it from its own page.
## Questions
Write to the address on the [contact page](/contact).
```

```ignite
const help = Doc { title = "Using the board", updated = "2026-11-08" }
view help_page() =
  article
    h1 Using the board
    p Every todo you add appears on {a("/", "the board")} in every open browser, without a reload.
    h2 Adding
    p Type a title and press {strong("Add")}. A title can be at most 200 characters; the {a("/todos", "JSON API")} accepts the same.
    h2 Finishing
    ul
      li Click a todo to mark it done.
      li Click it again to reopen it.
      li Delete it from its own page.
    h2 Questions
    p Write to the address on the {a("/contact", "contact page")}.
```

as elements written as calls, in JSX-style literals, Pug and HEEx:

```ignite
const help = Doc { title = "Using the board", updated = "2026-11-08" }
fn help_page() -> Html =
  article(
    h1("Using the board"),
    p("Every todo you add appears on ", a("/", "the board"), " in every open browser, without a reload."),
    h2("Adding"),
    p("Type a title and press ", strong("Add"), ". A title can be at most 200 characters; the ", a("/todos", "JSON API"), " accepts the same."),
    h2("Finishing"),
    ul(
      li("Click a todo to mark it done."),
      li("Click it again to reopen it."),
      li("Delete it from its own page.")),
    h2("Questions"),
    p("Write to the address on the ", a("/contact", "contact page"), "."))
```

```ignite
const help = Doc { title = "Using the board", updated = "2026-11-08" }
fn help_page() -> Html =
  <article>
    <h1>Using the board</h1>
    <p>Every todo you add appears on <a href="/">the board</a> in every open browser, without a reload.</p>
    <h2>Adding</h2>
    <p>Type a title and press <strong>Add</strong>. A title can be at most 200 characters; the <a href="/todos">JSON API</a> accepts the same.</p>
    <h2>Finishing</h2>
    <ul>
      <li>Click a todo to mark it done.</li>
      <li>Click it again to reopen it.</li>
      <li>Delete it from its own page.</li>
    </ul>
    <h2>Questions</h2>
    <p>Write to the address on the <a href="/contact">contact page</a>.</p>
  </article>
```

```pug
- const help = { title: "Using the board", updated: "2026-11-08" }
article
  h1 Using the board
  p Every todo you add appears on #[a(href="/") the board] in every open browser, without a reload.
  h2 Adding
  p Type a title and press #[strong Add]. A title can be at most 200 characters; the #[a(href="/todos") JSON API] accepts the same.
  h2 Finishing
  ul
    li Click a todo to mark it done.
    li Click it again to reopen it.
    li Delete it from its own page.
  h2 Questions
  p Write to the address on the #[a(href="/contact") contact page].
```

```elixir
def help(assigns) do
  ~H"""
  <article>
    <h1>Using the board</h1>
    <p>Every todo you add appears on <.link navigate={~p"/"}>the board</.link> in every open browser, without a reload.</p>
    <h2>Adding</h2>
    <p>Type a title and press <strong>Add</strong>. A title can be at most 200 characters; the <.link navigate={~p"/todos"}>JSON API</.link> accepts the same.</p>
    <h2>Finishing</h2>
    <ul>
      <li>Click a todo to mark it done.</li>
      <li>Click it again to reopen it.</li>
      <li>Delete it from its own page.</li>
    </ul>
    <h2>Questions</h2>
    <p>Write to the address on the <.link navigate={~p"/contact"}>contact page</.link>.</p>
  </article>
  """
end
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
