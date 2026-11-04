# Ignite protocols: RFC bindings, pluggable transports and streams

**Status:** future design, proposal; nothing here is implemented. **Date:** 1 November 2026.
**Scope:** the stack of transports, streams, connections, protocols, exchanges and applications;
how the stack is plugged together; binding protocol code to the RFCs it implements; placement;
compilation on both targets; a phased roadmap.

Origin (Timo, 1 November 2026): HTTP is "just one protocol, whereas TCP is a transport"; transport,
protocol, stream, request and response are decoupled and "completely pluggable"; and, following the
intent claim (design, opening paragraph), "whatever is claimed by the RFC is then put into the
primitives of ignite".

The document decides: each question gets the chosen design, the rejected alternatives and one line
on why. Citations: "design §n" for `design.md`; "slice §n" for `slice.md`; "wasm §n",
"actors §n", "llvm §n" for the companion designs; a file and a symbol (fn, type, constant, role)
for code, never a line number; `[Pn]` for the external sources of section 9. Proposals that
contradict a stated decision are marked **Conflict**.

State on 1 November 2026: `std/protocol/http.ignite` is in place (678 lines including 30 inline
`test` items). The lexer has 29 keywords. Both protocols' roles take a `TcpStream` (slice §4.2
Protocols), and type parameters have no bounds (slice §11.1 Generics).

## 0 Decisions at a glance

| # | Question | Decision |
|---|---|---|
| D1 | Stack | Six concepts: transport, stream, connection, protocol, exchange or session, application. A protocol sees only a stream. An application sees only its protocol's request, response or session types. |
| D2 | Pluggability | Interfaces with bounded generics, monomorphized: `fn serve<S: ByteStream>(..)`. Phase 1 ships the bound with one compiler-known interface, `ByteStream`. Phase 4 makes `interface` and `impl` user-declarable, with associated types. |
| D3 | Choosing the stack | A system names a transport value, e.g. `transport = Tls { inner = Tcp { port = 8443 } }`. The checker matches the transport's stream type against the protocol's bound before any effect. |
| D4 | RFC catalogues | `std/rfc/rfcNNNN.ignite` holds an RFC's requirements as `Requirement` constants extracted from the RFCXML `<bcp14>` marks, and its collected ABNF verbatim. |
| D5 | Linking | `protocol Http conforms [RFC9112]`, `fn .. satisfies [R..]`, `test ".." verifies [R..]`. A MUST in scope must be satisfied, declared unsupported with a reason, or not applicable; otherwise E090. `ignite conformance` prints the matrix. |
| D6 | Wire formats | `grammar` blocks in RFC 5234 ABNF compile to pure, incremental, bounded parsers and printers bound to records. Context-dependent framing is a pure transition function. Binary layouts in the EverParse 3D style come later. |
| D7 | State machines | A pure `step(state, input) -> (state, outputs)` over an enum, like today's `decode`. Convergence is checked by `std.check.explore` under `ignite test`. No `machine` keyword. |
| D8 | Limits | Every limit an RFC leaves open is a defaulted field of the protocol's `Limits` record, linked to the requirement that bounds it. |
| D9 | TLS | A sans-IO engine (rustls) in the substrate. The `Tls<T>` layer that drives it over any inner stream is Ignite. A Ignite TLS implementation is not planned. |
| D10 | Phase 1 | After slice 8: an in-memory `Pipe`, the `ByteStream` bound, generic roles, and `test` blocks running Telnet and HTTP over TCP and over a pipe. |

## 1 The abstraction stack

### 1.1 Concepts

| Concept | Meaning | Ignite form | Today |
|---|---|---|---|
| Transport | Makes connections (listen, accept, connect); owns addresses and credentials | A descriptor value whose type implements `Transport`: `Tcp` [P8], `Unix`, `Pipe`, `Tls<T>`, later `Quic` | the `TcpListener` resource kind (slice §6.1) |
| Stream | What a connection carries | Interfaces `ByteStream` (ordered reliable bytes), `MessageStream<M>` (framed messages, e.g. WebSocket), `Multiplexed` (QUIC) | builtin `TcpStream` |
| Connection | One established stream plus facts about it | The stream value; `info(stream) -> ConnectionInfo { secure, peer, local, alpn }`, pure like `local_port` (slice §5.1 Effect sets) | `local_port` |
| Protocol | Wire format, state machine and roles, bound to RFCs | `protocol P conforms [..] { roles }`, generic over a stream bound | `protocol Telnet`, `protocol Http` |
| Exchange or session | What a protocol offers an application | Request/response: `Exchange<Q, R, S>`, one call per request. Session: a protocol-specific record of callbacks | `HttpApp<S>` (`std/protocol/http.ignite`), `Console` (`std/console.ignite`) |
| Application | The value a role calls | A record whose fn fields are wasm's cut points (wasm §3.1) | `HttpApp { state = todos, handle }` |

HTTP shows why exchange and protocol are separate. RFC 9110 defines the aspects "shared by all
versions" [P4]. RFC 9112 maps them onto a byte stream [P5]. RFC 9114 is "a mapping of HTTP
semantics over QUIC" [P6]. In Ignite, `Request`, `Response` and `HttpApp` move to `std/http.ignite`
(RFC 9110). `protocol Http` in `std/protocol/http.ignite` is the RFC 9112 mapping, and a later
`protocol Http3` is the RFC 9114 mapping. An application written against `HttpApp<S>` runs on
either without change.

### 1.2 Minimal signatures

Proposed syntax. `Self` is the implementing type. Methods name their effects on parameters, as
declared fns do (slice §5.1), so roots and E016 work as they do today.

As built in P1 (slice 14, slice §18.3, D2): `ByteStream` is compiler-known, a builtin name of
`std.net` with the four methods `read`, `write_all`, `close` and `set_idle_timeout`; nothing is
declared in Ignite, `info` is P4's, and P1 has no `std/transport/`. P4 declares the interface and
moves it, which costs one import line (section 7).

```ignite
package std.transport.stream

type ConnectionInfo { secure: Bool = false, peer: Text = "", local: Text = "", alpn: Text = "" }

interface ByteStream {
  fn read(stream: Self, maximum: Int) -> IoResult<Input> requires [stream.read]
  fn write_all(stream: Self, data: Bytes) -> IoResult<Unit> requires [stream.write]
  fn close(stream: Self) -> IoResult<Unit> requires [stream.write]
  fn set_idle_timeout(stream: Self, ms: Int) -> IoResult<Unit> requires [stream.write]
  fn info(stream: Self) -> ConnectionInfo
}

interface Transport {
  type Listener
  type Accepted
  type Stream: ByteStream
  fn listen(transport: Self) -> IoResult<Self.Listener> requires [transport.*, net.listen]
  fn accept(transport: Self, listener: Self.Listener) -> IoResult<Self.Accepted>
    requires [listener.accept]
  fn open(transport: Self, accepted: Self.Accepted) -> IoResult<Self.Stream>
    requires [transport.*, accepted.*]
  fn connect(transport: Self) -> IoResult<Self.Stream> requires [transport.*, net.connect]
}

package std.http

type Exchange<Q, R, S> {
  state: S
  handle: fn(state: S, request: Q) -> R requires [state.*]
}
type HttpApp<S> = Exchange<Request, Response, S>

package std.protocol.http

protocol Http conforms [RFC9112] {
  fn serve<T: ByteStream, S>(stream: T, limits: HttpLimits, application: HttpApp<S>)
    -> IoResult<Unit> requires [stream.read, stream.write, application.*]
}
```

