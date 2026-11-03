# Standard library

The standard library (std) is Ignite code. Every module except the two substrate
modules is a file under
[`std/`](https://github.com/ignite-lang/ignite/tree/main/std): module `std.a.b` is
the file `std/a/b.ignite`. The compiler has no special knowledge of text handling, JSON,
consoles, contracts or deployment. Those are ordinary records, enums and functions, checked
and tested like your own code.

Protocols live here too. Telnet and HTTP are Ignite modules under `std/protocol/`, and so are
the component kinds that speak them (`TelnetServer`, `HttpServer`). The compiler and the
runtime know sockets and byte streams, not application protocols.

## Modules

| module | layer | import | purpose |
|---|---|---|---|
| `std.net` | base (substrate) | `import std.net.{TcpStream, Endpoint}` | sockets, in-process pipes, the `ByteStream` interface and the `Endpoint` record; see [std.net](net.md) |
| `std.io` | base (substrate) | `import std.io.IoError` | the other builtins of the substrate: standard input and output, `Input`, `IoError`, `Process`; it has no file and no page of its own |
| `std.text` | base | `import std.text.{split, trim_end}` | text operations called as methods: `split`, `find`, `contains`, `to_lower`, `lines`, ...; see [std.text](text.md) |
| `std.option` | base | `import std.option.{unwrap_or, map}` | helpers on `Option`: `unwrap_or`, `map`, `and_then`, `ok_or`, ...; see [std.option and std.result](option-result.md) |
| `std.result` | base | `import std.result.{map_err, ok}` | helpers on `Result`: `unwrap_or`, `map`, `map_err`, `ok`, ...; see [std.option and std.result](option-result.md) |
| `std.json` | base | `import std.json.{Json, parse, encode}` | JSON values with integer numbers (RFC 8259): `encode`, `parse` and member accessors; see [std.json](json.md) |
| `std.sli` | leaf | `import std.sli.{ProbeAttempt, meets}` | probe attempts, outcomes, windows, missing data and the target check; see [std.sli](sli.md) |
| `std.time` | leaf | `import std.time.{month_start, zone_known}` | civil time for calendar windows in `UTC` and `Europe/Berlin`; see [std.time](time.md) |
| `std.policy` | leaf | `import std.policy.{Restart, DrainRollout}` | restart and rollout policies and their backoff arithmetic; see [std.policy](policy.md) |
| `std.console` | general | `import std.console.{Console, Reply}` | a line-oriented console application, independent of the protocol that carries it; see [std.console](console.md) |
| `std.contract` | general | `import std.contract.{Availability, SLA}` | contracts over a probe's attempts: `Availability`, `ThresholdCompliance`, `SLA`; see [std.contract](contract.md) |
| `std.deploy` | general | `import std.deploy.{Deployment, nativeProcess}` | assemblies: where each member of a system runs; see [std.deploy](deploy.md) |
| `std.cli` | general | `import std.cli.{Application, Command}` | the declaration of a command-line application; see [std.cli](cli.md) |
| `std.control` | general | `import std.control.{Machine, step}` | the pure reducers `ignite run` steps: supervisor, probe scheduler, evaluator, rollout, and the apply transition; see [std.control](control.md) |
| `std.protocol.telnet` | protocol | `import std.protocol.telnet.{Telnet, TelnetServer}` | `protocol Telnet`, the kinds `TelnetServer`, `TelnetClient` and the probe `TelnetPrompt`; see [std.protocol.telnet](telnet.md) |
| `std.protocol.http` | protocol | `import std.protocol.http.{Http, HttpServer}` | `protocol Http` (an HTTP/1.1 server), `Request`, `Response`, `HttpLimits`, the kind `HttpServer`; see [std.protocol.http](http.md) |

## Importing

- `import std.text.{split, trim_end}` imports two names. `import std.json.Json` imports one.
  `import std.text.*` imports every name the module defines.
- A module exports every record, alias, enum (with its variants), function, constant and
  protocol it defines. Imports are not re-exported.
- An imported enum brings its variants into scope: after `import std.json.Json` you can write
  `Null` or `Str("x")`.
- `std.net` and `std.io` are the substrate. Their names are builtins, usable without any
  import; an import from them only checks that the name is a builtin.
- An unknown module or name is E031. See [Modules and imports](../modules.md) and
  [Diagnostics](../diagnostics.md).

Most std functions are called as methods. A call `x.f(args)` that is not a builtin method and
not a field of function type resolves to an imported function `f` whose first parameter
accepts `x` (uniform function call syntax). So once `split` is imported, `line.split(" ")`
is `split(line, " ")`.

```ignite
import std.text.{split, to_upper}
import std.option.unwrap_or

test "std functions are called as methods" {
  assert "a,b,,c".split(",") == ["a", "b", "", "c"]
  assert split("a,b,,c", ",") == ["a", "b", "", "c"]
  assert "MiXeD 42 ä".to_upper() == "MIXED 42 ä"
  assert Some(1).unwrap_or(2) == 1
}
```

`std.option` and `std.result` share names (`unwrap_or`, `map`, `is_some`/`is_ok`, ...). A
module that needs both imports the names it uses by name from one of the two: importing both
with `*` makes a call such as `x.map(f)` ambiguous (E041).

## Layers

The standard library has four layers. A module may import only from its own layer or a lower
one.

| layer | modules | may import |
|---|---|---|
| base | `std.io`, `std.net`, `std.option`, `std.result`, `std.text`, `std.json` | base |
| leaf | `std.sli`, `std.time`, `std.policy` | base, leaf |
| general | `std.control`, `std.contract`, `std.deploy`, `std.console`, `std.cli` | base, leaf, general |
| protocol | every `std.protocol.*` | every layer |

In plain words:

- A general module knows no application protocol. Only `std.protocol.*` modules import a
  protocol module, and only they declare names that contain a protocol name.
- A component kind that speaks one protocol (a server, a client, a probe) lives in that
  protocol's module.
- The leaf modules import nothing, so the reducers of `std.control` can read their
  vocabulary without an import cycle (E032).

The rule is enforced by a test of the compiler; the full statement is
[`docs/layering.md`](https://github.com/ignite-lang/ignite/blob/main/docs/layering.md).

## Tests

Every std module carries inline `test` blocks next to the code they test; they are the best
examples of each function's behaviour. `ignite test std/text.ignite` runs one file's tests
(`--target all` runs them on both targets). See [Testing with ignite test](../../guide/testing.md) and
[Tests and assertions](../tests.md).
