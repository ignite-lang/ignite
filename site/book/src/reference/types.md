# Types

Every expression in Ignite has a type, known when the program is checked. There are no
implicit conversions: an `Int` is never silently a `Byte`, a `Text` never silently `Bytes`.
This page lists the types of v0.3.0 and what each one supports. The intent behind them is in
[the design of intent and properties](../design/intent.md).

## At a glance

| family | types |
|---|---|
| scalars | `Int`, `Byte`, `Bool`, `Text`, `Unit` |
| collections | `List<T>`, `Bytes` (= `List<Byte>`), `Map<K, V>` |
| failure | `Option<T>`, `Result<T, E>`, `IoError` |
| user types | records (`type`), enums (`enum`), tuples, aliases |
| functions | `fn(A) -> R`, `fn(a: A) -> R requires [..]`, closures |
| quantities | `Duration`, `ByteSize`, `Ratio` |
| substrate handles | `TcpListener`, `TcpStream`, `PipeStream`, `Store<T>`, `Input` |
| system declarations | `Ref<K>`, `Ref`, `Signal<T>`, `SystemRef`, `Protocol`, `Module<I>`, `Exposure` |

Builtin type names are visible everywhere without an import. Declaring a type, fn or other
item with a builtin name is E004.

## Scalars

| type | values | literals |
|---|---|---|
| `Int` | signed 64-bit integers | `42`, `-7`, `0xFF`, `1_000` |
| `Byte` | unsigned 8-bit integers, 0 to 255 | `b'a'`, or an integer literal where a `Byte` is expected |
| `Bool` | `true`, `false` | `true`, `false` |
| `Text` | UTF-8 text | `"hello"`, `"n = {n}"`, `r"raw"` |
| `Unit` | the one value `()` | `()`, also written `Unit` |

A fn without a return type returns `Unit`. `()` is the Unit type as well as its value.

### Operators on scalars

| operators | `Int` | `Byte` | `Bool` | `Text` |
|---|---|---|---|---|
| `+ - * / %` | yes | yes | no | `+` joins texts |
| unary `-` | yes | no | no | no |
| `< > <= >=` | yes | yes | no | byte order |
| `== !=` | yes | yes | yes | yes |
| `&& \|\| !` | no | no | yes | no |

- Both operands of a binary operator have the same type. `Byte + Int` is E040; convert one
  side with `b.to_int()`, or with `i.to_byte()`, which returns `Option<Byte>` (`None` outside
  0 to 255).
- Integer division rounds toward zero; `%` is the remainder of that division.
- `&&` and `||` evaluate their right operand only when needed.
- `Text` orders by the bytes of its UTF-8 encoding, which is code point order: `"Z" < "a"`,
  `"z" < "é"`, and a prefix sorts first. A `Map` keeps `Text` keys in the same order.
- Any other combination is E045.

### Overflow

