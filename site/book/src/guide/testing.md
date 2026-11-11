# Test it

Ignite behaviour is tested inline: a `test` block sits in the same file as the code it
checks, and `ignite test` builds and runs those blocks.

## Write a test

Create `clamp.ignite`:

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

test "clamp of a reversed range" {
  assert clamp(5, 10, 0) == 5, "a reversed range"
}
```

- `test "name" { … }` is a declaration at the top level. Its body is ordinary code.
- `assert cond` and `assert cond, "message"` check a condition. For `a == b` and `a != b` the
  comparison is structural: field by field, variant and payload, element by element.
- A test may perform any effect, `spawn` included, without a `requires` clause.
- `?` works in a test body: on `Err(e)` or `None` the test fails and reports where.
- `test fn`, `test type`, `test enum` and `test const` declare helpers that exist only in test
  builds.

[Tests and assertions](../reference/tests.md) has the exact rules.

## Run it

```sh
ignite test clamp.ignite
```

The second test fails, because `clamp` checks `low` first:

```text
running 2 tests
test main/clamp keeps values inside the range ... ok
test main/clamp of a reversed range ... FAILED

failures:

---- main/clamp of a reversed range stderr ----
error: assert failed at clamp.ignite:18:3: clamp(5, 10, 0) == 5
  message: a reversed range
  left: 10
  right: 5

failures:
    main/clamp of a reversed range

test result: FAILED. 1 passed; 1 failed; 0 filtered out; finished in 0.00s
```

A file without a `package` line is the module `main`, so the tests' full names start with
`main/`. `ignite test` exits 1 when a test failed, 0 when every selected test passed.

A filter selects the tests whose name contains it. `--list` prints them and runs nothing:

```sh
ignite test clamp.ignite inside
ignite test clamp.ignite reversed --list
```

```text
running 1 test
test main/clamp keeps values inside the range ... ok

test result: ok. 1 passed; 0 failed; 1 filtered out; finished in 0.00s
main/clamp of a reversed range: test
1 test
```

Every option of `ignite test` is in the
[command-line reference](../reference/cli.md#ignite-test).

## Test a protocol without a network

`Pipe.pair()` in [`std.net`](../reference/std/net.md#pipe-and-pipestream) gives two connected
in-memory streams. The Telnet and HTTP roles are generic over any `ByteStream`, so a test can
talk to the real protocol code without opening a port. `examples/terminal/commands.ignite`
ends with one, shortened here:

```ignite,fragment
test "the terminal transcript over a pipe" {
  let (client, server) = Pipe.pair()
  client.set_idle_timeout(5000)?
  spawn Telnet.serve(server, SessionLimits {}, Console { prompt = "terminal> ", respond = respond, maximum_line = 4096 })
  assert read_once(client)? == "terminal> "
  client.write_all(b"help\r\n")?
  assert read_once(client)? == "help\r\necho <text>\r\nstatus\r\nquit\r\nterminal> "
  client.write_all(b"quit\r\n")?
  assert read_once(client)? == "bye\r\n"
}
```

`spawn` runs the server role in a task of its own. The test plays the client on the other end.
Run the example's tests:

```sh
ignite test examples/terminal/commands.ignite
```

```text
running 4 tests
test examples.terminal.commands/parse reads the four commands by their first word ... ok
test examples.terminal.commands/answer replies to each command ... ok
test examples.terminal.commands/respond trims the line and names unknown commands ... ok
test examples.terminal.commands/the terminal transcript over a pipe ... ok

test result: ok. 4 passed; 0 failed; 0 filtered out; finished in 0.00s
```

[Write a protocol and test it over a pipe](../howto/protocol.md) builds one from scratch.

## Both targets

`--target all` runs the same tests on the LLVM and the Rust target. With the pre-built
`ignite`, run it in the [toolchain image](../howto/container.md#tests-on-both-targets); an
`ignite` installed from source runs it directly:

```sh
ignite test clamp.ignite --target all
```

Next: [plan, explain, run and replay](operate.md) the full declaration of the terminal.
