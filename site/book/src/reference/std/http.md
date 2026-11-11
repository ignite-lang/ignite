# std.protocol.http

An HTTP/1.1 server (RFC 9112) over one byte stream, written in Ignite. It reads requests with
a `Content-Length` or chunked body, answers with a text, binary or streamed body, keeps the
connection alive until the client asks to close, and enforces the limits of `HttpLimits`. The
module holds `protocol Http` with its server role, the `Request` and `Response` records,
response helpers, and the component kind `HttpServer`.

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
half shut down, and up to 64 KiB of input drained, each read waiting at most 1 s. The client
then reads the reply instead of a reset. Both limits are `HttpLimits` fields (`drain`,
`drain_timeout`).

The head is parsed strictly, so a proxy in front cannot frame a request differently. Each of
these is 400: a bare CR or LF, a header name that is not a token, such as whitespace before the
colon or a line starting with whitespace, a control byte in a value, and a repeated
`Content-Length` or one that is not 1 to 18 digits.

A request body sent with `Transfer-Encoding: chunked` is decoded before the application sees
it: `Request.body` holds the joined chunks, and the request's headers are those of its head.
Chunk extensions are ignored. Trailer fields are checked like header fields and then
discarded. They are not merged into the headers. A chunk size is compared against the body limit digit by digit,
so a size with any number of digits cannot overflow.

The application's response is replaced by 500 when one of its header fields would break RFC
9112 or RFC 9110:

