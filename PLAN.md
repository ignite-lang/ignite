# Plan

The roadmap in outline. The reasoning and the dependency analysis are in `docs/vision.md`
(section 4 waves, section 4.1 slices, section 8 decisions); the contract of each slice is
`docs/slice.md`. Decided on 2026-11-01: operations first (D1 option A).

## Now

| step | state |
|---|---|
| slice 10, F1 system declaration | done 2026-11-02: 14.0 items 1 to 6 pass; 552 cargo tests, 97 inline tests per target |
| rename to Ignite, `README.md`, commit, push to `https://github.com/ignite-lang/ignite` | done 2026-11-02, commit `60aef38` |
| slice 11, F2 plan and explain (`docs/slice.md` section 15) | done 2026-11-02, published in `9273349`: 580 cargo tests, 102 inline tests per target; gate cold 64.8 to 54.5 s, warm 25.9 to 16.8 s; `todod` LLVM release 1.10 to 0.80 s |
| slice 11b: four pre-existing backend bugs found by the evaluator (indexed `push` lost on the Rust target, enum map keys, handle types as fn values, std name collisions in generated Rust), warm test step | done 2026-11-02, published in `9273349`: 582 cargo tests; warm test step 16.1 to 10.8 s at load 6 to 9 (target under 8 s at load under 5 not measurable that night) |
| slice 12 contract (`docs/slice.md` section 16) | reviewed 2026-11-02 with 15 amendments |
| slice 12, F3 run and control | done 2026-11-02, published in `9273349`: 16.0 items 1 to 11 and 13 pass; 636 cargo tests, 148 inline tests per target; item 12 (gate time) carried into 12b |
| slice 12b: gate time back within 10 % of 11b (the frontend loads only the std modules a program reaches, rustgen emits a reachable-only `ignite-std`, `ignite_test.rs` cut back) | implementation and measurement complete 2026-11-02, published in `9273349`: 647 cargo tests, 148 inline tests per target; cold gate 57.14 → 66.11 s (1.157), warm 13.37 → 16.79 s (1.255) against 11b; timing exceptions and measured causes in `AGENTS.md`, baseline unchanged |
| commit and push slices 11 to 12b | done 2026-11-02: `9273349` on `origin/main`; after the commit-message rewrite of 2026-11-02 the same tree is `28dfc75` on `main`, and tag `bench-12b-checkpoint` keeps `9273349` |
| slice 12c, restore the original performance budget | release tested fixes with documented gaps, 2026-11-02; performance acceptance remains open and further optimization is deferred by user; [`evidence`](docs/performance/12c/release-815cf74/README.md) |
| slice 13 contract (`docs/slice.md` section 17), reviewed before implementation agents are dispatched | proposed and independently reviewed: explicit foreground host, source-bound plans, replacement-first; [review](docs/slice13-review.md); approved 2026-11-02 17:33, merged as `50beccb`; implementation tracked as `epic:slice-13-local-apply` in `.engineering/planning/` |
| slice 14 contract (`docs/slice.md` section 18), wave B protocols P1 | contract written and independently reviewed 2026-11-03, findings resolved; [review](docs/slice14-review.md); approved in advance; units U1 to U5 in 18.16, to be tracked as `epic:slice-14-protocols-p1` in `.engineering/planning/` |

## Slices

