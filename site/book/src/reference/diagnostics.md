# Diagnostics

Every error Ignite reports about a program has a code, `E001` to `E151`. This page lists
each code, what it means and which command reports it. Most codes come with a small program
that triggers it. E032 and E125 show an excerpt. E054, E114, E119, E126 to E128, E142 and
E151 show the output of a repository fixture.

## Reading a diagnostic

A diagnostic is one line on standard error:

```text
file:line:column: error[Ennn]: message
```

Line and column start at 1. Diagnostics are sorted by position. The parser recovers at the
next top-level keyword after a syntax error, so one run can report several errors.

Most messages say how to fix the problem. For example:

```ignite,fails=E006
fn count() -> Int {
  let total = 0
  total = total + 1
  return total
}
```

```text
example.ignite:3:3: error[E006]: cannot assign to immutable binding `total`; declare it with `var`
```

## Which command reports what

Every command that reads a program first parses and checks it, so the codes of the
language stages (syntax, names, effects, types, system declarations) come from `ignite
check`, `ignite test`, `ignite build`, `ignite plan`, `ignite explain`, `ignite run` and
`ignite replay` alike. The other codes belong to one phase.

| codes | phase | reported by |
|---|---|---|
| E001 to E089, E096, E099 | syntax, names, effects, types, and E020, an unsupported or malformed declaration | every command that reads a program |
| E091, literal data | a catalogue constant, from the name checker | every command that reads a program |
| E100 to E109, E116, E140, E141 | system declarations and capabilities, from the type checker | every command that reads a program |
| E090 to E092 | conformance claims, from the evaluator | `check`, `conformance` |
| E110 to E113, E115, E117, E118 | the compile-time evaluator | `check`, `plan`, `explain`, `run`, `replay` |
| E114 | selecting an assembly | `plan`, `explain`, `run`, `replay` |
| E119 | selecting an explain path | `explain` |
| E120 to E124, E129, E150, E151 | run-time rules checked ahead of time | `check`, `plan`, `explain`, `run`, `replay` |
| E125 to E128 | running and replaying | `run`, `replay` |
| E142 | a setting a started member needs | `run`, `plan --for-apply` |

Exit status when a program has an error:

| command | exit status |
|---|---|
| `ignite check`, `ignite emit`, `ignite build`, `ignite plan`, `ignite explain`, `ignite replay` | 1 |
| `ignite test`, `ignite run`, `ignite conformance` | 2 |

`ignite check` prints nothing and exits 0 when the program is clean. A file that cannot be
read is no diagnostic: it prints `error: <file>: <reason>` and exits 3. See
[Command line](cli.md) for every command's options and exit statuses.

The examples on this page marked checked are complete programs. Each one is checked against
the standard library with `ignite check`, and the output shown below it is the real output,
with the file named `example.ignite`. Most fixtures in the repository have more cases. Each
section links them.

## Syntax

Reported by the parser, before any name is resolved. Fixtures:
[`crates/ignite-syntax/tests/fixtures`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite-syntax/tests/fixtures).

### E001 Syntax error {#e001}

The source does not follow the [grammar](grammar.md). E001 also covers a malformed string
interpolation or byte literal, an integer literal that does not fit in 64 bits, a refutable
pattern in `let` (`let Some(x) = o`), a one-element tuple `(1,)`, a keyword used as a
variable, fn or type name, a bound on a record's type parameter, and `await`, which Ignite
does not have.

```ignite,fails=E001
fn main() requires [stdin.read] {
  let input = await Stdin.read(16)
}
```

```text
example.ignite:2:15: error[E001]: remove `await`: effectful calls need no marker
```

### E002 Unexpected end of file {#e002}

The file ends in the middle of a declaration, usually because a `}` is missing.

```ignite,fails=E002
fn open() {
  loop {
    break
  }
```

```text
example.ignite:5:1: error[E002]: unexpected end of file, expected `}`
```

### E076 Rest pattern misplaced {#e076}