`accept` runs in the accept loop and is cheap; `open` runs in the connection's task and does the
expensive part, such as a TLS handshake. Credentials are descriptor fields, so reading them is an
effect rooted at `transport` (design §3.3, `SecretRef<T>`), with no ambient `secrets` effect.
`MessageStream<M>` has `receive`, `send` and `close` with the same roots. `Exchange` has the shape of
tower's `Service`, "an asynchronous function from a `Request` to a
`Response`" [P26], without `poll_ready`, because back-pressure belongs to the substrate (slice §7).
Roles gain a `limits` parameter (section 3.6). Slice §4.2's fixed role signatures and E030 change
with them, as slice §4.2's last rule anticipates.

### 1.3 Composition

| Stack | Transport value | Stream at the protocol | Protocol |
|---|---|---|---|
| HTTP over TCP | `Tcp { address = "127.0.0.1", port = 8080 }` | `TcpStream` | `Http` |
| HTTP over TLS over TCP | `Tls { inner = Tcp { port = 8443 }, identity = secret("todo-tls") }` | `TlsStream<TcpStream>` | `Http` |
| Telnet over TCP | `Tcp { port = 2323 }` | `TcpStream` | `Telnet` |
| Telnet over a pipe, in a test | none: `Pipe.pair()` in the test body | `PipeStream` | `Telnet` |
| HTTP/3 over QUIC, later | `Quic { port = 8443, identity = .. }` [P7] | `QuicConnection: Multiplexed` | `Http3`, same `HttpApp<S>` |

TLS can layer over any of these streams because "the only requirement from the underlying transport
is a reliable, in-order data stream" [P9]. HTTP/3 is a different protocol, not HTTP/1.1 over another
transport. It frames each QUIC stream "similar to the HTTP/2 framing" and delegates flow control to
QUIC [P6]. So `Http3`'s role takes `C: Multiplexed`, and binding it to `Tcp` is a type error (E096).
That fulfils PRD01's "incompatible bindings fail before effects" (design §1.3).

**Rejected:** one protocol abstraction over HTTP/1.1, HTTP/2 and HTTP/3 (their framing differs
[P6]; what they share, the exchange, is shared already); Netty-style handler pipelines assembled at
run time [P28] (Ignite fixes and checks the stack at build time).

## 2 Pluggability

### 2.1 Choice

| Option | Verdict | Why |
|---|---|---|
| Interfaces and bounded generics, monomorphized | **chosen** | Design §3.2 lists `Interface` as a declaration primitive; design §3.5 asks for "generics with a limited constraint system"; design §8.6 (version 0.2) named "Listener and byte-stream interfaces" as the TCP surface. Both targets already monomorphize (slice §11.1), so no layer costs a dynamic call. |
| Records of effectful fns as dictionaries (possible since slice 7) | rejected for streams and transports, kept for applications | The dictionary is threaded by hand, every read is an indirect call, and a record cannot say that `Tls<Tcp>`'s stream type follows from `Tcp`'s. At the application edge fn fields are wanted, because they are wasm's cut points (wasm §3.1). |
| Functors, as in MirageOS, where conduit's TLS functor applies `Tls_mirage.Make` to an inner flow [P29] | rejected | Ignite modules are files, and a program is one file plus std (slice §4.1 Imports and the standard library). There is no module language to apply functors in. Generic impls give the same composition. |
| Compiler-known kinds only | rejected as the end state, used as phase 1's step | Every transport would be a compiler change, against design §9.1 ("Adapter … No" compiler change) and the package-author job of design §1.2. |

Go does this with run-time interfaces: `net.Conn` is "a generic stream-oriented network connection"
[P30], and `Server.Serve` takes any `net.Listener` [P31]. hyper does it with static bounds:
`serve_connection` requires `I: Read + Write + Unpin` [P27]. Ignite takes hyper's static form, which
the throughput work of section 6.3 favours.

### 2.2 Interfaces and impls

```ignite
impl<S: ByteStream> ByteStream for TlsStream<S> {
  fn read(stream: TlsStream<S>, maximum: Int) -> IoResult<Input> requires [stream.read] = ..
  ..
}
impl Transport for Tcp { type Listener = TcpListener  type Accepted = TcpStream  type Stream = TcpStream  .. }
```

- **Grammar.** Type parameters take bounds: `'<' ident (':' bound ('+' bound)*)? .. '>'`. Two new
  declarations, `interface Name tparams? { assoc* fnsig* }` and
  `impl tparams? Name<args>? for type { .. }`. New keywords `interface` and `impl`; `Self` is a type
  name. That is 31 keywords against slice §2's 29, inside the 30–50 budget (design §1.5).
- **Method calls.** Slice §4.8's resolution order gains a fourth step: a
  method of an interface that bounds the receiver's type parameter, or of an impl for the
  receiver's type. Calls stay UFCS, `stream.read(n)`.
- **Effects.** An impl method may need at most what its interface method declares, mapped by
  parameter name. Needing more is E096. So a layer cannot acquire authority its interface hides: a
  TLS read cannot open a socket.
- **Coherence.** At most one impl per interface and type (E098). An impl lives in the module of the
  interface or of the type.
- **Builtins.** `TcpStream`, `PipeStream` and later `UnixStream` implement `ByteStream` through
  builtin impls. Their methods are the slice §7 substrate methods, which already have these names.
- **Unbounded parameters** keep slice §11.1's rules (E041, E045). The `Eq` bound comes in phase 4.

### 2.3 Choosing the stack in a system

```ignite
system TodoService {
  resource ingress: Listener {
    transport = Tls { inner = Tcp { address = "127.0.0.1", port = 8443 }, identity = secret("todo-tls") }
  }
  resource todos: Store<Todo>
  component server: Service {
    listener = ingress, protocol = Http, limits = HttpLimits { body = 4194304 }
    application = HttpApp { state = todos, handle }, executable = "todod"
  }
}
```

- `Listener { transport = e }` takes any value implementing `Transport`; `TcpListener { address,
  port }` stays as shorthand for `Listener { transport = Tcp { .. } }`.
- The checker instantiates the role with `T` = the transport's `Stream` and checks the bound (E096
  before any build). `limits` defaults to `HttpLimits {}`.
- `secret("..")` is design §3.3's `SecretRef<T>`. Until secrets exist, P4
  reads `--tls-cert` and `--tls-key`, as `--port` works today (slice §6.1).
- The main calls `listen` once and `accept` in its loop; each spawned task calls `open`, then the
  role. `ignite-io::run` and `spawn` keep owning entry and threads (AGENTS.md, Lifecycle ownership).

**Rejected:** putting `transport` on the component. The listener is the thing with an address and a
lifecycle, and two components may share it during a drain rollout (design §11.8 Lowered control behavior).

