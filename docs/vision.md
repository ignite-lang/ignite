# Ignite vision

Status: consolidation of the design documents, 2026-11-01. Written after reading every file
under `docs/`, `std/`, `examples/` and the crate contracts, and after `task gate` (exit 0:
53 cargo test suites, 483 passed, 0 failed, 1 ignored; 72 inline Ignite tests passed on each
target, 66 of them in `std/`).

What this document is for: one map of where Ignite goes, built from `design.md` (product
requirements, long-term semantics), `slice.md` and `llvm.md` (contracts of what is built),
`actors.md`, `protocols.md`, `wasm.md` (future designs) and `research/` (dated comparisons).
It states the ordering, the shared invariants, the contradictions found and the decisions
that are open. Precedence: `slice.md` and `llvm.md` describe current behaviour; this document
describes sequencing and resolves contradictions between documents; `design.md` keeps the
long-term intent until section 7 or 8 says otherwise.

## 1 What Ignite is

A typed language with capability-rooted effects. Protocols, applications and, later, runtime
logic are written in the language itself over one small IO substrate. Systems are declared
(resources, components; later contracts, policies, assemblies). The compiler produces native
executables today (Rust target, LLVM target), WebAssembly components with hot reload later, and
later still an operations controller that keeps declared objectives. Self-hosting is the end
state.

Ignite is also a replacement for Rust: the project exists because of rustc's compile times (Timo,
2026-11-04). Build time is a product requirement beside the operational goals, not only the gate
budget of invariant 9, and the LLVM target is the fast-build path (`todod` 0.309 s against
3.998 s on the Rust target, dev, `AGENTS.md` slice 12b rows). The external review of 2026-11-04
and what follows from it: `research/review-2026-11-04.md`; the second review, on agentic delivery and ESS: `research/review-2026-11-04-ecosystem.md`.

## 2 Where it stands

| layer | state | evidence |
|---|---|---|
| language core | records, enums, aliases, tuples, generics without bounds, exhaustive `match`, declared and inferred effects, effectful fn types, UFCS, field defaults, interpolation, inline `test` and `assert`; 29 keywords | `slice.md` §2 to §5, §10 to §12; `crates/ignite-syntax/src/lexer.rs` keyword table |
| standard library | `std/console`, `std/text`, `std/json`, `std/protocol/telnet` (602 lines, tests from line 148), `std/protocol/http` (678 lines, tests from line 138) | `wc -l`, `grep -n '^test'` |
| systems | `system` with `TcpListener` and `Store<T>` resources, `Service` and `Client` components, one executable per component | `slice.md` §6.1, §11.4 |
| targets | Rust (default, cargo) and LLVM (IR text, clang, `libignite_rt.a`, arenas with loop regions) | `llvm.md` §1, §4 |
| substrate | `ignite-io`: blocking `std::net`, one OS thread per task, write coalescing, `Store`, test runner; `ignite-rt`: C ABI adapter, arenas, handle table | `slice.md` §7, `llvm.md` §5 |
| CLI | `ignite check`, `emit`, `build`, `test` | `crates/ignite/src/main.rs` |
| examples | `examples/terminal` (`terminald`, `tn`), `examples/todo` (`todod`); `terminal.design.ignite` does not parse (E001 at `4096 bytes`, line 23) | `target/debug/ignite check` |
| size | Rust 25,731 lines in `src/`, 13,923 in `tests/`; Ignite 2,696 lines; docs 5,603 lines | `find … \| xargs wc -l` |
| version control | 0 commits; every file untracked; `.idea/` not in `.gitignore` | `git log`, `git status` |
| soundness | the gate passes, and section 10 lists 5 verified miscompilations or compiler panics on the LLVM target, 5 verified effect-laundering classes in the checker, 2 missing checker rules (mutability, unused type parameter) and 2 substrate behaviours that differ from `slice.md` §7 | `docs/audit/2026-11-01/` |

Not started: everything in design §4 (contracts), §5 (assemblies, planning), §7 (durable
effects, journal, leases), §10.1 beyond the four commands, §11.3 (contracts, policies, probes,
assembly), §12 (self-hosting); async (slice 5); wasm; actors; the protocol stack of
`protocols.md`.

The design's P0 is the operations layer (PRD02 to PRD04, PRD09). The implementation delivered
PRD01 in part, PRD05 in part and PRD07/PRD08 (native, P1) first. Section 8, D1, asks whether
that inversion is the intended direction.

## 3 Target architecture

One table for the layers every document agrees on. "Home" says which language it is written in.

| layer | contents | home | source |
|---|---|---|---|
| L0 substrate | sockets, TLS engine (rustls), stdio, terminal mode, tasks, timers, bounded mailboxes and one-shot slots, `Store`, `Buffer`, process entry and exit; tokio event loop in slice 5 | Rust: `crates/ignite-io`, C ABI in `crates/ignite-rt` | `AGENTS.md`; `slice.md` §7; `actors.md` §8.4; `wasm.md` §4.2, G2 |
| L1 language | values with value semantics, capability effects, generics; later bounds and interfaces, closures, `Map`, machine scalars, indexed assignment, rest patterns, ranges | Rust compiler until M6, then Ignite | `slice.md`; `protocols.md` §2; `wasm.md` §4.3; `friction.md` |
| L2 std | text, json; `transport/` (Pipe, Tcp, Tls); `protocol/` (Telnet, Http, later WebSocket, Http3); `actor/` (Step, Topic, Supervisor, Table); `rfc/` catalogues; `check/explore`; `wasm/` decoder, validator, interpreter | Ignite | `protocols.md` §4; `actors.md` §8.4; `wasm.md` §4.1 |
| L3 systems | `resource` kinds (TcpListener, Store, Topic, Listener with a transport), component kinds (Service, Client, Actor, Supervisor, application components), contracts, policies, assemblies | Ignite declarations, library kinds | design §3.2, §11.3; `actors.md` §2; `protocols.md` §2.3 |
| L4 operations | System IR, Plan IR, Controller IR; journal; probes; `ignite run`, `plan`, `apply`, `inspect`, `explain` | Ignite reducers (`actors.md` R8) over the substrate | design §2, §5 to §7, §10.1 |
| L5 targets | Rust, LLVM, wasm (+ `ignite-wasmrt` in Ignite), self-hosted compiler with retained seed | Rust emitters now; Ignite emitter from wasm P5 | `llvm.md`; `wasm.md` §2; design §12 |

