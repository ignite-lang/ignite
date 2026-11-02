### 6.1 System lowering

A `system` produces one executable per executable component (section 14.5): a component
whose kind `K` is a record type with `fn K.main(self: K) -> Result<Unit, IoError>`. The
compiler-known kinds `Service` and `Client` of slices 1 to 9 are retired in slice 10: their
accept loop, connect and exit codes are Telos in `std.protocol.telnet.TelnetServer`,
`std.protocol.telnet.TelnetClient` and `std.protocol.http.HttpServer` (section 14.6), and
the backends know no component kind by name.

```telos
system Terminal {
  resource ingress: TcpListener { address = "127.0.0.1", port = 2323 }
  component console: CommandConsole { prompt = "terminal> ", commands = module("examples.terminal.commands") }
  component server: TelnetServer { listener = ingress, application = console, executable = "terminald" }
  component client: TelnetClient { target = ingress, executable = "tn" }
}
```

The generated `main` of an executable (the Rust shown is the Rust target's; the LLVM
target's `main_<executable>.ll` does the same through `telos_rt_*`, `docs/llvm.md`
section 6):

```rust
// src/bin/terminald.rs
use terminal::*;
use telos_std::protocol::telnet::TelnetServer;      // the program's imports are repeated here
fn main() {
    std::process::exit(telos_io::run(|| {
        let port = telos_io::args::port()?;
        let ingress = telos_io::TcpListener::bind(String::from("127.0.0.1"), port.unwrap_or(2323))?;
        telos_io::log(&format!("listening on 127.0.0.1:{}", ingress.local_port()));
        let console = CommandConsole { prompt: String::from("terminal> "), commands: ConsoleCommands { respond: ... }, .. };
        let server = TelnetServer { listener: ingress, application: console.clone(), executable: String::from("terminald"), .. };
        TelnetServer__main(&server)
    }))
}

// src/bin/tn.rs
fn main() {
    std::process::exit(telos_io::run(|| {
        let port = telos_io::args::port()?;
        let ingress_endpoint = Endpoint { address: String::from("127.0.0.1"), port: port.unwrap_or(2323) };
        let client = TelnetClient { target: ingress_endpoint.clone(), executable: String::from("tn") };
        TelnetClient__main(&client)
    }))
}
```

In order: `telos_io::run`; every `Store` resource the component (or an embedded component)
uses, created first (`let s: telos_io::Store<T> = telos_io::Store::new();`); every
`TcpListener` resource it uses as a listener, bound and logged as `listening on
<address>:<port>` (the first one takes the `--port` override; `--port 0` binds a free port
and the line reports it); the `Endpoint` value of every `TcpListener` resource it names
where an `Endpoint` is expected (section 14.5; `--port` applies to it too); the embedded
components' values in dependency order; the component's own value with the kind's defaults
for omitted fields, `executable` defaulting to the component name and a `Signal` output
set to its reference text; then `K.main(value)`. A resource whose name is one of `main`'s
own (`application`, `port`, `stream`, `error`) gets a trailing `_`.

Exit statuses (design section 11.7):

| status | source |
|---|---|
| 0 | `K.main` returned `Ok` |
| 1 | bind failed, or `K.main` returned an `IoError` without an exit code of its own |
| 2 | `--port` malformed or out of range |
| 3 | `TelnetClient.main`: connecting to the target failed (`IoError.with_exit_code(3)`) |
| 4 | `TelnetClient.main`: the `connect` role returned `Err` (`with_exit_code(4)`) |

A `K.main` chooses its exit codes with `IoError.with_exit_code` (section 7); the generated
`main` only maps `Err(e)` to `e.exit_code` after printing `error: <message>` to stderr. A
resource of an unknown kind or a `system` in a std module is E020 from the type checker;
the component rules are E020 and E104 to E109 (section 14.11). A program with no `system`
produces a library crate with no executable.