### 2.4 Effects and authority through the layers

| Layer | Effects | Granted by |
|---|---|---|
| Transport `listen`, `connect` | `transport.*`, `net.listen` or `net.connect` | the generated main, a root of the effect graph (slice §5.1 Effect sets, "Roots") |
| Transport `accept`, `open` | `listener.accept`, `transport.*`, `accepted.*` | the main and its task |
| Protocol role | `stream.read`, `stream.write`, `application.*` | the main, through the role's `requires` (E030 if missing) |
| Application | what its fn fields declare, e.g. `[state.*]` | the role, through `Exchange`'s effectful fn type (slice §11.2 Effectful fn types) |

Effects stay on parameters at every layer, so the existing root rules carry them through. On wasm a
component's imports are derived from its effects (wasm §3.2). The todo application component still
imports only its store, whatever transport the system names. A Telnet component tested over a pipe
imports `ignite:io/pipe` and nothing that touches the network.

## 3 RFC binding

### 3.1 Catalogues

**Decision.** Each RFC a protocol binds to gets one module, `std/rfc/rfcNNNN.ignite`, holding:

- one `Requirement` constant per requirement;
- a `Spec` constant listing them, with the URL and a digest of the source they were extracted from;
- the RFC's collected ABNF, verbatim, in a `grammar` block (section 3.4; phase 3, the P2
  catalogues hold requirements only, slice §19.1).

The vocabulary module `std.rfc` is `std/rfc.ignite` (slice §19.3).

```ignite
package std.rfc

enum Level { Must, MustNot, Required, Shall, ShallNot, Should, ShouldNot, Recommended,
  NotRecommended, May, Optional }
enum Party { Any, Sender, Recipient, Client, Server, Proxy }
enum Source { Bcp14, Prose }
type Requirement { rfc: Int, section: Text, ordinal: Int, level: Level, party: Party = Any,
  source: Source = Bcp14, minimum: Option<Int> = None, text: Text }
type Spec { rfc: Int, title: Text, url: Text, digest: Text, requirements: List<Requirement> }

package std.rfc.rfc9112

const R3_0_4: Requirement = Requirement { rfc = 9112, section = "3", ordinal = 4,
  level = Recommended, minimum = Some(8000), text = "It is RECOMMENDED that all HTTP senders " +
  "and recipients support, at a minimum, request-line lengths of 8000 octets." }
```

Requirement ids in this document are illustrative; the importer assigns the real ones. The
importer writes no `minimum`; P2 adds none (slice §19.1).

- **Extraction.** `ignite rfc import docs/rfc/rfc9112.xml` reads a local RFCXML file (slice §19.8)
  and makes one requirement per sentence that
  contains a `<bcp14>` mark (RFC 7991 §2.9 [P14]); keywords count "only when they are in all
  capitals" (RFC 8174 [P13]). Existing extractors work by keyword on text [P15] or XML [P16]; Ignite
  uses XML wherever it exists.
- **Ids** are `R<section>_<ordinal>`; RFC 854 and 855 number no sections, so their section is
  the page, `p14`, and the id `RP14_1` (slice §19.4). A module may alias one (`const BARE_CR:
  Requirement = R2_2_3`) after P2; in P2 an alias is E091. The importer guesses the party ("A server MUST") and a human reviews it.
- **Prose RFCs.** RFC 854 (1983), 855 [P2] and 1143 [P3] predate BCP 14 and RFCXML. Their catalogues are
  hand-written with `source = Prose`, quoting the text, e.g. "only the IAC need be doubled to be
  sent as data" [P1].
- **Literal data only** (E091). `ignite rfc check` compares the digest with the current source.

**Rejected:** section annotations such as `@rfc("9112", "2.2")` (nothing could count what is
unimplemented); a `spec` declaration form (requirements are data, and protocol packages must
"compile into existing … forms", design §3.6); hand-written lists where XML exists (drift).

### 3.2 Linking

| Clause | Where | Meaning |
|---|---|---|
| `conforms [RFC9112, ..]` | `protocol P conforms [..] { .. }` | P claims these specs; every requirement in scope needs a status |
| `scope { parties = [Server, Recipient, Any], sections = ["2", .., "9"] }` | after `conforms` | narrows the claim; the rest is out of scope |
| `satisfies [RFC9112.R2_2_3, ..]` | after a fn's `requires`, on a type, or on a `Limits` field | the declaration implements these requirements (a claim) |
| `verifies [RFC9112.R2_2_3, ..]` | `test ".." verifies [..] { .. }` | the test is evidence for them |
| `unsupported RFC9112.R7_1_2 "reason"`, `not_applicable RFC9112.R3_2_1 "reason"` | protocol body | an explicit status |

A requirement is named `Spec.Id`, because every catalogue reuses ids such as `R2_1_1`; the
grammar is slice §19.5.

Each requirement in scope at MUST, MUST NOT, REQUIRED, SHALL or SHALL NOT level needs exactly one of
three statuses: satisfied by a declaration, unsupported, or not applicable. A missing status is
E090 in `ignite check`. A SHOULD or MAY with no status shows as `unclassified` and is not an error.
Naming a requirement outside the protocol's specs is E092. The clause names are contextual words,
not keywords.

**Rejected:** `conforms` alone. It cannot say which code meets which sentence.

### 3.3 The conformance matrix

`ignite conformance <file>` prints one row per requirement of each claimed spec, rows outside
`scope` included as `out of scope` (slice §19.10). Verdicts follow ESS's runners:
a skip is not a failure, and any skip makes the result inconclusive (compare-ess.md §4, citing ESS
`runners.md`, "Hold your own implementation to the suite").

| Verdict | When |
|---|---|
| pass | satisfied, and a `verifies` test passed; for a limit, the configured value meets `minimum` |
| fail | a `verifies` test failed |
| gap | MUST-level `unsupported` |
| deviation | SHOULD-level `unsupported`; RFC 2119 allows this when "the full implications must be understood" [P12] |
| inconclusive | satisfied but with no passing test, or a test was skipped |
| n/a, out of scope | `not_applicable`, or outside `scope` |

The claim holds when no MUST-level row is fail, gap or inconclusive. Today's HTTP server would not
hold it. RFC 9112 says "A recipient MUST be able to parse and decode the chunked transfer coding"
(§7.1), and that a server MUST answer 400 to an HTTP/1.1 request lacking Host (§3.2) [P5]. The
current server answers any `Transfer-Encoding` with 501 and does not check Host
(the module comment of `std/protocol/http.ignite`, `next_request`, `parse_head`). Illustrative
output; the format is slice §19.10, where a test without a result line is `not run`:

```
rfc9112 HTTP/1.1, protocol Http, parties Server Sender Recipient Any, sections all
R2_2_3  2.2  MUST         satisfied    parse_head; test "bare CR is 400" passed            pass
R3_0_4  3    RECOMMENDED  limit        HttpLimits.request_line = 8000, minimum 8000        pass
R3_2_6  3.2  MUST         unsupported  "Host is not checked"                               gap
R6_1_6  6.1  SHOULD       satisfied    next_request; test "unknown coding is 501" not run  inconclusive
R7_1_2  7.1  MUST         unsupported  "only Content-Length framing; 501 otherwise"        gap
claim conformance of Http to rfc9112: no (2 gaps, 0 failures, 1 inconclusive)
```

