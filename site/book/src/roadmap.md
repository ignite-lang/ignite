# Roadmap

Ignite is an early prototype. The latest release is v0.16.0. This page separates what is
released, what is built on `main` and not released, what is planned, and what exists only as a
design document. Designs that are not built appear only on this page.

<span class="badge">released</span> in a tagged release &nbsp;
<span class="badge wip">unreleased</span> work in progress, not in any release &nbsp;
<span class="badge design">design</span> a design document; nothing of it is implemented

The roadmap in outline is
[`PLAN.md`](https://github.com/ignite-lang/ignite/blob/main/PLAN.md). The reasoning and the
dependencies between the waves are in
[`docs/vision.md`](https://github.com/ignite-lang/ignite/blob/main/docs/vision.md) section 4.

## What works today

Ignite runs systems on one machine, in the foreground with `ignite run` or under a local host
with `ignite apply`. It has no cloud provider, no portable bundle and no stateful migration.
Released and documented in this book:

- the language: records, enums, pattern matching, generics with bounds, closures, quantities
  such as `4096 bytes` and `15 minutes`, and explicit effects;
- two native targets over one IO substrate, LLVM by default and Rust with `--target rust`,
  and static executables with `--static`;
- the standard library in Ignite: text, JSON and the `std.data` value model, files, child
  processes, settings, channels, and the Telnet, HTTP/1.1 and JSON-RPC protocols;
- inline `test` blocks, run on both targets by `ignite test`;
- the system declaration: resources, components, contracts, policies and assemblies;
- `ignite plan`, `ignite explain`, `ignite run`, `ignite replay`, and local `ignite apply`
  through `ignite host`, with `ignite recover`;
- protocols that state which RFC requirements they meet, checked by `ignite conformance`.

The measured state of each release, build times included, is on the [Quality](quality.md) page.

## Released

| release | <span class="badge">released</span> |
|---|---|
| **v0.1.0** | the language; the Rust and LLVM targets over one IO substrate; Telnet and HTTP written in Ignite; inline `test` blocks on both targets; the system declaration (`resource`, `component`, `contract`, `policy`, `assembly`, quantities with units); `ignite plan` and `ignite explain` with the compile-time evaluator; `ignite run` and `ignite replay` with probes, contract evaluation, `Restart`, `DrainRollout` and the journal |
| **v0.2.0** | local apply: `ignite host`, `ignite plan --for-apply`, `ignite apply`, `ignite inspect`; source-bound saved plans, durable receipts and crash recovery ([guide](guide/apply.md)) |
| **v0.3.0** | in-process streams: `Pipe.pair()`, the `ByteStream` bound `<S: ByteStream>`, the Telnet and HTTP roles generic over the stream with `SessionLimits` and `HttpLimits`, protocol tests without a network on both targets |
| **v0.4.0** | protocols that state which RFC requirements they meet: `std.rfc` and catalogues of RFC 854, 855, 1143, 9112 and 9110; the clauses `conforms`, `scope`, `satisfies`, `verifies`, `unsupported` and `not_applicable` with E090 to E092; `ignite conformance`, `ignite rfc import` and `ignite rfc check`; the claims of `Telnet` (RFC 855 met, RFC 854 with four gaps) and `Http` (gaps listed, nothing claimed) ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.4.0)) |
| **v0.5.0** | the language gaps of wave A on both targets: values move at their last use, calls of fn-typed expressions, rest patterns in list patterns with E076; store `upsert`, borrowed store reads and performance work on both targets ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.5.0)) |
| **v0.6.0** | processes, files, settings and random bytes for programs: `std.fs` with the `Directory` resource, `std.process`, one `--flag` per `Setting` with E142, random bytes (`Entropy`); `HttpServer` runs under `ignite run` and `ignite apply`; LLVM is the default `--target`, and an installed `ignite` carries its runtime ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.6.0)) |
| **v0.7.0** | one service operated: the todo service on the LLVM target keeps its data in a `Directory` across a kill and an `ignite apply` rollout, an HTTP probe kind with a typed outcome and an `Availability` contract over it, a hosted `Store`, host journal integrity checks, fault rows for lost acknowledgement, stale plans and corrupt journals, and `ignite recover`; also `CommandLine`, the language server example `ignite-lsp` written in Ignite, and fixes to rollouts, LLVM stream ownership and runtime panics ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.7.0)) |
| **v0.8.0** | static executables and the typed program as JSON: `ignite build --static` links against musl on both targets; `ignite emit --typed-json` prints the typed program in the format `ignite-typed/0.1`; `<`, `<=`, `>` and `>=` on `Text`, `Int.wrapping_add`, `wrapping_sub` and `wrapping_mul`, `Stderr`, list element assignment `xs[i] = v` and `List + List`; sibling modules that import each other, with their tests run by `ignite test`; fewer copies in the LLVM lowering ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.8.0)) |
| **v0.9.0** | checked arithmetic and Telnet to RFC 854: `Int.checked_add`, `checked_sub` and `checked_mul` return `Option<Int>`; `std.protocol.telnet` claims RFC 854 with GA, the Synch and a partial-line key; pattern bindings, `for` elements and consumed closure parameters move at their last use; fewer clones in generated Rust; the language server moved to its own repository, [ignite-lsp](https://github.com/ignite-lang/ignite-lsp) ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.9.0)) |
| **v0.10.0** | effectful `List.map`, a sharded Store and HTTP closer to RFC 9112 and 9110: `List.map` takes a fn with effects its caller covers; `Store` is sharded by id and copies no value under a lock; `std.protocol.http` checks the application's response headers, requires `Host`, accepts absolute-form targets and answers 414 and HEAD; apply identities come from a canonical form of the syntax tree ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.10.0)) |
| **v0.11.0** | chunked HTTP request bodies and safer generated Rust: `std.protocol.http` decodes chunked request bodies instead of answering 501, which leaves RFC 9112 and RFC 9110 with one gap each; a stream passed to `spawn` at its last use moves into the task on the Rust target; names the Rust generator invents never collide with names a program declares ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.11.0)) |
| **v0.12.0** | streamed and binary HTTP responses and effectful fn values in `map`: `std.protocol.http` sends chunked and binary response bodies, a reason phrase for every RFC 9110 status, and no body or `Content-Length` for 304, 1xx and a 2xx to CONNECT; `List.map` takes a local or parameter holding an effectful fn, and an untyped one is E042; `Bytes` set by index, `to_lower` and `to_upper` in place; associated fn names in generated Rust never collide ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.12.0)) |
| **v0.13.0** | spawn handovers in branches and Rust names that shadow nothing: on the LLVM target a stream end passed to `spawn` at its last use inside `if`/`else` branches or `match` arms moves into the task; on the Rust target `spawn` evaluates its arguments in source order, and a program may name things after Rust primitives, crate roots or the generator's own locals; the website's pages are checked for layout and accessibility on every merge, the FAQ is in the landing page's header and text is at least 14 px ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.13.0)) |
| **v0.14.0** | builtins a real service needs, a clean exit after a drain and a quality ratchet: environment variables (`Environment`), `Clock.sleep`, the wall clock `Clock.now()` and `Process.flush()`; `TcpStream.peer`, `timed_out` and `connect_within`; `Process.exit` during a drain exits with its code at once; user functions named like builtins and record literals as method receivers check; the chunked HTTP decoder copies no chunk, and `Store` reads no longer wait on an update; quality metrics with a bar that every release must hold (`task quality`), a differential runner over both targets and the evaluator; a toolchain image for every release ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.14.0)) |
| **v0.15.0** | bitwise operators, channels and throughput near plain Rust: bitwise operators; channels and timers in `std.sync`; a drain that keeps the listeners open; a higher open-files limit; `IoError` compared in `assert`; the `std.data` value model and its bridge to `std.json`; `List.with_capacity` and `Bytes.with_capacity`; faster generated code and runtime on both targets, so the todo and terminal examples answer at 0.93 to 1.07 times the replies per second of plain Rust; ThinLTO release builds on the LLVM target, and `spawn` no longer waits for the task to start; timing and benchmarks against plain Rust and Go on a dedicated host before each release ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.15.0)) |
| **v0.16.0** | tasks on a readiness scheduler, no leaks in a long-running server, and RESP3, NATS and TLS clients: spawned tasks run as stackful tasks on epoll workers inside the substrate (`IGNITE_IO=threads` keeps one thread each), each worker keeping its free stacks; the LLVM target releases every handle with its last owner and channel ends are inferred; `std.runtime` with `Runtime.census()` and `soak`; the modular 32-bit word `U32`; `TcpListener.bind_kept` keeps a listener bound at run time through the drain; SHA-1 and SHA-256 in `std.digest`, base64, base64url and hex in `std.encoding`; `std.json`'s `get` returns the last of a repeated member and `get_all` every one; `std.protocol.resp`, a RESP3 client with pipelining, pub/sub and Sentinel; `std.protocol.nats`, a NATS core client with reconnect; `Tls.client`, a TLS client session that is a `ByteStream`; `HttpApp` sees its connection (`Connection`, `TcpStream.local()`), holds a request, takes the connection over and answers an oversized body with 413; Rust-target release builds with fat LTO; `ignite --version` names the commit of a source build; the book reorganised by reader and job ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.16.0)) |