- a name that is not a token, or a value that holds CR or LF;
- `Transfer-Encoding` or `Content-Length`: Http frames every response itself (see
  [Responses](#responses));
- a repeated name, unless it is a list field of RFC 9110 or RFC 9111 (`Vary`, `Allow`,
  `Cache-Control`, `Connection`, …) or `Set-Cookie`;
- an empty element in a list field, such as `Vary: a,,b`;
- whitespace beside `=` in a parameter of `WWW-Authenticate`, `Proxy-Authenticate`,
  `Authentication-Info` or `Proxy-Authentication-Info`;
- a `Date`, `Last-Modified`, `Expires` or `Retry-After` whose value is not exactly an
  IMF-fixdate such as `Sun, 06 Nov 1994 08:49:37 GMT` (`Retry-After` may also be a number of
  seconds).

HTTP/1.1 keeps the connection unless the request asks to close. HTTP/1.0 closes it unless the
request asks to keep it. `Connection: close` wins over `keep-alive`.

A request-target in absolute form (`GET http://example.com:8080/p?q HTTP/1.1`) is accepted: the
application sees the path and query, and the target's authority as its `host` header in place
of the received `Host`. A `HEAD` request reaches the application as `GET`. The answer carries
the head of its response, with the `Content-Length` of the body, but not the body. A rejected
`HEAD` request gets the head of the rejection only.

## The protocol and the server

```ignite,fragment
type IoResult<T> = Result<T, IoError>

protocol Http {
  fn serve<T: ByteStream, S>(stream: T, limits: HttpLimits, application: HttpApp<S, T>) -> IoResult<Unit> requires [
    stream.read, stream.write, application.*
  ]
}

type HttpApp<S, T> {
  state: S
  handle: fn(state: S, request: Request) -> Response requires [state.*] = not_found
  piece: fn(state: S, request: Request) -> IoResult<Option<Bytes>> requires [state.*] = no_piece
  reply: Option<fn(state: S, request: Request) -> Reply<S, T> requires [state.*]> = None
  on_oversized: Option<fn(state: S, head: Request, limit: ByteSize) -> Response requires [state.*]> = None
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
  application: HttpApp<S, TcpStream>
  executable: Text
  limits: HttpLimits = HttpLimits {}
}

fn HttpServer.main<S>(self: HttpServer<S>) -> IoResult<Unit>
fn serve_connection<T: ByteStream, S>(stream: T, connection: Connection, limits: HttpLimits, application: HttpApp<S, T>, holds: Holds) -> IoResult<Unit>
fn serve_listener<S>(listener: TcpListener, limits: HttpLimits, application: HttpApp<S, TcpStream>) -> IoResult<Unit>
```

- `Http.serve` sets the stream's idle timeout, answers requests until the connection ends,
  and closes the stream. The application sees the connection as `Connection.unknown()`.
- `serve_connection` is `Http.serve` for a connection the application sees as `connection`,
  with its held requests registered in `holds` (see [Holding a request](#holding-a-request)).
- `HttpApp<S, T>` is what a server runs: shared state, and the handler called once per
  request with that state. The handler may use effects on the state, such as a `Store`.
  `piece` gives the body of a streamed response, see [Responses](#responses). It defaults to
  `no_piece`, which ends every body at once. `reply` and `on_oversized` are described in
  [The connection, holds and upgrades](#the-connection-holds-and-upgrades). `T` is the stream
  type a takeover gets: `TcpStream` under `HttpServer`, `PipeStream` over a pipe. `handle`
  defaults to `not_found`, which answers 404, for an application that answers through `reply`.
- `HttpLimits` bounds a connection: the longest head (else 431) and body (else 413), the idle
  time after which the connection is closed, and how much input a lingering close drains, each
  read waiting at most `drain_timeout`.
- `HttpServer.main` is `serve_listener`: it accepts connections and serves each under `limits`
  in one task per connection, the application seeing the stream's `Connection.of`. A
  connection whose handler fails ends its task, and the server goes on accepting. When
  `accept` fails, as it does once a SIGTERM drain began, every held request is answered at
  once and `main` returns while the open connections finish.

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
  connection: Connection = Connection { peer = Endpoint { address = "", port = 0 }, local = Endpoint { address = "", port = 0 } }
}

fn header(request: Request, name: Text) -> Option<Text>
fn segments(request: Request) -> List<Text>
fn keep_alive(request: Request) -> Bool
fn has_connection_option(request: Request, option: Text) -> Bool
fn parse_head(head: Text) -> Option<Request>
```

`path` is the request target without its query, `query` the text after `?`. Header names are
in lower case and values trimmed. `connection` is the connection the request came on, see
[The connection, holds and upgrades](#the-connection-holds-and-upgrades).

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
  content: Content = Inline
}

enum Content { Inline, Binary(Bytes), Streamed }

fn Response.json(status: Int, value: Json) -> Response
fn Response.plain(status: Int, body: Text) -> Response
fn Response.bytes(status: Int, content_type: Text, body: Bytes) -> Response
fn Response.streamed(status: Int, headers: List<(Text, Text)>) -> Response
fn no_piece<S>(state: S, request: Request) -> IoResult<Option<Bytes>>
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
| `Response.bytes` | a binary response: `Content-Type` as given, the bytes sent as they are |
| `Response.streamed` | a streamed response with `headers`: its body is what `HttpApp.piece` returns |
| `no_piece` | the default `HttpApp.piece`: `Ok(None)`, an empty streamed body |
| `status_response` | a plain response whose body is the reason phrase and a newline; 204 has no body |
| `method_not_allowed` | 405 with the `Allow` header listing the methods the target supports |
| `reason` | the reason phrase of every status code RFC 9110 section 15 defines (`HTTP/1.1 403 Forbidden`) and of 428, 429, 431 and 511 (RFC 6585); `Unknown` for any other |
| `checked` | the response, or 500 when one of its header fields is refused (see Behaviour) |
| `render` | the bytes of an `Inline` or `Binary` response: status line, headers, `Content-Length` (not for 204) and `Connection`, then the body |

`content` says where the body is. `Inline`, the default, sends `body`. `Binary(bytes)` sends
the bytes in place of `body`. Both go out with a `Content-Length`, so `Response.plain` and
`Response.json` are unchanged on the wire.

`Streamed` sends a body whose length is not known in advance, such as a process's output while
it runs. `Http.serve` writes the head at once, then calls `HttpApp.piece` with the state and
the request until it returns `Ok(None)`, and sends each piece as it comes:

| the request, the status | framing of the streamed body |
|---|---|
| HTTP/1.1, a status with content | `Transfer-Encoding: chunked`: one chunk per non-empty piece, then the last chunk `0` and an empty trailer section; the connection stays open as usual |
| HTTP/1.0, or a 2xx answer to `CONNECT` | no `Transfer-Encoding`: the pieces as they are, then the connection is closed (`Connection: close`) |
| a 1xx, 204 or 304 status | the head only, without `Transfer-Encoding` or `Content-Length`; `piece` is not called |
| `HEAD` | the head GET would get, without the body; `piece` is not called |

An `Err` from `piece` ends the connection without the last chunk, so the client sees the body
as incomplete. Http never sends a `Transfer-Encoding` in a 1xx or 204 response, in a 2xx
response to `CONNECT`, or to an HTTP/1.0 request (RFC 9112 section 6.1).

## The connection, holds and upgrades

```ignite,fragment
// std.net
type Connection {
  peer: Endpoint
  local: Endpoint
  tls: Bool = false
}

fn Connection.unknown() -> Connection
fn Connection.of(stream: TcpStream) -> Connection

// std.protocol.http
enum Reply<S, T> {
  Respond(Response)
  Hold(Receiver<Response>, Duration, Response)
  Upgrade(Response, fn(state: S, request: Request, stream: T, early: Bytes) -> IoResult<Unit> requires [state.*, stream.read, stream.write])
}

type Holds {
  events: Option<Sender<HoldEvent>>
}

enum HoldEvent { Held(Receiver<Response>), Drain }

fn Holds.none() -> Holds
fn Holds.start() -> Holds
fn drain_holds(holds: Holds) -> Unit
fn not_found<S>(state: S, request: Request) -> Response
```

### The connection

Every request carries the connection it came on as `request.connection`: the peer's address
and port, the local address and port, and whether TLS carries it. Under `HttpServer` that is
`Connection.of(stream)` of the accepted `TcpStream`; a `TcpStream` gives its ends with
`stream.peer()` and `stream.local()`. `Http.serve` knows no address of a stream it is given
and says `Connection.unknown()`: both ends empty, port 0. `serve_connection` takes the view
as an argument, which is how a test serves a pipe as a connection from a given peer. `tls` is
false until a TLS layer exists.

An allow list refuses every peer it does not name:

```ignite
import std.protocol.http.*

fn allow_listed(state: List<Text>, request: Request) -> Response {
  for address in state {
    if address == request.connection.peer.address {
      return Response.plain(200, "welcome\n")
    }
  }
  status_response(403)
}
```

### Holding a request

When `reply` is given it answers each request in place of `handle`. `Respond(response)`
answers at once. `Hold(receiver, deadline, on_timeout)` makes the connection's task wait on a
[`std.sync`](sync.md) receiver: the response sent on its channel is the answer, `on_timeout`
is the answer at the deadline or as soon as the channel is closed. Other connections are not
held up, since each runs in its own task; requests pipelined behind the held one wait for it.
After the wait the receiver is closed, so a `send` to an answered request returns false.

```ignite
import std.protocol.http.*

// Holds every request until a message or 30 s; the senders wait in the state.
fn poll<T>(state: Store<Sender<Response>>, request: Request) -> Reply<Store<Sender<Response>>, T> {
  let (tx, rx) = Channel.bounded(1)
  state.put(state.next_id(), tx)
  Hold(rx, 30 seconds, status_response(204))
}
```

`HttpServer` keeps one `Holds` for all its connections (`Holds.start()`): a register task
that each held receiver is sent to. When the drain begins, `accept` fails and `drain_holds`
tells the register, which closes every held receiver and ends: each held request is answered
at once with what was sent to it or with its `on_timeout`, and so is a request held after that.
`Http.serve` waits with `Holds.none()`, which no drain reaches.

### Taking the connection over

`Upgrade(response, takeover)` writes the head of `response` without a `Connection` field of
Http's own (the application names `Connection: upgrade` and `Upgrade` itself), then calls
`takeover` on the connection's task with the state, the request, the stream and the bytes
already read after the request. The connection is closed when `takeover` returns. A response
`checked` refuses is answered 500 instead, and nothing takes over. The takeover is generic in
the stream, so the same fn serves a pipe in a test and a `TcpStream` under `HttpServer`:

```ignite
import std.protocol.http.*

fn echo<T: ByteStream>(state: Unit, request: Request, stream: T, early: Bytes) -> IoResult<Unit> requires [stream.read, stream.write] {
  stream.write_all(early)?
  loop {
    match stream.read(4096)? {
      Eof => return Ok(())
      Data(bytes) => stream.write_all(bytes)?
    }
  }
}

fn upgrade<T: ByteStream>(state: Unit, request: Request) -> Reply<Unit, T> =
  Upgrade(Response { status = 101, headers = [("Upgrade", "echo"), ("Connection", "upgrade")] }, echo)
```

### A body over the limit

Without `on_oversized`, a body over `HttpLimits.body` is answered 413 and the connection
closed after a lingering close. With it, `on_oversized(state, head, limit)` decides the answer:
`head` is the request without its body, `limit` the body limit. The connection goes on when the
request keeps it, the answer does not say `Connection: close`, and the body has a
`Content-Length`: the server reads and drops the body, then reads the next request. A chunked
body over the limit is answered and the connection closed, since its remaining chunks are
not decoded.

## The lexical grammar

The pure checks the server parses and renders with live in `std.protocol.http.syntax`
([`std/protocol/http/syntax.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/protocol/http/syntax.ignite)),
over `std.text` alone: `is_token`, `trim_ows`, `is_target`, `absolute_form`, `is_host`,
`list_elements`, `is_imf_fixdate`, `request_line`, `hex`, `chunk` and the others. Importing
`std.protocol.http` does not import them, since imports are not re-exported; name the module:
`import std.protocol.http.syntax.{is_token, trim_ows}`. The checks that carry an RFC clause
(`is_field_value`, `parse_head`, …) stay in `std.protocol.http`, whose protocol they count for.

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

The handler routes on the method and the path segments. It is shortened here: the example
also handles `/todos/{id}`.

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
  assert reason(403) == "Forbidden"
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

A streamed body: the state is a pipe whose other end the test writes to, and `piece` reads
the next piece from it. The client reads the head and the first chunk before the second piece
exists:

```ignite
import std.protocol.http.*

fn stream(state: PipeStream, request: Request) -> Response =
  Response.streamed(200, [("Content-Type", "text/plain")])

fn next_piece(state: PipeStream, request: Request) -> IoResult<Option<Bytes>> requires [state.read] =
  match state.read(4096)? {
    Eof => Ok(None)
    Data(bytes) => Ok(Some(bytes))
  }

fn read_at_least(client: PipeStream, length: Int) -> IoResult<Text> requires [client.read] {
  var out = Bytes.empty()
  while out.len() < length {
    match client.read(4096)? {
      Eof => break
      Data(bytes) => out.append(bytes)
    }
  }
  Ok(out.to_text())
}

test "a streamed body is sent in chunks as its pieces come" {
  let (client, server) = Pipe.pair()
  let (control, source) = Pipe.pair()
  client.set_idle_timeout(5000)?
  source.set_idle_timeout(5000)?
  spawn Http.serve(server, HttpLimits {}, HttpApp { state = source, handle = stream, piece = next_piece })
  control.write_all(b"first")?
  client.write_all(b"GET /run HTTP/1.1\r\nHost: h\r\nConnection: close\r\n\r\n")?
  let head = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
  assert read_at_least(client, head.len() + 10)? == head + "5\r\nfirst\r\n"
  control.write_all(b"second")?
  _ = control.close()
  assert read_at_least(client, 16)? == "6\r\nsecond\r\n0\r\n\r\n"
}
```