Diagnostics. E090–E098 lie in the range `vision.md` §7 (T4) gives this document, E090 to E099,
and avoid slice §8, wasm's E080–E083 and actors' E060–E064.

| Code | Meaning |
|---|---|
| E090 | MUST-level requirement in scope with no status |
| E091 | catalogue constant that is not literal data, or a duplicate requirement id |
| E092 | `satisfies`, `verifies` or a status naming a requirement outside the protocol's specs |
| E093 | grammar: undefined rule, ambiguity, unbounded repetition, or over its size budget |
| E094 | `matches` record: field bound to a rule that does not occur exactly once, or a repetition bound to a non-`List` |
| E095 | local grammar rule with a terminal that no RFC grammar defines |
| E096 | bound not met: missing impl, an impl method needing more effects than its interface, or a stream-kind mismatch |
| E097 | unused: a test-only constructor outside a test is E051 since slice 8 (slice §12.3), and `Pipe.pair()` joins it (slice §18.4, D3) |
| E098 | overlapping impls |

### 3.4 Wire formats

**Decision:** one form per notation the RFCs use.

| RFC notation | Ignite form | Phase | Example |
|---|---|---|---|
| ABNF: RFC 5234 [P10], RFC 7405 case-sensitive strings [P11], RFC 9110's `#rule` lists [P4] | `grammar` block, verbatim, plus `matches` records | 3 | HTTP/1.1 head, chunked framing |
| Prose about escape bytes | pure transition function (section 3.5) | today | Telnet IAC framing (`decode` in `std/protocol/telnet.ignite`) |
| Binary layouts with dependent lengths | `layout` declarations in the style of EverParse 3D: fixed-width integers, constraints in braces, `where` clauses, tagged unions [P19] | 6 | WebSocket [P34], HTTP/2, QUIC |

**`grammar`.** Inside `grammar Name uses Other.{rule, ..} { .. }` the lexer reads ABNF lines until a
`}` alone on a line (ABNF has no braces). The importer writes the RFC's collected ABNF there
unchanged, so it diffs cleanly against the RFC. A protocol may declare a local grammar that only
composes imported rules (E095 otherwise).

**What is generated.** Each `type R matches G.rule { f: T from sub-rule, .. }` gets an incremental
parser, `parse_r(state: ParseState<R>, input: Bytes, limits: L) -> Parsed<R>` returning `Done(R,
consumed)`, `More(state)` or `Invalid(rule, offset)`, which resumes across reads as Spicy's parsers
proceed "fully incrementally" [P18]. It also gets a printer, `print_r(value: R) -> Result<Bytes,
PrintError>`, that emits only what the grammar accepts: Nail's "semantic bijection" [P20], tested
(section 3.7). Header injection becomes impossible by construction, replacing `checked`
(`std/protocol/http.ignite`). Both are ordinary fns and enums produced by elaboration, like slice §10.1's
desugarings, so the backends see nothing new and design §3.6's rule holds.

**Accepted subset.** ABNF describes languages; it does not specify parsers. The compiler accepts a
grammar when every rule compiles to a deterministic byte-level automaton with bounded lookahead, and
every repetition has a limit from `L` or a finite count. An ambiguity is E093 and names both
alternatives. What ABNF cannot say stays outside it. RFC 9112 §6.3 body length depends on header
values [P5], so framing is a transition function. A chunk's data length is a dependent binding,
`data: Bytes from chunk-data length size`, in 3D's style.

**Rejected:** hand-written parsers only, as today (`parse_head` in `std/protocol/http.ignite` names no rule
and nothing compares it with the grammar); Kaitai Struct [P17] (a YAML language outside Ignite's
checker); parser combinators such as Hammer [P21] (no line-for-line match with the RFC, and resuming
across reads needs state Ignite code cannot hold).

### 3.5 State machines

**Decision.** A protocol state machine is a pure `step(state, input) -> (state, outputs)` over an
enum, the shape of today's `decode` (`std/protocol/telnet.ignite`). E046 already guarantees that every state
handles every input. Exploration is a library function in `std/check/explore.ignite`:

```ignite
type Machine<S, I, O> { initial: S, step: fn(state: S, input: I) -> (S, List<O>) }
type Bounds { depth: Int = 16, channel: Int = 4, states: Int = 100000 }
type Report<S> { explored: Int, loops: List<List<S>>, stuck: List<S>, violations: List<S>, exhausted: Bool }
fn explore<S: Eq, I: Eq, O: Eq>(a: Machine<S, I, O>, b: Machine<S, I, O>, wire: fn(o: O) -> I,
  inputs: List<I>, holds: fn(a: S, b: S) -> Bool, bounds: Bounds) -> Report<S> = ..
```

It explores two peers joined by bounded FIFO channels, breadth first, and reports loops, stuck
states and states violating `holds`. It needs the phase-4 `Eq` bound. A clean run is evidence within
its bounds, not a proof, so the report carries `explored` and `exhausted`, as ESS's protocol checker
reports its bounds and counts (ESS `docs/design/protocol-specification.md`, "Checking and evidence").

**Rejected:** a `machine` keyword (the function is the machine; `match` and E046 give what a further
declaration form would, design §3.2); a separate model in P [P22] or TLA+ [P23] (it
drifts from the code; Ignite explores the code itself).

### 3.6 Limits

**Decision.** Each protocol has one `Limits` record. Every field has a safe default, and a
`satisfies` link where an RFC states a minimum. Roles read limits only from this record, and a
system may override any field (section 2.3).

```ignite
type HttpLimits {
  request_line: Int = 8000 satisfies [R3_0_4]
  field_section: Int = 8192
  body: Int = 1048576
  idle_timeout_ms: Int = 30000
  drain_bytes: Int = 65536
  drain_timeout_ms: Int = 1000
}
type TelnetLimits { maximum_line: Int = 4096, maximum_subnegotiation: Int = 1024, idle_timeout_ms: Int = 600000 }
```

As built in P1 (slice §18.5, D4, D5): Telnet's record is the existing `SessionLimits` with quantity
fields, gaining `maximum_subnegotiation: ByteSize = 1 KiB`; `maximum_line` stays on `Console`.
`HttpLimits` has quantity fields with the former constants as defaults: `head = 8 KiB`, `body =
1 MiB`, `idle_timeout = 30 seconds`, `drain = 64 KiB`, `drain_timeout = 1 second`.

`ignite conformance` checks each `minimum` against the default and against every system's override.
Since P1 there is one head limit, `HttpLimits.head` (8 KiB, `std/protocol/http.ignite`), covering request line and
fields together. The split turns RFC 9112's RECOMMENDED 8000-octet request line [P5] into a checked
value. RFC 9110 sets no field limit and requires a 4xx when a server refuses [P4]; 431 is
RFC 6585's [P35]. hyper exposes the same knobs (`max_buf_size`, `max_headers`,
`header_read_timeout`) [P27].

