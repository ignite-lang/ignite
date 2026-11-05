# Effects

An effect is something a function does to the world beyond computing its result: reading
stdin, writing a socket, accepting a connection, ending the process. Ignite tracks effects in
the type system. A function that performs none is **pure**; a function that performs some
says which, or the compiler works it out. Why the language is built this way is in
[Effects and capabilities](../design/effects.md).

## Effect names

An effect is a dotted name. It has one of two roots.

**Ambient effects** act on the process itself, through a builtin owner:

| effect | operations |
|---|---|
| `stdin.read` | `Stdin.read(n)` |
| `stdin.mode` | `Stdin.is_terminal()`, `Stdin.set_raw(on)` |
| `stdout.write` | `Stdout.write_all(bytes)` |
| `process.exit` | `Process.exit(code)` |
| `process.args` | `Endpoint.of(ref)` |
| `net.listen` | `TcpListener.bind(address, port)` |
| `net.connect` | `TcpStream.connect(address, port)` |
| `clock.read` | `Clock.monotonic()` |

**Parameter effects** act on a value the function was given. The root is the parameter's
name:

| effect | operations on the parameter `x` |
|---|---|
| `x.read` | `x.read(n)` on a stream, `x.get(id)` and `x.values()` on a `Store`, `x.received_batches()` on a pipe end |
| `x.write` | `x.write_all(b)`, `x.close()`, `x.set_idle_timeout(ms)` on a stream; `x.next_id()`, `x.put(..)`, `x.remove(..)`, `x.update(..)`, `x.upsert(..)` on a `Store` |
| `x.accept` | `x.accept()` on a `TcpListener` |
| `x.*` | every effect on `x` |

`log(text)` writes a line to stderr and returns `Unit`, but it is not a tracked effect: it
needs no `requires` entry and may be called where only pure code is allowed.

## Declaring effects with `requires`

`requires [..]` after the signature lists the effects a function may perform. It must be on
the signature's line.

```ignite
fn say(line: Text) -> Result<Unit, IoError> requires [stdout.write] {
  Stdout.write_all((line + "\n").to_bytes())
}

fn send(stream: TcpStream, line: Text) -> Result<Unit, IoError> requires [stream.write] {
  stream.write_all(line.to_bytes())
}
```

- A function with `requires` may perform only what it lists. An effect it performs and does
  not list is E016.
- Listing an effect the body does not need is allowed.
- Ambient effects are listed by their full name. `requires [net]` is accepted but covers
  nothing: `TcpListener.bind` still needs `net.listen`. `x.*` covers every effect on the root `x`,
  a parameter or an ambient root such as `stdout.*`.
- The list may span lines inside its brackets.
- A parameter may not be named `stdin`, `stdout`, `process` or `net` (E056), so a
  parameter effect never looks like an ambient one.

## Inference

A function without `requires` has its effects inferred from its body: the union of the
effects of everything it calls, computed across the whole program. A function whose inferred
set is empty is pure.

```ignite
fn say(line: Text) -> Result<Unit, IoError> requires [stdout.write] {
  Stdout.write_all((line + "\n").to_bytes())
}

fn greet() -> Result<Unit, IoError> {
  say("hello")?
  say("world")
}
```

`greet` has no `requires`, and its inferred effects are `[stdout.write]`. E016 is reported
only in functions that declare `requires`: a caller with `requires` that calls `greet`
must list `stdout.write`.

## How effects follow arguments

A parameter effect of a callee is charged to the caller through the argument it was given:

- When `send(stream, ..)` requires `stream.write` and the caller passes its own parameter
  `conn`, the caller performs `conn.write`.
- A local bound from a parameter (`let s = conn`, a field `app.state`, a pattern binding)
  keeps that parameter as its root, so `app.state.put(..)` is an effect on `app`.
- A value the function created itself, such as a stream from `listener.accept()` or
  `TcpStream.connect(..)`, belongs to the function. Effects on it need not be declared;
  creating it was the effect (`listener.accept`, `net.connect`).

```ignite
fn echo<S: ByteStream>(stream: S) -> Result<Unit, IoError> requires [stream.read, stream.write] {
  match stream.read(4096)? {
    Data(bytes) => stream.write_all(bytes)
    Eof => stream.close()
  }
}

fn serve_one(listener: TcpListener) -> Result<Unit, IoError> requires [listener.accept] {
  let stream = listener.accept()?
  echo(stream)
}
```

