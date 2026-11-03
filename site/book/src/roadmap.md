# Roadmap

Ignite is an early prototype. This page separates three things: what is released, what is
being built and not released, and what exists only as a design document. Only the first is
described in the rest of this book.

<span class="badge">released</span> in a tagged release &nbsp;
<span class="badge next">unreleased</span> work in progress, not in any release &nbsp;
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

## Being built: RFC catalogues and conformance <span class="badge next">unreleased</span>

The next slice ("protocols P2") is being implemented on a branch and is in no release. Its
contract, in `docs/slice.md` section 19 of that branch, adds:

- `std.rfc`, a vocabulary for requirements (`Level`, the BCP 14 keywords; `Party`; `Spec`;
  `Requirement`), and catalogue modules for RFC 854, 855, 1143, 9112 and 9110 whose
  constants are literal data;
- clauses that bind code to requirements: `conforms`, `scope`, `satisfies`, `verifies`,
  `unsupported` and `not_applicable`; no backend reads them, so they change no emitted byte;
- checks over those claims (E090 to E092): a MUST-class requirement in scope with no status
  is an error;
- `ignite rfc import` and `ignite rfc check`, reading an RFC's XML source from a local file;
- `ignite conformance FILE`, which runs the verifying tests and prints a matrix of
  requirements, evidence and verdicts, ending in a line such as
  `claim conformance of Telnet to rfc854: yes (0 gaps, 0 failures, 0 inconclusive)`;
- the claims of the standard library's own protocols: Telnet against RFC 854 and 855, HTTP
  against RFC 9112 and a server subset of RFC 9110, where the missing chunked coding and Host
  check are listed as gaps rather than claimed.

Until it is released, treat this list as a plan, not as documentation.

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
proposal for syntactic sugar. It, too, is not implemented; an item enters a slice only when
that slice's declarations need it.

## Rules every slice keeps

From [`PLAN.md`](https://github.com/ignite-lang/ignite/blob/main/PLAN.md):

- Both targets agree: the Rust target stays the default, and every feature lands on both.
- Compile time is a budget: the gate time and the program build times are measured before and
  after each slice and recorded; a rise over 10 % is a deviation to explain.
- Each slice starts with a contract section in `docs/slice.md`, reviewed before work starts,
  and each diagnostic code gets a fixture.
- Future designs stay proposals until a wave adopts them.
