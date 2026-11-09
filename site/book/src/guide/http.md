# An HTTP service

The todo example is an HTTP/1.1 backend with JSON bodies. It keeps its todos in memory and
writes every change to a directory, so they survive a restart. A health probe checks it. It
lives in [`examples/todo/`](https://github.com/ignite-lang/ignite/tree/main/examples/todo), in
the clone of [Get the examples](install.md#get-the-examples):

| file | what it holds |
|---|---|
| `todo.ignite` | the handler, the file layout, the system `TodoService` and its tests |
| `health.ignite` | the probe kind `HttpHealth` and its outcome type `HealthCheck` |

## The system

```ignite,fragment
system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource todos: Store<Todo>
  resource loaded: Store<Bool>
  resource data: Directory { path = "todo-data", create = true }

  component server: HttpServer {
    listener = ingress
    application = HttpApp { state = Todos { todos = todos, loaded = loaded, data = data }, handle }
    executable = "todod"
  }

  component health: HttpHealth {
    target = server
    path = "/health"
    interval = 10 seconds
    timeout = 2 seconds
  }

  contract reachable: Availability {
    signal = health.checks
    good = healthy
    target = 99%
    window = rolling(5 minutes)
    missing_data = "insufficient_evidence"
  }
}
```

- **`ingress`** is the listening socket on `127.0.0.1:8080`.
- **`todos`** is a `Store<Todo>`, a map from an `Int` id to a value, shared by every
  connection. `loaded` records whether this process has read the directory yet.
- **`data`** is a [`Directory`](../reference/std/fs.md): every file the service writes is under
  `todo-data`, relative to the working directory, created at launch when missing.
- **`server`** is an `HttpServer` from [`std.protocol.http`](../reference/std/http.md). Its
  `application` pairs a state with a handler, `fn handle(state: Todos, request: Request) ->
  Response`. The server runs one task per connection and calls `handle` for every request.
- **`health`** and **`reachable`** are the probe and the contract over it. `ignite run` uses
  them; the server does not.

The `system` makes two executables: `todod` from `server`, and `health` from the probe.

## The handler

`handle` routes on the method and the path segments:

```ignite,fragment
fn route(state: Todos, request: Request) -> Response = match (request.method, request.segments()) {
  ("GET", ["health"]) => Response.plain(200, "ok\n")
  ("GET", ["todos"]) => list(state)
  ("POST", ["todos"]) => create(state, request.body)
  (_, ["todos"]) => method_not_allowed("GET, POST")
  (method, ["todos", id]) => match Int.parse(id) {
    None => status_response(404)
    Some(n) => match method {
      "GET" => show(state, n)
      "PATCH" => patch(state, n, request.body)
      "DELETE" => delete(state, n)
      _ => method_not_allowed("GET, PATCH, DELETE")
    }
  }
  _ => status_response(404)
}
```

`create` parses the body with [`std.json`](../reference/std/json.md), takes a new id from the
store, puts the todo, writes its file and answers 201:

```ignite,fragment
fn create(state: Todos, body: Text) -> Response = match parse(body) {
  Ok(value) => match value.text("title") {
    Some(title) => {
      let todo = Todo { id = free_id(state.todos), title, done = value.bool("done").unwrap_or(false) }
      state.todos.put(todo.id, todo)
      stored(state, todo, Response.json(201, to_json(todo)))
    }
    None => status_response(400)
  }
  Err(_) => status_response(400)
}
```

`stored` writes the file before the response goes out. A failed write answers 500 `not
stored`. [Keep data in a directory](../howto/directory.md) explains the file layout.

## Build and run

```sh
ignite build examples/todo/todo.ignite
target/ignite/todo/bin/todod
```

```text
built target/ignite/todo/bin/todod
built target/ignite/todo/bin/health
listening on 127.0.0.1:8080
```

In a second terminal:

```sh
curl -s localhost:8080/health
curl -s -X POST localhost:8080/todos -d '{"title":"milk"}'
curl -s -X POST localhost:8080/todos -d '{"title":"eggs"}'
curl -s -X PATCH localhost:8080/todos/2 -d '{"done":true}'
curl -s localhost:8080/todos
```

```text
ok
{"id":1,"title":"milk","done":false}
{"id":2,"title":"eggs","done":false}
{"id":2,"title":"eggs","done":true}
[{"id":1,"title":"milk","done":false},{"id":2,"title":"eggs","done":true}]
```

A deletion answers 204, and the todo is gone:

```sh
curl -s -o /dev/null -w '%{http_code}\n' -X DELETE localhost:8080/todos/1
curl -s -i localhost:8080/todos/1
```

```text
204
HTTP/1.1 404 Not Found
Content-Type: text/plain; charset=utf-8
Content-Length: 10
Connection: keep-alive

Not Found
```

A body that is no JSON is 400, and a method the path does not take is 405 with `Allow`:

```text
$ curl -s -i -X PUT localhost:8080/todos
HTTP/1.1 405 Method Not Allowed
Content-Type: text/plain; charset=utf-8
Allow: GET, POST
Content-Length: 19
Connection: keep-alive

Method Not Allowed
```

## Restart it

Stop `todod` and start it again. The first request loads the newest version of
every todo from `todo-data`:

```sh
curl -s localhost:8080/todos
```

```text
[{"id":2,"title":"eggs","done":true}]
```

Each change is one file, `<shard>/<id>/<version>.json`. A deletion is a tombstone file, so a
deleted id is never given again:

```text
$ find todo-data | sort
todo-data
todo-data/todos
todo-data/todos/1
todo-data/todos/1/2.json
todo-data/todos/2
todo-data/todos/2/2.json
$ cat todo-data/todos/2/2.json
{"id":2,"title":"eggs","done":true,"version":2,"deleted":false}
```

## Test it

The handler is a pure function of the state and the request, so the tests call it without a
socket. `Directory.temporary()` gives each test an empty directory:

```ignite,fragment
test "a todo written is loaded by a fresh state" {
  let data = Directory.temporary()?
  let first = state_over(data)
  assert status(first, "POST", "/todos", r#"{"title":"milk"}"#) == 201
  assert status(first, "POST", "/todos", r#"{"title":"eggs"}"#) == 201
  assert status(first, "PATCH", "/todos/2", r#"{"done":true}"#) == 200
  assert status(first, "DELETE", "/todos/1", "") == 204
  let second = state_over(data)
  assert answer(second, "GET", "/todos", "") == (200, r#"[{"id":2,"title":"eggs","done":true}]"#)
  …
}
```

```sh
ignite test examples/todo/todo.ignite
```

```text
running 9 tests
test examples.todo/to_json ... ok
test examples.todo/handle creates, lists, reads, updates and deletes ... ok
test examples.todo/handle answers 400, 404 and 405 ... ok
test examples.todo/a todo written is loaded by a fresh state ... ok
test examples.todo/ids past 65,536 go to the next shard and load from it ... ok
test examples.todo/saves that finish out of order leave only the newest file ... ok
test examples.todo.health/a check line is the model encoding ... ok
test examples.todo.health/the status is read from the status line ... ok
test examples.todo.health/a path must start with a slash and hold no space ... ok

test result: ok. 9 passed; 0 failed; 0 filtered out; finished in 0.03s
```

The tests of the sibling module `examples.todo.health` run too.

## The health probe

`health` makes one attempt and exits: it sends `GET /health` to the server and prints the
outcome as one line. Started by hand, it needs the server's address as a reference binding:

```sh
target/ignite/todo/bin/health --ref TodoService.server=127.0.0.1:8080
```

```text
{"fields":{"answered":true,"latency":{"ns":318153,"type":"Duration"},"status":200},"type":"examples.todo.health.HealthCheck"}
```

With the server stopped, the attempt still exits 0 and reports `"answered":false` and
`"status":0`. Without `--ref` it exits 1 with `error: no binding for TodoService.server`.

`ignite run` starts the server, binds the reference and runs the probe every 10 seconds. The
contract `reachable` counts an attempt as good when `healthy(check)` holds, that is
`check.answered && check.status == 200`. Driven with `status`, `advance 30 seconds`, `status`
and `stop` on stdin:

```sh
ignite run examples/todo/todo.ignite --clock manual --port TodoService.ingress=0
```

```text
standing TodoService.reachable: Standing { contract: "TodoService.reachable", status: Status.Compliant, good: 4, executed: 4, expected: 4, completeness: 100%, from: 0 ns, to: 30 seconds, coverage: 10%, calculation: "ignite-sli/0.1", version: 57 }
port TodoService.ingress: 33709
```

That is the second `status`, shortened to the standing and the port. Four attempts at 0, 10,
20 and 30 seconds were good, and they cover 10 % of the 5-minute window.
[Write a probe and a contract over it](../howto/probe-contract.md) shows how `HttpHealth` is
written. [Plan, explain, run and replay](operate.md) covers `ignite run` in full.

## Known limits

Writes do not `fsync`: a SIGKILL keeps every acknowledged change, a power loss need not. Two
processes over one `todo-data`, such as the draining instance of a rollout and its
replacement, each give ids and versions from their own memory. A write the old one
acknowledges in the drain window can be lost or hidden from the new one.

Next: [test it](testing.md).