| slice | wave | content | acceptance | state |
|---|---|---|---|---|
| 1 to 8 | language, two native targets, protocols, inline tests | lexer to LLVM target, `std/`, Telnet and HTTP in the language, `test` blocks | `task gate` | done, commit `44334b0` |
| 9 | A0 soundness, A friction rows 1 to 17 | mutability through bindings (E006), ambient names (E056), unused type parameters (E057), LLVM arena fixes M1 to M5, audit probes as tests | gate green on both targets | done 2026-11-01 |
| 9b | invariant 9 | one test executable per target, one integration-test binary per crate | gate cold 129.9 s to 86.5 s, warm 35.4 s to 25.9 s | done 2026-11-01 |
| 9c | LLVM parity, T6 | overflow is a failure on both targets, debug metadata, pinned triple, `-O0` dev | identical overflow failure on both targets, `gdb` shows source lines | done 2026-11-01 |
| 10 | F1 system declaration | quantity literals, `resource`, `component name: Type { … }`, `contract`, `policy`, `assembly`, `module("…")`, `Map<K, V>`, named arguments, std types for contracts, policies, probes, deployment and CLI; T1 a failure ends its task, with file, line, column and operation on both targets; substrate items R2 to R5 | `ignite check examples/terminal/terminal.design.ignite` exit 0; both examples in component form with unchanged behaviour; `terminald` survives a failing handler | done 2026-11-02 |
| 11 | F2 plan and explain | `ignite plan` emits the versioned system model (D7, deterministic JSON), `ignite explain`, assembly bindings resolved, pure evaluator with fuel (D5) | model byte-identical across runs and golden-tested; every property value evaluated | done 2026-11-02 |
| 12 | F3 run and control | `ignite run <assembly>` and `ignite replay`: probes as protocol clients (one probe process per attempt), SLI windows and contract evaluation, `Restart` with backoff, `DrainRollout`, event journal with manual clock for tests, `event("…")` consequences; reducers in `std/control` stepped by the evaluator; signals and references get runtime behaviour | the terminal system runs under `ignite run`; a killed `terminald` restarts within the policy's backoff; a probe timeout is in the journal and the contract's standing; every test journal replays byte for byte | done 2026-11-02; gate time in 12b |
| 12c | invariant 9 | remove repeated std preparation in tests, profile inline-test and frontend work, close cold/warm compiler costs | every required unit ≤1.10 of frozen 11b, warm workspace ≤10.8 s, unchanged behaviour and coverage | tested fixes delivered; performance acceptance unmet and deferred by user; `docs/performance-plan.md` |
| 13 | F4 apply | `ignite apply` with plan diff, changes, deletion, recovery of a half-applied plan from the journal | a second `apply` is a no-op; an interrupted `apply` resumes | done 2026-11-03, release `v0.2.0`: the 17.7 matrix on both targets (coverage table after 17.7, `STATUS.md`); gate-budget exception in `AGENTS.md` |
| 14 | B protocols P1 | `Pipe.pair()` and `PipeStream` in `ignite-io` and `ignite-rt`, coalescing as TCP, with `received_batches`; the bound `<S: ByteStream>` on fns and roles (E096, E099); `Telnet` and `Http` roles generic over the stream, taking `SessionLimits` and the new `HttpLimits`; in-process protocol tests on both targets (`docs/slice.md` section 18) | the 18.0 matrix: `ignite test` of `std/protocol/telnet.ignite` and `http.ignite` passes on both targets; over a pipe, slice 9 item 4's transcript, `IAC DO ECHO` answered `IAC WONT ECHO`, `IAC IAC` as one byte, an oversized subnegotiation closing, 3 pipelined requests in 1 batch, and 400, 413 and 431; `terminal.rs` and `todo.rs` pass unchanged over TCP; the generated `ignite-std` calls `Telnet::serve::<ignite_io::TcpStream>` with no `dyn ByteStream`; no new dependency | done 2026-11-03, release `v0.3.0`: the 18.0 matrix holds (`STATUS.md`); gate time in `AGENTS.md` "Slice 14" |
| 15 | G protocols P2 | the vocabulary `std.rfc` and five catalogues under `std/rfc/` (RFC 854, 855, 1143, 9112, 9110 server subset) as literal data; the clauses `conforms`, `scope`, `satisfies`, `verifies`, `unsupported`, `not_applicable`; the claim model with E090 to E092; `ignite rfc import` and `ignite rfc check`; `ignite conformance`; the claims of `Telnet` and `Http` (`docs/slice.md` section 19) | the 19.0 matrix: removing a `satisfies` from a MUST is E090; the HTTP matrix lists the section 7.1 and 3.2 gaps and claims nothing; the Telnet matrix claims RFC 854 and RFC 855 | done 2026-11-04 on `s15/claims`, merge into `main` pending: units U1 to U6; every row of 19.0 passes, `task gate` exit 0 at 621070e (`STATUS.md`); P2 test 3 is not met in this slice: RFC 855 is claimed, RFC 854 is not (four MUST-class gaps: Go Ahead, Synch on send and receive, sending buffered data at once from `tn`; `docs/slice.md` 19.14 item 16); the U6 adversary's findings in 19.14 item 17 |
| 16 | A language gaps | last-use moves (one analysis in `ignite-types`, `TypedProgram::moves` and `consumed`, read by both targets; actors R1); calls of fn-typed expressions (`adder(1)(2)`, `fs[i](x)`, `(h.run)(2)`); rest patterns in list patterns (`[first, ..rest]`, `[.., last]`, E076, E046 split by length); a both-target fixture for every wave-A item that already held (`docs/slice.md` section 20) | the 20.0 matrix: R1 linear on both targets (200,000 steps under 8 times 50,000), moves keep value semantics under the 0xDB debug runtime, rest patterns run and agree with the evaluator | done 2026-11-05 on `feat/slice16-language-gaps`, merge into `main` pending: units U0 to U6; every row of 20.0 passes, `task gate` exit 0 at 426449e, 1009 cargo tests, 211 inline per target (`STATUS.md`); gate time in `AGENTS.md` "Slice 16", partly collected |
| 17 | playground substrate | capabilities as values (P3): `Directory` (files under a root), `Launcher` and `ChildProcess` (child processes), `Entropy` (random bytes), `Setting` (one command-line flag), each a resource kind and handle type on both targets over `crates/ignite-io`; `std.fs`, `std.process`; E140 to E142; capability resources hosted by their member's process; `HttpServer` gains the run hooks `TelnetServer` has (`docs/slice.md` section 21) | the 21.0 matrix: a handler with no `requires` writes, reads, runs and draws through its state on both targets (row 24) | done 2026-11-05 on `feat/slice17-processes-files`, merge into `main` pending: units U0 to U6; every row of 21.0 passes, `task gate` exit 0 at 3c5cc34f, 1089 cargo tests, 233 inline per target (`STATUS.md`); gate time in `AGENTS.md` "Slice 17", partly collected; `HttpServer`'s hooks make every HTTP program load `std.control` (`docs/friction.md` row 43) |
| 18 | F5 one service operated | the todo service on the LLVM target with its data in a `Directory`, an HTTP probe kind and its typed outcome written outside `std/`, an `Availability` contract over it, `ignite apply` rollouts that keep the data, fault-injection rows for lost acknowledgement, stale plan and corrupt journal; build time as an acceptance row (`docs/research/review-2026-11-04.md`, `epic:slice-18-one-service-operated`); `ignite recover`, the way out of a recovery no corrective resolves | the 22.0 matrix of `docs/slice.md` section 22 | done 2026-11-05 on `feat/slice18-one-service`, merge into `main` pending: units U0 to U8; every row of 22.0 passes, `task gate` exit 0 at f64adfec8, 1190 cargo tests, 322 inline per target (`STATUS.md`); gate time in `AGENTS.md` "Slice 18", partly collected; open gaps in 22.16 Q6 to Q10 |
| | agent delivery benchmark | the same requirements through direct Rust, ESS to service-sdk, direct Ignite and ESS to Ignite; tokens, cost, wall time, builds, memory, repairs at equal correctness (`docs/research/review-2026-11-04-ecosystem.md`, `epic:agent-delivery-benchmark`) | `docs/performance/agent-delivery/` | planned, beside slice 18 |
| | C actors P1 to P3 | `Address`, `ReplyTo`, `Outbox`, `Actor` kind, mailboxes, `std/actor`, `std/table`, `Topic`, `Sink`, `Supervisor`, todo example on an actor (D4) | | |
| | D slice 5 async | tokio inside the substrate, Rust-target futures, LLVM-target coroutines, `Task.yield_now` | 10,000 idle actors | |
| | E wasm P0 to P4 | machine scalars, `Buffer`, indexed assignment, `--target wasm`, runtime in the language, `ignite run --watch`, hot reload with `migrate` | | |
| | G protocols P2 to P5 | RFC catalogues, `ignite conformance`, ABNF grammars, user `interface` and `impl`, TLS and Unix transports | | |
| | H self-hosting M5 to M7 | packages and manifests, frontend in the language, emitter in the language, bootstrap chain | | |