**Rejected:** module constants, as before P1 (`MAXIMUM_SUBNEGOTIATION` in
`std/protocol/telnet.ignite`, `IDLE_TIMEOUT_MS` in `std/protocol/http.ignite`). A system cannot override them, and
they link to no requirement. P1 keeps three (slice §18.14 item 13): `MAXIMUM_SUBNEGOTIATION` for
the client role, whose arity 1 leaves no limits parameter (D17), and HTTP's `READ_SIZE` and
`MAXIMUM_LENGTH_DIGITS`, which are framing facts, not limits.

### 3.7 Conformance tests

Slice 8 brings `test "name" { .. }`, `assert` and `ignite test`. This document assumes a test body
behaves like a fn returning `IoResult<Unit>`.

| Source | What | Phase |
|---|---|---|
| Written | `test ".." verifies [..]` driving a role over `Pipe`, or a parser directly | 1, 2 |
| Grammar | per `matches` record: round trips `parse(print(x)) == x`; positive samples enumerated from the grammar; single-byte mutations that leave the language, expected `Invalid` | 3 |
| Limits | pairs at `limit` and `limit + 1` with the declared reaction (413, 431, close) | 3 |
| Exploration | `explore` runs, e.g. section 5.1 | 5 |

**ESS** checks another layer: the application contract, routes bound by `ess-http/1`, run against
`todod` through a target adapter (compare-ess.md §3, option (c)). Its protocol format excludes
byte-level parsing as "outside this format's admitted capability" (ESS
`docs/design/protocol-specification.md`). Ignite takes its verdict rules, later perhaps its traces (Q7).

## 4 What lives where

| Place | Holds | Rule |
|---|---|---|
| `crates/ignite-io` | Leaf transports: TCP (today), Unix sockets and `Pipe` (phase 1 and 4); TLS engine (rustls, phase 4); later QUIC | written once for every target; no application protocol (both in AGENTS.md, Rules of the layout) |
| `crates/ignite-rt` | C ABI adapter and handle kinds for new substrate types (llvm §5) | no IO of its own |
| `crates/ignite` | `ignite test`, `ignite conformance`, `ignite rfc import`, `ignite rfc check` | generic; no RFC number in Rust |
| `std/transport/` | `stream.ignite`, `transport.ignite`, `tcp.ignite`, `unix.ignite`, `pipe.ignite`, `tls.ignite` (`Tls<T>`, `TlsStream<S>`) | Ignite |
| `std/protocol/` | `telnet.ignite`, `http.ignite` (RFC 9112), later `websocket.ignite`, `http3.ignite` | Ignite |
| `std/http.ignite` | RFC 9110: `Request`, `Response`, methods, status codes, `Exchange`, `HttpApp` | Ignite |
| `std/rfc/` | `rfc.ignite`, `rfc5234.ignite`, `rfc854.ignite`, `rfc855.ignite`, `rfc1143.ignite`, `rfc9110.ignite`, `rfc9112.ignite` | literal data and verbatim ABNF only |
| `std/check/` | `explore.ignite` | Ignite |

**TLS.** rustls "does not take care of network IO"; ciphertext enters through `read_tls` and leaves
through `write_tls` [P32]. That makes it an engine rather than a socket. The substrate exposes it as
an opaque `TlsEngine` handle with bytes-in, bytes-out operations. `Tls<T>` in
`std/transport/tls.ignite` drives it over any inner `ByteStream` (TCP, Unix, or a pipe in a test).

**Resolved** (Timo, 2026-11-01, option A): AGENTS.md now reads "The substrate knows sockets and TLS sessions, not application protocols"; rustls goes in `crates/ignite-io`.
Proposed reading: the substrate holds sockets and security engines, never an application protocol.
Design §8.6 already put "TLS integration" in the runtime adapter (version 0.2), and design
§12.6 calls porting TLS libraries a separate investment. rustls replaces nothing, and its
commit says so (AGENTS.md, Conventions).

A TLS written in Ignite is not planned. Verified implementations exist: Project Everest combines
miTLS, HACL* and EverCrypt [P24], and HACL* compiles verified code to C [P25]. A Ignite TLS would be
unverified cryptography in a language with no constant-time guarantee.

**Rejected:** a compiler-known `TlsListener` kind (it cannot wrap a pipe or a Unix socket, and
every later wrapper would be a compiler change); a Ignite TLS over HACL* primitives now (it costs
more than everything else here, for nothing rustls lacks).

## 5 Worked examples

### 5.1 Telnet

Today (`protocol Telnet` and `decode` in `std/protocol/telnet.ignite`) the roles take a `TcpStream` and the limits are constants.
`decode` refuses every option:

```ignite
fn serve(stream: TcpStream, application: Console) -> IoResult<Unit> requires [stream.read, stream.write] {
  stream.set_idle_timeout(IDLE_TIMEOUT_MS)?
  ..
fn decode(state: Decoder, b: Byte) -> (Decoder, Event) = match (state, b) {
  ..
  (Negotiation(DO), _) => (Ground, Answer([IAC, WONT, b]))
  (Negotiation(WILL), _) => (Ground, Answer([IAC, DONT, b]))
```

The proposal, from phases 1, 2 and 5: the roles are generic over the stream and take a limits
record, RFC 1143's Q method is a pair of transition functions, and each claim names its
requirement.

```ignite
protocol Telnet conforms [RFC854, RFC855, RFC1143] {
  fn serve<S: ByteStream>(stream: S, limits: SessionLimits, application: Console)
    -> IoResult<Unit> requires [stream.read, stream.write] {
    stream.set_idle_timeout(limits.idle_timeout.to_ms())?
    let result = session(stream, limits, application)
    _ = stream.close()
    result
  }
}

enum Q { No, Yes, WantNo(Queue), WantYes(Queue) }
enum Queue { Empty, Opposite }
enum Negotiated { Silent, Send(Byte), Violation }

// RFC 1143 section 7, receipt of WILL (agree DO, refuse DONT) or DO (agree WILL, refuse WONT).
fn on_enable(side: Q, wanted: Bool, agree: Byte, refuse: Byte) -> (Q, Negotiated)
  satisfies [Q_RECEIVE_ENABLE, NO_ACK_CURRENT_MODE] = match (side, wanted) {
  (No, true) => (Yes, Send(agree))
  (No, false) => (No, Send(refuse))
  (Yes, _) => (Yes, Silent)
  (WantNo(Empty), _) => (No, Violation)
  (WantNo(Opposite), _) => (Yes, Violation)
  (WantYes(Empty), _) => (Yes, Silent)
  (WantYes(Opposite), _) => (WantNo(Empty), Send(refuse))
}

// Receipt of WONT or DONT.
fn on_disable(side: Q, agree: Byte, refuse: Byte) -> (Q, Negotiated)
  satisfies [Q_RECEIVE_DISABLE] = match side {
  No => (No, Silent)
  Yes => (No, Send(refuse))
  WantNo(Empty) => (No, Silent)
  WantNo(Opposite) => (WantYes(Empty), Send(agree))
  WantYes(_) => (No, Silent)
}
```

