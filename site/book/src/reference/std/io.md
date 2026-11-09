# std.io

The substrate builtins that are not network, files or processes: standard input, output and
error, the process's own operations, the clocks, the in-memory `Store`, the `IoError` every
substrate operation fails with, and the free function `log`. The substrate is the Rust crate
`ignite-io`, with the same code on every target.

- Layer: base (substrate).
- Source: none. `std.io` has no file under `std/`. The builtins are specified in
  [`docs/slice.md`](https://github.com/ignite-lang/ignite/blob/main/docs/slice.md) sections 4.4,
  5.1, 7, 11.3, 21.20, 25.3 and 28.
- Import: none needed. The names are visible in every module. `import std.io.IoError` is
  accepted and only checks that the name is a builtin; the std modules write it.

`Input`, the result of a read, is described on [std.net](net.md#input).

## Effects

`stdin`, `stdout`, `stderr`, `process` and `clock` are ambient heads: a function names them in
its `requires` without a parameter, and no parameter may take one of these names (E056). See
[Effects](../effects.md).

| operation | effect it needs |
|---|---|
| `Stdin.read(n)` | `stdin.read` |
| `Stdin.is_terminal()`, `Stdin.set_raw(on)` | `stdin.mode` |
| `Stdout.write_all(b)` | `stdout.write` |
| `Stderr.write_all(b)` | `stderr.write` |
| `Process.exit(code)` | `process.exit` |
| `Process.open_files_limit()` | `process.limit` |
| `Clock.monotonic()`, `Clock.now()` | `clock.read` |
| `Clock.sleep(d)` | `clock.sleep` |
| `Process.flush()`, `log(t)`, `IoError.new(m)` | none |
| `s.get(id)`, `s.values()` on a `Store` | `s.read` |
| `s.next_id()`, `s.put`, `s.remove`, `s.update`, `s.upsert` on a `Store` | `s.write` |

An `HttpApp` handler may use only the effects of its state (`requires [state.*]`). It cannot
call the clocks or `Process.open_files_limit` (E017). It may call `log` and `Process.flush`.

## Standard streams

```ignite,fragment
Stdin.read(maximum: Int) -> Result<Input, IoError>
Stdin.is_terminal() -> Result<Bool, IoError>
Stdin.set_raw(on: Bool) -> Result<Unit, IoError>
Stdout.write_all(data: Bytes) -> Result<Unit, IoError>
Stderr.write_all(data: Bytes) -> Result<Unit, IoError>
fn log(line: Text) -> Unit
```

| operation | what it does |
|---|---|
| `Stdin.read` | one read of up to `maximum` bytes: `Data(bytes)`, or `Eof` at the end of input |
| `Stdin.is_terminal` | whether standard input is a terminal |
| `Stdin.set_raw` | `true` switches the terminal to raw mode, `false` restores the saved mode |
| `Stdout.write_all` | writes `data` to standard output and flushes |
| `Stderr.write_all` | writes `data` to standard error and flushes |
| `log` | writes `line` and a newline to standard error in one write |

- `Stdin`, `Stdout` and `Stderr` are static owners only. They have no values, and using one
  as a type is E040.
- Raw mode is non-canonical, without echo, signals, flow control or CR translation, one byte
  per read. Output processing stays on. The saved mode is restored when `main` ends, also
  with `Err` or a panic. A process killed by a signal cannot restore it. On an input that is
  no terminal both calls do nothing and return `Ok`.
- In `ignite test` the runner captures standard error per test and shows it when the test
  fails.

## Process

```ignite,fragment
Process.exit(code: Int) -> Never
Process.flush() -> Unit
Process.open_files_limit() -> Int
```

| operation | what it does |
|---|---|
| `Process.exit` | restores the terminal mode, sends what the calling task wrote, and exits with `code` |
| `Process.flush` | sends what the calling task wrote to its streams, as a wait or the end of the task would |
| `Process.open_files_limit` | the open-files soft limit; the largest `Int` when unlimited, 0 when the system cannot report it |

- `Process.exit` has type `Never`, so it needs no `?` and fits any branch.
- A program raises its open-files soft limit to the hard limit before `K.main` runs, on both
  targets.
- When the program ends, every live child process is killed; see
  [std.process](process.md#child-processes).

## Clock

```ignite,fragment
Clock.monotonic() -> Duration
Clock.now() -> Duration
Clock.sleep(duration: Duration) -> Unit
```

| operation | what it does |
|---|---|
| `Clock.monotonic` | a monotonic instant, for measuring intervals |
| `Clock.now` | the wall clock since 1970-01-01T00:00:00Z, the instant of [std.time](time.md); `Clock.now().to_ms()` is epoch milliseconds |
| `Clock.sleep` | sends what the calling task wrote, then blocks the task for `duration`; zero or less does not block |

Under `ignite run --clock manual` a program's clocks stay real. The manual clock times only the
driver's reducers.

## IoError

```ignite,fragment
IoError.new(message: Text) -> IoError
fn with_exit_code(error: IoError, code: Int) -> IoError
```

The error of every substrate operation. `error.message` is its text. `IoError.new` makes one,
so Ignite code can fail with `return Err(IoError.new("not a port"))`.

A component's `main` that returns `Err(error)` ends the program with the message on standard
error and exit status 1. `error.with_exit_code(n)` chooses another status: `TelnetClient`
exits 3 when it cannot connect.

## Store

```ignite,fragment
resource todos: Store<Todo>
```

Shared in-memory storage of values of type `T`, keyed by `Int`. A system declares a store as a
resource without properties. Tests and test-only items make one with `Store.new()` (E051
elsewhere), its `T` taken from the expected type.

| method | result | what it does |
|---|---|---|
| `next_id()` | `Int` | the next id of this store: 1, 2, 3, …, atomic |
| `get(id)` | `Option<T>` | a copy of the value at `id` |
| `put(id, value)` | `Unit` | inserts or replaces a copy of `value` |
| `remove(id)` | `Option<T>` | removes the value and returns it |
| `values()` | `List<T>` | copies of every value, in ascending id order |
| `update(id, f: fn(T) -> T)` | `Option<T>` | replaces the value with `f` of it and returns the new value; `None` when `id` holds nothing |
| `upsert(id, f: fn(Option<T>) -> T)` | `T` | stores `f` of the current value, `None` when `id` holds nothing, and returns it |

- `f` is pure. `update` and `upsert` are atomic for their id: another task that reaches the
  same id waits until `f` is done.
- `upsert` is the atomic insert-or-update. Two tasks that upsert one absent id lose neither
  write. An `update` followed by a `put` can lose one.
- A store is a handle: copies share one storage. It lives as long as the process; nothing is
  written to disk.

## Example

```ignite
import std.option.unwrap_or

test "a store keeps values by id" {
  let names: Store<Text> = Store.new()
  let id = names.next_id()
  names.put(id, "milk")
  assert names.get(id) == Some("milk")
  assert names.update(id, |name| name.concat("!")) == Some("milk!")
  assert names.update(99, |name| name) == None
  assert names.upsert(5, |old| old.unwrap_or("new")) == "new"
  assert names.values() == ["milk!", "new"]
  assert names.remove(id) == Some("milk!")
}

test "an error carries its message and exit status" {
  let error = IoError.new("not a port").with_exit_code(2)
  assert error.message == "not a port"
}
```