## Invariants that every slice keeps

- Compile time is a budget (`docs/vision.md` invariant 9): the `task gate` wall time and the program
  build times are measured before and after each slice and recorded in `AGENTS.md`; a rise
  over 10 % is a deviation to explain.
- Both targets agree: every feature lands on both. D2 B makes LLVM the default `--target`, in
  the CLI and in `BuildOptions::default` since 2026-11-05 (`story:llvm-default-target`, X25 in
  `docs/vision.md` section 6); the Rust target stays as the differential check.
- Each slice starts with a contract section in `docs/slice.md`, reviewed against `docs/vision.md`
  section 6 before agents are dispatched; each diagnostic code gets a fixture.
- Future designs (`docs/wasm.md`, `docs/actors.md`, `docs/protocols.md`, `docs/syntax.md`) stay
  proposals until a wave adopts them (T2).

## Decisions taken

D1 to D9 of `docs/vision.md` section 8, taken 2026-11-01: D1 A operations first (Timo); D2 B LLVM
primary (LLVM the default `--target`, the Rust target the test oracle; by the steering session,
reversible, carried out 2026-11-05: X25); D3 B self-hosting after F, B and C; D4 A todo example moves to an actor
in C; D5 A pure evaluator at slice 11; D6 A components as record types at slice 10; D7 A
canonical model at slice 11; D8 all three async consumers; D9 B first self-hosted backend.
Slice-level decisions are in `docs/slice.md` (section 14.13 and 14.14 for slice 10).
