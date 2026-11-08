# std.net

The network part of the substrate: TCP listeners and streams, in-process pipes for tests, the
`ByteStream` interface that both kinds of stream implement, and the `Endpoint` record. Except
for `Endpoint`, these names are builtins implemented in the substrate (the Rust crate
`ignite-io`), the same code on every target. They are visible in every module without an
import.

- Layer: base (substrate).
- Source: [`std/net.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/net.ignite)
  declares only `Endpoint`; the builtins are specified in
  [`docs/slice.md`](https://github.com/ignite-lang/ignite/blob/main/docs/slice.md) sections 7,
  14.4, 16.9 and 18.3 to 18.4.
- Import: none needed. `import std.net.{TcpStream, ByteStream, Pipe, PipeStream}` is accepted
  and only checks that the names are builtins. `Endpoint` is declared in the file:
  `import std.net.Endpoint`.

The substrate has more builtins: `Stdin`, `Stdout`, `Input`, `IoError`, `Process`, `Clock`
and `log`. The std modules import `IoError` from `std.io`, the second substrate module, which
has no file; an import from either substrate module only checks that the name is a builtin.
`Input` is described here because every stream read returns it.

## Effects

Operations on streams and listeners need effects on the value they are called on. See
[Effects](../effects.md).

| operation | effect it needs |
|---|---|
| `TcpListener.bind(a, p)` | `net.listen` |
| `TcpStream.connect(a, p)` | `net.connect` |
| `x.accept()` | `x.accept` |
| `x.read(n)`, `x.received_batches()`, `x.urgent_pending()` | `x.read` |
| `x.write_all(b)`, `x.close()`, `x.set_idle_timeout(ms)`, `x.write_urgent(b)` | `x.write` |
| `x.local_port()`, `Pipe.pair()` | none |
| `Endpoint.of(ref)` | `process.args` |

`x` stands for each parameter the value comes from. A stream returned by `accept` or
`TcpStream.connect` is owned by the function and needs no effect of its own.

## TcpListener

```ignite,fragment
TcpListener.bind(address: Text, port: Int) -> Result<TcpListener, IoError>
fn accept(listener: TcpListener) -> Result<TcpStream, IoError>
fn local_port(listener: TcpListener) -> Int
```

| operation | what it does |
|---|---|
| `TcpListener.bind` | listen on `address` and `port`; port 0 picks a free port |
| `accept` | the next connection; retries failures that pass with time (out of descriptors, buffers or memory, a connection aborted before it was accepted), pausing 10 ms, doubling up to 1 s |
| `local_port` | the port the listener is bound to |

In a `system`, a listener is a resource with four properties:

```ignite,fragment
resource ingress: TcpListener {
  address = "127.0.0.1"
  port = 2323
  exposure = Exposure.Loopback
  backlog = 128
}
```

`address` and `port` are required. `exposure` defaults to `Loopback` and `backlog` to 128.
A fifth property, `keep_through_drain = true`, keeps the listener open through the SIGTERM
drain: it is not released and accepts until the process exits, so an admin or internal port
keeps answering while the others drain, and a task waiting on it does not hold up the exit.
The generated `main` binds every listener a component uses before it calls the component's
`main`, with the `--port` override and the `listening on` log line.

## Exposure

```ignite,fragment
enum Exposure { Loopback, Private, Public }
```

A builtin enum, written bare (`Loopback`) or qualified (`Exposure.Loopback`). A listener whose
`exposure` is written as `Loopback` with an address that is not a loopback address is E104.

## TcpStream

```ignite,fragment
TcpStream.connect(address: Text, port: Int) -> Result<TcpStream, IoError>
fn read(stream: TcpStream, maximum: Int) -> Result<Input, IoError>
fn write_all(stream: TcpStream, data: Bytes) -> Result<Unit, IoError>
fn close(stream: TcpStream) -> Result<Unit, IoError>
fn set_idle_timeout(stream: TcpStream, ms: Int) -> Result<Unit, IoError>
fn write_urgent(stream: TcpStream, data: Bytes) -> Result<Unit, IoError>
fn urgent_pending(stream: TcpStream) -> Result<Bool, IoError>
```

| operation | what it does |
|---|---|
| `TcpStream.connect` | open a connection |
| `read` | one read of up to `maximum` bytes: `Data(bytes)`, or `Eof` at the end of the stream |
| `write_all` | append `data` to the stream's send buffer |
| `close` | send the buffer and shut down the write half |
| `set_idle_timeout` | a read that waits longer than `ms` milliseconds ends with `Eof` |
| `write_urgent` | send the buffer and `data` at once, the last byte of `data` as TCP urgent data |
| `urgent_pending` | whether an urgent byte has arrived that no read has returned yet; never waits |

A stream is a handle: copies share one connection. A read and a write on the same stream do
not wait for each other, so one task can read while another writes.

**Urgent data.** Urgent data stays in the stream at its place. A read stops before the urgent
byte, and the next read starts with it; `urgent_pending()` is true from its arrival until then.
So after a read that finds `urgent_pending()` true, every byte that read returned came before
the urgent byte. Telnet's Synch is built on this (`std.protocol.telnet`).

**Writes are batched.** `write_all` only appends to a send buffer. The buffer is sent before
the writing task waits for anything (any read, `accept`, `connect`), on `close`, when the task
ends, and when it reaches 64 KiB. A task therefore sends everything it wrote before it waits,
and a server that answers a batch of pipelined requests sends the answers in one system call.
A failed send is reported by the stream's next `read`, `write_all` or `close`.

## Input

```ignite,fragment
enum Input { Data(Bytes), Eof }
```

What a read returns. `Data` and `Eof` are builtin variants.

## ByteStream

`ByteStream` is the one interface of the language. It is used only as a bound on a type
parameter of a function or protocol role, so one function serves a `TcpStream` in a program
and a `PipeStream` in a test:

```ignite,fragment
fn read<S: ByteStream>(stream: S, maximum: Int) -> Result<Input, IoError>
fn write_all<S: ByteStream>(stream: S, data: Bytes) -> Result<Unit, IoError>
fn close<S: ByteStream>(stream: S) -> Result<Unit, IoError>
fn set_idle_timeout<S: ByteStream>(stream: S, ms: Int) -> Result<Unit, IoError>
fn write_urgent<S: ByteStream>(stream: S, data: Bytes) -> Result<Unit, IoError>
fn urgent_pending<S: ByteStream>(stream: S) -> Result<Bool, IoError>
```

- `TcpStream`, `PipeStream` and a type parameter bounded by `ByteStream` implement it.
  Nothing else does.
- Inside the function a value of type `S` has these six methods and `copy`; any other method
  is E041.
- Calling the function with a type that does not implement `ByteStream` is E096.
- `ByteStream` as a type (a parameter, a field, a return type) is E099, and so is a bound
  that names anything else.
- The effects are those of `TcpStream`: `requires [stream.read, stream.write]`.

## Pipe and PipeStream

```ignite,fragment
Pipe.pair() -> (PipeStream, PipeStream)
fn received_batches(stream: PipeStream) -> Int
```

`Pipe.pair()` makes two connected in-process ends: what is written on one end is read on the
other. It may be called only in tests and test-only items (E051 elsewhere). A `PipeStream`
has the six `ByteStream` methods and `received_batches`.

- **Writing** works as on a `TcpStream`: `write_all` appends to the end's send buffer, and the
  writing task delivers it before its next read, on `close`, when it ends, or at once when the
  buffer reaches 64 KiB or the end's last handle is dropped.
- **Delivery** moves the whole buffer to the peer's queue in one step and counts one batch. An
  empty buffer delivers nothing and is not counted. `received_batches()` is the number of
  deliveries into this end since `Pipe.pair()`; reads do not change it. A test can therefore
  check that a server answered pipelined requests in one batch.
- **The queue is bounded** at 16 MiB of unread bytes. A delivery that would pass it fails as a
  whole with the error `pipe queue full`, kept on the writing end. A delivery never blocks.
- **Reading** first delivers the reading task's own writes, then returns up to `maximum`
  bytes (clamped to 1 byte to 1 MiB). It blocks while the queue is empty and the peer is open,
  and returns `Eof` when the queue is empty and the peer has closed, or when the idle timeout
  expires.
- **Closing** delivers the buffer, then closes the end's write half; the peer reads the queue,
  then `Eof`. A `write_all` after `close` is the error `write to a closed stream`. A second
  `close` is not an error.
- **A peer that is gone**: when every handle of one end is dropped, the other end's next
  `read`, `write_all` or `close` returns `pipe peer is gone`.
- `set_idle_timeout(ms)` sets the read timeout of the end; 0 or less removes it.
- **Urgent data** works as on a `TcpStream`: `write_urgent(b)` delivers the buffer and `b` as
  one batch, the last byte of `b` the peer's urgent byte; the peer's reads stop before it, and
  its `urgent_pending()` is true until a read returns it.

A pipe test closes every end whose `Eof` it expects, explicitly: when an end is dropped
differs between the Rust and LLVM targets. `Telnet.serve` and `Http.serve` close their stream
themselves.

## Endpoint

```ignite,fragment
type Endpoint {
  address: Text
  port: Int
}

Endpoint.of(reference: Ref<K>) -> Result<Endpoint, IoError>
```

`Endpoint` is the client-side target of a connection. In a `system`, a bare `TcpListener`
resource name where an `Endpoint` is expected gives that listener's address and port:
`TelnetClient { target = ingress }`.

`Endpoint.of(reference)` is a builtin static. Under `ignite run` it resolves a reference
through the process arguments `--ref <path>=<address>:<port>` that the driver passes to a
probe. Outside a run it fails with `no binding for <path>`. The Telnet probe `TelnetPrompt`
uses it to find its target.

## Examples

```ignite
import std.net.Endpoint

test "an endpoint is an address and a port" {
  let target = Endpoint { address = "127.0.0.1", port = 2323 }
  assert target.address == "127.0.0.1"
  assert target.port == 2323
}

test "an empty write delivers nothing" {
  let (a, b) = Pipe.pair()
  a.write_all(Bytes.empty())?
  _ = a.close()
  b.set_idle_timeout(5000)?
  assert b.read(10)? == Eof
  assert b.received_batches() == 0
}
```

A function generic over `ByteStream`, used here with a pipe end:

```ignite
// Everything `client` reads until the end of the stream.
fn read_to_end<T: ByteStream>(client: T) -> Result<Text, IoError> requires [client.read] {
  var out = Bytes.empty()
  loop {
    match client.read(4096)? {
      Eof => return Ok(out.to_text())
      Data(bytes) => out.append(bytes)
    }
  }
}

test "close delivers the buffer, then the peer reads Eof" {
  let (a, b) = Pipe.pair()
  b.set_idle_timeout(5000)?
  a.write_all(b"hi")?
  _ = a.close()
  assert read_to_end(b)? == "hi"
  assert b.received_batches() == 1
}
```