Invariants shared by every document:

1. One substrate, implemented once. A target differs only in how it suspends a task.
2. Everything above IO is Ignite in `std/` or the program. Nothing named after an application
   protocol lives under `crates/`.
3. Value semantics everywhere. The only shared things are handle types: `TcpStream`,
   `PipeStream` (slice 14), `TcpListener`, `Store<T>`, `Buffer` (wasm G2), `Address<M>` (actors). Copying a handle copies
   8 bytes and shares the resource.
4. Effects are capabilities on parameters. Authority is passed, never ambient. A component's
   imports (wasm) and an actor's reach (actors) derive from the same effect sets.
5. No silent loss: bounded queues, `insufficient_evidence`, `Result` for every failure, a
   declared `Drop` where dropping is wanted.
6. Domain nouns are library symbols. Keyword budget 30 to 50; 29 today; `protocols.md` adds
   `interface`, `impl`, `grammar`; design §3.2 adds `contract`, `policy`, `assembly`: 35.
7. Every language feature lands on both native targets with a golden and a fixture, until D2
   says otherwise.
8. Behaviour is tested inline with `test`; every diagnostic code has a fixture.
9. Compile time is a budget, measured per slice (Timo, 2026-11-01: a main goal is compile
   time kept to a minimum compared to Rust, with Rust-like safety and good error reporting,
   serving agentic development). Tests of a whole tree build into one test executable per
   target by default; the compiler's own cargo tests use one integration-test binary per
   crate. Baseline 2026-11-01, 20 cores, sccache forced to recompile: todo example cold on
   the Rust target 3.0 s dev and 3.9 s release (8 crates); on the LLVM target 0.8 s dev and
   0.9 s release for emit, clang and link, plus `libignite_rt.a` 3.0 s dev once. The compiler
   workspace builds 49 test executables (699 MB) in 7.4 s with sccache hits and relinks them
   in 4.2 s after one edit in `ignite-check`; `ignite test` builds 3 executables per target.

Two rules that this document generalises from three separate statements:

- **Names in a `system` are values.** Resource names (`slice.md` §11.4), actor names
  (`actors.md` §2.2) and application components (design §11.3, `wasm.md` §3.1) are all values
  inside component property expressions, typed by their kind.
- **A failure ends its task and is reported.** `actors.md` C1, `wasm.md` Q1 and `slice.md`
  §12.5 (`assert` outside tests) all touch this; section 7, T1, states the rule.

## 4 Sequencing

The documents number their phases independently (design M0 to M8, actors P1 to P7, protocols
P1 to P7, wasm P0 to P6, slices 1 to 8 with slice 5 still open). One ordering, with the
dependencies that force it. Sizes are the documents' own: `actors.md` and `protocols.md` use S
(about a third of slice 7), M (about slice 7) and L (about two slices); `wasm.md` gives line
counts. Sizes marked "estimate" are this document's.

| wave | content | size | needs | unblocks |
|---|---|---|---|---|
| A0 soundness fixes | the audit findings of section 10: E006 for in-place mutation through `let`, `for` and pattern bindings, `to_bytes()` not a mutable receiver (M1 to M3); index and `len` on `if`/`match` receivers (M4, M5); ambient capability names (E1 to E3); roots of `map` (E4) and of `push`/`append` on field receivers (E5); unused type parameters (R1); regression tests from `docs/audit/2026-11-01/` | S, estimate | none | everything: both targets must agree before more features land |
| A language gaps (done: slices 9, 10 and 16) | closures (`friction` 9, 13; actors P4; slice 9, calls of fn-typed expressions slice 16); `Option.unwrap_or` and `!=` on Option (7; slice 9); rest patterns `..` (protocols §5.2; slice 16); ranges in patterns (8; slice 9); raw strings (2; slice 9); tuple field access (10; slice 9); `break` as expression (4; slice 9); recursive enums by boxing (15; slice 9); `Map<K, V>` (design §3.3; slice 10); last-use moves (actors R1; slice 16); T1 failure rule (section 7; slice 10). Where each holds: `docs/slice.md` 20.1 | M, estimate | A0 | B, C, E, H |
| B protocols P1 | `Pipe.pair()` and `PipeStream` in the substrate; `ByteStream` bound with one compiler-known interface; `Telnet` and `Http` roles generic over the stream with a `Limits` record; in-process protocol tests | S | slice 8 (done) | C's tests, G, TLS |
| C actors P1 to P3 | `Address`, `ReplyTo`, `Outbox`; `Actor` kind; mailboxes and dispatch in the substrate; `std/actor`, `std/table`; `Topic`, `Sink`, `Supervisor`, monitors; exit status 5; todo example on an actor (D4) | M + S + L | A (last-use moves), T1 | events for the ESS bridge (`research/ess.md` option c), F |
| D slice 5 async | tokio inside `ignite-io`; Rust-target futures; LLVM-target coroutines; `Task.yield_now` (wasm G6) | L | B, C stable | E (host suspension), actors P5, 10,000 idle actors |
| E wasm P0 to P4 | machine scalars, `Buffer`, indexed assignment, `switch` lowering (wasm P0); `--target wasm` through `ignite-llvm` and a Wasmtime scaffold (P1); runtime in Ignite (P2); `ignite run --watch`, quiescent swap (P3); stored encoding, `migrate`, E080, E081, rollback, drain swap (P4). Prerequisite: the system surface moves to design §11.3's form (application as a component, D6) | P0 2 to 3k Rust; P1 2 to 3k Rust; P2 7 to 10k Ignite; P3 1.5k Ignite and 1k Rust; P4 1.5 to 2k Ignite and 0.5k Rust | A, D | hot reload, actors P6, P7 |
| F operations layer (design P0) | quantity literals, `contract`, `policy`, `assembly`, probes as protocol clients, controller reducers as actors, journal, `ignite run`, `plan`, `apply`, `explain`; canonical IR (D7) | not estimated; larger than any single phase above | C (reducers, supervision), B (probes), D | PRD02 to PRD04, PRD09; `terminal.design.ignite` compiles |
| G protocols P2 to P5 | RFC catalogues and `ignite conformance`; ABNF `grammar` and `matches`; user `interface` and `impl`, TLS and Unix transports; `std.check.explore`, Q-method negotiation | M + L + L + M | B, A (bounds need the generics work) | RFC-backed claims, HTTP/1.1 without MUST gaps |
| H self-hosting M5 to M7 | multi-module programs and manifests (wasm G5, design §9.2); frontend in Ignite; wasm emitter in Ignite (wasm P5) or LLVM IR text emitter; bootstrap chain S0, C1, C2, C3 | not estimated | A, E (emitter), a pure evaluator (D5) | PRD10, PRD11 |