## What's next

The work after v0.16.0 comes in waves of a few stories each. A wave is merged when every story
in it passes the gate, and a release follows every third wave. The order below is the current
plan, not a promise of dates.

### Being built <span class="badge wip">unreleased</span>

| work | content | state |
|---|---|---|
| native parity | Ignite programs within 1.1× of plain Rust: fixed-size lists `[T; N]` of scalars and `List.with_capacity` complete the minimal set for codecs ([`docs/fixed-width-ints.md`](https://github.com/ignite-lang/ignite/blob/main/docs/fixed-width-ints.md)) | wave 11 |

### Planned

| wave | content | state |
|---|---|---|
| 9 | protocol clients: WebSocket (RFC 6455) and an HTTP client role ([protocol clients](#protocol-clients-websocket-resp3-nats-design)); `Codec<T>` and its combinators ([data formats](#data-formats-json-yaml-and-toml-from-one-value-model-design)); a static build from source with no setup and no network | planned |
| 10 | one mid-level IR that both targets lower from; beside it, blocking work off the readiness scheduler's workers, its memory, and runtime metrics | planned |
| 11 | fixed-size lists; YAML 1.2.2, the safe subset ([data formats](#data-formats-json-yaml-and-toml-from-one-value-model-design)); `ignite fmt` | planned |
| 12 | `Store` keyed by text, the largest compiler files split, then the web framework ([x.web](#xweb-the-web-app-is-part-of-the-system-design)) | planned |

## Designs

Each design below has a document in the repository, and the code in it does not compile today. A
design stays a proposal until a wave adopts it.

### x.web: the web app is part of the system <span class="badge design">design</span>

`x.web` is a planned web framework, shipped with Ignite in a second module root `x` next to
`std`. You declare your data and what you expose, and the framework derives the routes, the
JSON, the OpenAPI document, caching, live page updates and the health probe with its contract.
Who may call a route is a handler parameter type. CORS, limits and timeouts are declared
properties instead of middleware. A called service is a resource, so `ignite plan` shows it as a
dependency. The framework needs language extensions: derived codecs and record types, views,
route annotations and files embedded at check time. A first version runs on today's language
with hand-built routes. Design: [`docs/web.md`](https://github.com/ignite-lang/ignite/blob/main/docs/web.md),
wave 12.

### Data formats: JSON, YAML and TOML from one value model <span class="badge design">design</span>

The value model `std.data` and its bridge to `std.json` (`read`, `write`, `to_data`, `of_data`)
are released in v0.15.0 ([std.data](reference/std/data.md)). The rest is planned in four layers.
`Codec<T>` is a pair of functions with combinators to build one for any type by hand, in `std`
only, planned for wave 9. `std.yaml` reads YAML 1.2.2 under the core schema and refuses anchors,
aliases, custom tags and the `<<` merge key, planned for wave 11. Last, `codec<T>()` is written by
the compiler from the type, and `std.toml` follows. A decode error names the path and what was
expected, such as `$[2].match_path: expected Text, found Int`. Design:
[`docs/serde.md`](https://github.com/ignite-lang/ignite/blob/main/docs/serde.md).

### Protocol clients: WebSocket, RESP3, NATS <span class="badge design">design</span>

Three more protocols in the standard library, written in Ignite like Telnet and HTTP and generic
over any byte stream. `std.protocol.websocket` has server and client roles, reached through the
HTTP upgrade, and declares `conforms [RFC6455]`. `std.protocol.resp` is a client for
Redis-compatible servers with RESP2 and RESP3 types, pipelining, pub/sub and Sentinel.
`std.protocol.nats` is a NATS core client with queue groups and request and reply with a
deadline. Each protocol's tests run over `Pipe.pair()` against a scripted peer, on both
targets. The RESP3 and NATS clients are released in v0.16.0, with a TLS client session that
either runs over; WebSocket is planned. Layering: [`docs/protocols.md`](https://github.com/ignite-lang/ignite/blob/main/docs/protocols.md),
wave 9.

### Fault injection and deterministic simulation <span class="badge design">design</span>

Failure paths that run under `ignite test`. Faults are injected where the program calls the
substrate: refused connects, resets, silent peers, partial reads and writes, latency, disk
errors, clock jumps and crashes. `Fault.point("name")` marks a place in your own code and
compiles to nothing outside test builds. A simulated world is driven by one seed, so
`ignite test FILE --sim --seed …` repeats a failing run. A nemesis draws fault schedules,
checks invariants written in Ignite, and shrinks a failure to a minimal schedule printed as
source. The design is a draft for decision, with options and a recommendation:
[`docs/fault-injection.md`](https://github.com/ignite-lang/ignite/blob/main/docs/fault-injection.md).

### Other designs <span class="badge design">design</span>

| design | content | source |
|---|---|---|
| actors | typed actors: `Address`, `ReplyTo`, `Outbox`, an `Actor` kind, mailboxes, `std/actor`, `std/table`, `Topic`, `Sink`, `Supervisor`; the todo example on an actor | [`docs/actors.md`](https://github.com/ignite-lang/ignite/blob/main/docs/actors.md) |
| WebAssembly | machine scalars, `Buffer`, `--target wasm`, a runtime written in Ignite, `ignite run --watch`, hot reload with `migrate` | [`docs/wasm.md`](https://github.com/ignite-lang/ignite/blob/main/docs/wasm.md) |
| protocols, later phases | ABNF grammars, user `interface` and `impl`, TLS and Unix transports | [`docs/protocols.md`](https://github.com/ignite-lang/ignite/blob/main/docs/protocols.md) |
| self-hosting | packages and manifests, the frontend in Ignite, an emitter in Ignite, a bootstrap chain | [`docs/design.md`](https://github.com/ignite-lang/ignite/blob/main/docs/design.md) section 12 |
| syntactic sugar | a proposal; an item is implemented only when a planned step needs it | [`docs/syntax.md`](https://github.com/ignite-lang/ignite/blob/main/docs/syntax.md) |

## Rules every step keeps

From [`PLAN.md`](https://github.com/ignite-lang/ignite/blob/main/PLAN.md):

- Both targets agree: every feature lands on both. LLVM is the default target and the Rust
  target the differential check.
- Compile time is a budget. Before each release, the build and check times are measured on a
  dedicated host, and a row worse than the last release beyond its tolerance refuses the release
  ([Quality](quality.md)).
- Each step starts with a contract section in `docs/slice.md`, reviewed before work starts,
  and each diagnostic code gets a fixture.
