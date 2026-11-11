# Effects and capabilities

Ignite separates computing a value from acting on the world. A function that only computes
is pure. A function that reads a socket, writes stdout or accepts a connection is
effectful, and its effects are part of its type: the compiler knows which capability each
one acts on, and every caller accounts for them. This page explains why the language is
built that way and where the rules come from. The exact rules, with every effect name, are
in the [effects reference](../reference/effects.md).

## Why effects are tracked

The design states the goal with an example: a pure parser cannot get network access by
calling a helper. If the helper opens a connection, the parser's effect set changes, and
every caller has to accept that change. Nothing acquires authority silently.

Three consequences follow, and the rest of the language depends on them:

- **Authority is passed, never ambient.** An effect is a capability on a parameter. A
  function that writes to `stream` needs `stream.write`; it cannot write to a stream it was
  not given. Only a few process-wide operations, such as `stdout.write`, `net.listen` and
  `clock.read`, are named directly. The [effect names](../reference/effects.md#effect-names)
  list them all.
- **Declarations can be evaluated at compile time.** Property expressions must be pure, so
  the compiler can compute every property of a system without touching the machine it runs
  on ([The compile-time evaluator](evaluator.md)).
- **Decisions can be replayed.** The control machines of `ignite run` are pure functions of
  their events, so a journal of the events is enough to reproduce every decision
  ([The run-time model](runtime.md)).

## Declared and inferred

An effect is written `<capability>.<operation>`: `stream.read`, `listener.accept`,
`stdout.write`. A function either declares its effects with `requires`, or has them
inferred from its body.

```ignite
fn parse_port(text: Text) -> Option<Int> = match Int.parse(text) {
  Some(port) => if port > 0 && port < 65536 { Some(port) } else { None }
  None => None
}

fn send_line(stream: TcpStream, line: Text) -> Result<Unit, IoError> requires [stream.write] {
  stream.write_all("{line}\r\n".to_bytes())
}

fn greet(stream: TcpStream) -> Result<Unit, IoError> {
  send_line(stream, "hello")?
  stream.close()
}

fn serve(listener: TcpListener) -> Result<Unit, IoError> requires [listener.accept] {
  let stream = listener.accept()?
  greet(stream)
}

test "parse_port is pure and needs no capability" {
  assert parse_port("2323") == Some(2323)
  assert parse_port("0") == None
}
```

- `parse_port` performs no effectful operation, so it is pure.
- `send_line` declares `stream.write`. A body that did more than it declares would be E016.
- `greet` has no `requires`. Its set is inferred: `send_line` needs `stream.write` on its
  first parameter, and `greet` passes its own `stream`, so `greet` needs `stream.write`.
- `serve` gets its stream from `accept`. A value the function created itself is owned by
  it, so `serve` lists only `listener.accept`, the effect that produced the stream.

Inference runs over a module's functions as a fixed point, so mutual recursion is covered and
declaration order does not matter. An effectful call needs no marker: there is no `await`.

A missing effect is reported at the call, with the fix:

```ignite,fails=E016
fn first_bytes(stream: TcpStream) -> Result<Input, IoError> requires [] {
  stream.read(4096)
}
```

```text
error[E016]: missing effect `stream.read` for `stream.read`; add it to the `requires` of `first_bytes`
```

## Effects follow arguments

A callee's effect on a parameter becomes the caller's effect on whatever it passed. The
compiler tracks the roots of every value: the parameters it may have come from. A field
`app.state`, a local bound from a parameter, or a list holding a parameter all keep that
root. A function cannot launder a capability by returning it, because the result of a call
keeps the roots of every argument that could hold a handle.

The same rule covers protocol roles. A role declares the full set its body needs, for
example `requires [stream.read, stream.write]` for a server role, and the caller that
spawns it pays for those effects on the stream it passes.

## Where effects start

Somebody has to be allowed to do everything. In Ignite that is the executable's entry. The
generated `main` binds the listeners, builds the component's value and calls `K.main`, whose
effects are inferred and granted. A `test` block is the other root: it may perform any
effect without a `requires`. A program's authority starts there and flows down through
parameters. A library function never has more than its caller gave it.

## Where effects are not allowed

Some code must be pure, because the compiler runs it or because its result must not depend
on the machine:

- a `const` value and a record field default ([E010](../reference/diagnostics.md#e010));
- a property expression in a system declaration ([E116](../reference/diagnostics.md#e116));
- the hooks `K.machine` and `K.validate` ([E109](../reference/diagnostics.md#e109)).

The exact rules are in the [effects reference](../reference/effects.md#where-effects-are-not-allowed).

```ignite,fails=E010
fn report(line: Text) -> Result<Unit, IoError> requires [stdout.write] {
  Stdout.write_all("{line}\n".to_bytes())
}

const STARTED: Result<Unit, IoError> = report("started")
```

A property expression that reads stdin, calls an effectful function, operates on a handle
or spawns a task is E116, even when the effect is only inferred:

```ignite,fails=E116
type Banner {
  text: Text
}

fn read_banner() -> Text {
  match Stdin.read(80) {
    Ok(Data(bytes)) => bytes.to_text()
    _ => ""
  }
}

system S {
  component banner: Banner { text = read_banner() }
}
```

```text
error[E116]: effectful call `read_banner` in the property `text` of component `banner`: a property expression is pure
```

Naming a handle is not an operation on it: `listener = ingress` is a pure property, and
`ingress.accept()` in the same place is E116. `Pipe.pair()` counts as an effect there too, so
a property cannot create a stream.

This is what lets `ignite check`, `ignite plan` and `ignite explain` evaluate every property
of a system: the evaluator runs only code whose effect set is empty.

## Effectful functions as values

A pure function is a value of a type such as `fn(Text) -> Reply`. An effectful function is a
value only where the expected type carries a `requires` that covers its effects, for example
the HTTP handler type `fn(state: S, request: Request) -> Response requires [state.*]`. Using
it anywhere else is E017. Calling such a value is an effectful operation, so the effects stay
visible at every step.

## Limits

- Effects are checked in Ignite code. The design notes that a capability declaration
  enforces a boundary only where access is mediated, and that in-process native code is
  trusted. The substrate behind the builtins is such code.
- Declaring an effect the body does not need is allowed.
- `log(text)` writes a line to stderr, but it is not a tracked effect: it needs no
  `requires` entry.

Next: [Protocols and roles](protocols.md).