Notes on the order:

- A is first because every later wave names an item in it, and each item is small.
- B before C: actors' acceptance tests drive protocols over sockets today; a pipe makes them
  in-process and deterministic, and B is the smallest wave.
- D is placed after B and C. `actors.md` §8.1 says threads suffice for tens of actors; protocols
  P1 does not need an event loop. `wasm.md` §7 needs D for suspension in the host, so D precedes
  E. D8 asks whether Timo wants D earlier, as declared on 2026-11-01.
- F after C because the design's controller machines (design §11.8) have the reducer shape of
  actors (`actors.md` R8) and probes are protocol clients. Building F before C would build a
  second reducer runtime. D1 asks whether F should move up anyway.
- H last, and its first backend port is one decision (D9).

### 4.1 Trajectory decided on 2026-11-01

Timo chose D1 option A, operations first ("A -> operations first!", 2026-11-01). The table
above keeps the dependency analysis; the order below is the one the implementing session
follows. Wave F runs on threads. Its controller is written as reducers in the shape of
`actors.md` R8, so that wave C later replaces the runtime under them, not the reducers. In
slice 12 the reducers are stepped by the evaluator inside `ignite run` (`docs/slice.md`
section 16, O27; amended 2026-11-02 from "the Rust target") and compiled on both targets by
the reducer trace fixture, so one engine plans, runs and replays. Each slice starts with a contract section in `docs/slice.md`, reviewed by the
steering session against section 6 before agents are dispatched.

