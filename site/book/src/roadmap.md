# Roadmap

Ignite is an early prototype. This page separates what is released, what is
being built and not released, and what exists only as a design document. The rest of this book
describes only what is released, except pages titled "Planned".

<span class="badge">released</span> in a tagged release &nbsp;
<span class="badge wip">unreleased</span> work in progress, not in any release &nbsp;
<span class="badge design">design</span> a design document; nothing of it is implemented

The roadmap in outline is
[`PLAN.md`](https://github.com/ignite-lang/ignite/blob/main/PLAN.md). The reasoning and the
dependencies between the waves are in
[`docs/vision.md`](https://github.com/ignite-lang/ignite/blob/main/docs/vision.md) section 4.

## Released

| release | <span class="badge">released</span> |
|---|---|
| **v0.1.0** | the language; the Rust and LLVM targets over one IO substrate; Telnet and HTTP written in Ignite; inline `test` blocks on both targets; the system declaration (`resource`, `component`, `contract`, `policy`, `assembly`, quantities with units); `ignite plan` and `ignite explain` with the compile-time evaluator; `ignite run` and `ignite replay` with probes, contract evaluation, `Restart`, `DrainRollout` and the journal |
| **v0.2.0** | local apply: `ignite host`, `ignite plan --for-apply`, `ignite apply`, `ignite inspect`; source-bound saved plans, durable receipts and crash recovery ([guide](guide/apply.md)) |
| **v0.3.0** | in-process streams: `Pipe.pair()`, the `ByteStream` bound `<S: ByteStream>`, the Telnet and HTTP roles generic over the stream with `SessionLimits` and `HttpLimits`, protocol tests without a network on both targets |
| **v0.4.0** | protocols that state which RFC requirements they meet: `std.rfc` and catalogues of RFC 854, 855, 1143, 9112 and 9110; the clauses `conforms`, `scope`, `satisfies`, `verifies`, `unsupported` and `not_applicable` with E090 to E092; `ignite conformance`, `ignite rfc import` and `ignite rfc check`; the claims of `Telnet` (RFC 855 met, RFC 854 with four gaps) and `Http` (gaps listed, nothing claimed) ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.4.0)) |
| **v0.5.0** | the language gaps of wave A on both targets: values move at their last use, calls of fn-typed expressions, rest patterns in list patterns with E076; store `upsert`, borrowed store reads and performance work on both targets ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.5.0)) |
| **v0.6.0** | processes, files, settings and random bytes for programs: `std.fs` with the `Directory` resource, `std.process`, one `--flag` per `Setting` with E142, `std.random`; `HttpServer` runs under `ignite run` and `ignite apply`; LLVM is the default `--target`, and an installed `ignite` carries its runtime ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.6.0)) |
| **v0.7.0** | one service operated (slice 18): the todo service on the LLVM target keeps its data in a `Directory` across a kill and an `ignite apply` rollout, an HTTP probe kind with a typed outcome and an `Availability` contract over it, a hosted `Store`, host journal integrity checks, fault rows for lost acknowledgement, stale plans and corrupt journals, and `ignite recover`; also `CommandLine`, the language server example `ignite-lsp` written in Ignite, and fixes to rollouts, LLVM stream ownership and runtime panics ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.7.0)) |
| **v0.8.0** | static executables and the typed program as JSON: `ignite build --static` links against musl on both targets; `ignite emit --typed-json` prints the typed program in the format `ignite-typed/0.1`; `<`, `<=`, `>` and `>=` on `Text`, `Int.wrapping_add`, `wrapping_sub` and `wrapping_mul`, `Stderr`, list element assignment `xs[i] = v` and `List + List`; sibling modules that import each other, with their tests run by `ignite test`; fewer copies in the LLVM lowering ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.8.0)) |
| **v0.9.0** | checked arithmetic and Telnet to RFC 854: `Int.checked_add`, `checked_sub` and `checked_mul` return `Option<Int>`; `std.protocol.telnet` claims RFC 854 with GA, the Synch and a partial-line key; pattern bindings, `for` elements and consumed closure parameters move at their last use; fewer clones in generated Rust; the language server moved to its own repository, [ignite-lsp](https://github.com/ignite-lang/ignite-lsp) ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.9.0)) |
| **v0.10.0** | effectful `List.map`, a sharded Store and HTTP closer to RFC 9112 and 9110: `List.map` takes a fn with effects its caller covers; `Store` is sharded by id and copies no value under a lock; `std.protocol.http` checks the application's response headers, requires `Host`, accepts absolute-form targets and answers 414 and HEAD; apply identities come from a canonical form of the syntax tree ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.10.0)) |
| **v0.11.0** | chunked HTTP request bodies and safer generated Rust: `std.protocol.http` decodes chunked request bodies instead of answering 501, which leaves RFC 9112 and RFC 9110 with one gap each; a stream passed to `spawn` at its last use moves into the task on the Rust target; names the Rust generator invents never collide with names a program declares ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.11.0)) |
| **v0.12.0** | streamed and binary HTTP responses and effectful fn values in `map`: `std.protocol.http` sends chunked and binary response bodies, a reason phrase for every RFC 9110 status, and no body or `Content-Length` for 304, 1xx and a 2xx to CONNECT; `List.map` takes a local or parameter holding an effectful fn, and an untyped one is E042; `Bytes` set by index, `to_lower` and `to_upper` in place; associated fn names in generated Rust never collide ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.12.0)) |
| **v0.13.0** | spawn handovers in branches and Rust names that shadow nothing: on the LLVM target a stream end passed to `spawn` at its last use inside `if`/`else` branches or `match` arms moves into the task; on the Rust target `spawn` evaluates its arguments in source order, and a program may name things after Rust primitives, crate roots or the generator's own locals; the website's pages are checked for layout and accessibility on every merge, the FAQ is in the landing page's header and text is at least 14 px ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.13.0)) |
| **v0.14.0** | builtins a real service needs, a clean exit after a drain and a quality ratchet: environment variables (`Environment`), `Clock.sleep`, the wall clock `Clock.now()` and `Process.flush()`; `TcpStream.peer`, `timed_out` and `connect_within`; `Process.exit` during a drain exits with its code at once; user functions named like builtins and record literals as method receivers check; the chunked HTTP decoder copies no chunk, and `Store` reads no longer wait on an update; quality metrics with a bar that every release must hold (`task quality`), a differential runner over both targets and the evaluator; a toolchain image for every release ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.14.0)) |
| **v0.15.0** | bitwise operators, channels and throughput near plain Rust: bitwise operators; channels and timers in `std.sync`; a drain that keeps the listeners open; a higher open-files limit; `IoError` compared in `assert`; the `std.data` value model and its bridge to `std.json`; `List.with_capacity` and `Bytes.with_capacity`; faster generated code and runtime on both targets, so the todo and terminal examples answer at 0.93 to 1.07 times the replies per second of plain Rust; ThinLTO release builds on the LLVM target, and `spawn` no longer waits for the task to start; timing and benchmarks against plain Rust and Go on a dedicated host before each release ([release notes](https://github.com/ignite-lang/ignite/releases/tag/v0.15.0)) |

## What's next

The work after v0.15.0 comes in waves of a few stories each. A wave is merged when every story
in it passes the gate, and a release follows every third wave. The order below is the current
plan, not a promise of dates. The plan itself is
[`.engineering/waves/plan-2026-11-07.md`](https://github.com/ignite-lang/ignite/blob/main/.engineering/waves/plan-2026-11-07.md).

### Being built <span class="badge wip">unreleased</span>

| wave | content | state |
|---|---|---|
| native parity | Ignite programs within 1.1× of plain Rust: an IO scheduler that waits on readiness instead of running one thread per connection, fixed-width integers for codecs | in progress |

### Planned

| wave | content | state |
|---|---|---|
| 8 | protocol clients: WebSocket (RFC 6455), RESP3 with Sentinel, NATS ([protocols and clients](planned/clients.md)); `Codec<T>` and its combinators ([data formats](planned/data.md)); `ignite fmt` with a format check | planned |
| 9 | YAML 1.2.2, the safe subset ([data formats](planned/data.md)); benchmarks of Ignite programs against the same programs in plain Rust and Go | planned |
| 10 | one mid-level IR that both targets lower from, checked by a differential runner | planned |
| 11 | `Store` keyed by text | planned |
| 12 | the largest compiler files split | planned |

## Designs <span class="badge design">design</span>

These waves follow in [`PLAN.md`](https://github.com/ignite-lang/ignite/blob/main/PLAN.md).
Each has a design document or a row in the plan. None of it is implemented.

| wave | content | source |
|---|---|---|
| C actors P1 to P3 | typed actors: `Address`, `ReplyTo`, `Outbox`, an `Actor` kind, mailboxes, `std/actor`, `std/table`, `Topic`, `Sink`, `Supervisor`; the todo example on an actor | [`docs/actors.md`](https://github.com/ignite-lang/ignite/blob/main/docs/actors.md) |
| D async | tokio inside the substrate, Rust-target futures, LLVM-target coroutines, `Task.yield_now` | [`PLAN.md`](https://github.com/ignite-lang/ignite/blob/main/PLAN.md), [`docs/vision.md`](https://github.com/ignite-lang/ignite/blob/main/docs/vision.md) |
| E WebAssembly P0 to P4 | machine scalars, `Buffer`, `--target wasm`, a runtime written in Ignite, `ignite run --watch`, hot reload with `migrate` | [`docs/wasm.md`](https://github.com/ignite-lang/ignite/blob/main/docs/wasm.md) |
| G protocols P3 to P5 | ABNF grammars, user `interface` and `impl`, TLS and Unix transports | [`docs/protocols.md`](https://github.com/ignite-lang/ignite/blob/main/docs/protocols.md) |
| x.web | a web framework shipped with Ignite under `x.*`: the web app declared in the system, routes, JSON, OpenAPI, live pages, caching and contracts derived; identity as a type, declared policies, outgoing calls as dependencies, views and Markdown pages, and the language extensions they need ([page](planned/web.md)) | [`docs/web.md`](https://github.com/ignite-lang/ignite/blob/main/docs/web.md) |
| data formats | one value model in `std.data`, codecs the compiler derives, JSON, YAML and TOML ([page](planned/data.md)); the first steps are waves 7 to 9 above | [`docs/serde.md`](https://github.com/ignite-lang/ignite/blob/main/docs/serde.md) |
| fault injection and simulation | faults at the IO seam, named fault points, a simulated world driven by one seed, generated fault schedules with shrinking ([page](planned/simulation.md)); a draft for decision | [`docs/fault-injection.md`](https://github.com/ignite-lang/ignite/blob/main/docs/fault-injection.md) |
| H self-hosting M5 to M7 | packages and manifests, the frontend in Ignite, an emitter in Ignite, a bootstrap chain | [`docs/design.md`](https://github.com/ignite-lang/ignite/blob/main/docs/design.md) section 12 |

[`docs/syntax.md`](https://github.com/ignite-lang/ignite/blob/main/docs/syntax.md) is a
proposal for syntactic sugar. It, too, is not implemented. An item is implemented only when
the declarations of a planned step need it.

## Rules every step keeps

From [`PLAN.md`](https://github.com/ignite-lang/ignite/blob/main/PLAN.md):

- Both targets agree: every feature lands on both. LLVM is the default target and the Rust
  target the differential check.
- Compile time is a budget: the gate time and the program build times are measured before and
  after each step and recorded. A rise over 10 % is a deviation to explain.
- Each step starts with a contract section in `docs/slice.md`, reviewed before work starts,
  and each diagnostic code gets a fixture.
- Future designs stay proposals until a wave adopts them.
