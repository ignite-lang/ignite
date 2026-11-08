# x.web: a web framework <span class="badge design">design</span>

`x.web` is a planned web framework for HTTP JSON services. It will ship with Ignite, in a
second module root `x` next to `std`, and be versioned with the compiler. Nothing on this page
is implemented yet; the code below is a sketch and does not compile today. The full design,
with every decision, its measurements and the planned steps, is
[`docs/web.md`](https://github.com/ignite-lang/ignite/blob/main/docs/web.md).

The aim: a typical web service in as few source tokens as possible, written by a person or an
agent, while the compiler still catches the errors and the service stays explicit about its
effects, its limits and its wire format.

## The todo service in 30 seconds

```ignite,fragment
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

What this checks with no line written for it:

| case | answer |
|---|---|
| a body that is not a `NewTodo` | 400, a problem naming the member, before `create` runs |
| `/todos/abc` | 404: `id` is an `Int`, so no route matches |
| `PUT /todos` | 405 with `Allow: GET, POST`, taken from the table |
| two routes for `GET /todos/:id` | an error at `ignite check` |
| `show` writing to the store | an error at `ignite check`: a GET handler may only read |
| a handler reading the clock | an error at `ignite check`: a handler has only the authority its state holds |

## Routes are data

The second argument of `web` is a list of routes, one value per endpoint. `get`, `post`, `put`,
`patch` and `delete` build a route from a path pattern and a handler; `:name` marks a path
parameter. Because the table is a value, `ignite check` reads it: a duplicate route, a route an
earlier one hides, a malformed pattern or a path parameter the handler does not take is a
diagnostic, not a surprise at run time. `scope("/admin", layers, routes)` groups routes under a
prefix with their own middleware.

## Handlers are plain functions

A handler takes the state first, then its parameters, then the body for `post`, `put` and
`patch`, and returns `Reply<T>`, which is `Result<T, Fail>`. Nothing marks a function as a
handler, so a test can call it directly.

| route | handler type | success |
|---|---|---|
| `get(path, h)` | `fn(state: S, params: P) -> Reply<O>`, read-only | 200 |
| `post(path, h)` | `fn(state: S, params: P, body: B) -> Reply<O>` | 201 |
| `put`, `patch` | as `post` | 200 |
| `delete(path, h)` | `fn(state: S, params: P) -> Reply<O>` | 200, or 204 when `O` is `Unit` |

`status = 202` on the route changes the success status. Parameters come from the path and the
query by name; a route lists the headers it reads (`headers = ["if-none-match"]`), so a query
parameter can never stand in for a header.

## Effects stay visible

A handler states what it touches by what its state holds. The state above is a
`Store<Todo>`; a larger service passes a record of its resources, such as a store, a
`Directory` and a `Launcher`. These are the same effect rules as everywhere in Ignite
([Effects and capabilities](../design/effects.md)): no handler reaches the clock, standard
output or a file it was not given. GET handlers are typed read-only, so writing from one is a
compile error.

## Bodies use std.data codecs

Request and response bodies go through the codecs of the planned `std.data` module: one value
model for JSON, YAML and TOML, and a codec the compiler derives for each type. A field with a
default is optional on input, an `Option` field may be absent, and a decoding error names the
path (`$.title`) and what was expected. The body size is bounded by the server's `HttpLimits`
before the framework sees it.

## One error shape

Every error is `application/problem+json` (RFC 9457): `type`, `title`, `status`, `detail`,
and for a body that does not decode, the list of invalid members. The framework writes 404,
405, 400, 413 and 415 itself. A handler returns `Err(not_found())`, `Err(conflict("…"))` or
`Err(fail(500, "…"))`; `found()` turns a missing value into a 404.

## Middleware is a value

A layer is a record of two functions, `before` and `after`, that get the state, so it has the
same authority as the handlers and no more. `bearer(valid)` checks a token, `max_body(16 KiB)`
narrows the body limit, `before(f)` runs a check first. Access logging is a property of the
`HttpServer` component, because writing to standard error is not an effect a handler holds.

## Testing without a socket

```ignite,fragment
import x.web.client.{client}

test "create and read" {
  let api = client(web(Store.new(), routes()), Pipe.pair())
  assert api.post("/todos", r#"{"title":"milk"}"#).status == 201
  assert api.get("/todos/1").body == r#"{"id":1,"title":"milk","done":false}"#
}
```

The test client speaks HTTP/1.1 over an in-memory pipe, so a test runs through the same
parser, framing and limits as a real connection, with no port and no network.

## OpenAPI from the table

The framework writes an OpenAPI 3.1 document from the route table at check time, with no build
and no running server:

```text
ignite explain todo.ignite TodoService.server.application.description --raw > openapi.json
```

## Static files and streaming

`files("/assets", |s| s.assets)` serves a `Directory`; paths that leave it, absolute paths and
symbolic links are refused by the directory itself. `stream(path, start, next)` sends a body in
pieces as they come, chunked, for long responses and server-sent events.

## Still an HttpServer

`web(state, routes)` returns an ordinary `HttpApp`, and the component stays `HttpServer`. So
`ignite plan`, `ignite explain`, `ignite run` and `ignite apply` work as they do for any HTTP
service ([Plan, explain, run and replay](../guide/operate.md)).

## How many tokens

The same in-memory todo service in four frameworks and in Ignite, counted with the
`cl100k_base` tokenizer, imports included, comments and indentation not. The other programs
are written from their documentation's idioms; all of them are in the design document.

| | Ignite today | FastAPI | Hono | Axum | Go `net/http` | `x.web` |
|---|---|---|---|---|---|---|
| tokens | 592 | 296 | 348 | 563 | 620 | 362 |
| lines | 68 | 44 | 38 | 77 | 102 | 31 |

The `x.web` column needs two compiler features beyond the framework: codecs derived by the
compiler, and parameter defaults. With the framework alone and hand-written codecs the service
is 639 tokens, longer than today but fully checked; derived codecs bring it to 444. Ignite
writes a few tokens the others leave to a command line: the listener's address and port, the
store and the executable's name.

## Plan

The framework comes after three smaller steps: the module root `x` in the compiler and the
release archive, the value model and codecs of `std.data`, and two fields on `HttpApp` for the
route checks and the OpenAPI document. The order and the open questions are in
[`docs/web.md`](https://github.com/ignite-lang/ignite/blob/main/docs/web.md) sections 11 and 12;
the [roadmap](../roadmap.md) lists it with the other designs.
