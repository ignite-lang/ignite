# Intent and properties

A system declaration says what exists and what it is for, in typed values rather than
strings and comments. Every declaration has a kind, every kind is a type, and every property
is an expression the compiler checks and evaluates. This page covers the declaration forms,
the value types they use, how members get their identity, and why the vocabulary of systems
lives in the standard library instead of in the grammar.

## Declaration forms

| form | meaning | example |
|---|---|---|
| `package` | the module's name | `package examples.terminal` |
| `import` | names taken from another module | `import std.net.TcpListener` |
| `type` | a record or an alias | `type SessionId = Text` |
| `const` | a named value of a pure expression | `const MAXIMUM_LINE: Int = 4096` |
| `enum` | closed alternatives, with optional payloads | `enum Action { Continue, Close }` |
| `fn` | a pure or effectful function | `fn parse(input: Text) -> Option<Command>` |
| `protocol` | roles over a byte stream, written in Ignite | `protocol Telnet { fn serve(...) ... }` |
| `test` | an inline test, run by `ignite test` | `test "first word" { ... }` |
| `system` | a named topology of members | `system Terminal { ... }` |
| `resource` | an owned or bound external thing with a lifecycle | `resource ingress: TcpListener { ... }` |
| `component` | a deployable or embeddable unit of computation | `component server: TelnetServer { ... }` |
| `contract` | a measured objective or a commitment | `contract reachable: Availability { ... }` |
| `policy` | bounded decision logic and the responses it allows | `policy restart: Restart { ... }` |
| `assembly` | where each member of a system runs | `assembly local: Deployment { ... }` |

`resource`, `component`, `contract` and `policy` appear inside a `system`; `assembly` is a
top-level declaration that names its system. The design also lists `interface`; user
interfaces are not implemented, and the only interface today is the compiler-known
`ByteStream` ([Protocols and roles](protocols.md)).

## A declaration is a record literal

`component name: K { … }` names a kind `K`, and `K` is an ordinary record type. The braces
hold a record literal of `K`: fields with defaults may be left out, and a field without a
default that is left out is E104. Nothing about the kind is special to the compiler.

```ignite
type Cache {
  listener: TcpListener
  capacity: ByteSize
  entry_ttl: Duration = 5 minutes
  hit_target: Ratio = 95%
}

system Shop {
  resource ingress: TcpListener {
    address = "127.0.0.1"
    port = 8080
  }

  component cache: Cache {
    listener = ingress
    capacity = 64 MiB
  }
}
```

`listener = ingress` passes the resource itself: inside a `system`, a bare member name is a
value of its declared type. [Components](components.md) explains kinds in full.

Resource kinds are still compiler-known: `TcpListener` (with `exposure` and `backlog`
defaults) and `Store<T>`. Component, contract, policy and assembly kinds are any record type,
from the standard library or from the program.

## Values and types

| family | types | rule |
|---|---|---|
| scalar | `Bool`, `Int`, `Byte`, `Text` | `Int` overflow is a failure on every target, not a wrap-around |
| structured | records, enums, tuples, `List<T>`, `Map<K, V>` | maps iterate, compare and print in key order |
| quantity | `Duration`, `ByteSize`, `Ratio` | a unit belongs to one type; mixing types is E101 |
| failure | `Option<T>`, `Result<T, E>` | missing values and failures are values that must be handled |
| system | `Ref<K>`, `Signal<T>`, `SystemRef`, `Module<I>` | references to members, outputs, systems and sibling modules |

A quantity literal is a number followed by a unit on the same line: `4096 bytes`, `16 KiB`,
`15 minutes`, `100 ms`, `99.9%`. The unit makes the intent part of the type, so a timeout
cannot be passed where a buffer size is expected:

```ignite
test "quantities carry their unit" {
  assert 1 KiB == 1024 bytes
  assert 90 seconds == 1 minute + 30 seconds
  assert (99.9%).ppm() == 999000
  assert (2 minutes).to_ms() == 120000
}
```

```ignite,fails=E101
const TIMEOUT: Duration = 2 seconds + 512 bytes
```

