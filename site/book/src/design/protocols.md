# Protocols and roles

A protocol in Ignite is ordinary code in the standard library, not a feature of the
compiler or the runtime. `protocol Telnet` and `protocol Http` are written in Ignite, in
[`std/protocol/`](https://github.com/ignite-lang/ignite/tree/main/std/protocol). Their roles
are generic over any byte stream, so the same session code runs over a TCP connection in a
program and over an in-memory pipe in a test. This page explains the protocol form, the
`ByteStream` bound, the limits records and the layering rule that keeps protocols out of
the general layers.

## A protocol is a set of roles

`protocol Name { … }` declares the roles of a protocol over one stream:

| role | parameters | called by |
|---|---|---|
| `serve` | `(stream, application)` or `(stream, limits, application)` | a server kind's `main`, once per accepted connection |
| `connect` | `(stream)` | a client kind's `main` |

Each role declares its `requires`. Any other function inside a protocol, a role without
`requires` or a wrong parameter count is E030. Helpers are ordinary module-level functions
beside the protocol. A role is called as `Telnet.serve(stream, limits, console)`, and the
call is effectful.

The server role of Telnet, from
[`std/protocol/telnet.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/protocol/telnet.ignite):

```ignite,fragment
protocol Telnet {
  fn serve<S: ByteStream>(stream: S, limits: SessionLimits, application: Console) -> IoResult<Unit> requires [
    stream.read, stream.write
  ] {
    stream.set_idle_timeout(limits.idle_timeout.to_ms())?
    let result = session(stream, limits, application)
    _ = stream.close()
    result
  }
}
```

`session` decodes the incoming bytes one at a time (so a command split across reads is
handled), shows the console's prompt, passes each line to the console's `respond` and
renders the reply. The console itself knows nothing about Telnet: it is a `Console` record
from [`std.console`](../reference/std/console.md).

## Generic over the stream

`<S: ByteStream>` bounds a type parameter of a function or role by the one interface the
compiler knows. A value of type `S` then has six methods:

| method | signature | effect on its root |
|---|---|---|
| `read` | `(stream: S, maximum: Int) -> IoResult<Input>` | `read` |
| `write_all` | `(stream: S, data: Bytes) -> IoResult<Unit>` | `write` |
| `close` | `(stream: S) -> IoResult<Unit>` | `write` |
| `set_idle_timeout` | `(stream: S, ms: Int) -> IoResult<Unit>` | `write` |
| `write_urgent` | `(stream: S, data: Bytes) -> IoResult<Unit>` | `write` |
| `urgent_pending` | `(stream: S) -> IoResult<Bool>` | `read` |

`TcpStream`, `PipeStream` and a type parameter bounded by `ByteStream` implement it; nothing
else does. Passing another type is E096. `ByteStream` in a type position (a parameter, a
field, a return type), an unknown interface in a bound, or two interfaces in one bound is
E099. User-declared interfaces are not implemented.

The bound is resolved statically. The generated Rust calls
`Telnet::serve::<ignite_io::TcpStream>` and contains no `dyn ByteStream`; the LLVM target
emits one instance of a bounded function per stream type, and each instance calls only the
runtime entries of its own stream kind.

The roles:

| role | signature |
|---|---|
| `Telnet.serve` | `serve<S: ByteStream>(stream: S, limits: SessionLimits, application: Console) -> IoResult<Unit> requires [stream.read, stream.write]` |
| `Telnet.connect` | `connect<S: ByteStream>(stream: S) -> IoResult<Unit> requires [stream.read, stream.write, stdin.read, stdin.mode, stdout.write, process.exit]` |
| `Http.serve` | `serve<T: ByteStream, S>(stream: T, limits: HttpLimits, application: HttpApp<S>) -> IoResult<Unit> requires [stream.read, stream.write, application.*]` |

## Limits are a record

A server role takes its protocol's limits as a record with defaults, so a declaration can
set them as typed quantities:

```ignite,fragment
type SessionLimits {
  maximum: Int = 1000
  idle_timeout: Duration = 10 minutes
  input_buffer: ByteSize = 16 KiB
  output_buffer: ByteSize = 64 KiB
  slow_reader_deadline: Duration = 30 seconds
  maximum_subnegotiation: ByteSize = 1 KiB
}

type HttpLimits {
  head: ByteSize = 8 KiB
  body: ByteSize = 1 MiB
  idle_timeout: Duration = 30 seconds
  drain: ByteSize = 64 KiB
  drain_timeout: Duration = 1 second
}
```

`Telnet.serve` reads `idle_timeout` and `maximum_subnegotiation`; the other Telnet fields are
declared and validated, without behaviour yet. Every `HttpLimits` field has behaviour: a head
over `head` is answered 431, a body over `body` 413, and a rejected request is followed by a
lingering close that drains up to `drain` bytes for at most `drain_timeout`. The terminal
design file sets `sessions = { idle_timeout = 15 minutes, … }` on its server, and that value
reaches `Telnet.serve` for every connection.

## Kinds that speak a protocol

The protocol module also holds the component kinds that use it. `TelnetServer.main` is the
accept loop, one task per connection:

```ignite,fragment
fn TelnetServer.main(self: TelnetServer) -> IoResult<Unit> {
  let console = console_of(self.application)
  loop {
    let stream = self.listener.accept()?
    spawn Telnet.serve(stream, self.sessions, console)
  }
}
```

A failing session ends its own task; the server goes on accepting. `HttpServer<S>` has the same
accept loop. `TelnetClient` and `TelnetPrompt` are kinds with a `main` too, but they connect
once instead of accepting.
[Components](components.md) explains how a kind with `main` becomes an executable.

## Tests without a network

`Pipe.pair()` returns two connected in-memory streams, `PipeStream`. What one end writes, the
other reads. A pipe behaves like a TCP connection where it matters for protocol tests:

- writes are buffered and delivered together at the writer's next flush (before a read, on
  `close`, at task end), as `TcpStream` coalesces them;
- `received_batches()` counts the deliveries into an end, so a test can assert that three
  pipelined requests arrived in one batch;
- the receive queue is bounded at 16 MiB; a delivery past it fails the writer instead of
  blocking it;
- `Pipe.pair()` is test-only: in a program it is E051, because a program's streams come from
  a listener or a connect.

A protocol of your own is tested the same way:

```ignite
type Greeting {
  text: Text
}

protocol Greet {
  fn serve<S: ByteStream>(stream: S, application: Greeting) -> Result<Unit, IoError> requires [
    stream.read, stream.write
  ] {
    stream.write_all("{application.text}\r\n".to_bytes())?
    stream.close()
  }
}

test "the greeting arrives over a pipe" {
  let (client, server) = Pipe.pair()
  spawn Greet.serve(server, Greeting { text = "hello" })
  let reply = match client.read(4096)? {
    Data(bytes) => bytes.to_text()
    Eof => ""
  }
  assert reply == "hello\r\n"
}
```

The Telnet and HTTP session tests in `std/protocol/` run this way on both targets; one test
in each module stays on loopback TCP so the `TcpStream` instance is exercised too. The
[testing guide](../guide/testing.md#protocol-tests-without-a-network) and the
[terminal walkthrough](../guide/terminal.md) show the console's transcript over a pipe.

## A general layer knows no protocol

The standard library is layered, and a test in the repository enforces it:

| layer | modules | may import |
|---|---|---|
| base | `std.io`, `std.net`, `std.option`, `std.result`, `std.text`, `std.json` | base |
| leaf | `std.sli`, `std.time`, `std.policy` | base, leaf |
| general | `std.control`, `std.contract`, `std.deploy`, `std.console`, `std.cli` | base, leaf, general |
| protocol | every `std.protocol.*` | every layer |

- No module outside `std.protocol.*` imports a protocol module or declares a name that
  contains a protocol's name.
- A kind that speaks one protocol (a server, a client, a probe) lives in that protocol's
  module: `TelnetServer`, `TelnetClient` and `TelnetPrompt` in `std.protocol.telnet`,
  `HttpServer` in `std.protocol.http`.
- The compiler crates and the substrate contain no protocol name. The substrate knows
  sockets, pipes, stdio and tasks, not Telnet or HTTP.

So a new protocol is a new module, not a change to the compiler.
[`docs/layering.md`](https://github.com/ignite-lang/ignite/blob/main/docs/layering.md) has
the full rule.

## What is not built

User `interface` and `impl` declarations, ABNF grammars, and TLS and Unix-socket transports
are a design, not implemented ([roadmap](../roadmap.md)). RFC catalogues and conformance
claims (`std.rfc`, `conforms`, `ignite conformance`) are built and released.

Next: [Components](components.md).
