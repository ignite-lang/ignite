# Testing with ignite test

Ignite behaviour is tested inline: a `test` block sits in the same file as the code it
checks, and `ignite test` builds and runs those blocks on either target or on both.

## Writing tests

```ignite
fn clamp(value: Int, low: Int, high: Int) -> Int {
  if value < low {
    low
  } else if value > high {
    high
  } else {
    value
  }
}

test "clamp keeps values inside the range" {
  assert clamp(5, 0, 10) == 5
  assert clamp(-3, 0, 10) == 0
  assert clamp(42, 0, 10) == 10, "values above the range become the upper bound"
}
```

- `test "name" { … }` is a declaration at the top level. Its body is ordinary code.
- `assert cond` and `assert cond, "message"` check a condition. For `a == b` and `a != b` the
  comparison is structural: field by field, variant and payload, element by element.
- A test is an effect root: its body may perform any effect, `spawn` included, without a
  `requires` clause. That is how protocol tests open pipes and sockets.
- `?` works in a test body: on `Err(e)` or `None` the test fails and reports where.
- `test fn`, `test type`, `test enum` and `test const` declare helpers that exist only in test
  builds. `ignite build` drops them, and `ignite check` still type-checks them, so they cannot
  rot.

The [Tests and assertions](../reference/tests.md) reference has the exact rules.

## Running tests

```sh
ignite test std/text.ignite
ignite test std/text.ignite split        # only tests whose name contains "split"
ignite test std/text.ignite --list       # list, run nothing
ignite test examples/terminal/commands.ignite
```

Arguments ending in `.ignite` are files; at most one other argument is the filter. All named
files share one test executable per target: one compile and one link. Every module the
files load is built normally, with its own tests dropped.

| flag | meaning |
|---|---|
| `--target rust\|llvm\|all` | which target to build for; `all` runs every supported target with shared preparation (default `rust`) |
| `--exact` | run only the test whose full name equals the filter |
| `--list` | print `<full name>: test` per selected test, run nothing |
| `--jobs N` | tests run at once (default: the CPU count) |
| `--timeout SECS` | a test still running after this is killed and fails (default 60) |
| `--release` | release profile on Rust, clang `-O2` on LLVM |
| `--out DIR` | where the test builds go (default `target/ignite-test`) |
| `--runtime FILE`, `--clang PATH`, `--triple T` | the LLVM runtime archive, clang and target triple |

Each test runs as its own child process, so one test that crashes, hangs or calls
`Process.exit` cannot take the others down. Output is modelled on `cargo test`:

```text
running 1 test
test hello/greet puts the name in the sentence ... ok

test result: ok. 1 passed; 0 failed; 0 filtered out; finished in 0.01s
```

A test's full name is `<module>/<test name>`, for example
`std.text/split_once splits at the first separator`. The output of a test is captured and
shown only when it fails.

## When a test fails

A failed `assert` prints its position, the source of the condition, the message if one was
given, and both sides of a comparison; then the test fails (exit code 101 of the test
process). The report is shown under the test's name in the `failures:` section:

```text
---- main/assert message stderr ----
error: assert failed at fail.ignite:9:3: 1 + 1 == 3
  message: arithmetic
  left: 2
  right: 3
```

A `?` that meets an `Err` or a `None` prints the position and the value it met:

```text
error: ? failed at fail.ignite:13:17: Err(boom)
error: ? failed at fail.ignite:18:17: None
```

A test also fails when it times out (`FAILED (timed out after 60 s)`), exits with another
code or is killed by a signal.

`ignite test` exits 0 when every selected test passed (or none exist, or with `--list`), 1
when a test failed, and 2 for a compile error, with diagnostics as for `ignite check`.

## Both targets

The Rust and LLVM targets must agree, so the repository runs every inline test on both:

```sh
task test-ignite
task test-ignite FILTER=json
```

`task test-ignite` builds the compiler and the LLVM runtime with the test runner
(`cargo build -p ignite-rt --features runner`), then runs every file of `std/` and
`examples/` with `--target=all`. To run the LLVM target by hand, pass that runtime:

```sh
cargo build -p ignite-rt --features runner --target-dir target/ignite-test/rt
ignite test std/json.ignite --target llvm --runtime target/ignite-test/rt/debug/libignite_rt.a
```

## Protocol tests without a network

`Pipe.pair()` in `std.net` returns two connected in-process streams. The Telnet and HTTP
roles are generic over any `ByteStream`, so their session tests run over a pipe, on both
targets, with no port. The
[terminal walkthrough](terminal.md#the-same-transcript-without-a-socket) shows one.