| type | units | stored as |
|---|---|---|
| `Duration` | `ns`, `us`, `ms`, `second(s)`, `minute(s)`, `hour(s)`, `day(s)` | Int nanoseconds |
| `ByteSize` | `byte(s)`, `KiB`, `MiB`, `GiB` (1024-based), `KB`, `MB`, `GB` (1000-based) | Int bytes |
| `Ratio` | `%` | Int parts per million, at most 4 decimals |

The design's byte-size type is called `Bytes`; the implementation names it `ByteSize`,
because `Bytes` is already `List<Byte>`. The design also proposes `Decimal`, `Rate<Event>`,
`CPU`, `Money` and evidence types such as `Observation<T>`. They are not implemented.
[Types](../reference/types.md#quantities) has the exact operations.

## Identity and edges

Every member has a stable logical identity, `<System>.<name>`: `Terminal.server`,
`Terminal.reachable`. The system model lists members by that id, sorted, so reordering
declarations in the source changes nothing in the model.

A name can be used in two ways inside a system:

- **as a value**, `application = console`: the console's record is embedded in the server;
- **as a reference** of type `Ref<K>`, `target = Terminal.server` or `Terminal.ingress =
  localSocket()`: it names the member without embedding it. A bare name where a `Ref<K>` is
  expected is a reference too, so `target = server` in a probe means the same as
  `target = Terminal.server`.

From these uses the compiler derives the edges of the system graph, each with a kind:

| edge | from a | example |
|---|---|---|
| `uses` | bare member name in a property | `Terminal.server` uses `Terminal.ingress` |
| `references` | `Ref` value | `Terminal.promptProbe` references `Terminal.server` |
| `observes` | `Signal` value | `Terminal.reachable` observes `Terminal.promptProbe.attempts` |
| `hosts` | assembly resolution | `Terminal.server` hosts `Terminal.console` |
| `provisioning`, `readiness`, `recovery` | run-time roles | the listener is provisioned by the server; the probe waits for the server's readiness |

The design separates these kinds because only some impose an order. A protocol in which
both sides talk to each other is not a dependency cycle. A cycle of bare-name uses among
components is one, and is E105.

## Library vocabulary, not keywords

The grammar knows `system`, `resource`, `component`, `contract`, `policy` and `assembly`. It
does not know `Restart`, `Availability`, `TelnetServer` or `NativeProcess`. Those are types in
`std/`:

| family | library symbols | module |
|---|---|---|
| contracts | `Availability`, `ThresholdCompliance`, `SLA` | [`std.contract`](../reference/std/contract.md) |
| policies | `Restart`, `Backoff`, `DrainRollout` | [`std.policy`](../reference/std/policy.md) |
| indicators | `ProbeAttempt`, `Sample`, `Verdict`, `Window`, `MissingData` | [`std.sli`](../reference/std/sli.md) |
| deployment | `Deployment`, `Binding`, `localSocket()`, `nativeProcess(..)` | [`std.deploy`](../reference/std/deploy.md) |
| consoles | `CommandConsole`, `Reply`, `Action` | [`std.console`](../reference/std/console.md) |
| protocols | `TelnetServer`, `TelnetClient`, `TelnetPrompt`, `HttpServer` | [`std.protocol.telnet`](../reference/std/telnet.md), [`std.protocol.http`](../reference/std/http.md) |
| command lines | `Application`, `Command`, `Argument` | [`std.cli`](../reference/std/cli.md) |

The compiler reaches this vocabulary through a few function names that any module can
provide: `main`, `machine` and `validate` on a kind, and `admits`, `includes` and `entries`
on a binding type. Assembly resolution, for example, asks the binding type's `admits`
whether a member may go there; the compiler does not know the variants of
`std.deploy.Binding`. The run-time driver knows the types of `std.control` and the hook
names. So a new protocol, policy or deployment target is meant to be a library change, not
a compiler change. The rule and its remaining exceptions are in
[`docs/layering.md`](https://github.com/ignite-lang/ignite/blob/main/docs/layering.md).

The cost of this rule is a small keyword set: 32 keywords today (the lexer's table), inside the
budget of 30 to 50 reserved words that design section 1.5 sets. The [lexical reference](../reference/lexical.md) lists them.
