# Standard library

The standard library (std) is Ignite code. Every module but `std.io` is a file under
[`std/`](https://github.com/ignite-lang/ignite/tree/main/std): module `std.a.b` is the file
`std/a/b.ignite`. The compiler has no special knowledge of text handling, JSON, consoles,
contracts or deployment. Those are ordinary records, enums and functions, checked and tested
like your own code.

The substrate modules `std.io`, `std.net`, `std.fs`, `std.process` and `std.sync` are mostly
builtins implemented in the substrate, the Rust crate `ignite-io`, with the same code on every
target. Their files declare only the records that cross into the substrate, such as
`Endpoint` and `Invocation`, and their tests.

Protocols live here too. Telnet, HTTP and JSON-RPC are Ignite modules under `std/protocol/`,
and so are the component kinds that speak them (`TelnetServer`, `HttpServer`). The compiler
and the runtime know sockets and byte streams, not application protocols.

## Modules

| module | layer | import | purpose |
|---|---|---|---|
| [`std.io`](io.md) | base (substrate) | `import std.io.IoError` | standard input, output and error, `Process`, `Clock`, `Store`, `IoError` and `log`; it has no file |
| [`std.text`](text.md) | base | `import std.text.{split, trim_end}` | text operations called as methods: `split`, `find`, `contains`, `to_lower`, `lines`, … |
| [`std.option`](option-result.md) | base | `import std.option.{unwrap_or, map}` | helpers on `Option`: `unwrap_or`, `map`, `and_then`, `ok_or`, … |
| [`std.result`](option-result.md) | base | `import std.result.{map_err, ok}` | helpers on `Result`: `unwrap_or`, `map`, `map_err`, `ok`, … |
| [`std.data`](data.md) | base | `import std.data.{Value, FormatError}` | the value model every data format reads into and writes from |
| [`std.json`](json.md) | base | `import std.json.{Json, parse, encode}` | JSON values (RFC 8259), every number kept exactly: `encode`, `parse`, member accessors and the bridge to `std.data` |
| [`std.encoding`](encoding.md) | base | `import std.encoding.{base64, hex}` | base64, base64url and hex of RFC 4648 |
| [`std.digest`](digest.md) | base | `import std.digest.{sha1, sha256}` | SHA-1 and SHA-256, at once or in pieces |
| [`std.fs`](fs.md) | base (substrate) | `import std.fs.{Directory, read_text_file}` | files under one root directory: the capability `Directory`, its methods and the text helpers |
| [`std.process`](process.md) | base (substrate) | `import std.process.{Invocation, outcome}` | child processes, random bytes, settings, the command line and environment variables |
| [`std.net`](net.md) | base (substrate) | `import std.net.{TcpStream, Endpoint}` | sockets, in-process pipes, the `ByteStream` interface and the `Endpoint` record |
| [`std.sync`](sync.md) | base (substrate) | none needed | bounded channels between the tasks of one process |
| [`std.runtime`](runtime.md) | base | `import std.runtime.{soak}` | the census of live runtime objects and the soak that finds leaks |
| [`std.time`](time.md) | leaf | `import std.time.{month_start, zone_known}` | month boundaries in `UTC` and `Europe/Berlin` for calendar windows |
| [`std.console`](console.md) | general | `import std.console.{Console, Reply}` | a line-oriented console application, independent of the protocol that carries it |
| [`std.protocol.telnet`](telnet.md) | protocol | `import std.protocol.telnet.{Telnet, TelnetServer}` | `protocol Telnet`, the kinds `TelnetServer`, `TelnetClient` and the probe `TelnetPrompt` |
| [`std.protocol.http`](http.md) | protocol | `import std.protocol.http.{Http, HttpServer}` | `protocol Http`, an HTTP/1.1 server: `Request`, `Response`, `HttpLimits`, the kind `HttpServer` |
| [`std.protocol.http.syntax`](http.md#the-lexical-grammar) | protocol | `import std.protocol.http.syntax.{is_token, trim_ows}` | the lexical grammar `std.protocol.http` reads and writes: tokens, OWS, field values, request-targets, hosts, IMF-fixdate, chunk framing |
| [`std.protocol.jsonrpc`](jsonrpc.md) | protocol | `import std.protocol.jsonrpc.{JsonRpc, JsonRpcApp}` | `protocol JsonRpc`: JSON-RPC 2.0 with Content-Length framing |
| [`std.rfc`](rfc.md) | leaf | `import std.rfc.{Level, Party}` | the vocabulary of requirements, and one catalogue module `std.rfc.*` per specification |
| [`std.sli`](sli.md) | leaf | `import std.sli.{ProbeAttempt, meets}` | the prompt probe's outcome and its predicates, samples and verdicts, windows, missing data and the target check |
| [`std.policy`](policy.md) | leaf | `import std.policy.{Restart, DrainRollout}` | restart and rollout policies and their backoff arithmetic |
| [`std.contract`](contract.md) | general | `import std.contract.{Availability, SLA}` | contracts over a probe's attempts: `Availability`, `ThresholdCompliance`, `SLA` |
| [`std.deploy`](deploy.md) | general | `import std.deploy.{Deployment, nativeProcess}` | assemblies: where each member of a system runs |
| [`std.cli`](cli.md) | general | `import std.cli.{Application, Command}` | the declaration of a command-line application |
| [`std.control`](control.md) | general | `import std.control.{Machine, step}` | the pure reducers `ignite run` steps, and the apply transition |

## Importing

- A module exports every record, alias, enum, function, constant and protocol it defines.
  Imports are not re-exported. See [Modules and imports](../modules.md).
- The builtins of the substrate are usable without an import. An import of one from a
  substrate module only checks that the name is a builtin.
- The records a substrate file declares need an import: `import std.net.Endpoint` (E048
  without it), `import std.process.Invocation`.
- Most std functions are called as methods: once `split` is imported, `line.split(" ")` is
  `split(line, " ")`. See [Method calls and UFCS](../modules.md#method-calls-and-ufcs).
- `std.option` and `std.result` share names, so a module imports those by name from the one
  it means; see [std.option and std.result](option-result.md).
- An unknown module or name is E031. See [Diagnostics](../diagnostics.md).

## Layers

The standard library has four layers. A module may import only from its own layer or a lower
one.

| layer | modules | may import |
|---|---|---|
| base | `std.io`, `std.net`, `std.fs`, `std.process`, `std.sync` (substrate), `std.runtime`, `std.option`, `std.result`, `std.text`, `std.json`, `std.data`, `std.encoding`, `std.digest` | base |
| leaf | `std.sli`, `std.time`, `std.policy`, `std.rfc` and every `std.rfc.*` | base, leaf |
| general | `std.control`, `std.contract`, `std.deploy`, `std.console`, `std.cli` | base, leaf, general |
| protocol | every `std.protocol.*` | every layer |

In plain words:

- A general module knows no application protocol. Only `std.protocol.*` modules import a
  protocol module, and only they declare names that contain a protocol name. A catalogue
  `std.rfc.*` may quote the name of a protocol that imports it, as `JSONRPC_2` does.
- A component kind that speaks one protocol (a server, a client, a probe) lives in that
  protocol's module.
- `std.sli`, `std.time` and `std.policy` import nothing, so the reducers of `std.control` can
  read their vocabulary without an import cycle (E032).

The rule is enforced by a test of the compiler; the full statement is
[`docs/layering.md`](https://github.com/ignite-lang/ignite/blob/main/docs/layering.md).

## Tests

Every std module carries inline `test` blocks next to the code they test; they are the best
examples of each function's behaviour. `ignite test std/text.ignite` runs one file's tests
(`--target all` runs them on both targets). See [Testing with ignite test](../../guide/testing.md) and
[Tests and assertions](../tests.md).
