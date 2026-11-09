# Intent and properties

A system declaration says what exists and what it is for, in typed values rather than
strings and comments. Every declaration has a kind, every kind is a type, and every property
is an expression the compiler checks and evaluates. This page explains why declarations are
records, why quantities carry units, how members get their identity, and why the vocabulary
of systems lives in the standard library instead of in the grammar.

## Six forms describe a system

Most of a program is ordinary code: `type`, `enum`, `fn`, `const`, `protocol` and `test`.
Six forms describe the system itself:

- a `system` is a named topology of members;
- a `resource` is an owned or bound external thing with a lifecycle, such as a listener;
- a `component` is a deployable or embeddable unit of computation;
- a `contract` is a measured objective or a commitment;
- a `policy` is bounded decision logic and the responses it allows;
- an `assembly` says where each member of a system runs.

The first five appear inside a `system`. An `assembly` is a top-level declaration that names
its system. The design also lists `interface`, which is not implemented. The only interface
today is the compiler-known `ByteStream` ([Protocols and roles](protocols.md)). The
[grammar](../reference/grammar.md) has every form.

## A declaration is a record literal

`component name: K { … }` names a kind `K`, and `K` is an ordinary record type. The braces
hold a record literal of `K`. Fields with defaults may be left out. A field without a default
that is left out is E104. Nothing about the kind is special to the compiler.

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

`listener = ingress` passes the resource itself. Inside a `system`, a bare member name is a
value of its declared type. [Components](components.md) explains kinds in full.

Resource kinds are compiler-known: `TcpListener`, `Store<T>`, `Directory`
([`std.fs`](../reference/std/fs.md)) and the capabilities of
[`std.process`](../reference/std/process.md). Component, contract, policy and assembly kinds
are any record type, from the standard library or from the program.

## Quantities carry their unit

A property is a typed value, and a duration, a size and a share are different types. A
quantity literal is a number followed by a unit on the same line: `4096 bytes`, `16 KiB`,
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

Missing values and failures are values too, `Option<T>` and `Result<T, E>`, so a declaration
cannot leave one unhandled. `Int` overflow is a failure on every target, not a wrap-around.

The design's byte-size type is called `Bytes`. The implementation names it `ByteSize`,
because `Bytes` is already `List<Byte>`. The design also proposes `Decimal`, `Rate<Event>`,
`CPU`, `Money` and evidence types such as `Observation<T>`, which are not implemented. The
units are in [Lexical structure](../reference/lexical.md#quantity-and-percentage-literals),
the operations in [Types](../reference/types.md#quantities).

## Identity and edges

Every member has a stable logical identity, `<System>.<name>`: `Terminal.server`,
`Terminal.reachable`. The system model lists members by that id, sorted. Reordering
declarations in the source changes nothing in the model.

A name can be used in two ways inside a system:

- **as a value**, `application = console`: the console's record is embedded in the server;
- **as a reference** of type `Ref<K>`, `target = server`: it names the member without
  embedding it.

From these uses the compiler derives the edges of the system graph, each with a kind. The
design separates the kinds because only some impose an order. A protocol in which both sides
talk to each other is not a dependency cycle. A cycle of bare-name uses among components is
one, and is E105. [Systems and assemblies](systems.md) explains the graph. The edge kinds are
in [System declarations](../reference/systems.md#edges).

## Library vocabulary, not keywords

The grammar knows `system`, `resource`, `component`, `contract`, `policy` and `assembly`. It
does not know `Restart`, `Availability`, `TelnetServer` or `NativeProcess`. Those are types
in `std/`, grouped by family in the [standard library](../reference/std/index.md).

The compiler reaches this vocabulary through a few function names that any module can
provide: `main`, `machine` and `validate` on a kind, and `admits`, `includes` and `entries`
on a binding type. Assembly resolution, for example, asks the binding type's `admits`
whether a member may go there. The compiler does not know the variants of
`std.deploy.Binding`. The run-time driver knows the types of `std.control` and the hook
names. A new protocol, policy or deployment target is meant to be a library change, not a
compiler change. The rule and its remaining exceptions are in
[`docs/layering.md`](https://github.com/ignite-lang/ignite/blob/main/docs/layering.md).

The cost of this rule is a small keyword set: 32 keywords today, inside the budget of 30 to
50 reserved words that design section 1.5 sets. The [lexical reference](../reference/lexical.md#keywords)
lists them.

Next: [Effects and capabilities](effects.md).
