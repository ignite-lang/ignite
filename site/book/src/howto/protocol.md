# Write a protocol and test it over a pipe

A protocol is a module-level `protocol` declaration with the roles `serve` and `connect`. A
role that is generic over `S: ByteStream` runs on a TCP stream in a program and on an
in-memory pipe in a test, so its tests need no port. This page writes a server role and tests
it over `Pipe.pair()`. [Protocols and roles](../design/protocols.md) explains the model.

## Write the role and its tests

Create `shout.ignite`:

```ignite
package shout

import std.text.to_upper

type Shouting {
  suffix: Text = "!"
}

protocol Shout {
  fn serve<S: ByteStream>(stream: S, application: Shouting) -> Result<Unit, IoError> requires [
    stream.read, stream.write
  ] {
    loop {
      match stream.read(4096)? {
        Eof => return stream.close()
        Data(bytes) => {
          let line = bytes.to_text().trim()
          stream.write_all("{line.to_upper()}{application.suffix}\r\n".to_bytes())?
        }
      }
    }
  }
}

test fn read_text<S: ByteStream>(stream: S) -> Result<Text, IoError> requires [stream.read] = match stream.read(4096)? {
  Data(bytes) => Ok(bytes.to_text())
  Eof => Ok("")
}

test "the server shouts every line back" {
  let (client, server) = Pipe.pair()
  client.set_idle_timeout(5000)?
  spawn Shout.serve(server, Shouting {})
  client.write_all(b"hello\r\n")?
  assert read_text(client)? == "HELLO!\r\n"
  client.write_all(b"again\r\n")?
  assert read_text(client)? == "AGAIN!\r\n"
}

test "the server closes when the client does" {
  let (client, server) = Pipe.pair()
  client.set_idle_timeout(5000)?
  spawn Shout.serve(server, Shouting { suffix = "?" })
  client.write_all(b"done\r\n")?
  assert read_text(client)? == "DONE?\r\n"
  client.close()?
}
```

- `serve` takes the stream and the application record. A role with three parameters also takes
  a limits record, as `Telnet.serve` does. Any other role name or shape is
  [E030](../reference/diagnostics.md#e030).
- `requires [stream.read, stream.write]` declares the role's effects on its stream.
- `Pipe.pair()` returns two connected ends. `spawn` runs the server end in its own task, and
  the test drives the client end.
- `set_idle_timeout(5000)` ends a read that waits more than 5 seconds, so a server that never
  answers fails the test instead of hanging it.
- `test fn read_text` is a helper that exists only in test builds.

[`std.net`](../reference/std/net.md#bytestream) lists the methods of `ByteStream`, and
[Pipe and PipeStream](../reference/std/net.md#pipe-and-pipestream) says how a pipe delivers
writes.

## Run the tests on both targets

```sh
ignite test shout.ignite --target all
```

```text
running 2 tests
test shout/the server shouts every line back ... ok
test shout/the server closes when the client does ... ok

test result: ok. 2 passed; 0 failed; 0 filtered out; finished in 0.00s
running 2 tests
test shout/the server shouts every line back ... ok
test shout/the server closes when the client does ... ok

test result: ok. 2 passed; 0 failed; 0 filtered out; finished in 0.00s
```

The first block is the LLVM target, the second the Rust target. The Rust target needs the IO
substrate's crate. The [toolchain image](container.md) and an `ignite` installed from source
have it. With the pre-built binary alone, run `ignite test shout.ignite`, which tests on the
LLVM target only.

The Telnet and HTTP session tests in `std/protocol/` run this way on both targets. One test in
each module stays on loopback TCP, so the `TcpStream` instance is exercised too.

## Serve it on a socket

In a program, a kind's `main` accepts connections and spawns the role on each `TcpStream`.
[Write a component kind](component-kind.md) shows that accept loop. To claim that a protocol
meets an RFC, see [Check a protocol against its RFC](conformance.md).
