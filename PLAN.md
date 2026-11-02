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
| commit and push slices 11 to 12b | done 2026-11-02: `9273349` on `origin/main`; after the commit-message rewrite of 2026-11-02 the same tree is `28dfc75` on `main`, and branch `bench/12b-checkpoint` keeps `9273349` |
| slice 12c, restore the original performance budget | release tested fixes with documented gaps, 2026-11-02; performance acceptance remains open and further optimization is deferred by user; [`evidence`](docs/performance/12c/release-815cf74/README.md) |
| slice 13 contract (`docs/slice.md` section 17), reviewed before agents are dispatched | next: review the slice-13 contract; user deferred remaining 12c performance work |

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
| 13 | F4 apply | `ignite apply` with plan diff, changes, deletion, recovery of a half-applied plan from the journal | a second `apply` is a no-op; an interrupted `apply` resumes | |
| 14+ | B protocols P1 | `Pipe.pair()`, `ByteStream` bound, roles generic over the stream, in-process protocol tests | | |
| | C actors P1 to P3 | `Address`, `ReplyTo`, `Outbox`, `Actor` kind, mailboxes, `std/actor`, `std/table`, `Topic`, `Sink`, `Supervisor`, todo example on an actor (D4) | | |
| | D slice 5 async | tokio inside the substrate, Rust-target futures, LLVM-target coroutines, `Task.yield_now` | 10,000 idle actors | |
| | E wasm P0 to P4 | machine scalars, `Buffer`, indexed assignment, `--target wasm`, runtime in the language, `ignite run --watch`, hot reload with `migrate` | | |
| | G protocols P2 to P5 | RFC catalogues, `ignite conformance`, ABNF grammars, user `interface` and `impl`, TLS and Unix transports | | |
| | H self-hosting M5 to M7 | packages and manifests, frontend in the language, emitter in the language, bootstrap chain | | |

## Invariants that every slice keeps

- Compile time is a budget (`docs/vision.md` invariant 9): the `task gate` wall time and the program
  build times are measured before and after each slice and recorded in `AGENTS.md`; a rise
  over 10 % is a deviation to explain.
- Both targets agree: the Rust target stays the default (D2), every feature lands on both.
- Each slice starts with a contract section in `docs/slice.md`, reviewed against `docs/vision.md`
  section 6 before agents are dispatched; each diagnostic code gets a fixture.
- Future designs (`docs/wasm.md`, `docs/actors.md`, `docs/protocols.md`, `docs/syntax.md`) stay
  proposals until a wave adopts them (T2).

## Decisions taken

D1 to D9 of `docs/vision.md` section 8, taken 2026-11-01: D1 A operations first (Timo); D2 B two
targets, Rust default; D3 B self-hosting after F, B and C; D4 A todo example moves to an actor
in C; D5 A pure evaluator at slice 11; D6 A components as record types at slice 10; D7 A
canonical model at slice 11; D8 all three async consumers; D9 B first self-hosted backend.
Slice-level decisions are in `docs/slice.md` (section 14.13 and 14.14 for slice 10).
