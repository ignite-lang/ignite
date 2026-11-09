# std.protocol.jsonrpc

JSON-RPC 2.0 over one byte stream, written in Ignite. Messages are framed as in the Language
Server Protocol's base protocol: a header of `Name: value` lines ended by an empty line, then
exactly `Content-Length` bytes of UTF-8 JSON. The module holds the framing, the messages and
their encoding through [std.json](json.md), and `protocol JsonRpc` with its server role.

- Layer: protocol.
- Source: [`std/protocol/jsonrpc.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/protocol/jsonrpc.ignite).
- Import: `import std.protocol.jsonrpc.*`, or by name:
  `import std.protocol.jsonrpc.{JsonRpc, JsonRpcApp, JsonRpcLimits, Request, ErrorObject, standard_error}`.

`JsonRpc` declares `conforms [JSONRPC_2]` for the parties a server is
([std.rfc](rfc.md)). Batches are not supported: a batch that holds a request is answered with
one invalid-request error, and a batch of notifications only is passed to the application and
not answered.

## The protocol

```ignite,fragment
protocol JsonRpc conforms [JSONRPC_2] scope { parties = [Server, Any] } {
  fn serve<T: ByteStream, S>(stream: T, limits: JsonRpcLimits, application: JsonRpcApp<S>) -> IoResult<Unit> requires [stream.read, stream.write, application.*]
}

type JsonRpcApp<S> {
  state: S
  call: fn(state: S, request: Request) -> Result<Json, ErrorObject> requires [state.*]
  notify: fn(state: S, notification: Notification) -> Unit requires [state.*]
}

type JsonRpcLimits {
  header: ByteSize = 8 KiB
  body: ByteSize = 16 MiB
}
```

`serve` reads messages until the stream ends or a framing error, then closes the stream.

- A request goes to `call`. Its outcome is sent as the response under the request's id, so
  every request is answered.
- A notification goes to `notify` and is never answered.
- A body that is not UTF-8 JSON, or no message, is answered with its error under the id it
  holds, or a null id when none can be read.
- A response from the peer is dropped: the server sends no requests of its own.

`JsonRpcLimits` bounds one message: `header` holds the header with its empty line, `body` the
body.

## Framing

```ignite,fragment
enum FrameError { MissingLength, InvalidLength, MalformedHeader, HeaderTooLarge, BodyTooLarge, Truncated }
enum Frame { Got(Bytes, Bytes), Closed, Failed(FrameError) }
enum Split { Framed(Frame), Partial }

fn frame(body: Text) -> Bytes
fn write_message<T: ByteStream>(stream: T, body: Text) -> IoResult<Unit>
fn read_message<T: ByteStream>(stream: T, limits: JsonRpcLimits, buffer: Bytes) -> IoResult<Frame>
fn split_frame(data: Bytes, limits: JsonRpcLimits) -> Split
```

| function | what it does |
|---|---|
| `frame` | `Content-Length: <n>\r\n\r\n` and `body`, `n` its length in bytes |
| `write_message` | writes `frame(body)` to the stream |
| `read_message` | the next message, starting with the bytes already read in `buffer`: `Got(body, rest)`, `Closed` at the end of the stream between messages, or `Failed(error)` |
| `split_frame` | the framing of `read_message` over bytes already read, for a reader that is no `ByteStream` such as standard input: `Framed(frame)`, or `Partial` when more bytes are needed |

| error | cause |
|---|---|
| `MissingLength` | the header has no `Content-Length` |
| `InvalidLength` | a `Content-Length` that is not 1 to 18 decimal digits, or a second one |
| `MalformedHeader` | a header line without `:` |
| `HeaderTooLarge`, `BodyTooLarge` | past `limits.header` or `limits.body` |
| `Truncated` | the stream ended inside a message |

`Content-Type` and every other header are read and ignored. After a framing error the stream
cannot be resynchronised.

## Messages

```ignite,fragment
enum Id { NumberId(Int), TextId(Text), DecimalId(Text) }

type Request {
  id: Id
  method: Text
  params: Option<Json> = None
}

type Notification {
  method: Text
  params: Option<Json> = None
}

type ErrorObject {
  code: Int
  message: Text
  data: Option<Json> = None
}

type Response {
  id: Option<Id>
  outcome: Result<Json, ErrorObject>
}

enum Message { Call(Request), Notify(Notification), Reply(Response) }

fn encode_message(message: Message) -> Text
fn decode(text: Text) -> Result<Message, ErrorObject>
fn decode_body(body: Bytes) -> Result<Message, ErrorObject>
fn is_utf8(bytes: Bytes) -> Bool
fn standard_error(code: Int) -> ErrorObject
```

| function | what it does |
|---|---|
| `encode_message` | the JSON text of `message`, `"jsonrpc":"2.0"` first, the body `write_message` frames |
| `decode` | the message of a text: the parse error when it is not JSON, the invalid-request error when it is no message |
| `decode_body` | as `decode` for a framed body; a body that is not UTF-8 is the parse error |
| `is_utf8` | whether `bytes` is well-formed UTF-8; `Bytes.to_text()` is lossy, so a body is checked first |
| `standard_error` | the error object of a standard code, with the specification's message |

An id that is a number with a fraction, an exponent, `-0` or beyond `Int` is a `DecimalId` of
its exact text, so the response carries it as sent. A response to a request whose id could not
be read has none, written `null`.

| constant | code | message |
|---|---|---|
| `PARSE_ERROR` | -32700 | `Parse error` |
| `INVALID_REQUEST` | -32600 | `Invalid Request` |
| `METHOD_NOT_FOUND` | -32601 | `Method not found` |
| `INVALID_PARAMS` | -32602 | `Invalid params` |
| `INTERNAL_ERROR` | -32603 | `Internal error` |

`standard_error` of any other code has the message `Server error`.

## Examples

```ignite
import std.protocol.jsonrpc.*
import std.json.Json
import std.option.unwrap_or

test fn answer(state: Unit, request: Request) -> Result<Json, ErrorObject> = if request.method == "echo" {
  Ok(request.params.unwrap_or(Null))
} else {
  Err(standard_error(METHOD_NOT_FOUND))
}

test fn ignore(state: Unit, notification: Notification) -> Unit = ()

// Every body `client` reads until the server closes.
test fn replies_of(client: PipeStream) -> IoResult<List<Text>> {
  var replies = []
  var buffer = Bytes.empty()
  loop {
    match read_message(client, JsonRpcLimits {}, buffer)? {
      Got(body, rest) => {
        replies.push(body.to_text())
        buffer = rest
      }
      _ => return Ok(replies)
    }
  }
}

test "serve answers each request and no notification" {
  let (client, server) = Pipe.pair()
  client.set_idle_timeout(5000)?
  spawn JsonRpc.serve(server, JsonRpcLimits {}, JsonRpcApp { state = (), call = answer, notify = ignore })
  write_message(client, r#"{"jsonrpc":"2.0","id":1,"method":"echo","params":["hi"]}"#)?
  write_message(client, r#"{"jsonrpc":"2.0","method":"note"}"#)?
  write_message(client, r#"{"jsonrpc":"2.0","id":"b","method":"nope"}"#)?
  _ = client.close()
  assert replies_of(client)? == [
    r#"{"jsonrpc":"2.0","id":1,"result":["hi"]}"#,
    r#"{"jsonrpc":"2.0","id":"b","error":{"code":-32601,"message":"Method not found"}}"#
  ]
}

test "a message round-trips through its text" {
  let call = Call(Request { id = TextId("a"), method = "shutdown" })
  assert decode(encode_message(call)) == Ok(call)
  assert frame(r"{}") == b"Content-Length: 2\r\n\r\n{}"
}
```