`serve_one` reads and writes `stream`, but `stream` came from its own `accept`, so only
`listener.accept` is listed.

## Where effects are not allowed

- A `const` initialiser and a record field default must be pure. An effectful call there is
  E010.
- Property expressions in system declarations are evaluated at compile time and must be pure
  (E116).
- A `test` block is an effect root: it may perform any effect without declaring it.

## Effectful fn types

A plain fn type `fn(A) -> R` is pure. A function value that performs effects needs a fn type
that names its parameters and lists effects on them:

```ignite
fn handler(state: Store<Int>, n: Int) -> Int requires [state.write] {
  state.put(n, n)
  n
}

type App {
  state: Store<Int>
  handle: fn(state: Store<Int>, n: Int) -> Int requires [state.*]
}

fn run_app(app: App) -> Int requires [app.*] = app.handle(app.state, 1)

test "an effectful field" {
  let s: Store<Int> = Store.new()
  let app = App { state = s, handle = handler }
  assert run_app(app) == 1
  assert s.get(1) == Some(1)
}
```

- An effectful fn fits an effectful fn type when the type's effects cover the fn's, matched
  by parameter position.
- Using an effectful fn as a value where the expected type is a pure fn type, or where
  nothing expects a fn type that covers it, is E017.
- A closure that performs effects on a captured value charges them to the function that
  creates the closure, at the point of creation (E016 if that function's `requires` lacks
  them).

## `spawn`

`spawn f(args)` starts `f(args)` as a new task and returns `Unit` at once. Tasks run
concurrently; each has its own thread in the substrate.

```ignite
fn echo<S: ByteStream>(stream: S) -> Result<Unit, IoError> requires [stream.read, stream.write] {
  match stream.read(4096)? {
    Data(bytes) => stream.write_all(bytes)
    Eof => stream.close()
  }
}

fn accept_loop(listener: TcpListener) -> Result<Unit, IoError> requires [listener.accept] {
  loop {
    let stream = listener.accept()?
    spawn echo(stream)
  }
}
```

- The operand must be a call of a user fn or protocol role that performs effects. Spawning a
  pure fn or a substrate operation directly is E013.
- The spawned call must return `Result<Unit, IoError>` (E040 otherwise).
- The spawning function is charged the callee's effects, as for a normal call.
- When a spawned task fails or returns `Err`, the substrate logs
  `task ended with error: <report>` and the rest of the program keeps running.

## `?` and failure

`?` is error propagation, not an effect, but it is where effectful code most often meets the
type system:

- In a function, `e?` takes a `Result<T, E>`. It needs the function to return
  `Result<_, E>` with the same `E` (E040: "`?` needs a function returning `Result`", or
  "`?` propagates `IoError`, the function returns errors of type `Text`").
- `?` on an `Option` in a function is E040 ("`?` needs a `Result`, found `Option<Int>`");
  convert it with `ok_or` from [std.option](std/option-result.md).
- In a `test` block, `?` also takes an `Option`, and a failure fails the test. See
  [Tests and assertions](tests.md#-in-tests).

## Protocol roles

A `protocol` groups the roles of one wire protocol. A role is declared with `fn` inside the
protocol's braces, called as `Protocol.role(..)`, and may be generic.

```ignite
protocol Echo {
  fn serve(stream: TcpStream, greeting: Text) -> Result<Unit, IoError> requires [stream.read, stream.write] {
    stream.write_all(greeting.to_bytes())?
    match stream.read(4096)? {
      Data(bytes) => stream.write_all(bytes)
      Eof => stream.close()
    }
  }
}

fn handle(stream: TcpStream) -> Result<Unit, IoError> requires [stream.*] {
  Echo.serve(stream, "hi\r\n")
}
```

- Every role must declare `requires` (E030).
- The roles are `serve`, with two or three parameters, and `connect`, with one.
- Roles take type parameters with the `ByteStream` bound, like fns:
  `serve<S: ByteStream>(stream: S, ..)`.
- The standard protocols are [Telnet](std/telnet.md) and [HTTP](std/http.md).

The rules behind each code are on the [diagnostics](diagnostics.md) page.
