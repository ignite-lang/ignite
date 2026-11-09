# Protocols and roles

A protocol in Ignite is ordinary code in the standard library, not a feature of the
compiler or the runtime. `protocol Telnet`, `protocol Http` and `protocol JsonRpc` are
written in Ignite, in
[`std/protocol/`](https://github.com/ignite-lang/ignite/tree/main/std/protocol). Their roles
are generic over any byte stream, so the same session code runs over a TCP connection in a
program and over an in-memory pipe in a test. This page explains why protocols take that
shape, and the layering rule that keeps them out of the general layers of the library.

## A protocol is a set of roles

`protocol Name { … }` declares the roles of a protocol over one stream. A server role,
`serve`, is called by a server kind's `main` once per accepted connection. A client role,
`connect`, is called by a client kind's `main`. Each role declares its `requires`, so the
caller pays for every effect the session has on the stream. Helpers are ordinary
module-level functions beside the protocol. Anything else inside a protocol is E030.

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

`session` decodes the incoming bytes one at a time, so a command split across reads is
handled. It shows the console's prompt, passes each line to the console's `respond` and
renders the reply. The console knows nothing about Telnet: it is a `Console` record from
[`std.console`](../reference/std/console.md). Every role's signature is on its module's page:
[`std.protocol.telnet`](../reference/std/telnet.md#the-protocol),
[`std.protocol.http`](../reference/std/http.md#the-protocol-and-the-server).

## Generic over the stream

`<S: ByteStream>` bounds a type parameter by the one interface the compiler knows. A value
of type `S` can be read, written, closed and given an idle timeout, and nothing else. The
role cannot open a socket, so it cannot reach anything but the stream it was given.
`TcpStream`, `PipeStream` and a type parameter bounded by `ByteStream` implement the
interface. Its methods are in [`std.net`](../reference/std/net.md#bytestream).

The bound is resolved statically. The generated Rust calls
`Telnet::serve::<ignite_io::TcpStream>` and contains no `dyn ByteStream`. The LLVM target
emits one instance of a bounded function per stream type, and each instance calls only the
runtime entries of its own stream kind. Being generic costs nothing at run time.

## Limits are a record

A server role takes its protocol's limits as a record with defaults: `SessionLimits` for
Telnet, `HttpLimits` for HTTP. A declaration sets them as typed quantities, and the value
reaches the role for every connection. The terminal design file sets
`sessions = { idle_timeout = 15 minutes, … }` on its server, and `Telnet.serve` applies it.
Which fields have behaviour is on each module's page.

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

A failing session ends its own task, and the server goes on accepting. `HttpServer<S>` has
the same accept loop. `TelnetClient` and `TelnetPrompt` connect once instead of accepting.
[Components](components.md) explains how a kind with `main` becomes an executable.

## Tests without a network

Because a role is generic, a test can hand it one end of `Pipe.pair()`, two connected
in-memory streams, and talk to it from the other end. A pipe coalesces writes as a TCP
connection does, so a test sees the batches a client would. It is test-only, because a
program's streams come from a listener or a connect. Its behaviour is in
[`std.net`](../reference/std/net.md#pipe-and-pipestream). The session tests in
`std/protocol/` run this way on both targets, and one test in each module stays on loopback
TCP. [Write a protocol and test it over a pipe](../howto/protocol.md) shows the steps.

## A general layer knows no protocol

The standard library is layered, and a test in the repository enforces the layers. Base
modules such as `std.net` and `std.text` import only base modules. The general modules,
such as `std.control` and `std.deploy`, import no protocol module. Every `std.protocol.*`
module may import every layer. The [standard library](../reference/std/index.md#layers)
lists the modules of each layer.

- No module outside `std.protocol.*` imports a protocol module or declares a name that
  contains a protocol's name.
- A kind that speaks one protocol lives in that protocol's module: `TelnetServer`,
  `TelnetClient` and `TelnetPrompt` in `std.protocol.telnet`, `HttpServer` in
  `std.protocol.http`.
- The compiler and the substrate contain no protocol name. The substrate knows sockets,
  pipes, stdio and tasks, not Telnet or HTTP.

A new protocol is a new module, not a change to the compiler.
[`docs/layering.md`](https://github.com/ignite-lang/ignite/blob/main/docs/layering.md) has
the full rule.

## Protocols and their specifications

A protocol can name the specifications it follows: `protocol Telnet conforms [RFC854,
RFC855]`. A function states the requirements it implements with `satisfies`, and a test the
requirements it checks with `verifies`. `ignite conformance` runs those tests and prints
whether each claim holds. [Check a protocol against its RFC](../howto/conformance.md) shows the steps, and
[`std.rfc`](../reference/std/rfc.md) holds the catalogues.

User `interface` and `impl` declarations, ABNF grammars, and TLS and Unix-socket transports
are designs, not implemented ([roadmap](../roadmap.md)).

Next: [Components](components.md).