A list pattern holds at most one `..`, and a named rest `..name` must be its last element.
The two messages are ``a list pattern has at most one `..` `` and the one below. See
[rest patterns](statements.md#list-patterns-and-rest).

```ignite,fails=E076
fn ends(xs: List<Int>) -> Int = match xs {
  [first, .., last] => first + last
  [..init, last] => last
  _ => 0
}
```

```text
example.ignite:3:4: error[E076]: `..name` must be the last element of a list pattern
```

### E053 Test name empty or interpolated {#e053}

A test name must be a plain, non-empty string literal. See [Tests and assertions](tests.md).

```ignite,fails=E053
fn one() -> Int = 1

test "" {
  assert one() == 1
}
```

```text
example.ignite:3:6: error[E053]: a test name cannot be empty; describe what the test checks
```

### E054 Nesting too deep {#e054}

Brackets, type arguments, prefix operators or `return` nested deeper than 256 levels, or
string literals nested deeper than 32 interpolations. The message is `nesting deeper than
256; split the expression`. The fixture is a five-line program whose second line nests 257 parentheses:
[`e054_nesting.ignite`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite-syntax/tests/fixtures/e054_nesting.ignite).

## Names and modules

Reported by the name checker. Fixtures:
[`crates/ignite-check/tests/fixtures`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite-check/tests/fixtures).
See [Modules and imports](modules.md).

### E003 Unknown name {#e003}

A name that nothing declares or imports. E003 also covers: a call with the wrong number of
arguments (a UFCS call counts the receiver), `log` used as a value, a constant used as a
pattern with payload patterns, an associated fn whose owner is no record or enum of the
module, a qualified variant pattern the enum does not declare, and a bare variant of an
enum that is not in scope where the expected type does not decide it (from the type
checker).

```ignite,fails=E003
fn f() -> Int {
  let a = 1
  return a + b
}
```

```text
example.ignite:3:14: error[E003]: unknown name `b`
```

### E004 Duplicate definition {#e004}

Two declarations with one name. This includes a declaration named like a builtin
(`Tuple`, `Bytes`, `PipeStream`, ...), a role or record field declared twice, a repeated
type parameter, an associated fn declared twice or named like a variant of its owner, a
constant named like a variant, and a repeated closure parameter.

```ignite,fails=E004
fn twice() -> Int {
  return 1
}

fn twice() -> Int {
  return 2
}
```

```text
example.ignite:5:4: error[E004]: duplicate definition of `twice`
```

### E005 Ambiguous variant {#e005}

A bare variant name that several enums in scope declare, where the expected type does not
decide which one is meant. Qualify it. Reported by the type checker.

```ignite,fails=E005
enum Action { Continue, Close }
enum Flow { Continue, Stop }

fn f() -> Int {
  let undecided = Continue
  return 0
}
```

```text
example.ignite:5:19: error[E005]: ambiguous variant `Continue`: declared by `Action`, `Flow`; qualify it, e.g. `Action.Continue`
```

### E006 Assignment to something immutable {#e006}

Assignment to a `let` binding, a parameter or a constant. Also `push`, `append` or `clear`
through a `let`, a `for` element or a pattern binding, on `x.to_bytes()`, or on a binding a
closure captured. Only a place rooted in a `var` or a parameter changes in place. The
example under [Reading a diagnostic](#reading-a-diagnostic) shows the `let` case.

```ignite,fails=E006
fn bump(n: Int) -> Int {
  n = n + 1
  return n
}
```

```text
example.ignite:2:3: error[E006]: cannot assign to parameter `n`; parameters are immutable
```

### E031 Unknown module or imported name {#e031}

An import of a module that does not exist, of a name the module does not have, or from a
module that is neither `std.*` nor a sibling of the program's package. Imports are not
re-exported: import a name from the module that declares it.

```ignite,fails=E031
import std.nope.{X}
```

```text
example.ignite:1:8: error[E031]: unknown module `std.nope`
```

### E032 Import cycle {#e032}

Modules that import each other in a circle. It needs several files, so the example is a
fragment: `app.ignite` (package `demo`) imports `demo.a`, and the two siblings import each
other.

```ignite,fragment
// a.ignite
package demo.a

import demo.b.{g}

fn f() -> Int = g()

// b.ignite
package demo.b

import demo.a.{f}

fn g() -> Int = 1

fn h() -> Int = f()
```

```text
./a.ignite:3:1: error[E032]: import cycle: demo.a -> demo.b -> demo.a
```

### E049 Binding inside an or-pattern {#e049}

An or-pattern `a | b` cannot bind names. Match the alternatives in separate arms.

```ignite,fails=E049
enum Shape { Dot, Circle(Int), Square(Int) }

fn size(shape: Shape) -> Int {
  return match shape {
    Circle(r) | Square(r) => r
    Dot => 0
  }
}
```

```text
example.ignite:5:12: error[E049]: `r` is bound inside an or-pattern; an or-pattern cannot bind names, match the alternatives in separate arms
example.ignite:5:24: error[E049]: `r` is bound inside an or-pattern; an or-pattern cannot bind names, match the alternatives in separate arms
```

### E050 Duplicate test name {#e050}

Two tests of one module have the same name.

```ignite,fails=E050
fn one() -> Int = 1

test "one" {
  assert one() == 1
}

test "one" {
  assert one() != 2
}
```

```text
example.ignite:7:6: error[E050]: duplicate test name "one"; test names are unique within a module
```

### E051 Test-only item outside tests {#e051}

Code that is not a test or a test-only item uses a test-only item, or an import names one.
`Store.new()` and `Pipe.pair()` are test-only too: a program's stores are resources, and
its streams come from a listener or a connect. A UFCS use is reported by the type checker.

```ignite,fails=E051
test fn helper(x: Int) -> Int = x

fn uses_fn() -> Int = helper(1)
```

```text
example.ignite:3:23: error[E051]: `helper` is test-only; only tests and other test-only items can use it
```

### E055 Record that contains itself {#e055}

A record that contains itself through fields and tuples alone, so a value would be
infinite. Recursion through an enum payload, `Option`, `Result` or `List` is allowed.

```ignite,fails=E055
type Node {
  value: Int
  next: Node
}
```

```text
example.ignite:1:6: error[E055]: record `Node` contains itself through fields and tuples alone, so a value would be infinite; put the nested `Node` in an `Option`, an enum payload or a `List`
```

### E056 Parameter named like an ambient capability {#e056}

A parameter of a fn, role, fn type or closure may not be named `stdin`, `stdout`,
`process` or `net`. See [Effects](effects.md).

```ignite,fails=E056
fn helper(stdin: Int) -> Int = stdin
```

```text
example.ignite:1:11: error[E056]: parameter `stdin` is named like the ambient capability `stdin`; rename it
```

### E057 Unused type parameter {#e057}

A type parameter of a generic record, enum or alias that no field, payload or aliased type
uses.

```ignite,fails=E057
type Marker<T> {
  n: Int
}
```

```text
example.ignite:1:13: error[E057]: type parameter `T` of `Marker` is never used; remove it or use it in a field
```

### E099 Malformed bound {#e099}

A bound that names an interface other than `ByteStream`, a type parameter with more than
one bound, or `ByteStream` used as a type. `ByteStream` is the only interface. Use it as a
bound, `<S: ByteStream>`.

```ignite,fails=E099
fn as_type(stream: ByteStream) -> Int = 0
```

```text
example.ignite:1:20: error[E099]: `ByteStream` is an interface, not a type; declare a type parameter `<S: ByteStream>`
```

## Effects and protocols

Reported by the name checker (E010 to E016 and E018 to E030) and the type checker (E017). See
[Effects](effects.md).

### E010 Effectful call in a pure context {#e010}

An effectful call or a `spawn` where only pure code is allowed: the value of a constant or
the default of a record field.

```ignite,fails=E010
fn fetch() -> Int requires [net] {
  return 1
}

const FETCHED: Int = fetch()
```

```text
example.ignite:5:22: error[E010]: effectful call `fetch` outside an effectful function: the value of constant `FETCHED` is pure
```

### E013 Spawn of something that is not an effectful call {#e013}

`spawn` takes a call of an effectful user function or protocol role. A pure function, a
fn value, a literal or a substrate operation such as `stream.close()` cannot be spawned.

```ignite,fails=E013
fn double(n: Int) -> Int = n * 2

fn start() -> Result<Unit, IoError> requires [stdout.write] {
  spawn double(1)
  return Ok(Unit)
}
```

```text
example.ignite:4:3: error[E013]: `spawn` needs a call to an effectful function or protocol role
```

### E015 Constant cycle {#e015}

A constant whose value depends on itself, directly or through other constants. A cycle
through a function body (`const A: Int = f()` with `fn f() -> Int = A + 1`) is not detected.

```ignite,fails=E015
const A: Int = B * 2
const B: Int = A - 1
```

```text
example.ignite:1:7: error[E015]: constant `A` refers to itself: `A` -> `B` -> `A`
example.ignite:2:7: error[E015]: constant `B` refers to itself: `B` -> `A` -> `B`
```

### E016 Effect missing from requires {#e016}

A function with a `requires` list performs an effect the list does not name. A function
without `requires` has its effects inferred instead. A closure's effects are charged where
the closure is created.

```ignite,fails=E016
fn greet() -> Result<Unit, IoError> requires [stdin.read] = Stdout.write_all("hi".to_bytes())
```

```text
example.ignite:1:61: error[E016]: missing effect `stdout.write` for `Stdout.write_all`; add it to the `requires` of `greet`
```

### E017 Effectful function used as a value {#e017}

An effectful function or closure used as a value where no effectful fn type covering its
effects is expected. Give the binding or field an effectful fn type
(`fn(..) -> R requires [..]`). Also a fn passed to `List.map` whose effects the calling
function's `requires` does not cover, or that has an effect on its own parameter. Reported by
the type checker.

```ignite,fails=E017
fn send(stream: TcpStream) -> Result<Unit, IoError> requires [stream.write] {
  return stream.write_all("hi".to_bytes())
}

fn run(stream: TcpStream) -> Result<Unit, IoError> requires [stream.write] {
  let handler = send
  return send(stream)
}
```

```text
example.ignite:6:17: error[E017]: effectful function `send` is not a value here; only an effectful fn type (`fn(..) -> R requires [..]`) accepts it
```

### E030 Protocol role missing or malformed {#e030}

A protocol without a role, a function in a protocol that is not `serve` or `connect`, a
role without `requires`, a role with the wrong number of parameters, or a call to a role
the protocol does not declare. `serve` takes `(stream, application)` or
`(stream, limits, application)`; `connect` takes `(stream)`.

```ignite,fails=E030
protocol Empty {}
```

```text
example.ignite:1:10: error[E030]: protocol `Empty` declares no role; declare `serve` or `connect`
```

## Types

Reported by the type checker. Fixtures:
[`crates/ignite-types/tests/fixtures`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite-types/tests/fixtures).
See [Types](types.md).

### E040 Type mismatch {#e040}

A value of one type where another is expected: an argument, a return value, a `match` arm,
an assignment, a pattern, or `?` on something that is no `Result` (or, in a test, no
`Option`).

```ignite,fails=E040
fn f() -> Int {
  var n = 0
  n = "zero"
  return n
}
```

```text
example.ignite:3:7: error[E040]: type mismatch: expected `Int`, found `Text`
```

### E041 Unknown method or field {#e041}

The receiver's type has no such method or field. E041 also covers an ambiguous or
inapplicable method call, a tuple index past the tuple, a fn named like a builtin method of
its first parameter's type (so `x.f(..)` could never call it), and a name two glob imports
provide, used without an explicit import.

```ignite,fails=E041
fn f(t: Text) -> Int = t.size()
```

```text
example.ignite:1:26: error[E041]: `Text` has no method `size`
```

`std.option` and `std.result` share names such as `unwrap_or`, so a glob import of both
is E041 where such a name is used:

```ignite,fails=E041
import std.option.*
import std.result.*

fn f(o: Option<Int>) -> Int = unwrap_or(o, 0)
```

```text
example.ignite:4:31: error[E041]: `unwrap_or` is provided by the glob imports of `std.option`, `std.result`; import it explicitly
```

### E042 Type cannot be inferred {#e042}

Neither the expected type nor a later use decides a type: an empty list, `None`, `Ok(..)`,
`Err(..)`, a type parameter, or a closure parameter without annotation. Also an effectful
operation whose receiver type is not declared, so its effects cannot be checked. Add a type
annotation.

```ignite,fails=E042
fn f() -> Int {
  let xs = []
  return 0
}
```

```text
example.ignite:2:12: error[E042]: cannot infer the element type of `[]`; add a type annotation
```

### E043 Wrong number of arguments {#e043}

A call of a builtin method or a fn value with the wrong number of arguments (for a call of
a declared function this is E003). Also a range pattern whose start is above its end.

```ignite,fails=E043
fn f(t: Text) -> Int = t.len(1)
```

```text
example.ignite:1:24: error[E043]: wrong number of arguments to `len`: expected 0, found 1
```

### E044 Byte out of range {#e044}

An integer literal used as a `Byte` or a `U32` that is outside its range: 0 to 255 for `Byte`,
0 to 4294967295 for `U32`.

```ignite,fails=E044
const BIG: Byte = 256
```

```text
example.ignite:1:19: error[E044]: integer literal 256 is out of range for `Byte` (0 to 255)
```

### E045 Operator not defined for the type {#e045}

An operator applied to a type that does not support it. This includes `==` on a type that
is not comparable (one that holds a function, handle, store, `IoError` or type parameter),
a constant pattern of a type without `==`, `%` on a quantity, and a range bound that is not
a literal or a constant with a literal value.

```ignite,fails=E045
fn f(b: Bool) -> Bool = b < b
```

```text
example.ignite:1:25: error[E045]: operator `<` is not defined for `Bool`
```

### E046 Match not exhaustive {#e046}

A `match` that does not cover every value. The message names the missing cases. Ranges
split `Int` and `Byte` into intervals. Literal, list and constant patterns need a
catch-all arm.

```ignite,fails=E046
enum Command { Help, Echo(Text), Quit }

fn name(command: Command) -> Text {
  return match command {
    Command.Help => "help"
    Command.Echo(text) => text
  }
}
```

```text
example.ignite:4:10: error[E046]: match does not cover `Command.Quit`
```

### E047 Record literal field {#e047}

A record literal with a missing field that has no default, an unknown field, or a field
given twice. Inside a system, also a repeated map key or a `TcpListener` property it does
not know.

```ignite,fails=E047
type Point {
  x: Int
  y: Int
}

fn partial() -> Point {
  return Point { x = 0 }
}
```

```text
example.ignite:7:10: error[E047]: missing field `y` in `Point` literal; it has no default
```

### E048 Unknown type {#e048}

An unknown type name, the wrong number of type arguments, or a type alias that refers to
itself.

```ignite,fails=E048
type Bag {
  label: Shape
}
```

```text
example.ignite:2:10: error[E048]: unknown type `Shape`
```

### E052 Assert condition not Bool {#e052}

The condition of `assert` must be a `Bool`. See [Tests and assertions](tests.md).

```ignite,fails=E052
fn check(x: Int) {
  assert x
}
```

```text
example.ignite:2:10: error[E052]: `assert` needs a `Bool` condition, found `Int`
```

### E096 ByteStream bound not met {#e096}

A call passes a type that does not implement `ByteStream` where a type parameter bounded by
`ByteStream` is expected. Pass a `TcpStream`, a `PipeStream` or a type parameter that has
the bound itself.

```ignite,fails=E096
fn same<S: ByteStream>(stream: S) -> S = stream

fn with_int() -> Int = same(1)
```

```text
example.ignite:3:24: error[E096]: `Int` does not implement `ByteStream`, which `S` of `same` requires; pass a `TcpStream`, a `PipeStream` or a type parameter bounded by `ByteStream`
```

### E140 Path leaves its directory {#e140}

A `Text` literal passed as the path of a `Directory` method is absolute, has a `..`
component, or holds a NUL. The message ends in `absolute`, `leaves its root` or
`contains NUL`. A path that is not a literal is checked at run time instead. See
[`std.fs`](std/fs.md).

```ignite,fails=E140
fn load(files: Directory) -> Result<Bytes, IoError> requires [files.read] {
  return files.read_file("../secrets")
}
```

```text
example.ignite:2:26: error[E140]: path `../secrets` of `read_file`: leaves its root
```

## System declarations

Systems, components, resources, contracts and assemblies are checked by the type checker,
so these codes come from every command that reads a program. Fixtures:
[`crates/ignite-types/tests/fixtures`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite-types/tests/fixtures).
See [Systems and assemblies](../design/systems.md).

### E020 Unsupported or malformed declaration {#e020}

An unsupported or malformed resource, component, contract, policy or assembly: a resource
kind other than `TcpListener`, `Store`, `Directory`, `Launcher`, `Entropy`, `Setting`,
`CommandLine` or `Environment`, a kind that is no record type, two components with one
executable name, or a `system` in a standard-library module.

```ignite,fails=E020
system S {
  component c: Int {}
}
```

```text
example.ignite:2:16: error[E020]: unsupported component kind `Int`: a kind is a record type declared in the standard library or the program
```

### E141 Flag used twice {#e141}

The `flag` of a `Setting` or a `Directory` is also the flag of another resource of the
system, or it is `--port` or `--ref`, which the generated `main` reads itself. The other
message is ``flag `--port` of resource `<name>` is reserved: `--port` and `--ref` are the
substrate's own``. See [Compiled executables](cli.md#compiled-executables).

```ignite,fails=E141
system Tool {
  resource data: Directory { path = "data", flag = "--root" }
  resource mode: Setting { flag = "--root" }
}
```

```text
example.ignite:3:35: error[E141]: flag `--root` of resource `mode` is also the flag of resource `data`; each flag names one resource
```

### E100 Brace literal without an expected type {#e100}

A brace literal `{ .. }` needs an expected type, a record or a `Map`. Write the type name
before `{` or annotate the binding.

```ignite,fails=E100
fn f() -> Int {
  let m = { admin = 1, guest = 2 }
  return 0
}
```

```text
example.ignite:2:11: error[E100]: a brace literal `{ .. }` needs an expected type (a record or a `Map`); write the type name before `{` or annotate the binding
```

### E101 Quantity units do not fit {#e101}

An operation that mixes quantity types, or a quantity with an `Int` where that is not
allowed. `+`, `-` and comparisons take two values of one quantity type. `*` and `/` take a
quantity and an `Int`.

```ignite,fails=E101
fn f(d: Duration, b: ByteSize) -> Duration = d + b
```

```text
example.ignite:1:46: error[E101]: operator `+` mixes `Duration` and `ByteSize`: quantity arithmetic takes two values of one quantity type (`+`, `-`, comparisons) or a quantity and an `Int` (`*`, `/`)
```

### E102 module(..) cannot be filled {#e102}

`module("p")` names a module that does not exist, or the record it fills has a field the
module has no fn for, a fn of the wrong type, or a field that is not of fn type.

```ignite,fails=E102
type Ops {
  origin: fn() -> Int
}

fn ops() -> Module<Ops> = module("std.nowhere")
```

```text
example.ignite:5:34: error[E102]: module `std.nowhere` not found; `module(..)` names a standard-library module or a sibling file `<name>.ignite` of the program's package
```

### E103 Named argument misuse {#e103}

A named argument with an unknown name, given twice, followed by a positional argument, or
used in a call that is not a direct call of a fn declared in Ignite.

```ignite,fails=E103
fn area(width: Int, height: Int) -> Int = width * height

fn f() -> Int = area(width = 2, depth = 3)
```

```text
example.ignite:3:33: error[E103]: `area` has no parameter `depth`; its parameters are `width`, `height`
```

### E104 Component or resource property {#e104}

A property of a component or resource is wrong: a required field is missing, an output field
of type `Signal<T>` is set, or `exposure = Loopback` is given with an address that is not a
loopback address. For the capability resources E104 also covers a `Directory` `path` that is
empty or holds a NUL, a `Launcher` `maximum_children` outside 1 to 1024, a `flag` that is not
`--` followed by lowercase letters, digits or `-`, and a property that is no literal where a
literal is needed. An unknown property is E047.

```ignite,fails=E104
system S {
  resource ingress: TcpListener {
    port = 7071
  }
}
```

```text
example.ignite:2:3: error[E104]: required field `address` of `TcpListener` is missing
```

### E105 Component cycle {#e105}

Components that use each other through bare names. A component embeds the values it
names, so the chain must end.

```ignite,fails=E105
type Node {
  name: Text
  next: Option<Node>
}

system S {
  component a: Node { name = "a", next = Some(b) }
  component b: Node { name = "b", next = Some(a) }
}
```

```text
example.ignite:7:13: error[E105]: components use each other through bare names: a -> b -> a; a component embeds the values it names, so the chain must end
```

### E106 Map key type {#e106}

A `Map` key must be `Int`, `Byte`, `Text`, `Bool`, an enum without payloads or `Ref`.

```ignite,fails=E106
fn f(by_list: Map<List<Int>, Int>) -> Int = 0
```

```text
example.ignite:1:19: error[E106]: `List<Int>` cannot be a `Map` key; keys are `Int`, `Byte`, `Text`, `Bool`, an enum without payloads or `Ref`
```

### E107 Quantity literal out of range {#e107}

A quantity literal with more than 4 decimals, or one that is not a whole number of the base
unit or does not fit in an `Int`.

```ignite,fails=E107
fn f() -> Duration = 1.23456 seconds
```

```text
example.ignite:1:22: error[E107]: quantity literal `1.23456 seconds` is out of range for `Duration`: it has more than 4 decimals
```

### E108 Assembly binding key {#e108}

A key of an assembly's `bindings` that is not a member of the assembly's system.

```ignite,fails=E108
type Leaf {
  value: Int
}

type Deployment {
  system: SystemRef
  bindings: Map<Ref, Text>
}

system S {
  component c: Leaf { value = 1 }
}

assembly local: Deployment {
  system = S
  bindings = {
    S.c = "native"
    S.x = "nowhere"
  }
}
```

```text
example.ignite:18:5: error[E108]: `S.x` is not a member of system `S`
```

### E109 Hook signature {#e109}

An executable kind's `main` has the wrong signature, or a `machine` or `validate` hook has
the wrong signature or performs an effect. The expected signatures are
`fn K.main(self: K) -> Result<Unit, IoError>`, `fn K.machine(self: K, id: Text) -> Machine`
and `fn K.validate(self: K) -> List<Text>` (pure).

```ignite,fails=E109
type Worker {
  name: Text
}

fn Worker.main(self: Worker) -> Int = 0

system S {
  component w: Worker { name = "w" }
}
```

```text
example.ignite:8:16: error[E109]: `Worker.main` must have the signature `fn Worker.main(self: Worker) -> Result<Unit, IoError>`, found `fn(Worker) -> Int`
```

### E116 Effectful property expression {#e116}

A property expression inside a `system` must be pure: no effectful call, effectful
operation or `spawn`.

```ignite,fails=E116
type Probe {
  input: Result<Input, IoError>
}

fn noisy() -> Result<Input, IoError> requires [stdin.read] = Stdin.read(1)

system S {
  component probe: Probe { input = noisy() }
}
```

```text
example.ignite:8:36: error[E116]: effectful call `noisy` in the property `input` of component `probe`: a property expression is pure
```

## Conformance claims

A protocol that declares `conforms [Spec]` makes a claim about every requirement of that spec
in its scope ([grammar](grammar.md#protocol-clauses)). The evaluator checks the claim's
statuses. `ignite check` and `ignite conformance` report these codes. E091's first rule comes
from the name checker, so every command that reads a program reports it. Fixtures:
[`crates/ignite/tests/fixtures`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite/tests/fixtures),
named `e090_*`, `e091_*` and `e092_*`. See [`ignite conformance`](cli.md#ignite-conformance).

### E090 Requirement without a status, or with two {#e090}

A MUST-class requirement in the scope of a conforming protocol has no status: no
declaration names it in `satisfies`, and the protocol does not mark it `unsupported` or
`not_applicable`. MUST class is `MUST`, `MUST NOT`, `REQUIRED`, `SHALL` and `SHALL NOT`. The
other message is for a requirement with two statuses, such as `satisfies` and `unsupported`:
``requirement `RFC7002.R2_2` has two statuses in `Echo`: unsupported and not_applicable``.

```ignite,fails=E090
import std.rfc.{Level, Requirement, Spec}

const RFC7001: Spec = Spec {
  rfc = 7001
  title = "A line echo"
  url = "https://example.invalid/rfc7001"
  digest = "sha256:7001"
  requirements = [R2_1]
}

const R2_1: Requirement = Requirement {
  rfc = 7001
  section = "2"
  ordinal = 1
  level = Must
  text = "A server MUST echo every line."
}

protocol Echo conforms [RFC7001] {
  fn serve(stream: TcpStream, application: fn(Text) -> Text) -> Result<Unit, IoError> requires [stream.write] {
    return stream.write_all(b"ok")
  }
}
```

```text
example.ignite:19:25: error[E090]: requirement `RFC7001.R2_1` (MUST, section 2) is in scope of `Echo` and has no status; add `satisfies`, `unsupported` or `not_applicable`
```

### E091 Catalogue not literal data, or inconsistent {#e091}

A constant of type `std.rfc.Requirement` or `std.rfc.Spec` must be literal data: no call, no
interpolated text, no `if`, no name of another constant. The evaluator also checks the
catalogue of the checked file. An ordinal must be 1 or more. A requirement's name must be its
id, for example `R2_1` for section 2, ordinal 1. Each requirement is listed once in the
`Spec`, has the spec's `rfc`, and a module declares one `Spec` constant.

```ignite,fails=E091
import std.rfc.{Level, Requirement, Spec}

fn section() -> Text = "2"

const RFC7001: Spec = Spec {
  rfc = 7001
  title = "A line echo"
  url = "https://example.invalid/rfc7001"
  digest = "sha256:7001"
  requirements = [R2_1]
}

const R2_1: Requirement = Requirement {
  rfc = 7001
  section = section()
  ordinal = 1
  level = Must
  text = "A server MUST echo every line."
}
```

```text
example.ignite:15:13: error[E091]: catalogue constant `R2_1` must be literal data, found a call
```

### E092 Requirement of a spec nobody conforms to {#e092}

A `satisfies`, a `verifies` or a status names a requirement of a spec that its protocol, or
no protocol of its module, conforms to.

```ignite,fails=E092
import std.rfc.rfc854.{RFC854}
import std.rfc.rfc855.{RFC855}

fn answer(line: Text) -> Text satisfies [RFC855.RP1_1] = line

protocol Echo conforms [RFC854] scope { sections = ["99"] } {
  fn serve(stream: TcpStream, application: fn(Text) -> Text) -> Result<Unit, IoError> requires [stream.write] {
    return stream.write_all(b"ok")
  }
}
```

```text
example.ignite:4:42: error[E092]: `RFC855.RP1_1` belongs to RFC 855, which no protocol of this module conforms to
```

## Evaluation and assemblies

The compile-time evaluator computes every property and resolves each assembly through the
`admits` and `includes` functions of its binding type (`std.deploy` for the standard
bindings). `ignite check`, `ignite plan`, `ignite explain`, `ignite run` and `ignite replay`
report these codes, since `run` and `replay` plan first. Fixtures:
[`crates/ignite/tests/fixtures`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite/tests/fixtures).
See [The compile-time evaluator](../design/evaluator.md) and
[Plan, explain, run and replay](../guide/operate.md).

### E110 Member neither bound nor included {#e110}

A resource or component of the system that the assembly neither binds nor includes in a
binding.

```ignite,fails=E110
import std.deploy.{Deployment, localSocket, nativeProcess}

type Worker {
  listener: TcpListener
  executable: Text
}

fn Worker.main(self: Worker) -> Result<Unit, IoError> {
  let stream = self.listener.accept()?
  stream.close()?
  Ok(Unit)
}

system S {
  resource ingress: TcpListener { address = "127.0.0.1", port = 0 }
  resource spare: TcpListener { address = "127.0.0.1", port = 0 }
  component w: Worker { listener = ingress }
}

assembly a: Deployment {
  system = S
  bindings = {
    S.ingress = localSocket()
    S.w = nativeProcess(includes = [])
  }
}
```

```text
example.ignite:22:14: error[E110]: `S.spare` is neither a bindings key nor included by a binding of assembly `a`
```

### E111 Member bound twice {#e111}

A member that is both a bindings key and included by another binding, or included by two
bindings.

```ignite,fails=E111
import std.deploy.{Deployment, localSocket, nativeProcess, workstationExecutable}

type Helper {
  name: Text
}

type Worker {
  listener: TcpListener
  helper: Helper
  executable: Text
}

fn Worker.main(self: Worker) -> Result<Unit, IoError> {
  let stream = self.listener.accept()?
  stream.close()?
  Ok(Unit)
}

system S {
  resource ingress: TcpListener { address = "127.0.0.1", port = 0 }
  component h: Helper { name = "h" }
  component w: Worker { listener = ingress, helper = h }
}

assembly a: Deployment {
  system = S
  bindings = {
    S.ingress = localSocket()
    S.w = nativeProcess(includes = [S.h])
    S.h = workstationExecutable()
  }
}
```

```text
example.ignite:30:5: error[E111]: `S.h` is both a bindings key and included by `S.w`
```

### E112 Binding not admitted {#e112}

The binding's `admits` function refuses the member. The message carries its reason. Also a
contract or policy used as a bindings key.

```ignite,fails=E112
import std.deploy.{Deployment, nativeProcess}

type Worker {
  listener: TcpListener
  executable: Text
}

fn Worker.main(self: Worker) -> Result<Unit, IoError> {
  let stream = self.listener.accept()?
  stream.close()?
  Ok(Unit)
}

system S {
  resource ingress: TcpListener { address = "127.0.0.1", port = 0 }
  component w: Worker { listener = ingress }
}

assembly a: Deployment {
  system = S
  bindings = {
    S.ingress = nativeProcess(includes = [])
    S.w = nativeProcess(includes = [])
  }
}
```

```text
example.ignite:22:5: error[E112]: binding of `S.ingress` is not admitted: a native process hosts an executable component, not a resource
```

### E113 Includes differ from the embedded components {#e113}

A binding's `includes` list differs from the components its member embeds.

```ignite,fails=E113
import std.deploy.{Deployment, localSocket, nativeProcess, workstationExecutable}

type Helper {
  name: Text
}

type Worker {
  listener: TcpListener
  helper: Helper
  executable: Text
}

fn Worker.main(self: Worker) -> Result<Unit, IoError> {
  let stream = self.listener.accept()?
  stream.close()?
  Ok(Unit)
}

system S {
  resource ingress: TcpListener { address = "127.0.0.1", port = 0 }
  component h: Helper { name = "h" }
  component w: Worker { listener = ingress, helper = h }
}

assembly a: Deployment {
  system = S
  bindings = {
    S.ingress = localSocket()
    S.w = nativeProcess(includes = [])
    S.h = workstationExecutable()
  }
}
```

```text
example.ignite:29:5: error[E113]: binding of `S.w` does not include `S.h`, which `S.w` embeds
```

### E114 No assembly or system selectable {#e114}

The program declares several assemblies and the command got no `--assembly`, or
`--assembly` names one that does not exist. `ignite check` accepts such a program, because
it checks every assembly. `ignite plan`, `ignite explain`, `ignite run` and `ignite replay`
must pick one. From the fixture
[`e114_assembly_selection.ignite`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite/tests/fixtures/e114_assembly_selection.ignite),
which declares the assemblies `hosted` and `workstation`:

```text
$ ignite plan e114_assembly_selection.ignite
e114_assembly_selection.ignite:45:1: error[E114]: several assemblies (hosted, workstation) and no `--assembly`: select one
$ ignite plan e114_assembly_selection.ignite --assembly nowhere
e114_assembly_selection.ignite:1:1: error[E114]: no assembly named `nowhere`: the program declares hosted, workstation
```

### E115 Evaluation limit exceeded {#e115}

Evaluating a property ran out of fuel, built a value that is too large, or nested calls too
deeply. The limits are 1,000,000 fuel units (expressions, iterations, calls) per property
expression, 1,000,000 nodes per value and 10,000 nested fn and closure calls. `--fuel N` on
`plan` and `explain` changes the fuel.

```ignite,fails=E115
type Worker {
  steps: Int
}

fn forever(n: Int) -> Int {
  var i = n
  while i >= 0 {
    i = i + 1
  }
  i
}

system S {
  component w: Worker { steps = forever(0) }
}
```

```text
example.ignite:14:33: error[E115]: evaluating S.w.steps: the fuel limit of 1000000 units exceeded in main.forever
```

### E117 Evaluation failed {#e117}

Evaluating a property failed, for example by a division by zero, an overflow, a failed
`assert` or an index out of range. The message carries the same report text a compiled
program prints.

```ignite,fails=E117
type Worker {
  steps: Int
}

fn ratio(a: Int, b: Int) -> Int = a / b

system S {
  component w: Worker { steps = ratio(1, 0) }
}
```

```text
example.ignite:8:33: error[E117]: evaluating S.w.steps: / failed at example.ignite:5:35: 1 / 0
```

### E118 Value the model cannot encode {#e118}

A property's value cannot be written into the system model, for example a closure.

```ignite,fails=E118
type Worker {
  scale: fn(Int) -> Int
}

fn adder(n: Int) -> fn(Int) -> Int = |x| x + n

system S {
  component w: Worker { scale = adder(1) }
}
```

```text
example.ignite:8:33: error[E118]: evaluating S.w.scale: a closure cannot be encoded in the model
```

### E119 Unknown explain path {#e119}

The path given to `ignite explain` names no system, member, property or assembly of the
program. From the fixture
[`e119_unknown_path.ignite`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite/tests/fixtures/e119_unknown_path.ignite):

```text
$ ignite explain e119_unknown_path.ignite S.w.nowhere
e119_unknown_path.ignite:1:1: error[E119]: `S.w.nowhere` names no system, member, property or assembly of this program
```

## Rules for running, checked ahead of time

These rules concern what `ignite run` will do, but they are checked before anything runs:
`ignite check` and `ignite plan` report them for every assembly, and `ignite explain`,
`ignite run` and `ignite replay` report them too. E120, E123 and E124 are checked per system, so a program without an assembly gets
them too. E121, E122 and E129 are checked per assembly. Fixtures:
[`crates/ignite/tests/fixtures`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite/tests/fixtures).

### E120 Declaration refused by validate {#e120}

A declaration that its kind's `validate` hook refuses. The message carries the reason the
hook returns. The standard library's hooks refuse, for example, an unknown outcome or time
zone, `maximum_attempts` below 1, a `target` outside (0 %, 100 %], or a backoff whose
initial delay is above its maximum.

```ignite,fails=E120
type Worker {
  name: Text
  limit: Int = 1
}

fn Worker.validate(self: Worker) -> List<Text> = if self.limit < 1 {
  ["limit must be at least 1"]
} else {
  []
}

system S {
  component w: Worker { name = "w", limit = 0 }
}
```

```text
example.ignite:13:3: error[E120]: declaration `S.w` of kind `main.Worker` is refused: limit must be at least 1
```

### E121 Signal without a producer {#e121}

A contract's `signal` has no producer that the assembly runs: the member that produces it
is not bound to a host that runs it, or the reference is not an output.

```ignite,fails=E121
import std.deploy.Member

type Pinger {
  attempts: Signal<Int>
}

type Uptime {
  signal: Signal<Int>
}

enum Host { Shelf }

fn shelf() -> Host = Shelf

fn admits(binding: Host, member: Member) -> Option<Text> = None

fn includes(binding: Host) -> List<Ref> = []

fn entries(binding: Host) -> List<Text> = []

type Plan {
  system: SystemRef
  bindings: Map<Ref, Host>
}

system S {
  component ping: Pinger {}
  contract up: Uptime { signal = ping.attempts }
}

assembly a: Plan {
  system = S
  bindings = {
    S.ping = shelf()
  }
}
```

```text
example.ignite:28:34: error[E121]: `S.up.signal` has no producer that assembly `a` runs: `S.ping` is not bound to a host that runs it
```

### E122 Unresolvable run-time reference {#e122}

A run-time `Ref`, such as a probe's `target`, names a member the assembly does not run as
a process, or resolves to no endpoint or to several.

```ignite,fails=E122
import std.deploy.Member

type Server {
  listener: TcpListener
  executable: Text
}

fn Server.main(self: Server) -> Result<Unit, IoError> {
  let stream = self.listener.accept()?
  stream.close()?
  Ok(Unit)
}

type Pinger {
  target: Ref
  executable: Text
  attempts: Signal<Int>
}

fn Pinger.main(self: Pinger) -> Result<Unit, IoError> = Ok(Unit)

enum Host { Shelf, Runner }

fn shelf() -> Host = Shelf

fn runner() -> Host = Runner

fn admits(binding: Host, member: Member) -> Option<Text> = None

fn includes(binding: Host) -> List<Ref> = []

fn entries(binding: Host) -> List<Text> = match binding {
  Shelf => []
  Runner => ["main"]
}

type Plan {
  system: SystemRef
  bindings: Map<Ref, Host>
}

system S {
  resource spare: TcpListener { address = "127.0.0.1", port = 0 }
  component shelved: Server { listener = spare }
  component stopped: Pinger { target = shelved }
}

assembly a: Plan {
  system = S
  bindings = {
    S.spare = shelf()
    S.shelved = shelf()
    S.stopped = runner()
  }
}
```

```text
example.ignite:45:40: error[E122]: `S.stopped.target` names `S.shelved`, which assembly `a` does not run as a process
```

### E123 Probe timing {#e123}

A probe's `interval` or `timeout` is not positive, or `timeout` is not below `interval`.
Probes of one member never overlap.

```ignite,fails=E123
type Pinger {
  interval: Duration = 10 seconds
  timeout: Duration = 2 seconds
  attempts: Signal<Int>
}

system S {
  component slow: Pinger { interval = 1 second, timeout = 1 second }
}
```

```text
example.ignite:8:59: error[E123]: `S.slow.timeout` (1 second) must be below `S.slow.interval` (1 second): probes of one member never overlap
```

### E124 Contract window too short {#e124}

A contract's window is shorter than the interval of its signal, so it can hold no expected
attempt.

```ignite,fails=E124
import std.sli.Window

type Pinger {
  interval: Duration = 10 seconds
  attempts: Signal<Int>
}

type Uptime {
  signal: Signal<Int>
  window: Window
}

system S {
  component ping: Pinger { interval = 10 seconds }
  contract up: Uptime { signal = ping.attempts, window = rolling(5 seconds) }
}
```

```text
example.ignite:15:58: error[E124]: `S.up.window` (5 seconds) is shorter than the interval (10 seconds) of its signal `S.ping.attempts` (`S.up.signal`), so it can hold no expected attempt
```

### E129 Hosted member lacks an entry {#e129}

The assembly hosts a member with a binding that runs it, but the member's kind lacks a
function the binding needs. The binding type's `entries` function lists them: in
`std.deploy`, `NativeProcess` needs `K.machine` and `ScheduledProbeHost` needs `K.main` and
`K.machine`.

```ignite,fails=E129
import std.deploy.Member

type Worker {
  name: Text
  executable: Text
}

fn Worker.main(self: Worker) -> Result<Unit, IoError> = Ok(Unit)

enum Host { Runner }

fn runner() -> Host = Runner

fn admits(binding: Host, member: Member) -> Option<Text> = None

fn includes(binding: Host) -> List<Ref> = []

fn entries(binding: Host) -> List<Text> = ["main", "machine"]

type Plan {
  system: SystemRef
  bindings: Map<Ref, Host>
}

system S {
  component w: Worker { name = "w" }
}

assembly a: Plan {
  system = S
  bindings = {
    S.w = runner()
  }
}
```

```text
example.ignite:32:5: error[E129]: `S.w` is hosted by a binding that runs it, but its kind `main.Worker` declares no `Worker.machine`
```

### E150 Contract without classify {#e150}

A contract's machine subscribes to a signal, and the contract's kind declares no
`classify(self, sample: T) -> Bool` for the signal's element type `T`. The evaluator calls
`classify` on every sample to decide whether it was good. See
[System declarations](systems.md#hooks).

```ignite,fails=E150
import std.control.{Machine, Evaluator}
import std.sli.{MissingData, rolling}

type Beat {
  alive: Bool
}

type Pulse {
  interval: Duration = 10 seconds
  beats: Signal<Beat>
}

type Watch {
  signal: Signal<Beat>
}

fn Watch.machine(self: Watch, id: Text) -> Machine = Machine.Evaluate(Evaluator {
  id = "{id}.evaluator"
  contract = id
  signal = self.signal.to_text()
  target = 99%
  window = rolling(1 hour)
  missing_data = MissingData.InsufficientEvidence
  minimum_completeness = 90%
})

system S {
  component p: Pulse {}
  contract c: Watch { signal = p.beats }
}
```

```text
example.ignite:29:32: error[E150]: contract `S.c` of kind `main.Watch` subscribes to `S.p.beats` of `main.Beat` but declares no `fn Watch.classify(self: Watch, sample: main.Beat) -> Bool`
```

### E151 Signal element is no outcome type {#e151}

A signal that the assembly runs and a machine subscribes to carries a type that a probe
cannot print as an outcome: a fn, a handle, a reference or a signal. From the fixture
[`e151_undecodable_element.ignite`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite/tests/fixtures/e151_undecodable_element.ignite):

```text
e151_undecodable_element.ignite:79:32: error[E151]: signal `S.p.beats` carries `fn() -> Int`; a probe outcome holds no fn, handle, reference or signal
```

## Running and replaying

Reported by `ignite run` and `ignite replay`, and E142 also by `ignite plan --for-apply`. `ignite check`
and plain `ignite plan` accept these programs. `ignite run` exits 2 on E125, E126 and E142. `ignite replay` exits 1 on E126 and E127.
The fixtures in
[`crates/ignite/tests/fixtures`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite/tests/fixtures)
name the command on their first line. See [Plan, explain, run and replay](../guide/operate.md).

### E125 Nothing to run {#e125}

`ignite run` on a program without an assembly, or on an assembly that runs nothing (no
member it hosts has a machine).

```ignite,fragment
type Service {
  name: Text
}

fn Service.main(self: Service) -> Result<Unit, IoError> = Ok(Unit)

system S {
  component svc: Service { name = "svc" }
}
```

```text
$ ignite run example.ignite
example.ignite:1:1: error[E125]: `ignite run` needs an assembly, and the program declares none
```

The other message is ``assembly `<name>` runs nothing: no member it hosts has a machine``.

### E126 Journal of another model, or a held lock {#e126}

The state directory's journal was written for another model or another format version, or
another `ignite run` or `ignite host` holds the state directory's lock, or the directory is
the state of an `ignite host`. `ignite run` reports it before anything is built. `ignite replay`
reports it for a journal without a header or written for another model. Use another
`--state`, or `--fresh` to start a new journal.

```text
$ ignite run e126_journal_mismatch.ignite --state DIR --clock manual
e126_journal_mismatch.ignite:1:1: error[E126]: the journal was written for another model: use another --state or --fresh
```

### E127 Journal does not replay {#e127}

A journal that does not replay: a line before the last does not parse, or a record the
reducers derive again differs from the journaled one. The message names the first
differing record by its `seq`. `ignite replay` reports it, and so does `ignite run` when it
resumes from an existing journal. From the fixture
[`e127_replay_diverged.ignite`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite/tests/fixtures/e127_replay_diverged.ignite),
after one journaled effect was altered by hand:

```text
e127_replay_diverged.ignite:1:1: error[E127]: journal record 3 does not replay: journaled {"at":{"ns":0,"type":"Duration"},"kind":"effect","machine":"S.daemon.supervisor","seq":3,"value":{"fields":["S.daemon",2],"type":"std.control.Effect","variant":"Spawn"}}, derived {"at":{"ns":0,"type":"Duration"},"kind":"effect","machine":"S.daemon.supervisor","seq":3,"value":{"fields":["S.daemon",1],"type":"std.control.Effect","variant":"Spawn"}}
```

### E128 Reducer failed at run time {#e128}

A reducer of `std.control` failed while `ignite run` stepped it, for example because it ran
out of fuel (`--fuel` bounds each step). E128 is not a diagnostic line: the run journals an
`error` record that carries the code, stops that machine, keeps running the others and
exits 1 at the end. From the fixture
[`e128_reducer_failed.ignite`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite/tests/fixtures/e128_reducer_failed.ignite),
run with `--fuel 1`:

```text
{"at":{"ns":0,"type":"Duration"},"kind":"error","machine":"S.daemon.supervisor","seq":3,"value":{"code":"E128","report":"reducer of S.daemon.supervisor failed: the fuel limit of 1 units exceeded in std.control.step"}}
```

### E142 Setting without a default {#e142}

A member that `ignite run` or `ignite plan --for-apply` would start uses a `Setting` that has
no `default`. Neither command passes setting values, so the member would exit 2 at every
start. Both refuse the system before anything starts, and `plan --for-apply` before it
contacts the host. Give the setting a `default`. `ignite check` accepts the program. From the
fixture
[`e142_setting_without_default.ignite`](https://github.com/ignite-lang/ignite/blob/main/crates/ignite/tests/fixtures/slice17/e142_setting_without_default.ignite):

```text
slice17/e142_setting_without_default.ignite:1:1: error[E142]: setting `Bench.tool` (`--tool`) has no `default`, and `Bench.runner` uses it: `ignite run` and `ignite apply` pass no setting values, so the member would exit 2 at every start; give it a `default`
```

## Local apply

`ignite host`, `ignite apply`, `ignite inspect` and `ignite recover` have no diagnostic codes
of their own. They answer with their exit statuses, listed in [Command line](cli.md#ignite-host).
`ignite plan --for-apply` checks the program first, so it reports the codes above, E142
among them. `ignite run` on a directory that an `ignite host` owns is E126, and a host's
journal that does not replay is E127.

## Retired and unused codes

| code | state |
|---|---|
| E011 | retired: was an effectful call not awaited; Ignite has no `await` |
| E012 | retired: was `await` on a pure call |
| E014 | retired: was a constant used as a pattern; constant patterns are allowed |
| E021 | reserved, unused |
| E097 | unused; `Store.new()` and `Pipe.pair()` outside a test are E051 |

Every code the compiler emits has an entry on this page.
