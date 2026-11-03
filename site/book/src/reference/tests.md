# Tests and assertions

Tests are part of the language. A `test` block sits next to the code it tests, in any module,
and `ignite test` builds and runs it on either target. This page gives the language rules;
the workflow (filters, targets, CI) is in the [testing guide](../guide/testing.md).

## Test blocks

```ignite
fn parse_port(text: Text) -> Result<Int, Text> = match Int.parse(text) {
  Some(n) => if n >= 0 && n <= 65535 { Ok(n) } else { Err("out of range: {n}") }
  None => Err("not a number: {text}")
}

test "parse_port reads a port" {
  assert parse_port("8080") == Ok(8080)
  assert parse_port("x") == Err("not a number: x")
}

test "parse_port rejects large numbers" {
  let result = parse_port("70000")
  assert result == Err("out of range: 70000"), "got {result.debug()}"
}
```

- `test "name" { .. }` is a top-level declaration. Its name is a plain string literal;
  escapes are decoded. An empty or interpolated name is E053.
- Tests take no part in the module's namespace. Two tests of one module with the same name
  are E050; a test may share its name with a fn or type.
- A test body is typed like the body of a `fn` with no parameters returning `Unit`.
  `return` ends the test, and it passes.
- A test is an effect root: it may read stdin, open sockets and `spawn` without a `requires`
  clause. See [Effects](effects.md).
- A test's full name is `<module>/<name>`: `std.text/split_once splits at the first
  separator`, or `main/<name>` in a file without `package`.

## `assert`

`assert cond` and `assert cond, message` are statements. They work in every block, inside
tests and outside them.

- `cond` must be `Bool` (E052). `message` must be `Text` (E040); it is usually an
  interpolated string.
- When the condition is false, the task ends with a failure. In a test the test fails; in a
  program's main task the process exits with status 101.
- For `assert a == b` and `assert a != b`, the comparison at the top of the condition, both
  sides are evaluated once and kept for the report. Both sides need one type with structural
  equality (E040 for two types, E045 for a type without equality; see
  [Types](types.md#equality)).

## `?` in tests

In a test body, `?` takes a `Result` or an `Option`. On `Ok(x)` or `Some(x)` its value is
`x`. On `Err(e)` or `None` the test fails at once; `?` does not return a value from the test.

```ignite
test "? in a test" {
  let n = Int.parse("12")?
  assert n == 12
  let bytes = "hi".to_bytes()
  let first = bytes.at(0).to_text()
  assert first == "104"
  let (a, b) = Pipe.pair()
  a.write_all(bytes)?
  a.close()?
  assert b.read(16)? == Data(b"hi")
}
```

In a fn, `?` keeps its normal rule: `Result` only, in a fn that returns a `Result` with the
same error type (see [Effects](effects.md#-and-failure)).

## Test-only items

`test fn`, `test type` (a record), `test enum` and `test const` declare helpers that exist
only for tests.

```ignite
fn double(n: Int) -> Int = n * 2

test type Case { input: Int, expected: Int }

test const CASES: List<Case> = [
  Case { input = 0, expected = 0 },
  Case { input = 21, expected = 42 }
]

test fn passes(c: Case) -> Bool = double(c.input) == c.expected

test "double, table-driven" {
  for c in CASES {
    assert passes(c), "case {c.input}"
  }
}
```

- A test-only item shares the module namespace (E004 on a clash with another name).
- It is visible only inside tests and other test-only items of its own module. Using it
  anywhere else, by name, call, method call, type, literal, variant or pattern, is E051.
- Test-only items are not exported: importing one is E051.
- `test type N = T` is E001. An alias is no code and needs no marker.
- `Store.new()` and `Pipe.pair()` are allowed only in tests and test-only items (E051
  elsewhere). `Store.new()` gives a fresh, empty `Store<T>`; `Pipe.pair()` gives two
  connected in-memory stream ends, `(PipeStream, PipeStream)`, so protocol code can be
  tested without sockets.

```ignite,fails=E051
test fn helper() -> Int = 1

fn production() -> Int = helper()
```

## Failures

Every failure ends the task that ran it with one report of the form
`<operation> failed at <file>:<line>:<column>: <detail>`. If `parse_port("70000")` above returned
`Ok(70000)`, the second test would report:

```text
error: assert failed at app.ignite:13:3: result == Err("out of range: 70000")
  message: got Ok(70000)
  left: Ok(70000)
  right: Err("out of range: 70000")
```

- `message:` appears only when the assert has a message; `left:` and `right:` only for
  `==` and `!=`. The position is that of the `assert` keyword.
- A failed `?` reports `? failed at <file>:<line>:<column>: Err(<e>)`, or `None` for an
  `Option`. The position is that of the `?`.
- Overflow, division by zero, an index out of range and a `match` without a matching arm
  fail the same way, with their own operation name.
- A value in a report is shown by its `to_text` when it has one (builtin, or a pure
  module-level `to_text(value: T) -> Text` in scope), otherwise by its
  [debug text](types.md#debug-text). An `IoError` shows its message.
- The main task prints `error: <report>` on stderr and exits with status 101. A spawned
  task logs `task ended with error: <report>` and the program keeps running.

## What `ignite test` does

- `ignite check` types tests and test-only items like all other code, so they cannot rot.
- `ignite build` and `ignite emit` drop every test and test-only item after checking. A
  built program contains no test code.
- `ignite test FILE..` builds one test executable per target for all files given. It
  includes the tests of the named files only; the standard library modules they load are
  built without their tests.
- Each test runs in its own child process, with a timeout (60 s by default). Exit status 0
  passes; 101 fails; a timeout, another exit status or a signal also fails. Output is shown
  for failing tests only.
- `ignite test` exits 0 when every selected test passed (or none exist), 1 when a test
  failed, and 2 on a compile error.

The command's flags are on the [command line](cli.md) page.