Integer arithmetic never wraps. On `Int` and `Byte`, an overflowing `+`, `-` or `*`, a
division or remainder by zero, `-` of the smallest `Int`, and the smallest `Int` divided by
`-1` are failures, on both targets and in dev and release builds alike. A failure ends the
task that ran it with a report of the form
`<operation> failed at <file>:<line>:<column>: <detail>`. In a test it fails the test; in the
main task the process exits with status 101. See [Tests and assertions](tests.md#failures).

Arithmetic that is meant to wrap says so: `a.wrapping_add(b)`, `a.wrapping_sub(b)` and
`a.wrapping_mul(b)` on `Int` return the result modulo 2^64 in two's complement, as Go's `int`
and Rust's `i64::wrapping_*` do, and never fail.

```ignite
const MAX: Int = 9223372036854775807

test "wrapping arithmetic" {
  assert MAX.wrapping_add(1) == -MAX - 1
  assert (-MAX - 1).wrapping_sub(1) == MAX
  assert MAX.wrapping_mul(2) == -2
}
```

To test for overflow before it happens, use `a.checked_add(b)`, `a.checked_sub(b)` and
`a.checked_mul(b)` on `Int`: each returns `Some` of the result when it fits in `Int` and `None`
when it does not, and never fails. On an `Int` receiver these builtin methods, like the
wrapping ones, take precedence over a user fn of the same name.

```ignite
const MAX: Int = 9223372036854775807

test "checked arithmetic" {
  assert MAX.checked_add(1) == None
  assert (-MAX - 1).checked_sub(1) == None
  assert (-MAX - 1).checked_mul(-1) == None
  assert (3).checked_mul(4) == Some(12)
}
```

### Conversions

| call | result |
|---|---|
| `i.to_text()`, `Int.to_text(i)` | decimal text: `"-3"` |
| `b.to_text()` on a `Byte` | decimal text: `"255"` |
| `x.to_text()` on a `Bool` | `"true"` or `"false"` |
| `b.to_int()` | the `Byte` as an `Int` |
| `i.to_byte()` | `Option<Byte>`: `None` outside 0 to 255 |
| `Int.parse(t)` | `Option<Int>`: decimal with an optional leading `-`, no spaces, no `+`; `None` on overflow |
| `t.to_bytes()` | the UTF-8 bytes of a `Text` |
| `b.to_text()` on `Bytes` | the bytes decoded as UTF-8; invalid sequences become U+FFFD |

```ignite
fn digit(b: Byte) -> Option<Int> = match b {
  b'0'..=b'9' => Some((b - b'0').to_int())
  _ => None
}

test "conversions" {
  assert digit(b'7') == Some(7)
  assert digit(b'x') == None
  assert 300.to_byte() == None
  assert Int.parse("-12") == Some(-12)
  assert Int.parse("+12") == None
  assert "héllo".to_bytes().len() == 6
  assert (7).to_text() + "!" == "7!"
}
```

## Text and Bytes

`Text` is UTF-8 text. `Bytes` is `List<Byte>`, so every list operation works on it. A `Text`
is turned into `Bytes` with `to_bytes()` and back with `to_text()`.

Builtin methods of `Text`: `len()` (in bytes), `is_empty()`, `trim()`, `starts_with(p)`,
`strip_prefix(p) -> Option<Text>`, `concat(t)`, `to_bytes()`, `to_text()`. Everything else
(`split`, `contains`, `find`, `to_lower`, ...) is Ignite code in [std.text](std/text.md), used
by method call syntax after an import.

Offsets into a `Text` are byte offsets, and matching is byte for byte.

## Lists

`List<T>` is an ordered list of values of one type. `[1, 2, 3]` is a literal; `[]` and
`List.empty()` are the empty list, whose element type comes from an annotation or a later
use.

| method | type | note |
|---|---|---|
| `len()`, `is_empty()` | `Int`, `Bool` | |
| `l[i]`, `l.at(i)` | `T` | out of range is a failure |
| `slice(start, end)` | `List<T>` | `end` exclusive; out of range is a failure |
| `l[i] = x`, `set(i, x)` | `Unit` | replace the element at `i` in place; out of range is a failure, as for `l[i]` |
| `push(x)`, `append(other)`, `clear()` | `Unit` | change the list in place |
| `a + b` | `List<T>` | a new list: the elements of `a`, then those of `b` |
| `map(f)` | `List<U>` | `f: fn(T) -> U`, pure |
| `join(sep)` | `Text` | on `List<Text>` only |
| `to_text()` | `Text` | on `Bytes` only |
| `Bytes.of(list)`, `Bytes.empty()` | `Bytes` | statics |

`for x in list { .. }` visits the elements in order.

Lists are values: assigning or passing a list copies it, as far as the program can tell.
`set` (and `l[i] = x`), `push`, `append` and `clear` change a place rooted in a `var` or a
parameter. Through a `let` binding, a `for` element or a pattern binding they are E006.

```ignite
fn squares(n: Int) -> List<Int> {
  var out: List<Int> = []
  var i = 1
  while i <= n {
    out.push(i * i)
    i = i + 1
  }
  out
}

test "lists" {
  let xs = squares(4)
  assert xs == [1, 4, 9, 16]
  assert xs[0] + xs.at(3) == 17
  assert xs.slice(1, 3) == [4, 9]
  assert xs.map(|x| x.to_text()).join(",") == "1,4,9,16"
  let copy = xs
  var changed = xs
  changed.push(25)
  assert copy.len() == 4 && changed.len() == 5
}
```

## Maps

`Map<K, V>` maps keys to values, kept in key order.

- A brace literal makes a map where a `Map` is expected: `{ a = 1, b = 2 }` with identifier
  keys is a `Map<Text, Int>`. `Map.empty()` is the empty map.
- Methods: `get(k) -> Option<V>`, `put(k, v)`, `remove(k) -> Option<V>`, `keys() ->
  List<K>`, `len()`, `contains(k)`.
- `for (k, v) in m { .. }` visits the entries in key order.
- Key types: `Int`, `Byte`, `Text`, `Bool`, enums without payloads (ordered by declaration)
  and `Ref`. Any other key type is E106.
- Maps are values, like lists; `put` and `remove` need a `var` or a parameter.

```ignite
fn counts(words: List<Text>) -> Map<Text, Int> {
  var m: Map<Text, Int> = Map.empty()
  for w in words {
    let n = match m.get(w) {
      Some(c) => c
      None => 0
    }
    m.put(w, n + 1)
  }
  m
}

test "maps" {
  let m = counts(["b", "a", "b"])
  assert m.get("b") == Some(2)
  assert m.keys() == ["a", "b"]
  let fixed: Map<Text, Int> = { a = 1, b = 2 }
  assert fixed == counts(["a", "b", "b"])
}
```

## Option and Result

| type | variants | meaning |
|---|---|---|
| `Option<T>` | `Some(x)`, `None` | a value that may be missing |
| `Result<T, E>` | `Ok(x)`, `Err(e)` | an operation that may fail with an `E` |
| `IoError` | (opaque) | the error of a substrate operation; `e.message` is its text |

- `e?` on a `Result` returns its `Err` from the enclosing fn, which must return a `Result`
  with the same error type (E040 otherwise). Its value is the `Ok` payload.
- In a fn, `?` works on `Result` only. In a `test` body it also takes an `Option`, and it
  fails the test instead of returning: see [Tests and assertions](tests.md#-in-tests).
- `IoError.new(message)` makes an error; `e.with_exit_code(n)` sets the exit status a
  program ends with when the error reaches its `main`.
- Helpers such as `unwrap_or`, `map`, `and_then` and `ok_or` are in
  [std.option and std.result](std/option-result.md).

```ignite
fn port(text: Text) -> Result<Int, Text> = match Int.parse(text) {
  Some(n) => if n >= 1 && n <= 65535 { Ok(n) } else { Err("out of range: {n}") }
  None => Err("not a number: {text}")
}

fn both(a: Text, b: Text) -> Result<(Int, Int), Text> {
  let first = port(a)?
  let second = port(b)?
  Ok((first, second))
}

test "results" {
  assert both("80", "443") == Ok((80, 443))
  assert both("80", "x") == Err("not a number: x")
}
```

## Records

A record is a named set of fields.

```ignite
type Server {
  host: Text
  port: Int = 8080
  tags: List<Text> = []
}

fn Server.local() -> Server = Server { host = "127.0.0.1" }

fn moved(s: Server, port: Int) -> Server = Server { port, ..s }

test "records" {
  let s = Server.local()
  assert s.port == 8080
  let t = moved(s, 9090)
  assert t.port == 9090 && t.host == "127.0.0.1"
  var u = t
  u.port = 1
  assert t.port == 9090
}
```

- Fields are separated by `,` or line breaks.
- A field may have a default: `port: Int = 8080`. The default is a pure expression of the
  record's module; it may use constants and pure fns, no locals.
- A record literal `Server { host = "x" }` must set every field that has no default
  (E047 otherwise). An unknown or repeated field is E047 too.
- `Server { port }` is short for `Server { port = port }`.
- `Server { port = 1, ..s }` copies `s` and replaces the fields given.
- `s.port = 1` assigns a field of a `var`. Parameters are immutable, so assigning a field
  of one is E006.
- Where the expected type is a record, the name may be left out: `{ host = "x" }` is a brace
  literal of that record. Without an expected type a brace literal is E100.
- `fn Server.local()` is an associated fn of `Server`, called `Server.local()`. The record
  must be declared in the same module.

## Enums

An enum is a closed set of variants. A variant may carry a payload of one or more values.

```ignite
enum Shape {
  Circle(Int)
  Rect(Int, Int)
  Empty
}

fn area(s: Shape) -> Int = match s {
  Circle(r) => 3 * r * r
  Shape.Rect(w, h) => w * h
  Empty => 0
}

fn Shape.square(side: Int) -> Shape = Rect(side, side)

test "enums" {
  assert area(Shape.square(3)) == 9
  assert area(Circle(1)) == 3
  let none: Shape = Empty
  assert area(none) == 0
}
```

- A variant is written bare (`Empty`) or qualified (`Shape.Empty`).
- A bare variant resolves against the expected type, whether or not its enum is imported.
  When the expected type does not decide, the enum in scope that declares the name owns it.
  Two enums in scope declaring it is E005; qualify it.
- A variant with a payload used without its arguments is E003.
- `match` must cover every variant (E046). Payload patterns are checked recursively.
- `Some`, `None`, `Ok`, `Err`, `Data` and `Eof` are builtin variants.

## Tuples

`(A, B)` is a tuple type of two or more elements. `(1, "a")` is a tuple value, `t.0` and
`t.1` its elements, and `let (n, s) = t` takes it apart. An index past the tuple is E041. A
one-element tuple does not exist: `(a,)` is E001.

```ignite
fn min_max(xs: List<Int>) -> (Int, Int) {
  var low = xs[0]
  var high = xs[0]
  for x in xs {
    if x < low { low = x }
    if x > high { high = x }
  }
  (low, high)
}

test "tuples" {
  let (low, high) = min_max([3, 9, 1])
  assert low == 1 && high == 9
  assert min_max([5]).1 == 5
}
```

## Recursive types

A type may contain itself through an enum payload, `Option`, `Result` or `List`; such
positions are boxed by the compiler. A record that contains itself through records and
tuples alone would be infinite and is E055.

```ignite
enum Tree<T> { Leaf, Node(T, Tree<T>, Tree<T>) }

type Chain { value: Int, next: Option<Chain> }

fn size<T>(t: Tree<T>) -> Int = match t {
  Leaf => 0
  Node(_, left, right) => 1 + size(left) + size(right)
}

test "recursive types" {
  let t = Node(1, Node(2, Leaf, Leaf), Leaf)
  assert size(t) == 2
  let c = Chain { value = 1, next = Some(Chain { value = 2, next = None }) }
  assert c.next != None
}
```

## Type aliases

`type Name = T` gives another name to a type; `type Name<P> = T` takes parameters. An alias
is exactly the type it names: no value or diagnostic tells them apart. An alias that refers
to itself is E048; a wrong number of type arguments is E048 too.

```ignite
type Port = Int
type IoResult<T> = Result<T, IoError>

fn parse_port(text: Text) -> IoResult<Port> = match Int.parse(text) {
  Some(n) => Ok(n)
  None => Err(IoError.new("not a port: {text}"))
}

test "aliases" {
  let p: Int = parse_port("80")?
  assert p == 80
}
```

## Function types and closures

- `fn(A, B) -> R` is the type of a pure function. Without `-> R` the return type is `Unit`.
- A named fn used as a value (`apply(double, 4)`) has a fn type.
- A closure `|x| x + 1`, `|x: Int, y: Int| x * y` or `|| 0` is a fn value written in place.
  Its body is an expression or a block. Parameter types come from an annotation or from the
  expected fn type (E042 when neither gives one).
- A closure captures the bindings it reads **by copy**, at the moment it is created. A later
  change to the outer binding does not affect it. Assigning to a captured binding inside the
  closure is E006.
- `return`, `?`, `break` and `continue` inside a closure belong to the closure.
- A field of fn type is called like a method: `app.handle(x)`.
- An effectful fn type names its parameters and lists effects:
  `fn(state: Store<Todo>, request: Request) -> Response requires [state.*]`. It is the only
  kind of fn type an effectful function fits; see [Effects](effects.md#effectful-fn-types).

```ignite
fn apply(f: fn(Int) -> Int, x: Int) -> Int = f(x)

fn double(n: Int) -> Int = n * 2

fn adder(by: Int) -> fn(Int) -> Int = |x| x + by

type Rule { name: Text, check: fn(Int) -> Bool }

test "fn values" {
  assert apply(double, 4) == 8
  assert apply(|x| x - 1, 4) == 3
  let add10 = adder(10)
  assert add10(5) == 15
  var base = 1
  let add_base = |x: Int| x + base
  base = 100
  assert add_base(1) == 2
  let positive = Rule { name = "positive", check = |n| n > 0 }
  assert positive.check(3)
}
```

## Generics

Records, enums, aliases, fns, associated fns and protocol roles take type parameters.

```ignite
type Pair<A, B> { first: A, second: B }

fn swap<A, B>(p: Pair<A, B>) -> Pair<B, A> = Pair { first = p.second, second = p.first }

fn first_or<T>(items: List<T>, fallback: T) -> T = if items.is_empty() { fallback } else { items[0] }

test "generics" {
  let p = swap(Pair { first = 1, second = "one" })
  assert p.first == "one"
  assert first_or([], 7) == 7
  assert first_or(["a"], "z") == "a"
}
```

- Type arguments are inferred from the arguments and the expected type. They are never
  written at a call. When nothing decides a parameter it is E042; annotate the binding.
- Inside a generic declaration, a value of a type parameter can be bound, passed, returned,
  stored and copied (`x.copy()`). Operators, `==`, `to_text`, `debug()` and other methods are
  E045 or E041.
- A type parameter of a record, enum or alias that no field, payload or aliased type uses is
  E057.
- A fn or role type parameter may have the bound `ByteStream`: `<S: ByteStream>`. Inside the
  fn, a value of type `S` has the stream methods `read`, `write_all`, `close`,
  `set_idle_timeout`, `write_urgent` and `urgent_pending`. A call that passes a type that is
  not a stream is E096. `ByteStream`
  is the only interface; any other bound, or two bounds, is E099, and `ByteStream` used as a
  type is E099.

```ignite
fn send_line<S: ByteStream>(stream: S, line: Text) -> Result<Unit, IoError> requires [stream.write] {
  stream.write_all((line + "\r\n").to_bytes())
}

test "a bounded type parameter" {
  let (a, b) = Pipe.pair()
  send_line(a, "hello")?
  a.close()?
  assert b.read(64)? == Data(b"hello\r\n")
}
```

## Quantities

Quantity types carry a unit, so durations, sizes and ratios cannot be mixed with each other
or with plain numbers by accident. They are written as [quantity
literals](lexical.md#quantity-and-percentage-literals).

| type | unit of storage | conversions |
|---|---|---|
| `Duration` | nanoseconds | `d.to_ms()`, `d.to_ns()` |
| `ByteSize` | bytes | `s.to_int()` |
| `Ratio` | parts per million (`1%` is 10,000) | `r.ppm()` |

- `+`, `-` and the comparisons take two values of one quantity type.
- `*` and `/` take a quantity and an `Int`: `100 ms * 3`, `3 * 100 ms`, `1 KiB / 4`.
- Mixing quantity types (`1 ms + 1 bytes`), or dividing two quantities, is E101. `%` and
  unary `-` are E045.
- Overflow is a failure, as for `Int`.
- `debug()` prints the literal in the largest unit that divides the value exactly:
  `1500 ms`, `15 minutes`, `16 KiB`, `99.9%`. Quantities have no `to_text`; interpolate
  `d.debug()` or `d.to_ms()`.

```ignite
const IDLE: Duration = 15 minutes

fn deadline(attempts: Int) -> Duration = 200 ms * attempts + 1 second

test "quantities" {
  assert deadline(5) == 2 seconds
  assert IDLE > 10 minutes
  assert IDLE.to_ms() == 900000
  assert (64 KiB).to_int() == 65536
  assert "waited {deadline(1).debug()}" == "waited 1200 ms"
}
```

## Equality

`==` and `!=` compare two values of one type structurally:

- `Int`, `Byte`, `Bool`, `Unit`: same value. `Text`: same bytes.
- `List` (and `Bytes`), `Map`, `Option`, `Result`, `Input`, tuples, records and enums: element
  by element, field by field, variant and payloads.

A type is comparable when nothing inside it is a fn, a handle (`TcpStream`, `TcpListener`,
`PipeStream`, `Store`), an `IoError` or a type parameter. `==` on any other type is E045.

## Debug text

`x.debug()` gives a `Text` that shows any value whose type holds no type parameter. Failed
assertions use the same text. It is the same on both targets.

| type | debug text |
|---|---|
| `Int`, `Byte` | `-3`, `255` |
| `Bool` | `true` |
| `Unit` | `()` |
| `Text` | `"a\"b"`: quoted, with `\"`, `\\`, `\n`, `\r`, `\t`, `\0`, other control characters as `\u{1b}` |
| `List<T>`, `Bytes` | `[1, 2]` |
| `Map<K, V>` | `{"a": 1}` |
| tuple | `(1, "a")` |
| `Option`, `Result` | `Some(1)`, `None`, `Ok(1)`, `Err("x")` |
| `Input` | `Data([104, 105])`, `Eof` |
| record | `Point { x: 1, y: 2 }` (fields in declaration order; `Empty {}` without fields) |
| enum | `Shape.Circle(2)`, `Shape.Empty` |
| quantity | `1500 ms`, `16 KiB`, `99.9%` |
| `IoError` | `IoError("connection refused")` |
| fn value | `<fn>` |
| handles | `<TcpStream>`, `<TcpListener>`, `<PipeStream>`, `<Store>` |

```ignite
type Point { x: Int, y: Int }

test "debug text" {
  assert Point { x = 1, y = 2 }.debug() == "Point {{ x: 1, y: 2 }}"
  assert [Some(1), None].debug() == "[Some(1), None]"
  assert "say \"hi\"".debug() == "\"say \\\"hi\\\"\""
  assert (1, true).debug() == "(1, true)"
}
```

## Substrate and system types

These types come from the substrate (the IO layer compiled programs link) and from system
declarations. They are listed here for completeness; their operations are in the
[std.net](std/net.md) reference and the [Effects](effects.md) page.

| type | what it is |
|---|---|
| `TcpListener` | a bound TCP listening socket: `accept()`, `local_port()`; created by `TcpListener.bind(address, port)` or a `resource` |
| `TcpStream` | a TCP connection: `read(n)`, `write_all(b)`, `close()`, `set_idle_timeout(ms)`, `write_urgent(b)`, `urgent_pending()`; `TcpStream.connect(address, port)` |
| `PipeStream` | one end of an in-process pipe from `Pipe.pair()` (tests only); the same methods as `TcpStream` plus `received_batches()` |
| `Input` | the result of a read: `Data(Bytes)` or `Eof` |
| `Store<T>` | shared in-memory storage keyed by `Int`: `next_id()`, `get(id)`, `put(id, v)`, `remove(id)`, `values()`, `update(id, f)`, `upsert(id, f)`; a `resource`, or `Store.new()` in tests |
| `Stdin`, `Stdout`, `Stderr`, `Process`, `Clock`, `Pipe` | static owners only: `Stdin.read(n)`, `Stdout.write_all(b)`, `Stderr.write_all(b)`, `Process.exit(code)`, `Process.flush()`, `Process.open_files_limit()`, `Clock.monotonic()`, `Clock.now()`, `Clock.sleep(d)`, `Pipe.pair()` |
| `Ref<K>`, `Ref` | a reference to a member of a system, `System.member`; `Ref` is the kind-erased form |
| `Signal<T>` | an output field of a component kind |
| `SystemRef`, `Protocol` | a system or protocol named as a value |
| `Module<I>` | a record `I` of fns filled from a module by `module("path")`; see [Modules](modules.md#module-bindings) |
| `Exposure` | `Loopback`, `Private` or `Public`, for a `TcpListener` resource |

Handles are shared: a copy of a `TcpStream` or a `Store` refers to the same socket or
storage. Every other value behaves as a copy.
