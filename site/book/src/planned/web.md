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

That is the whole program. It serves:

| what | where it comes from |
|---|---|
| `GET` and `POST /todos`, `GET`, `PATCH` and `DELETE /todos/{id}` with JSON bodies | `collection(todos)` and the type `Todo` |
| 400 naming the bad member, 404, 405 with `Allow`, all as `application/problem+json` | the route table |
| an HTML page at `/` that updates in every open browser when a todo is added | `page(board)`, because `board` reads `todos` |
| `ETag` headers and `304 Not Modified` | the stores each route reads |
| a `/health` route, a probe and an availability contract that `ignite run` evaluates | `availability = 99%` |
| an OpenAPI 3.1 document | the route table, printed at check time |

## Why only Ignite can do this

Ignite tracks effects. A function that reads a store says so in its type, and a handler can
only reach what it is given: no clock, no global, no file it was not handed. That has three
consequences no other web framework can rely on.

- **Live pages without subscriptions.** The compiler knows that `board` reads `todos`. After
  any write to `todos`, from any route, the server renders `board` again and sends it to every
  open page. There is no subscribe and no broadcast to write, and none to forget.
- **Caching that is correct by construction.** The answer to a GET depends only on the request
  and the stores it reads, so each store's version is the ETag.
- **The app is a system member.** `ignite plan` shows the routes and the derived probe and
  contract, `ignite run` probes the service and reports whether it meets 99 %, and
  `ignite apply` rolls it out ([Plan, explain, run and replay](../guide/operate.md)).

## Markup is checked

Markup is part of the language: elements, attributes, children and `{expressions}`. Text is
escaped unless you say otherwise, a capitalised tag such as `<Card title="x"/>` calls the
function `Card`, and every element and attribute name is checked against the HTML vocabulary.
The page is also checked against the routes: `<input name="titel">` or `action="/todo"` is a
compile error, because no route takes them.

## Exceptions are plain functions

When a route is more than a collection, write it. A validation rule and a custom action:

```ignite,fragment
given todos: Store<Todo>

fn titled(new: New<Todo>) = if new.title.is_empty() { Err(bad_request("title is empty")) } else { Ok(()) }
fn complete(id: Int) = todos.update(id, |t| Todo { done = true, ..t })?
```

with the routes

```ignite,fragment
routes = {
  "/todos" = collection(todos, check = titled)
  "/todos/{id}/done" = post(complete)
}
```

`given` makes `todos` a parameter of every function that uses it, so its effects stay visible.
Path parameters bind by name (`{id}` to `id: Int`), the body and query decode into typed
parameters, `?` on a missing value answers 404, and a GET handler that writes is a compile
error. A service with no collection at all is written as annotated functions:

```ignite,fragment
@get("/todos/{id}") fn show(id: Int) = todos.get(id)?
@patch("/todos/{id}") fn change(id: Int, c: Patch<NewTodo>) = todos.update(id, |t| t.with(c))?
```

## Testing without a socket

```ignite,fragment
test "create and read" {
  let api = client(TodoService.server, Pipe.pair())
  let made: Todo = api.post("/todos", New<Todo> { title = "milk" })?
  assert api.get("/todos/{made.id}").status == 200
  assert api.get("/").body.contains("milk")
}
```

The test client speaks HTTP/1.1 over an in-memory pipe through the same parser, framing and
limits as a real connection.

## How many tokens

The same todo JSON service in four frameworks and in `x.web`, counted with the `cl100k_base`
tokenizer, imports included, comments and indentation not. The other programs are written from
their documentation's idioms; all are in the design document.

| | FastAPI | Hono | Axum | Go `net/http` | `x.web`, handlers written | `x.web`, declared |
|---|---|---|---|---|---|---|
| tokens | 296 | 348 | 563 | 620 | 230 | 89 |
| lines | 44 | 38 | 77 | 102 | 20 | 12 |

The live page: the whole Ignite program above, API included, is 176 tokens; the Phoenix
LiveView module for the same page alone is 214. The durable todo example of this repository,
with its health probe and contract, goes from 1,883 tokens to 179.

## Plan

The framework needs language extensions: derived codecs, record types derived from others
(`New<T>`, `Patch<T>`), members a component derives into the system model, markup literals,
effect sets readable at check time, `given`, route annotations and pattern literals. A first
version ships on today's language with hand-built routes, and each extension shortens it. The
order and the open questions are in
[`docs/web.md`](https://github.com/ignite-lang/ignite/blob/main/docs/web.md) sections 5, 8 and
10; the [roadmap](../roadmap.md) lists it with the other designs.
