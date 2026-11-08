# Modules and imports

A module is one `.ignite` file. A program is a main file, its sibling modules and the
standard library. There is no package manager and no project file: the compiler finds every
module from the file it is given.

## The `package` line

A file may start with `package a.b.c`, its module path.

- In the standard library the package must equal the file's path: `std/protocol/telnet.ignite`
  is `package std.protocol.telnet` (E031 otherwise).
- In a program, the main file's package is the prefix of its sibling modules. Its last
  segment names the generated program (for the Rust target, the Cargo package).

## Import forms

| form | brings into scope |
|---|---|
| `import std.text.split` | one item |
| `import std.text.{split, contains}` | the listed items; line breaks are allowed inside the braces |
| `import std.option.*` | every exported item of the module (standard library only) |

```ignite
import std.text.{split, contains}
import std.option.*
import std.json.Json

fn first_word(line: Text) -> Text = line.split(" ").at(0)

test "imports" {
  assert first_word("hello world") == "hello"
  assert "abc".contains("b")
  assert Int.parse("x").unwrap_or(0) == 0
  let nothing: Json = Null
  assert nothing == Json.Null
}
```

- An import names an item, not a module: `import std.text` alone is E031. An item the module
  does not have is E031 ("module `std.text` has no `Json`").
- Importing an enum also brings its variants into scope: after `import std.json.Json`,
  `Null` is a name.
- Importing a record or enum makes its associated fns available too:
  `import std.contract.Availability` gives `Availability.validate(..)`.
- Builtin types and their operations (`List`, `TcpStream`, `Stdout.write_all`, ...) need no
  import.
- `std.net` and `std.io` are partly substrate builtins. Importing a builtin name from them is
  allowed and only checks that the name exists (E031 "`std.net` has no builtin `Nope`").

## Name conflicts

- Declaring a name that is also imported is E004 ("duplicate definition of `split`, which is
  imported from `std.text`").
- Importing one name explicitly from two modules is E004 ("`unwrap_or` is already imported
  from `std.option`").
- A name that two glob imports both provide is not an error until it is used. The use is
  E041 ("provided by the glob imports of ...; import it explicitly"). An explicit import or
  an own declaration of the name wins over every glob.

```ignite,fails=E041
import std.option.*
import std.result.*

fn port(text: Text) -> Int = Int.parse(text).unwrap_or(0)
```

`std.option` and `std.result` both define `unwrap_or` and `map`. Import those by name from
the module you mean, and the rest by glob.

## The standard library root

`std.a.b` is the file `std/a/b.ignite` under the standard library root. By default the CLI reads
`PREFIX/share/ignite/std` for a driver at `PREFIX/bin/ignite` when that directory exists (the
layout of a release archive), else the `std/` directory of the source tree it was built from
(an `ignite` installed with `cargo install` keeps reading its clone); `--std DIR` names another
root. The modules are listed
in the [standard library reference](std/index.md).

## Sibling modules

A sibling module is a file next to the main file whose package extends the main file's
package by one segment. With the main file `app.ignite` in `package demo`:

```text
demo/
  app.ignite      package demo
  util.ignite     package demo.util
  names.ignite    package demo.names
```

```ignite,fragment
package demo.util

fn double(n: Int) -> Int = n * 2
```

```ignite,fragment
package demo.names

import demo.util.double

fn four() -> Int = double(2)
```

```ignite,fragment
package demo

import demo.util.{double}
import demo.names.four

fn quad(n: Int) -> Int = double(double(n))

test "siblings" {
  assert quad(2) == 8
  assert four() == 4
}
```

- Sibling modules import each other with the same import forms, except glob: a glob import
  of a sibling is E031.
- A sibling is found from its file name: `demo.util` is `util.ignite` in the main file's
  directory. Files in the directory with another package are ignored.
- A sibling given as the file (`ignite check util.ignite`) loads the siblings it imports:
  `demo.names` is `names.ignite` beside it.
- Another program file of the package can be imported as a module: an import of
  `demo.reader` loads `reader.ignite` when that file declares `package demo`. Its functions,
  kinds, tests and systems are those of the module `demo.reader`; `ignite build app.ignite`
  builds the systems of `app.ignite` only. Only imported program files are loaded.
- Importing any module that is neither standard library nor a sibling is E031 ("only
  standard-library (`std.*`) modules and the sibling modules of the program's package can be
  imported").
- `ignite check`, `ignite test` and `ignite build` of the main file check every sibling
  module it imports; `ignite test` also runs their tests. The [terminal example](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.ignite)
  keeps its commands in two siblings.

## Visibility

- Every top-level item of a module is exported: types, enums, fns, constants, protocols and
  systems. There is no `pub`.
- Test-only items (`test fn`, `test type`, `test enum`, `test const`) are not exported.
  Importing one is E051. See [Tests and assertions](tests.md#test-only-items).
- Imports are not re-exported. `std.console` imports `is_whitespace` from `std.text`, and
  `import std.console.is_whitespace` is E031 ("module `std.console` does not export
  `is_whitespace`; imports are not re-exported, import it from `std.text`").

## Module bindings

`module("path")` is a value: a record whose fields are all of fn type, filled from the
public fns of the module `path`. Each field takes the fn of the same name, which must have
the field's type. A missing fn or a type mismatch is E102.

```ignite,fragment
package demo

type Math { double: fn(Int) -> Int }

fn math() -> Math = module("demo.util")

test "a module binding" {
  let m = math()
  assert m.double(3) == 6
}
```

- The expected type decides which record is filled. It can be a fn's return type, a field's
  type or an annotated `let`.
- The path is a sibling module or a standard library module.
- `Module<I>` is the type of a declared component's `implementation` field: the record `I`
  filled by `module(..)` in a system declaration. See
  [std.console](std/console.md) for `CommandConsole`.

## Method calls and UFCS

`x.f(args)` looks for `f` in this order:

1. a builtin method of `x`'s type (`len`, `push`, `to_text`, ...);
2. a field `f` of fn type, when `x` is a record: the field's value is called with `args`;
3. a module-level fn `f` in scope (declared in this module or imported, never a local)
   whose first parameter accepts `x`: it is called as `f(x, args..)`. This is uniform
   function call syntax (UFCS).

```ignite
import std.text.split

type Greeter { greet: fn(Text) -> Text }

fn shout(text: Text, marks: Int) -> Text {
  var out = text
  var i = 0
  while i < marks {
    out = out + "!"
    i = i + 1
  }
  out
}

test "method calls" {
  assert "a,b".split(",").len() == 2
  assert "hi".shout(2) == "hi!!"
  let g = Greeter { greet = |name| "hello " + name }
  assert g.greet("you") == "hello you"
}
```

- `"a,b".split(",")` is UFCS of the imported `split`; `.len()` is a builtin.
- `std.text` and the other helper modules work this way: import the fn, call it as a method.
- A builtin method name is never looked up by 2 or 3. On a type without that builtin it is
  E041.
- A fn field and a module-level fn that both match is E041 (ambiguous).
- Declaring a fn named like a builtin method of its first parameter's type is E041 at the
  declaration: `fn len(t: Text) -> Int` can never be called as a method.
- Arity is checked with the receiver counted. This also applies to a fn field
  that shares its name with a module-level fn in scope: `m.double(3)` with an imported
  `double(n: Int)` is E003 (two arguments). Give fn fields names that no imported fn uses.
- Associated fns (`fn Server.local()`) are not methods: they are called on the type name,
  `Server.local()`.
