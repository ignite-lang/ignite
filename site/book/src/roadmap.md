# Roadmap

Ignite is an early prototype. This page separates three things: what is released, what is
being built and not released, and what exists only as a design document. Only the first is
described in the rest of this book.

<span class="badge">released</span> in a tagged release &nbsp;
<span class="badge wip">unreleased</span> work in progress, not in any release &nbsp;
<span class="badge design">design</span> a design document; nothing of it is implemented

The roadmap in outline is
[`PLAN.md`](https://github.com/ignite-lang/ignite/blob/main/PLAN.md); the reasoning and the
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

## Being built <span class="badge wip">unreleased</span>

No slice is being implemented. The next waves are the designs below; their order is in
[`PLAN.md`](https://github.com/ignite-lang/ignite/blob/main/PLAN.md).

## Designs <span class="badge design">design</span>

These waves follow in [`PLAN.md`](https://github.com/ignite-lang/ignite/blob/main/PLAN.md).
Each has a design document or a row in the plan; none of it is implemented.

| wave | content | source |
|---|---|---|
| C actors P1 to P3 | typed actors: `Address`, `ReplyTo`, `Outbox`, an `Actor` kind, mailboxes, `std/actor`, `std/table`, `Topic`, `Sink`, `Supervisor`; the todo example on an actor | [`docs/actors.md`](https://github.com/ignite-lang/ignite/blob/main/docs/actors.md) |
| D async | tokio inside the substrate, Rust-target futures, LLVM-target coroutines, `Task.yield_now` | [`PLAN.md`](https://github.com/ignite-lang/ignite/blob/main/PLAN.md), [`docs/vision.md`](https://github.com/ignite-lang/ignite/blob/main/docs/vision.md) |
| E WebAssembly P0 to P4 | machine scalars, `Buffer`, `--target wasm`, a runtime written in Ignite, `ignite run --watch`, hot reload with `migrate` | [`docs/wasm.md`](https://github.com/ignite-lang/ignite/blob/main/docs/wasm.md) |
| G protocols P3 to P5 | ABNF grammars, user `interface` and `impl`, TLS and Unix transports | [`docs/protocols.md`](https://github.com/ignite-lang/ignite/blob/main/docs/protocols.md) |
| H self-hosting M5 to M7 | packages and manifests, the frontend in Ignite, an emitter in Ignite, a bootstrap chain | [`docs/design.md`](https://github.com/ignite-lang/ignite/blob/main/docs/design.md) section 12 |

[`docs/syntax.md`](https://github.com/ignite-lang/ignite/blob/main/docs/syntax.md) is a
proposal for syntactic sugar. It, too, is not implemented; an item is implemented only when
the declarations of a planned step need it.

## Rules every step keeps

From [`PLAN.md`](https://github.com/ignite-lang/ignite/blob/main/PLAN.md):

- Both targets agree: every feature lands on both; LLVM is the default target and the Rust
  target the differential check.
- Compile time is a budget: the gate time and the program build times are measured before and
  after each step and recorded; a rise over 10 % is a deviation to explain.
- Each step starts with a contract section in `docs/slice.md`, reviewed before work starts,
  and each diagnostic code gets a fixture.
- Future designs stay proposals until a wave adopts them.
