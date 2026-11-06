# std.protocol.http

An HTTP/1.1 server (RFC 9112) over one byte stream, written in Ignite. It reads requests with
a `Content-Length` body, keeps the connection alive until the client asks to close, and
enforces the limits of `HttpLimits`. The module holds `protocol Http` with its server role, the
`Request` and `Response` records, response helpers, and the component kind `HttpServer`.

- Layer: protocol.
- Source: [`std/protocol/http.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/protocol/http.ignite).
- Import: `import std.protocol.http.*`, or by name:
  `import std.protocol.http.{HttpServer, HttpApp, Request, Response, status_response}`.

## Behaviour

With the default limits:

| the client sends | the server answers |
|---|---|
| a request line over 8 KiB (its request-target too long) | 414 |
| a head (request line and fields) over 8 KiB | 431 |
| a body over 1 MiB, with `Content-Length` or decoded from chunks | 413 |
| a malformed request | 400 |
| an HTTP/1.1 request without `Host`, or any request with two `Host` lines or an invalid one | 400 |
| a `Transfer-Encoding` whose final coding is not `chunked`, `chunked` twice, both `Transfer-Encoding` and `Content-Length`, or `Transfer-Encoding` in HTTP/1.0 | 400 |
| a coding other than `chunked` before the final `chunked` (`gzip, chunked`) | 501 |
| a chunk-size line or a trailer section over 8 KiB | 431 |
| nothing for 30 s | the connection is closed |

Each rejection closes the connection after a lingering close: the reply is written, the write
half shut down, and up to 64 KiB of input drained, each read waiting at most 1 s, so the
client reads the reply instead of a reset. Both limits are `HttpLimits` fields (`drain`,
`drain_timeout`).

The head is parsed strictly, so a proxy in front cannot frame a request differently: a bare CR
or LF, a header name that is not a token (whitespace before the colon, a line starting with
whitespace), a control byte in a value, and a repeated `Content-Length` or one that is not 1
to 18 digits are all 400.

A request body sent with `Transfer-Encoding: chunked` is decoded before the application sees
it: `Request.body` holds the joined chunks, and the request's headers are those of its head.
Chunk extensions are ignored; trailer fields are checked like header fields and then discarded,
never merged into the headers. A chunk size is compared against the body limit digit by digit,
so a size with any number of digits cannot overflow.

The application's response is replaced by 500 when one of its header fields would break RFC
9112 or RFC 9110:

- a name that is not a token, or a value that holds CR or LF;
- `Transfer-Encoding` or `Content-Length`: Http frames every response by its own
  `Content-Length` and applies no transfer coding;
- a repeated name, unless it is a list field of RFC 9110 or RFC 9111 (`Vary`, `Allow`,
  `Cache-Control`, `Connection`, …) or `Set-Cookie`;
- an empty element in a list field, such as `Vary: a,,b`;
- whitespace beside `=` in a parameter of `WWW-Authenticate`, `Proxy-Authenticate`,
  `Authentication-Info` or `Proxy-Authentication-Info`;
- a `Date`, `Last-Modified`, `Expires` or `Retry-After` whose value is not exactly an
  IMF-fixdate such as `Sun, 06 Nov 1994 08:49:37 GMT` (`Retry-After` may also be a number of
  seconds).

HTTP/1.1 keeps the connection unless the request asks to close; HTTP/1.0 closes it unless the
request asks to keep it. `Connection: close` wins over `keep-alive`.

A request-target in absolute form (`GET http://example.com:8080/p?q HTTP/1.1`) is accepted: the
application sees the path and query, and the target's authority as its `host` header in place
of the received `Host`. A `HEAD` request reaches the application as `GET`; the answer carries
the head of its response, with the `Content-Length` of the body, but not the body. A rejected
`HEAD` request gets the head of the rejection only.

## The protocol and the server

```ignite,fragment
type IoResult<T> = Result<T, IoError>

protocol Http {
  fn serve<T: ByteStream, S>(stream: T, limits: HttpLimits, application: HttpApp<S>) -> IoResult<Unit> requires [
    stream.read, stream.write, application.*
  ]
}

type HttpApp<S> {
  state: S
  handle: fn(state: S, request: Request) -> Response requires [state.*]
}

type HttpLimits {
  head: ByteSize = 8 KiB
  body: ByteSize = 1 MiB
  idle_timeout: Duration = 30 seconds
  drain: ByteSize = 64 KiB
  drain_timeout: Duration = 1 second
}

type HttpServer<S> {
  listener: TcpListener
  application: HttpApp<S>
  executable: Text
  limits: HttpLimits = HttpLimits {}
}

fn HttpServer.main<S>(self: HttpServer<S>) -> IoResult<Unit>
```

- `Http.serve` sets the stream's idle timeout, answers requests until the connection ends,
  and closes the stream.
- `HttpApp<S>` is what a server runs: shared state, and the handler called once per request
  with that state. The handler may use effects on the state, such as a `Store`.
- `HttpLimits` bounds a connection: the longest head (else 431) and body (else 413), the idle
  time after which the connection is closed, and how much input a lingering close drains, each
  read waiting at most `drain_timeout`.
- `HttpServer.main` accepts connections and runs `Http.serve` under `limits` in one task per
  connection. A connection whose handler fails ends its task, and the server goes on
  accepting.

The constants `READ_SIZE` (65536, the size of one read) and `MAXIMUM_LENGTH_DIGITS` (18, the
longest `Content-Length` value) are framing facts, not limits.

## Requests

```ignite,fragment
type Request {
  method: Text
  path: Text
  query: Text
  version: Text
  headers: List<(Text, Text)>
  body: Text
}

fn header(request: Request, name: Text) -> Option<Text>
fn segments(request: Request) -> List<Text>
fn keep_alive(request: Request) -> Bool
fn has_connection_option(request: Request, option: Text) -> Bool
fn parse_head(head: Text) -> Option<Request>
```

`path` is the request target without its query, `query` the text after `?`. Header names are
in lower case and values trimmed.

| function | what it does |
|---|---|
| `header` | the value of header `name` (lower case), the first if repeated |
| `segments` | the non-empty segments of the path: `"/todos/3"` is `["todos", "3"]` |
| `keep_alive` | whether the connection stays open after this request |
| `has_connection_option` | whether a `Connection` header lists `option` (lower case); the value is a comma-separated list and the header may repeat |
| `parse_head` | the request line and header lines of a head, without the blank line; `None` when malformed |

## Responses

```ignite,fragment
type Response {
  status: Int
  headers: List<(Text, Text)> = []
  body: Text = ""
}

fn Response.json(status: Int, value: Json) -> Response
fn Response.plain(status: Int, body: Text) -> Response
fn status_response(status: Int) -> Response
fn method_not_allowed(allow: Text) -> Response
fn reason(status: Int) -> Text
fn checked(response: Response) -> Response
fn render(response: Response, keep_alive: Bool) -> Bytes
```

| function | what it does |
|---|---|
| `Response.json` | a JSON response: `Content-Type: application/json`, the body [`encode`](json.md)d |
| `Response.plain` | a plain-text response: `Content-Type: text/plain; charset=utf-8` |
| `status_response` | a plain response whose body is the reason phrase and a newline; 204 has no body |
| `method_not_allowed` | 405 with the `Allow` header listing the methods the target supports |
| `reason` | the reason phrase of 200, 201, 204, 400, 404, 405, 413, 414, 431, 500 and 501; `Unknown` for any other |
| `checked` | the response, or 500 when one of its header fields is refused (see Behaviour) |
| `render` | the bytes of the response: status line, headers, `Content-Length` (not for 204) and `Connection` |

## A server in a system

From the [todo example](https://github.com/ignite-lang/ignite/blob/main/examples/todo/todo.ignite),
where `handle` is `fn handle(todos: Store<Todo>, request: Request) -> Response`:

```ignite,fragment
system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource todos: Store<Todo>

  component server: HttpServer {
    listener = ingress
    application = HttpApp { state = todos, handle }
    executable = "todod"
  }
}
```

The handler routes on the method and the path segments (shortened here; the example also
handles `/todos/{id}`):

```ignite,fragment
fn handle(todos: Store<Todo>, request: Request) -> Response = match (request.method, request.segments()) {
  ("GET", ["todos"]) => list(todos)
  ("POST", ["todos"]) => create(todos, request.body)
  (_, ["todos"]) => method_not_allowed("GET, POST")
  _ => status_response(404)
}
```

## Examples

The request and response helpers are pure:

```ignite
import std.protocol.http.*
import std.json.Json

test "responses carry their reason, type and body" {
  assert reason(431) == "Request Header Fields Too Large"
  assert Response.plain(200, "hi") == Response { status = 200, headers = [("Content-Type", "text/plain; charset=utf-8")], body = "hi" }
  assert status_response(404).body == "Not Found\n"
  let created = render(Response.json(201, Object([])), true).to_text()
  assert created == "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\n" + r"{}"
}

test "parse_head reads the request line and headers" {
  let request = parse_head("POST /todos?x=1&y HTTP/1.1\r\nHost: example:80")?
  assert request.path == "/todos"
  assert request.query == "x=1&y"
  assert request.header("host") == Some("example:80")
  assert request.segments() == ["todos"]
  assert request.keep_alive()
  assert parse_head("GET / HTTP/2") == None
}
```

A server over a pipe, as the module's own tests run it:

```ignite
import std.protocol.http.*

// Answers `<method> <path>?<query> <body>`.
fn answer(state: Unit, request: Request) -> Response =
  Response.plain(200, "{request.method} {request.path}?{request.query} {request.body}")

fn reply(body: Text, connection: Text) -> Text {
  let length = body.len()
  "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {length}\r\nConnection: {connection}\r\n\r\n{body}"
}

test "three pipelined requests over a pipe are answered in one batch" {
  let (client, server) = Pipe.pair()
  client.set_idle_timeout(5000)?
  server.set_idle_timeout(5000)?
  spawn Http.serve(server, HttpLimits {}, HttpApp { state = (), handle = answer })
  client.write_all(b"GET /a HTTP/1.1\r\nHost: h\r\n\r\nGET /b HTTP/1.1\r\nHost: h\r\n\r\nGET /c HTTP/1.1\r\nHost: h\r\nConnection: close\r\n\r\n")?
  var out = Bytes.empty()
  loop {
    match client.read(4096)? {
      Eof => break
      Data(bytes) => out.append(bytes)
    }
  }
  assert out.to_text() == reply("GET /a? ", "keep-alive") + reply("GET /b? ", "keep-alive") + reply("GET /c? ", "close")
  assert client.received_batches() == 1
}
```