The arms follow RFC 1143 §7 case by case [P3]. On WILL in state NO, the receiver agrees with DO or
refuses with DONT. In YES it ignores the request. WANTNO is an error. WANTYES with OPPOSITE sends
DONT. Today's `decode` is the special case `wanted = false` with every option in `No`. The arm
`(Yes, _) => (Yes, Silent)` is RFC 854's rule that a request to enter a mode already in effect
"should not be acknowledged" [P1].

```ignite
test "DO ECHO is refused and IAC IAC is data" verifies [NO_ACK_CURRENT_MODE, IAC_DOUBLED] {
  let (client, server) = Pipe.pair()
  spawn Telnet.serve(server, SessionLimits {}, Console { prompt = "> ", respond = echo, maximum_line = 80 })
  assert read_text(client, 2)? == "> "
  client.write_all([IAC, DO, ECHO, b'a', IAC, IAC, b'\r', b'\n'])?
  assert read_bytes(client, 3)? == [IAC, WONT, ECHO]
  assert read_bytes(client, 7)? == [b'a', 0xEF, 0xBF, 0xBD, b'\r', b'\n', b'>']
}
```

The data byte 255 comes back as EF BF BD: the server decodes the line with `to_text`, which
turns the invalid byte into U+FFFD, and the echo console sends that (slice §18.0 row 5).
`assert` is a statement (slice §12.3). `read_text` and `read_bytes` read exactly n bytes or fail;
they are test fns of the module, since P1 has no `std/transport/` (slice §18.14 item 5; P1's
tests read whole batches instead, slice §18.12). A second
test, "Q method converges", runs `explore` over two peers built from `on_enable` and `on_disable`
and asserts no loops, no violations and unexhausted bounds (phase 5).

### 5.2 HTTP/1.1 framing from ABNF

The catalogue holds RFC 9112's ABNF verbatim [P5] (excerpt):

```ignite
grammar Rfc9112 uses Rfc9110.{token, field-name, field-value, OWS}, Rfc5234.{CRLF, SP, HEXDIG} {
HTTP-message   = start-line CRLF *( field-line CRLF ) CRLF [ message-body ]
start-line     = request-line / status-line
request-line   = method SP request-target SP HTTP-version
status-line    = HTTP-version SP status-code SP [ reason-phrase ]
field-line     = field-name ":" OWS field-value OWS
chunked-body   = *chunk last-chunk trailer-section CRLF
chunk          = chunk-size [ chunk-ext ] CRLF chunk-data CRLF
chunk-size     = 1*HEXDIG
last-chunk     = 1*("0") [ chunk-ext ] CRLF
}
```

The protocol binds records to the grammar and adds one local rule composed from RFC rules:

```ignite
grammar Head uses Rfc9112.{request-line, field-line, CRLF} {
request-head   = request-line CRLF *( field-line CRLF ) CRLF
}

type RequestLine matches Rfc9112.request-line {
  method: Text from method
  target: Text from request-target
  version: Text from HTTP-version
}
type Field matches Rfc9112.field-line { name: Text from field-name, value: Text from field-value }
type RequestHead matches Head.request-head satisfies [R2_2_3, R5_1_2] {
  line: RequestLine from request-line
  fields: List<Field> from field-line
}
type Chunk matches Rfc9112.chunk { size: Int from chunk-size as hex, data: Bytes from chunk-data length size }

enum Framing { Empty, Length(Int), Chunked, Reject(Int) }

// RFC 9112 section 6.3: how a request body is delimited.
fn framing(head: RequestHead, limits: HttpLimits) -> Framing satisfies [R3_2_4, R6_3_5, R7_1_1] =
  match (values(head, "host"), values(head, "transfer-encoding"), values(head, "content-length")) {
    ([], _, _) => Reject(400)
    ([_, _, ..], _, _) => Reject(400)
    (_, [], []) => Empty
    (_, [], [n]) => match decimal(n) {
      Some(length) => if length > limits.body { Reject(413) } else { Length(length) }
      None => Reject(400)
    }
    (_, [], _) => Reject(400)
    (_, codings, _) => if last_is_chunked(codings) { Chunked } else { Reject(400) }
  }
```

`[_, _, ..]` is a rest pattern that slice §3 lacks; until then that arm is a `len()` test. The
generated parser replaces about 120 lines (`parse_head`, `is_token`, `is_token_byte`,
`is_field_value`, `is_target`, `read_head`, `checked`, all in `std/protocol/http.ignite`). RFC 9112
§5.1's ban on whitespace before the colon (400) [P5] needs no check: `field-name` is a `token` and
no `OWS` precedes `":"`; the generated mutation `"Host : x"` is expected `Invalid`. `R6_3_5` is
§6.3's 400-and-close for a final coding other than chunked [P5].

### 5.3 The todo service with a transport swap

`examples/todo/todo.ignite` from `Todo` to `handle` (types, `handle`, CRUD) stays as it is, and so do
its imports of `HttpApp`, `Request` and `Response` from `std.protocol.http`, which stays that
module's path (slice §18.14 item 9). The application runs in three bindings: the
present system (`system TodoService`, `TcpListener` on port 8080); TLS, by replacing the resource line
with section 2.3's `Listener { transport = Tls { .. } }`; and a test with no listener at all:

```ignite
test "create and read back over a pipe" verifies [R9_3_2] {
  let (client, server) = Pipe.pair()
  let todos: Store<Todo> = Store.new()
  spawn Http.serve(server, HttpLimits {}, HttpApp { state = todos, handle })
  client.write_all(b"POST /todos HTTP/1.1\r\nHost: t\r\nContent-Length: 17\r\n\r\n{\"title\":\"milk\"}")?
  client.write_all(b"GET /todos/1 HTTP/1.1\r\nHost: t\r\n\r\n")?
  let first = read_response(client)?
  let second = read_response(client)?
  assert first.status == 201 && second.status == 200 && second.body.contains("milk")
  assert client.received_batches() == 1
}
```

`R9_3_2` is §9.3's persistence rule [P5]. `Store.new()` is allowed only in tests (E051), because
stores are otherwise system resources (slice §11.3 `Store<T>`). `received_batches()` is a test method of
the pipe that counts the deliveries caused by the peer's flushes. It is 1 because the server wrote
both answers before its next wait (section 6.3). `handle`, `Todo` and the application's wasm
component are the same in all three bindings (wasm §3.2).

## 6 Compilation

### 6.1 Rust target

