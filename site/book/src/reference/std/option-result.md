# std.option and std.result

Small helpers on the builtin types `Option<T>` and `Result<T, E>`, called as methods once
imported: `value.bool("done").unwrap_or(false)`. The variants `Some`, `None`, `Ok` and `Err`,
the operators `==` and `!=`, and `?` are builtin and need no import.

- Layer: base.
- Sources: [`std/option.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/option.ignite),
  [`std/result.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/result.ignite).
- Import: `import std.option.{unwrap_or, map}`, `import std.result.{map_err, ok}`.

The two modules use the same names for the same ideas. A module that needs both imports the
names it uses by name, each from one of the two. Importing both with `*` makes a call such as
`x.map(f)` ambiguous (E041).

## std.option

```ignite,fragment
fn unwrap_or<T>(value: Option<T>, fallback: T) -> T
fn map<T, U>(value: Option<T>, f: fn(T) -> U) -> Option<U>
fn and_then<T, U>(value: Option<T>, f: fn(T) -> Option<U>) -> Option<U>
fn is_some<T>(value: Option<T>) -> Bool
fn is_none<T>(value: Option<T>) -> Bool
fn ok_or<T, E>(value: Option<T>, error: E) -> Result<T, E>
```

| function | what it does |
|---|---|
| `unwrap_or` | the value, or `fallback` when there is none |
| `map` | `f` applied to the value, if there is one |
| `and_then` | `f` applied to the value, if there is one; `None` when `f` gives none |
| `is_some`, `is_none` | whether there is a value |
| `ok_or` | `Ok` with the value, or `Err(error)` when there is none |

```ignite
import std.option.*

test "Option helpers" {
  assert Some(1).unwrap_or(2) == 1
  let none: Option<Int> = None
  assert none.unwrap_or(5) == 5
  let offset = 10
  assert Some(1).map(|n| n + offset) == Some(11)
  let half = |n: Int| if n % 2 == 0 { Some(n / 2) } else { None }
  assert Some(4).and_then(half) == Some(2)
  assert Some(3).and_then(half) == None
  assert Some(0).is_some() && none.is_none()
  assert Some(1).ok_or("missing") == Ok(1)
  assert none.ok_or("missing") == Err("missing")
}
```

## std.result

```ignite,fragment
fn unwrap_or<T, E>(value: Result<T, E>, fallback: T) -> T
fn map<T, U, E>(value: Result<T, E>, f: fn(T) -> U) -> Result<U, E>
fn map_err<T, E, F>(value: Result<T, E>, f: fn(E) -> F) -> Result<T, F>
fn is_ok<T, E>(value: Result<T, E>) -> Bool
fn is_err<T, E>(value: Result<T, E>) -> Bool
fn ok<T, E>(value: Result<T, E>) -> Option<T>
```

| function | what it does |
|---|---|
| `unwrap_or` | the `Ok` value, or `fallback` for an error |
| `map` | `f` applied to the `Ok` value; an error stays as it is |
| `map_err` | `f` applied to the error; an `Ok` value stays as it is |
| `is_ok`, `is_err` | which side the result is |
| `ok` | the `Ok` value as an `Option`; the error is dropped |

```ignite
import std.result.*

test "Result helpers" {
  let good: Result<Int, Text> = Ok(2)
  let bad: Result<Int, Text> = Err("no")
  assert good.unwrap_or(7) == 2
  assert bad.unwrap_or(7) == 7
  let factor = 3
  assert good.map(|n| n * factor) == Ok(6)
  assert bad.map(|n| n * factor) == Err("no")
  assert bad.map_err(|e| "error: {e}") == Err("error: no")
  assert good.is_ok() && bad.is_err()
  assert good.ok() == Some(2)
  assert bad.ok() == None
}
```

## Using both

Import each name from the module whose type you call it on:

```ignite
import std.option.{unwrap_or, ok_or}
import std.result.{map_err, is_ok}

test "names from both modules" {
  assert Some(1).unwrap_or(2) == 1
  let parsed: Result<Int, Text> = Some(1).ok_or("missing")
  assert parsed.is_ok()
  assert parsed.map_err(|e| e.len()) == Ok(1)
}
```
