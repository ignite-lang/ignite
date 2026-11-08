# x.web: the web app is part of the system <span class="badge design">design</span>

`x.web` is a planned web framework. It will ship with Ignite, in a second module root `x` next
to `std`, and be versioned with the compiler. Nothing on this page is implemented yet: the code
below is the language as it is planned to grow for the framework, and it does not compile
today. The full design, with its measurements, the language extensions and the planned steps,
is [`docs/web.md`](https://github.com/ignite-lang/ignite/blob/main/docs/web.md).

The idea: you declare your data and what you expose. Ignite derives the routes, the JSON, the
OpenAPI document, the caching, the live updates and the health checks, and checks your pages
against your routes. You write only the exceptions.

## A JSON API and a live page

```ignite,fragment
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

That is the whole program. It serves:

| what | where it comes from |
|---|---|
| `GET` and `POST /todos`, `GET`, `PATCH` and `DELETE /todos/{id}` with JSON bodies | `collection(todos)` and the type `Todo` |
| 400 naming the bad member, 404, 405 with `Allow`, all as `application/problem+json` | the route table |
| an HTML page at `/` that updates in every open browser when a todo is added | `page(board)`, because `board` reads `todos` |
| `ETag` headers and `304 Not Modified` | the stores each route reads |
| a `/health` route, a probe and an availability contract that `ignite run` evaluates | `availability = 99%` |
| an OpenAPI 3.1 document | the route table, printed at check time |
| request ids, an access log, compression | defaults of the `Web` kind |

## What effect tracking gives

Ignite tracks effects. A function that reads a store says so in its type, and a handler can
only reach what it is given: no clock, no global, no file it was not handed.

- **Live pages without subscriptions.** The compiler knows that `board` reads `todos`. After
  any write to `todos`, from any route, the server renders `board` again and sends it to every
  open page. There is no subscribe and no broadcast to write.
- **Caching that is correct by construction.** The answer to a GET depends only on the request
  and the stores it reads, so each store's version is the ETag.
- **The app is a system member.** `ignite plan` shows the routes, the services they call and
  the derived probe and contract, `ignite run` probes the service and reports whether it meets
  99 %, and `ignite apply` rolls it out
  ([Plan, explain, run and replay](../guide/operate.md)).

## Pages are views

A `view` is an indented block: one element per line, its text after the name, children
indented below it, `{expressions}` where values go, `for` and `if` lines for control. Text is
escaped. An element is a typed function of the HTML provider, so `form("/todos", post)` passes
the action and the method by position, and a misspelt element or attribute is a compile error.
The page is also checked against the routes: `input("titel")` or `form("/todo", post)` does not
compile, because no route takes them. A form for a signed-in user gets its CSRF token with no
line written.

## Prose is Markdown

Text-heavy pages are Markdown files with typed front matter:

```markdown
---
title: Using the board
updated: 2026-11-08
---
# Using the board

Every todo you add appears on [the board](/) in every open browser, without a reload.
The [JSON API](/todos) accepts the same.
```

```ignite,fragment
type Help { title: Text, updated: Text }

routes = {
  "/help" = doc<Help>(embed("help.md"), layout = article)
}
```

The front matter is checked against `Help`, and every link that starts with `/` is checked
against the routes: a link to a page that does not exist is a compile error with the file and
line. The Markdown is rendered once, at build time.

## Who may call a route

Who may call a route is a type. A handler that takes `user: User` can only run with a `User`,
and the only place a `User` comes from is the identity the component declares:

```ignite,fragment
type User { sub: Int, name: Text, roles: List<Text> = [] }
type Admin { user: User }

fn admin(user: User) -> Option<Admin> = user.roles.contains("admin").then(Admin { user })

@get("/me") fn me(user: User) = user
@delete("/todos/{id}") fn remove(_: Admin, id: Int) = todos.remove(id)?
```

```ignite,fragment
component server: Web {
  identity = jwt<User>(jwt_secret)
  roles = [admin]
}
```

There is no guard to forget: `remove` needs an `Admin`, an `Admin` needs a `User`, and a `User`
needs a valid token. Without one the answer is 401, without the role 403. The same works with
sessions, API keys and OpenID Connect sign-in, and `ignite explain` lists which routes are
public.

## Policies

There is no middleware chain to put in order:

```ignite,fragment
component server: Web {
  cors = ["https://app.example.org"]
  limit = per_client(100, 1 minute)
  timeout = 5 seconds
}
```

Each concern is a property, and `x.web` applies them in the one order that works: a CORS
preflight before sign-in, a per-client limit before sign-in, a per-user limit after it, the
log after the answer, compression last. The checker refuses combinations that cannot be right,
such as CORS for any origin with cookies, or a timeout longer than the rollout's drain.

## Calling other services

A service you call is a resource:

```ignite,fragment
resource weather_api: Service {
  base = "https://api.weather.example/v1"
  timeout = 2 seconds
  attempts = 3
  breaker = 5
}

@get("/weather/{city}") fn weather(city: Text) = weather_api.get<Weather>("/current?city={city}")?
```

`{city}` is encoded, never pasted in. The answer is decoded into `Weather`, and a failed call
answers 502, a timeout 504. Only idempotent requests are retried. Because a handler can only
call what it is given, `ignite plan` shows `weather_api` as a dependency of `server`, and a
probe and a contract can cover it. Another Ignite service is called through a client derived
from its routes, `Client<TodoService.server>`, so a removed route breaks the caller's build,
not production.

## Testing without a socket

```ignite,fragment
test "create and read" {
  let api = client(TodoService.server)
  let made: Todo = api.post("/todos", New<Todo> { title = "milk" })?
  assert api.get("/todos/{made.id}").status == 200
  assert api.get("/").body.contains("milk")
}

test "weather" {
  let api = client(TodoService.server.with(weather_api = fake(|_| Weather { temp_c = 21, wind_kmh = 5 })))
  assert api.get<Weather>("/weather/Berlin")? == Weather { temp_c = 21, wind_kmh = 5 }
}
```

The test client speaks HTTP/1.1 over an in-memory pipe through the same parser, framing and
limits as a real connection, and a fake service runs behind the same HTTP client code as the
real one.

## How many tokens

Counted with the `cl100k_base` tokenizer, imports included, comments and indentation not. The
other programs are written from their documentation's idioms. All are in the design document.

| task | FastAPI | Hono | Axum | Go | `x.web` |
|---|---|---|---|---|---|
| the todo JSON service | 296 | 348 | 563 | 620 | 89 declared, 230 with every handler written |
| bearer tokens and an admin role | 237 | 217 | 398 | 369 | 122 |
| request id, log, CORS, rate limit, timeout, compression, error format | 351 | 231 | 316 | 259 | 40 |
| a third-party JSON call with timeout, retries and circuit breaker, without its test | 205 | 238 | 349 | 324 | 88 |

The live page: the whole Ignite program above, API included, is 150 tokens. The Phoenix
LiveView module for the same page alone is 214. The view itself is 48 tokens, against 77 in
JSX and HEEx. The prose page is 116 tokens as Markdown, against 197 in JSX. The durable todo
example of this repository, with its health probe and contract, goes from 1,883 tokens to 179.

## Plan

The framework needs language extensions: derived codecs, record types derived from others
(`New<T>`, `Patch<T>`), members a component derives into the system model, views, effect sets
readable at check time, `given`, route annotations and pattern literals, handler parameters
filled by type, modules derived from another service's routes, and files embedded at check
time. A first version ships on today's language with hand-built routes, and each extension
shortens it. The order and the open questions are in
[`docs/web.md`](https://github.com/ignite-lang/ignite/blob/main/docs/web.md) sections 5, 8 and
10. The [roadmap](../roadmap.md) lists it with the other designs.