An interface becomes a trait, `pub trait ByteStream: Clone + Send + Sync + 'static`, and a bound
becomes `<S: ByteStream + Clone + Send + Sync + 'static>` (slice §6's bounds plus the trait).
Builtin impls are `impl ByteStream for ignite_io::TcpStream` (and `PipeStream`) in `ignite-io`,
delegating to the inherent methods. Associated types and generic impls map one to one. A call on a
bounded `S` is `ByteStream::read(&stream, n)`, statically dispatched; rustc monomorphizes
`serve::<TcpStream>`, and neither the program crate nor `ignite-std` contains `dyn ByteStream`;
the only `dyn` is a fn value's `Arc<dyn Fn>` (slice §13.3, §18.14 item 2, D9). As built in P1
(slice §18.10) the call is qualified with the type, `<S as ignite_io::ByteStream>::read(&stream,
n)`, because the unqualified form infers `Self = &S`. `grammar` and `matches` are
elaborated before generation. `conforms`, `satisfies`, `verifies` and catalogues emit nothing; only
`ignite conformance` reads them. Tests compile into a separate crate (slice 8), so `PipeStream`
instances never reach `todod`.

### 6.2 LLVM target

Instances already exist: one function per instance (slice §11.1). A bound adds one step. After the
instance is substituted, a call of an interface method on `S` resolves to the concrete type's impl:
a direct call to `ignite_rt_tcp_read`, or to the Ignite function for `TlsStream<TcpStream>`. There
are no vtables. An associated type is substituted like a type argument. `PipeStream` becomes one
more handle kind in `ignite-rt`'s table (llvm §5).

### 6.3 Throughput: one write per reply

The measured baseline before coalescing made "exactly 2 sendto per reply" on both targets
(throughput baseline, "Syscalls per 1000 replies"). Telnet now writes reply and prompt in one
`write_all` (`answer_line` in `std/protocol/telnet.ignite`), and the substrate sends a buffer only before its writer waits,
ends or flushes, so pipelined answers leave in one system call (slice §7, "Write coalescing"). Rules that keep
this:

- **Protocols.** A role builds a whole reply and writes it once; generated printers return `Bytes`.
- **Layers.** A Ignite layer keeps no bytes across `write_all`: `TlsStream.write_all` encrypts one
  record and hands it on at once. A layer buffering in Ignite would deadlock request/response, since
  the substrate cannot flush what it does not hold. No `flush` in the interface: every protocol would
  have to call it in the right place.
- **Pipes.** `PipeStream` coalesces as `TcpStream` does, so `received_batches` (section 5.3) checks
  production batching for every protocol on both targets.
- **Parsers.** Generated parsers scan runs of one character class in a tight loop; P3 gates on
  pipelined `todod` throughput.

### 6.4 Slice 5, wasm and actors

Slice 5 makes the substrate async and Rust-target fns futures (AGENTS.md, Rules of the layout). Interface methods
then lower to async trait methods, with the source unchanged (Q6). On wasm each instance of a
generic role is its own export, and `TlsEngine` maps to a `ignite:io/tls` import (wasm §3.2). Actors
stay behind the application edge, where the protocol is "the byte-level edge" (actors §2.2).

## 7 Phased roadmap

Sizes as in actors §11. Phase 1 is slice 14 (vision §4), whose contract is slice §18; where this
row and the code disagree, slice §18.14 records the point.

| Phase | Deliverable | Acceptance test | Size | Needs |
|---|---|---|---|---|
| **P1** Pipe and the stream bound | `Pipe.pair()` and `PipeStream` in `ignite-io` and `ignite-rt`, coalescing as TCP, with `received_batches`; bound syntax with one builtin interface, `ByteStream` (from `std.net`); `Telnet` and `Http` roles generic over `S: ByteStream`, taking a `limits` record; `Pipe.pair()` in tests only (E051, as `Store.new()`); E096 and E099. Moving HTTP to `std/protocol/http.ignite` is already done. | `ignite test std/protocol/telnet.ignite` and `ignite test std/protocol/http.ignite` pass on both targets. Telnet over a pipe: slice §9 item 4's transcript, `IAC DO ECHO` answered `IAC WONT ECHO`, `IAC IAC` as one literal byte, and an oversized subnegotiation closing the session. HTTP over a pipe: 3 pipelined requests answered in 1 batch, plus the 400, 413 and 431 cases. `crates/ignite/tests/terminal.rs` and `todo.rs` pass unchanged over TCP. The generated `ignite-std` of the terminal calls `Telnet::serve::<ignite_io::TcpStream>` and holds no `dyn ByteStream` (slice §18.0 row 10). | S | slice 8 (`test`, `assert`, `ignite test`) |
| P2 Catalogues and matrix | `std/rfc.ignite`; `ignite rfc import` and `check`; catalogues for RFC 854, 855 and 1143 (prose), 9112 §2–§9 and the 9110 server subset; `conforms`, `scope`, `satisfies`, `verifies`, the statuses; literal-constant evaluation; `ignite conformance`; E090–E092 | Removing one `satisfies` from a MUST makes `ignite check` fail with E090. The HTTP matrix lists the §7.1 and §3.2 gaps and claims nothing. The Telnet matrix claims RFC 854 for an NVT that refuses every option. | M | P1 |
| P3 ABNF grammars | `grammar`, `matches`, `from`, `as hex`, `length`; generated parsers, printers and tests; HTTP head and chunked body; Host check; split `HttpLimits`; E093–E095 | The hand-written head functions are gone from `http.ignite`; `todo.rs` passes on both targets with chunked bodies; the RFC 9112 matrix has no MUST gap in §2–§7; pipelined `todod` throughput is within 10 % of P1's. | L | P2 |
| P4 Interfaces and transports | user `interface` and `impl`, associated types, generic impls, `Eq`; `std/transport/*`; `Listener { transport = .. }`; `TlsEngine` and Unix sockets in `ignite-io`; E098 | `todo.rs` passes over TCP, TLS and a Unix socket, with `todo.ignite` differing only in the `ingress` line. A stream-kind mismatch is E096. The application's wasm imports do not change. | L | P1; decision on Q1 |
| P5 Exploration, Q method | `std.check.explore`; Telnet negotiation by RFC 1143, supporting ECHO and SUPPRESS-GO-AHEAD and refusing the rest | "Q method converges" passes without exhausting its bounds. Deleting the `WantYes(Opposite)` arm makes it fail. Slice §9 items 4 to 9 still pass. | M | P4 (`Eq`) |
| P6 Messages and layouts | `MessageStream<M>`; `layout`; WebSocket [P34] as a protocol over an HTTP/1.1 upgrade and as a message-stream transport | A Telnet session over WebSocket over TLS over TCP passes P1's transcript. | L | P4 |
| P7 QUIC and HTTP/3 | QUIC engine in `ignite-io`, `Multiplexed`; `Http3` with QPACK [P33] in Ignite | `todo.rs` passes over HTTP/3 with the same `handle`. | L+ | slice 5, P6 |

P1 fits soon after slice 8. It adds one bound and one substrate type and edits two std files; no
system syntax changes. It fixes the names `ByteStream`, `read`, `write_all`, `close` and
`set_idle_timeout`, which phase 4 then declares in Ignite. Protocol code written in P1 therefore
compiles unchanged later, apart from one import line.

## 8 Risks and open questions

| # | Risk or question | Handling |
|---|---|---|
| Q1 | **Conflict** with AGENTS.md, Rules of the layout ("not protocols"): a TLS engine (later QUIC) in `crates/ignite-io` is a protocol in the substrate. | Proposed rule: "the substrate knows sockets and security engines, not application protocols". Resolved by Timo on 2026-11-01: option A, rule changed in AGENTS.md. |
| Q2 | ABNF describes languages: ambiguity, `obs-fold`, `obs-text` and context-dependent lengths may leave too little for a generated parser. | E093 makes the gaps visible, and obsolete forms become `unsupported` rows. P3 starts by measuring how much of RFC 9112 compiles. |
| Q3 | A generated parser may be slower than today's `bytes_find_from` scan for `\r\n\r\n` (`read_head` in `std/protocol/http.ignite`). | Run scanning, and P3's 10 % gate. Fallback: a hand-written scan delimits the head, and the generated parser validates it. |
| Q4 | Catalogues are large and partly hand-classified; a `satisfies` is only a claim. | Scopes keep claims small. A claim without a passing test is inconclusive. Mutation audits of `verifies` tests follow ESS (compare-ess.md §4). |
| Q5 | Keywords grow from 29 to 32 (`interface`, `impl`, `grammar`; `test` is one since slice 8), and design §3.6 asks for existing forms. | Not a conflict: grammars, catalogues and claims elaborate into fns, enums and constants, and 32 is inside the 30–50 budget (design §1.5). |
| Q6 | Slice 5 makes interface methods async; async trait methods with `Send` bounds are the least settled part of the Rust target. | P4 lands before or with slice 5's lowering. If they collide, the Rust target emits concrete instances itself, as the LLVM target does. |
| Q7 | Should `explore` emit ESS's `ess-prototrace/1` traces? | Decide after P5, if ESS adopts byte-level protocol targets. |
| Q8 | Design §8.6 (version 0.2) named libuv and llhttp as candidates. | Superseded by the direction paragraph at the top of `slice.md` (protocols in Ignite) and AGENTS.md, Rules of the layout (tokio). Design §8.6 says so since version 0.3. |

## 9 References

| ID | Source | Used for |
|---|---|---|
| P1 | [RFC 854 Telnet Protocol Specification](https://www.rfc-editor.org/rfc/rfc854.html) | IAC doubling; no acknowledgment of the current mode |
| P2 | [RFC 855 Telnet Option Specifications](https://www.rfc-editor.org/rfc/rfc855.html) | Subnegotiation framing |
| P3 | [RFC 1143 The Q Method of Implementing Telnet Option Negotiation](https://www.rfc-editor.org/rfc/rfc1143.html) | §7 states, queue, transitions |
| P4 | [RFC 9110 HTTP Semantics](https://www.rfc-editor.org/rfc/rfc9110.html) | Version-independent semantics; BCP 14 use (§2.2); field limits (§5.4); roles; `#rule` |
| P5 | [RFC 9112 HTTP/1.1](https://www.rfc-editor.org/rfc/rfc9112.html) | ABNF; §2.2 bare CR; §3 8000 octets; §3.2 Host; §5.1 colon; §6.3 body length; §7.1 chunked; §9.3 persistence |
| P6 | [RFC 9114 HTTP/3](https://www.rfc-editor.org/rfc/rfc9114.html) | HTTP semantics over QUIC; per-stream framing; flow control in QUIC |
| P7 | [RFC 9000 QUIC](https://www.rfc-editor.org/rfc/rfc9000.html) | Multiplexed transport |
| P8 | [RFC 9293 Transmission Control Protocol](https://www.rfc-editor.org/rfc/rfc9293.html) | TCP as the leaf byte-stream transport |
| P9 | [RFC 8446 TLS 1.3](https://www.rfc-editor.org/rfc/rfc8446.html) | Reliable in-order stream is the only transport requirement |
| P10 | [RFC 5234 ABNF](https://www.rfc-editor.org/rfc/rfc5234.html) | Grammar notation, core rules |
| P11 | [RFC 7405 Case-Sensitive String Support in ABNF](https://www.rfc-editor.org/rfc/rfc7405.html) | `%s` strings |
| P12 | [RFC 2119 Key words](https://www.rfc-editor.org/rfc/rfc2119.html) | MUST, SHOULD, MAY |
| P13 | [RFC 8174 Uppercase vs Lowercase in RFC 2119 Key Words](https://www.rfc-editor.org/rfc/rfc8174.html) | Keywords only in capitals |
| P14 | [RFC 7991 The xml2rfc Version 3 Vocabulary](https://www.rfc-editor.org/rfc/rfc7991.html) | §2.9 `<bcp14>` |
| P15 | [rfcparser](https://github.com/sriramkandukuri/rfcparser) | Requirement lists by keyword |
| P16 | [RFCXML MCP server](https://glama.ai/mcp/servers/shuji-bonji/rfcxml-mcp) | MUST/SHOULD/MAY extraction from RFCXML |
| P17 | [Kaitai Struct user guide](https://doc.kaitai.io/user_guide.html) | `.ksy` format descriptions |
| P18 | [Spicy](https://docs.zeek.org/projects/spicy/en/latest/) | Incremental protocol parser generator |
| P19 | [EverParse 3D](https://project-everest.github.io/everparse/3d-lang.html) | Dependent binary layouts, verified validators |
| P20 | [Nail, OSDI 2014](https://www.usenix.org/conference/osdi14/technical-sessions/presentation/bangert) | Semantic bijection; parse and generate |
| P21 | [Hammer](https://github.com/UpstandingHackers/hammer) | Parser combinators for binary formats |
| P22 | [P language](https://p-org.github.io/P/whatisP/) | Communicating state machines |
| P23 | [TLA+](https://lamport.azurewebsites.net/tla/tla.html) | TLC model checker |
| P24 | [Project Everest](https://www.microsoft.com/en-us/research/project/project-everest-verified-secure-implementations-https-ecosystem/) | miTLS, HACL*, EverCrypt |
| P25 | [HACL*](https://github.com/project-everest/hacl-star) | Verified cryptography compiled to C |
| P26 | [tower `Service`](https://docs.rs/tower/latest/tower/trait.Service.html) | Request to response function |
| P27 | [hyper `http1::Builder`](https://docs.rs/hyper/latest/hyper/server/conn/http1/struct.Builder.html) | `serve_connection`; buffer, header and timeout limits |
| P28 | [Netty `ChannelPipeline`](https://netty.io/4.1/api/io/netty/channel/ChannelPipeline.html) | Run-time handler pipelines |
| P29 | [MirageOS `Conduit_mirage`](https://mirage.github.io/ocaml-conduit/conduit-mirage/Conduit_mirage/index.html) | TCP and TLS functors |
| P30 | [Go `net.Conn`](https://pkg.go.dev/net#Conn) | Generic stream-oriented connection |
| P31 | [Go `net/http`](https://pkg.go.dev/net/http) | `Server.Serve(l net.Listener)` |
| P32 | [rustls](https://docs.rs/rustls/latest/rustls/) | No network IO; `read_tls`, `write_tls` |
| P33 | [RFC 9204 QPACK](https://www.rfc-editor.org/rfc/rfc9204.html) | HTTP/3 field compression |
| P34 | [RFC 6455 WebSocket](https://www.rfc-editor.org/rfc/rfc6455.html) | Message stream over HTTP/1.1 upgrade |
| P35 | [RFC 6585 Additional HTTP Status Codes](https://www.rfc-editor.org/rfc/rfc6585.html) | 431 |