| slice | wave | content | acceptance |
|---|---|---|---|
| 9 (done 2026-11-01, gate 520 cargo tests, 78 inline tests per target) | A0 + A rows 1 to 17 of `docs/friction.md` | the friction fixes as decided by the implementing session, plus every fix of section 10.1, 10.2 and R1 of 10.3, with the probes of `docs/audit/2026-11-01/` wired into `task test` | `task gate` green on both targets; every probe behaves as its README's "expected after the fix" column says |
| 9b (done 2026-11-01, gate 524 cargo tests with 9c, 1 test executable per target, 8 integration-test binaries; times in `AGENTS.md`) | invariant 9 | `ignite test` builds one executable per target for all files given, programs namespaced by package; the compiler's 39 integration-test files become one `tests/main.rs` per crate; `task gate` wall time before and after recorded in `AGENTS.md` | `task test-ignite` links 1 executable per target; `cargo test --workspace --no-run` yields at most one integration binary per crate; test counts unchanged |
| 9c (done 2026-11-01; live gate with 9b: 680 tests, 59 s wall; dev `-O0` 0.7 s vs `-O1` 3.0 s for `test-ignite` on LLVM; `todod` +0.7 % with `-g`) | LLVM parity, T6 | `Int` overflow is a failure on both targets in every profile (LLVM `with.overflow` intrinsics; `overflow-checks = true` in the generated crate's release profile); debug metadata in the IR with Ignite source files and lines (`DIFile`, `DISubprogram`, `!dbg`), clang `-g`; `target triple` and `datalayout` pinned in the IR and a `--triple` flag passed to clang; dev optimisation level chosen by measurement (`-O0` unless the inline test suite runs more than twice as slow as at `-O1`) | an overflowing `test` fails identically on both targets; `gdb` and `perf` show `todo.ignite` lines for the LLVM binary; emitted IR names the triple; compile and test times before and after in `llvm.md` |
| 10 (done 2026-11-02, commit `60aef38`: 552 cargo tests, 97 inline tests per target) | F1 system declaration | quantity literals with units (`4096 bytes`, `16 KiB`, `15 minutes`, `100 ms`, `99.9%`), `resource`, `component name: Type { … }` (D6), `contract`, `assembly`, `module("…")` references; `std/contract`, `std/policy`, `std/probe`, `std/deploy`, `std/cli` as Ignite types; T1 failure rule, with every failure report (assert, `?`, overflow, index) naming the Ignite file, line and operation on both targets (9c left overflow with rustc's panic text and no position); R2 to R5 | `ignite check examples/terminal/terminal.design.ignite` exit 0; `examples/terminal/terminal.ignite` and `examples/todo/todo.ignite` in the component form, `ignite build` output unchanged in behaviour |
| 11 (done 2026-11-02, uncommitted: 580 cargo tests, 102 inline tests per target; gate cold 54.5 s, warm 16.8 s; `todod` LLVM release 0.80 s; 11b the same day: four target bugs fixed, 582 cargo tests, warm test step 10.8 s at load 6 to 9) | F2 plan and explain | `ignite plan` emits the versioned system model (D7, JSON, deterministic per design §6.3); `ignite explain`; assembly bindings resolved; the pure evaluator with fuel (D5) for property expressions such as `exponential(100 ms, 10 seconds)` and `rolling(30 days)` | the model for `terminal.design.ignite` is byte-identical across two runs and golden-tested; every property value in it is evaluated, none is source text |
| 12 (done 2026-11-02, uncommitted, `docs/slice.md` section 16: 636 cargo tests, 148 inline tests per target; gate time rose over 10 % on 7 units, measured causes std 3,033 to 5,135 lines loaded per spawn and `ignite-std` 2,191 to 3,792 lines, carried into 12b: reachable-only std loading and emission) | F3 run and control | `ignite run <assembly>` and `ignite replay` over its journal: probes as protocol clients (`TelnetPrompt` over the telnet `connect` role), SLI windows and contract evaluation (design §4.1), `Restart` with backoff and `DrainRollout` (design §4.2, §7.4), event journal, `event("…")` consequences | the terminal system runs under `ignite run`; killing `terminald` restarts it within the policy's backoff; a probe timeout is visible in the journal and in the contract's standing |
| 13 | F4 apply | `ignite apply` with plan diff, changes and deletion (design §7.3), recovery of a half-applied plan from the journal | a second `apply` of the same plan is a no-op; an interrupted `apply` resumes |
| 14 | B protocols P1 | `Pipe.pair()` and `PipeStream`, the bound `<S: ByteStream>` on fns and roles, `Telnet` and `Http` generic over the stream with their limits records (`docs/slice.md` section 18) | the 18.0 matrix of `docs/slice.md` |
| 16 (done 2026-11-05, `docs/slice.md` section 20; counts and gate time in `AGENTS.md` "Slice 16") | A language gaps | the items of wave A that slices 9 and 10 had not delivered: last-use moves (one analysis in `ignite-types`, read by both targets; actors R1), calls of fn-typed expressions (`adder(1)(2)`, `fs[i](x)`), rest patterns in list patterns (`docs/syntax.md` P9, E076); a both-target fixture for every item that already held | the 20.0 matrix of `docs/slice.md` |
| 15+ | slice 15 is G's P2 (`docs/slice.md` section 19); after it B (P2 on), C, D, E, G (P3 to P5), H | slice 15: RFC catalogues as literal data, the claim clauses and E090 to E092, `ignite rfc import` and `ignite rfc check`, `ignite conformance`, the claims of `Telnet` and `Http`; after it as the table above, in that order | slice 15: the 19.0 matrix of `docs/slice.md`; after it as the table above |

Consequences for the open decisions of section 8: D5 moves from the start of H to slice 11;
D6 moves from "before wasm P1" to slice 10; D7 is scheduled at slice 11; D3 reads "after F,
B and C". `docs/syntax.md` (a sugar proposal drafted on 2026-11-01) stays a proposal under T2;
a sugar item enters a slice only when that slice's declarations need it.

## 5 Language gaps by consumer

Consolidated from `friction.md`, `wasm.md` §4.3, `protocols.md` §2.2 and §5.2, `actors.md` §3.3
and §11, `research/ess.md` §3 and `research/lean.md` §3.

| gap | wanted by | wave |
|---|---|---|
| closures capturing locals | todo PATCH (`friction` 9), `map` arguments (13), actors `call(fn)` and dynamic actors (P4) | A: slice 9 (13.3); calls of fn-typed expressions slice 16 (20.4) |
| `Option.unwrap_or`, `!=` on `Option` | todo (`friction` 7) | A: slice 9 |
| rest pattern `[a, ..]`, range patterns | protocols §5.2, `digit_value` (`friction` 8) | A: range patterns slice 9; rest patterns slice 16 (20.5) |
| raw strings, `\u{..}`, interpolation holding a string literal | json fixtures (`friction` 1, 2, 12) | A: slice 9 |
| tuple field access `.0`, `break` as an expression, static fns on records | tests (`friction` 10, 4, 17) | A: slice 9 |
| recursive enums without `List` boxing (E055) | `Tree<T>`, json (`friction` 15) | A: slice 9 (13.4) |
| `Map<K, V>` with canonical order | design §3.3; a self-hosted frontend (H) | A: slice 10 (14.3) |
| last-use move of a `var` passed and reassigned | actors R1 (state through the loop) | A: slice 16 (20.3) |
| typed bounds on generics, `interface`, `impl`, associated types, `Eq` | protocols §2.2 (P1 ships one builtin bound; P4 user-declared) | B, G |
| machine scalars `I32`, `F32`, `F64`; `Buffer`; `xs[i] = e`; dense `match` to `switch` | wasm G1 to G4 | E |
| `Task.yield_now` | wasm G6 under tokio | D |
| non-std imports, packages, lockfile, language version pin | wasm G5, design §9.2, `research/ess.md` §4 | H |
| nominal newtypes with declared conversions | `research/ess.md` §4 | H or earlier if ESS import (option b) is wanted |
| quantity literals (`4096 bytes`, `200 ms`), percentages, `Duration`, `Decimal` | design §3.3, `terminal.design.ignite` | F |
| pure evaluator at check time (constants folded, pure tests at `ignite check`, exhaustion checks) | design §3.5; `research/lean.md` §3.1, §3.2 | D5 |
| inferred totality | `research/lean.md` §3.4; a proof export | after H starts |

## 6 Contradictions and stale statements

Each row names the two places, what disagrees, and the resolution this document proposes.
"Verified" means read in both places today.

| # | places | disagreement | resolution |
|---|---|---|---|
| X1 | design §8.10 "LLVM replaces the Rust-generation backend"; `llvm.md` §0 and `AGENTS.md` "The Rust target stays and stays the default" | which backend is primary | D2 |
| X2 | design §2, §8.10, M1 "an interpreter executes the specification first"; implementation has no interpreter, constants are runtime fns (`slice.md` §6) | interpreter | D5; resolved by slice 11: `ignite-eval` evaluates property expressions at check time (`docs/slice.md` §15.3); constant folding and pure tests stay open |
| X3 | design §6.2 canonical JSON "executable specification", three IRs; implementation goes AST → typed AST → Rust or LLVM IR; `research/ess.md` §3 notes the name collides with ESS | IR layer | D7; resolved by slice 11: the system model of `docs/slice.md` §15.5, `formatVersion` `ignite-model/0.1` |
| X4 | design §1.3 P0 = operations; implemented waves are language, native targets, protocols | priority order | D1 |
| X5 | `slice.md` §7 "a task that panics ends its thread" (module comment of `crates/ignite-io/src/task.rs`); `llvm.md` §4 `ignite_rt_panic` exits the process with 101 (`PANIC_EXIT_CODE` in `crates/ignite-rt/src/process.rs`); `actors.md` C1 and `wasm.md` Q1 ask for one rule | task failure semantics | T1 (section 7) |
| X6 | `actors.md` proposes E050 to E054; `slice.md` §12.8 took E050 to E053 for tests and E054 is the nesting limit; `protocols.md` §3.3 noticed E054 only | diagnostic codes | actors renumber to E060 to E064 (S1) |
| X7 | `actors.md` §10 and `wasm.md` §9 "slice 8 planned, no contract in `slice.md` yet", sketch uses `expect` and `fail` | slice 8 landed with `assert` (`slice.md` §12) | S2 |
| X8 | `actors.md` D7 moves the todo example to an actor; `protocols.md` §5.3 "`examples/todo/todo.ignite:15-81` stays as it is" with `Store<Todo>` | one example, two futures | D4 |
| X9 | design §8.6 libuv, libcurl, llhttp, NATS C client; `AGENTS.md` tokio, protocols in Ignite, rustls in `ignite-io`; `protocols.md` Q8 | substrate libraries | S3: design v0.3 |
| X10 | design §8.8 example uses `await`; slice 6 made `await` an error (`slice.md` §10.2) | effect marker | S3 |
| X11 | design §3.2 twelve forms without `protocol`, `const`, `test`; the slice has all three as keywords | declaration inventory | S3 |
| X12 | design §3.3 lists `String`; the language has `Text` | type name | S3 |
| X13 | design §11.2 "this deliverable does not include implementation files"; status line "does not claim that a compiler … is implemented" | stale status | S3 |
| X14 | `slice.md` §1 "archived in `target/archive/terminal-reference.tar.gz`"; the path does not exist and `target/` is gitignored | lost archive | S4 |
| X15 | `slice.md` title "slice 1"; the file holds slices 1 to 8 | naming | S4 |
| X16 | `slice.md` §10 acceptance "`std/protocol/telnet.ignite` at most 400 lines"; file is 602 lines including inline tests (first test item at line 148) | rule predates inline tests | S4: "excluding tests" |
| X17 | `protocols.md` §0 "`std/protocol/http.ignite` … 367 lines", "25 keywords", "grow to 29" | 678 lines, 29 keywords today | S5 |
| X18 | `protocols.md`, `actors.md`, `wasm.md` cite `telnet.ignite:114`, `:119-144`, `http.ignite:8-23` and similar | line numbers into living sources rotted within one day | S6: cite symbols |
| X19 | `AGENTS.md` layout table omits `crates/ignite-load`, `std/console.ignite`, `std/text.ignite`, `std/json.ignite`, `std/protocol/http.ignite`, `examples/todo/` | table stale | S7 |
| X20 | `research/lean.md` and `research/ess.md` say `std/protocol/http.ignite` does not exist and `ignite-io` has no `Store` | snapshots from earlier on 2026-11-01 | none; they are dated. S8 adds a note to each |
| X21 | design §11.3 `component console` + `application = console`; `slice.md` §6.1 `application = Console { … }` inline; `wasm.md` §3.1 follows the design | system surface | D6 |
| X22 | `slice.md` §12.5 `assert` outside a test exits the process with 101; `actors.md` wants a failure to end only its task | one bad request kills a server | T1 covers `assert` too |
| X23 | `llvm.md` §7 and `slice.md` §1 "a program that passes both is valid for the Rust and the LLVM target"; section 10.1 shows programs the checker accepts that rustc rejects and the LLVM target miscompiles | the checker lacks a mutability rule | wave A0: E006 for in-place mutation |
| X24 | `slice.md` §5.1 "The capability is a parameter of the function (`stream.read`) or an ambient one (`stdin.read`)"; the implementation cannot tell the two apart when a parameter carries an ambient's name | section 10.2, E1 to E3 | wave A0: refuse or namespace ambient names |
| X25 | section 8 D2 option B "LLVM becomes the default `--target`", taken 2026-11-01 by the steering session as reversible; `BuildOptions::default` in `crates/ignite/src/lib.rs` (`target: Target::Rust`) and every `--target` of `crates/ignite/src/main.rs` still default to Rust | which target is the default | resolved 2026-11-04: D2 B stands (Timo: build time is why Ignite exists, `vision.md` section 1); `BuildOptions::default` and the CLI switch to LLVM after slice 16 U4 (`story:slice-16-llvm-moves`) lands, in `story:llvm-default-target` |

Logic flaws inside one document, found by reading:

| # | document | flaw | proposed fix |
|---|---|---|---|
| L1 | `actors.md` §5.3 with §6.1 | The journal records a message before `handle`; recovery replays every journaled message. A message that crashes `handle` is replayed on every restart, so the supervisor's intensity is spent on one poison message and the actor never recovers. | Journal the crash: mark the message skipped (or snapshot after the crash without it) before the restart. |
| L2 | `actors.md` §4.4 with §6.1 | An actor whose `Outbox` sends block past `send_deadline_ms` fails as `Overloaded`; its mailbox survives; the restarted actor processes the same backlog and blocks again. | Count `Overloaded` toward intensity only after the backlog shrank, or let the supervisor drop the mailbox on `Overloaded`. Needs a decision inside C. |
| L3 | `wasm.md` §4.3 with `llvm.md` §4 | `Buffer` shares storage; the region soundness argument assumes values never share buffers. `wasm.md` answers it (storage outside arenas, 8-byte handle). | Promote the answer to invariant 3 (handle types) so it is stated once. Done above. |
| L4 | `slice.md` §5.1 "Roots" | A stream returned by `accept` on parameter `l` needs no effect of its own: `requires [l.accept]` grants every operation on every accepted connection. | Keep, and state it as a rule: authority over a listener is authority over its connections. |

## 7 Decisions taken in this document

T1. **A failure ends its task and is reported.** A panic, a failed `assert` or a failed `?`
outside a test ends the task that ran it, on every target. The substrate reports it: logged as
`task ended with error` today, delivered as `Down` to a supervisor once actors exist. The
process exits 101 only when the failing task is the main task or a test. The LLVM target must
contain a panic to its task (unwind or `longjmp` to the task entry, then `task_end`). Reason:
`actors.md` C1 and `wasm.md` §8 need it, design §4.3 "let it crash" assumes it, and a server
that dies on one bad request violates design §7.4. Cost: one change in `crates/ignite-rt`, one
rule in `slice.md` §7 and §12.5, one in `llvm.md` §4.

T2. **Future designs are proposals until a wave adopts them.** `actors.md`, `protocols.md` and
`wasm.md` each say "nothing here is implemented". A wave that implements a phase makes that
phase's section a contract and moves it into `slice.md` or a sibling. Nothing in them is binding
before that.

T3. **Line-number citations are banned in design documents.** Cite a symbol (`decode` in
`std/protocol/telnet.ignite`) or a section. X18 shows the rot.

T4. **Diagnostic code ranges are reserved per wave** (steering, 2026-11-01). E001 to E059:
slices 1 to 9 (`slice.md`). E060 to E069: actors. E070 to E079: the sugar proposal
(`syntax.md`). E080 to E089: wasm. E090 to E099: protocols. E100 to E109: slice 10 (F1),
E110 to E119: slice 11 (F2), E120 to E129: slice 12 (F3), E130 to E139: slice 13 (F4). A
document that cites a code from another range cites it, it does not define it. Reason: on
2026-11-01 four documents used codes E080 to E098 with no written range table; the check that
day found `wasm.md` defining E080 to E083 and `protocols.md` E090 to E098, the others citing
them, so the table records the practice and the slice 10 to 13 ranges extend it.

T5. **Declaration syntax for slice 10** (steering, 2026-11-01, answers `syntax.md` Q4 and
Q5). (a) Named arguments without defaults: `f(a, b, name = e)`, positional first, only on fns
declared in Ignite with a parameter of that name, not through fn values or roles; defaults
stay in `syntax.md` P10. (b) A brace literal `{ k = e, … }` without a type name takes its
type from the expected type: a record type makes it a record literal, `Map<K, V>` makes it a
map literal whose keys are identifiers (`K = Text`) or component references (`K` the reference
type), so `sessions = { maximum = 1000 }`, `commands = { connect = Command { … } }` and
`bindings = { Terminal.ingress = localSocket() }` are one rule; a brace literal with no
expected type is a diagnostic in the E100 range. (c) `Map<K, V>` (design §3.3) arrives in
slice 10 with `get`, `put`, `remove`, `keys`, `len` and iteration in key order; `syntax.md`
P11's `["k": v]` stays unscheduled. Reason: the design example is the acceptance, and one
expected-type rule covers all three uses in it.

T6. **`Int` arithmetic overflow is a failure on every target and in every profile** (steering,
2026-11-01, after Timo asked to address the LLVM target's disadvantages). It ends the task
like a failed `assert` (T1). The Rust target sets `overflow-checks = true` in the generated
crate's release profile; the LLVM target emits the `with.overflow` intrinsics. Reason: a
program must not change meaning between targets or between dev and release; wrapping silently
is the silent loss invariant 5 forbids.

T7. **A general layer knows no application protocol** (Timo, 2026-11-02). std has four
layers, base, leaf (O45), general and `std.protocol.*`; a module imports only its own layer
or a lower one, and a kind that speaks one protocol lives in that protocol's module.
`crates/*/src` names no protocol (`crates/ignite-load` exempt). Table, audit and open items:
`docs/layering.md`; the test `layering::` in `crates/ignite-check` enforces it. Reason: slice
10 put the Telnet probe `TelnetPrompt` in the general module `std.probe`, which imported
`std.protocol.telnet`; it now lives in `std.protocol.telnet` and `std/probe.ignite` is gone.
The slice-10 row above and the slice records keep the old path as history.

## 8 Open decisions

Each needs Timo's answer; each changes what the next wave builds. The default is the
recommended option.

Taken on 2026-11-01. D1: A, by Timo. D2 B, D3 B (read "after F, B and C"), D4 A, D5 A (at
slice 11, see 4.1), D6 A (at slice 10), D7 A (at slice 11), D8 all three, D9 B: by the
steering session, as the recommended defaults, reversible on Timo's word. The options stay
below for the record.

**D1. Direction: operations first or language first.** Design §1.3 puts contracts, plans and
recovery at P0. The implementation built the language and two native targets first.

| option | what it does | cost | recommended |
|---|---|---|---|
| A operations first | Wave F next: quantities, contracts, policies, assembly, probes, a controller over threads, `ignite run`/`plan`/`apply` | Builds a reducer runtime and a probe client before actors and the protocol stack exist, then rebuilds both on them | |
| B language and runtime first | Waves A to E, then F | PRD02 to PRD04 stay unimplemented longest | yes: F reuses C (reducers, supervision), B (probes as clients) and D |
| C drop the operations layer | Ignite stays a systems language with protocols and actors; design §4 to §7 become a separate product | Loses the intent claim of design §1 | |

**D2. Two native targets.** Every feature is implemented twice (friction 16 is one cost; slice
5 would be futures and coroutines).

| option | what it does | cost | recommended |
|---|---|---|---|
| A peers for ever | status quo | double work per feature, for ever | |
| B LLVM primary from slice 5 | LLVM becomes the default `--target`; the Rust target stays the test oracle and may lag; wasm and self-hosting build on LLVM | slice 5 on the LLVM target needs coroutine lowering with no Rust fallback | yes |
| C Rust primary | drop LLVM coroutines; wasm through rustc's wasm32 target | contradicts `wasm.md` §2.1 (reuse `ignite-llvm`) and design §8.1; self-hosting then emits Rust and needs cargo | |

**D3. Slice 5 timing.** Declared on 2026-11-01 as the next substrate step.

| option | what it does | cost | recommended |
|---|---|---|---|
| A before B and C | async substrate now; actors and pipe land on tokio | the largest wave first; B and C wait | |
| B after B and C | threads carry protocols P1 and actors P1 to P3; tokio before wasm | actors P5 and 10,000 idle actors wait | yes |
| C never | threads only | blocks `wasm.md` §7 and design §8.7 | |

**D4. The todo example.**

| option | what it does | cost | recommended |
|---|---|---|---|
| A actor | `examples/todo` moves to a `Todos` actor at actors P1 (`actors.md` §9.2); `protocols.md` §5.3 amended; `Store<Todo>` survives in `test` blocks and in wasm's reload tests | one example changes shape once | yes |
| B keep Store | todo stays on `Store`; actors get a new example (a chat room over Telnet) | a third example to maintain; the ESS bridge (option c) gets no events from todo | |
| C both | two todo programs | duplicate maintenance | |

**D5. A pure evaluator.** Design §3.5 requires bounded compile-time evaluation; nothing
implements it; `research/lean.md` §3.1 and §3.2 show what it buys.

| option | what it does | cost | recommended |
|---|---|---|---|
| A build it in wave A or H | an interpreter over the typed AST with fuel: constants folded, pure `test` blocks run at `ignite check`, exhaustion over finite types | M; a third execution path to keep equivalent | yes, at the start of H (the self-hosted frontend needs it for `const` anyway) |
| B never | constants stay runtime fns; tests run only under `ignite test` | design §3.5 is dropped | |

**D6. System surface.** Design §11.3 declares the application as a component; the slice
inlines it as a value. `wasm.md` §3.1 needs the component form.

| option | what it does | cost | recommended |
|---|---|---|---|
| A move to the design's form before wasm P1 | `component console: Console { … }`, `application = console`; component names are values (section 3) | a syntax change in both examples and the goldens | yes |
| B keep inline, derive the cut | the wasm target cuts at the fn-typed fields of the inline value | the cut is implicit; actors still need named components | |

**D7. The canonical IR.** Design §6.2's JSON "executable specification" and the three IRs
exist nowhere; wasm Q3 asks what the Code IR is; ESS uses the name.

| option | what it does | cost | recommended |
|---|---|---|---|
| A keep, rename, schedule with F | "system model" (JSON, versioned) emitted by `ignite plan`; Code IR is wasm P5's serialized typed AST | nothing until F | yes |
| B drop | the typed AST is the IR; no public JSON | design §6.2 and PRD03 go | |

**D8. Accept the three future designs as direction.** Multi-select.

| option | what it does | cost |
|---|---|---|
| A `protocols.md` | waves B and G as written | B is S; G is three L phases |
| B `actors.md` | wave C as written, with L1 and L2 fixed first | M + L |
| C `wasm.md` | wave E as written | 5.5 to 7.5k Rust and 10 to 13.5k Ignite over P0 to P4 (sum of `wasm.md` §10) |

Recommended: all three, with the fixes in section 6.

**D9. First self-hosted backend.** `wasm.md` §2.1 makes a wasm emitter in Ignite the first
backend piece; design §8.5 and §12.4 make LLVM IR text the backend.

| option | what it does | cost | recommended |
|---|---|---|---|
| A wasm emitter first | binary encoding plus component wrapping in Ignite; compiles inside the runtime | larger; serves the dev loop | |
| B LLVM IR text first | text emission; the bootstrap chain of design §12.5 as written | needs clang in the loop; no on-the-fly compile | yes for H; A stays wasm P5's own deliverable |

## 9 Suggested edits to other documents

Not applied by this document. Each is one commit.

| # | file | edit |
|---|---|---|
| S1 | `docs/actors.md` | renumber E050 to E054 → E060 to E064; add L1 and L2 handling; §10 on slice 8's `assert` |
| S2 | `docs/wasm.md` | §4.3 row G6 and §9 row 1: slice 8 landed; cite `slice.md` §12 |
| S3 | `design.md` | version 0.3: status line and §11.2 (implementation exists); §3.2 inventory adds `protocol`, `const`, `test`; §3.3 `String` → `Text`; §8.1 and §8.10 both targets; §8.6 tokio, protocols in Ignite, rustls; §8.8 no `await` |
| S4 | `docs/slice.md` | title "slices 1 to 8"; drop the archive sentence or restore the archive under `docs/archive/`; §10 acceptance "excluding tests" |
| S5 | `docs/protocols.md` | §0 state line (678 lines), §2.2 and Q5 keyword counts (29 today) |
| S6 | all design docs | replace line citations by symbol citations (T3) |
| S7 | `AGENTS.md` | layout rows for `crates/ignite-load`, `std/console.ignite`, `std/text.ignite`, `std/json.ignite`, `std/protocol/http.ignite`, `examples/todo/`, `vision.md` |
| S8 | `docs/research/*.md` | one line each: "snapshot; `std/protocol/http.ignite` and `Store` exist since later on 2026-11-01" |
| S9 | `.gitignore` | add `.idea/` |
| S10 | repository | first commit |
| S11 | `docs/slice.md` | split into a language reference (lexical, grammar, names, effects, diagnostics), a Rust-target contract, a substrate contract, and archived per-slice contracts with their placeholders; `llvm.md` keeps the LLVM target |

## 10 Soundness audit

Three read-only audits ran on 2026-11-01 against the code, with reproductions in the session's
scratch directory (logs and scripts copied to `docs/performance/evidence/telos/telos-steer-1f73fa06/`). Their results are in section 10.1 to 10.3 once complete; findings that
change a contract are added to section 6.

The probe programs are kept under `docs/audit/2026-11-01/` (one directory per audit); every
probe passes `ignite check`. "Verified" rows were reproduced twice: by the audit and again by
the author of this document.

### 10.1 LLVM arenas and loop regions

Four verified unsound cases, one shared cause: the mutation analysis of `llvm.md` §3 and §4
works by variable name, while three constructs hand out a pointer into another variable's
memory under a different name or no name. The Rust target gives the right answer in every
case (rustc rejects two of the programs with E0596, which makes the third row below a checker
gap as well).

| # | claim | probe | LLVM target | Rust target |
|---|---|---|---|---|
| M1 | `x.to_bytes()` returns the receiver's own pointer; `x.to_bytes().push(b)` mutates `x` although `x` counts as unmodified, so a borrowed parameter is changed under the caller and in a loop `x` is not carried and its regrown buffer is released and reused. Fix (slice 9): E006 on the direct form `x.to_bytes().push(b)` (`slice.md` §13.11 a), plus a copy on the alias `var b = x.to_bytes(); b.push(..)`, tested by the three `var to_bytes alias` tests of `crates/ignite/tests/fixtures/audit/aliases.ignite` (`var_aliases_leave_the_source_unchanged` in `crates/ignite/tests/slice9.rs`, both targets). M1 counts as fixed only while those tests show `x` unchanged on the LLVM target | `llvm/to_bytes_push.ignite` | 2 tests fail: `s is abc! (len 4)`, `prefix is ... len 20003` | 2 pass |
| M2 | `for w in words { w.push(..) }`: the element binding is never carried; its regrown buffer is released at the back-edge while `words[i]` still points at it | `llvm/for_push.ignite`, `llvm/for_push2.ignite` | `words[0] is .....`; one run panics in `copy_nonoverlapping` | rustc E0596 |
| M3 | `match o { Some(b) => b.push(..) }` in a loop: the arm mutates the alias `b` of `o`'s payload; `o` is neither copied nor carried | `llvm/match_push2.ignite` | `payload is ...... len 6` | rustc E0596 |
| M4 | `(if c { xs } else { ys })[0]`: the receiver of an index is lowered without a value, the index is never emitted and the binding is read uninitialised | `llvm/index_if_only.ignite` | `y is  len 0`; IR has no `check_index` | passes |
| M5 | `(match o { .. }).len()` panics the compiler (`expected an aggregate, got Unit` in `lower.rs`) | `llvm/len_match.ignite` | compiler panic | passes |

Checked and found correct (verified with `llvm/sound_ok.ignite` and `llvm/later_arg.ignite`):
a callee returning an alias of a borrowed argument is deep-copied into the result slot; `break`
out of an inner loop with a carried variable; per-instance copy glue for generics;
`Store.update` with heavy allocation under the lock (70,000 updates); a `for` whose body
reassigns the list and a `match` whose arm mutates the scrutinee through a nested loop both
copy; a later argument that modifies the borrowed place is copied at evaluation; fn-field and
fn-pointer calls always copy. Read but not executed: the spawn handshake, the heap glue of
`Store`, the inline arena helpers, `xs.append(xs)`.

Contract consequence: `llvm.md` §7 "a program that type-checks is valid for both" does not
hold today. The checker has no rule that `push`, `append` and `clear` need a `var`-rooted
place (`llvm/let_push.ignite`: `ignite check` exit 0; the Rust build fails with three rustc
E0596 errors, exit 2). Fix in one place: E006 for an in-place mutation through a `let`, a
`for` element or a pattern binding, and `to_bytes()` refused as a receiver of a mutating
builtin; or the LLVM lowering copies such receivers. Section 4, wave A, carries it.

### 10.2 Effect system

Five verified laundering classes, all accepted by `ignite check` with exit 0. The first three
have one cause: a `requires` head is resolved as a parameter whenever a parameter of that name
exists, so a parameter named like an ambient capability (`stdin`, `stdout`, `process`, `net`)
absorbs the ambient effect and the caller maps it onto the argument's roots, which an `Int`
argument does not have.

| # | claim | probe |
|---|---|---|
| E1 | `fn helper(stdin: Int) requires [stdin.read] = Stdin.read(..)`; `fn outer() requires [] = helper(1)` is accepted; same for `process.exit`, `net.connect`, `stdout.write`, and with an inferred callee | `effects/a01_param_named_stdin.ignite`, `a02`, `a03` |
| E2 | The same with `requires [stdin.*]` on a `TcpStream` parameter named `stdin`, and inside a protocol role called from `requires []` | `effects/b08_ambient_param_variants.ignite` |
| E3 | The same through an effectful fn type or fn field `fn(stdin: Int) -> R requires [stdin.read]` | `effects/c01_fn_field_ambient_name.ignite` |
| E4 | `l.map(f)` has no roots: `streams.map(same)[0].read(4096)` and `stores.map(same)[0].get(1)` pass under `requires []`, also through a record and an inferred helper | `effects/a05_map_launder.ignite`, `a06`, `b11` |
| E5 | `push` and `append` on a field or index receiver add no roots: `h.items.push(s)` then `h.items[0].read(..)` needs only `h.read`; the Rust lowering mutates in place, so the hole is real at run time | `effects/a07_push_field.ignite`, `b10` |

Two further observations: a record literal bound without an annotation gives E042 at a later
`Store` or fn-field call (`effects/c03`), consistent with `slice.md` §11.7's list, which omits
record literals; component property expressions are not walked by the checker at all
(`effects/a21`: an effectful call inside `application = ..` is accepted), consistent with
"mains are roots" but without an E010-like rule. Closed by slice 11: a property expression must be pure, E116 (`docs/slice.md` §15.2); the probe now expects E116.

Correctly rejected (23 probes, `effects/a08` to `a22`, `b01` to `b07`): generic identity and
unboxing, shadowing and `var` reassignment across branches, swapped fn-field arguments, UFCS on
a non-receiver parameter, `Store` through generic fields and aliases, `spawn` of inferred fns,
effectful calls in constants and defaults, effectful fns into pure fn types, fn values of
unknown type, test-only items by name and by UFCS, roles calling inferred helpers.

Fixes, one line each: resolve a `requires` head as a parameter only when the fn declares it
and refuse parameter names that equal an ambient capability (or namespace ambients, e.g.
`ambient.stdin.read`); give `map` the receiver's roots plus the fn argument's; extend the
roots of the receiver's place root for `push` and `append`. Section 4, wave A, carries them.

### 10.3 Substrate and Rust lowering

One verified bug, three verified behaviours that stretch the contract wording, two inferred
risks. The Rust lowering of `slice.md` §6 held against every other probe (guarded arms at
depth, borrowed and copied parameters, fn values, defaults on generic literals, block values,
`spawn` of field arguments; `io/p4` to `p10`).

| # | severity | claim | probe or harness | evidence |
|---|---|---|---|---|
| R1 | bug | A generic record, enum or alias with an unused type parameter passes `ignite check`; rustc rejects the generated crate (E0392, E0091, E0283), so `ignite build` exits 2 with rustc output | `io/p2_unused.ignite` | verified twice: check exit 0, build exit 2 |
| R2 | contract | Dropping the last handle of a stream with unsent bytes neither sends nor closes until the task's next flush point, because the thread's pending list holds a strong reference to the stream; `slice.md` §7 lists "when the last handle of the stream is dropped" as a send trigger | `io/iotest` `drop` | verified: the peer reads nothing for 507 ms, then the bytes after `flush()` |
| R3 | contract | `write_all` after another clone's `close` returns `Ok`, fails with EPIPE at the writer's next flush, and the error is delivered to the other task's `read` on that stream; once, not lost | `io/iotest` `cross` | verified |
| R4 | inferred | `accept` retries only ECONNABORTED, EMFILE, ENFILE, ENOBUFS and ENOMEM; Linux also returns pending network errors (EPROTO, ENETDOWN, EHOSTUNREACH, ENETUNREACH, ENONET) from `accept`, which end the generated accept loop and the process | `transient` in `crates/ignite-io/src/net.rs` | code reading, no fault injection |
| R5 | inferred, low | `exit` locks and flushes stdout; a task stalled in `Stdout.write_all` on a full pipe blocks the exit; the buffer is always empty because `Stdout.write_all` flushes per call | `exit` in `crates/ignite-io/src/lib.rs` | code reading |

Checked and found correct: a panicking task still sends its pending bytes (`iotest panic`);
thread reuse starts the next task with an empty pending list (`iotest reuse`); `Store.update`
cannot be re-entered from Ignite because a fn reaching a `Store` through its argument is
effectful and E017 refuses it as `update`'s argument (`io/p7_store.ignite`); the spawn handoff
holds no lock across park and unpark; `set_idle_timeout` is a bare `setsockopt`; EINTR is
retried in `accept`, socket reads and stdin reads.

Fixes, one line each: R1, a checker diagnostic for an unused type parameter (or `PhantomData`
in the generated type); R2, a weak reference in the pending list and send-plus-close in the
stream's `Drop`; R3, a `closed` flag set by `close` and checked by `write_all`; R4, extend
`transient`; R5, delete the flush. R1 goes to wave A0; R2 to R5 are substrate items for the
slice that touches `ignite-io` next (D3 decides which).
