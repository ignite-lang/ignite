# Plan

The roadmap in outline. The reasoning and the dependency analysis are in `docs/vision.md`
(section 4 waves, section 4.1 slices, section 8 decisions); the contract of each slice is
`docs/slice.md`. Decided on 2026-11-01: operations first (D1 option A).

## Now

| step | state |
|---|---|
| slice 10, F1 system declaration | in progress: frontend, Rust target and LLVM target accepted; std modules, examples in component form, design-file edits and the 14.0 acceptance running |
| rename the project to Ignite: strings, paths, crate names, file extensions, then a full gate | after slice 10's gate |
| `README.md` for humans with a logo | after the rename |
| one commit of everything, push to the GitHub organisation `ignite-lang` | after the README |

## Slices

| slice | wave | content | acceptance | state |
|---|---|---|---|---|
| 1 to 8 | language, two native targets, protocols, inline tests | lexer to LLVM target, `std/`, Telnet and HTTP in the language, `test` blocks | `task gate` | done, commit `44334b0` |
| 9 | A0 soundness, A friction rows 1 to 17 | mutability through bindings (E006), ambient names (E056), unused type parameters (E057), LLVM arena fixes M1 to M5, audit probes as tests | gate green on both targets | done 2026-11-01 |
| 9b | invariant 9 | one test executable per target, one integration-test binary per crate | gate cold 129.9 s to 86.5 s, warm 35.4 s to 25.9 s | done 2026-11-01 |
| 9c | LLVM parity, T6 | overflow is a failure on both targets, debug metadata, pinned triple, `-O0` dev | identical overflow failure on both targets, `gdb` shows source lines | done 2026-11-01 |
| 10 | F1 system declaration | quantity literals, `resource`, `component name: Type { … }`, `contract`, `policy`, `assembly`, `module("…")`, `Map<K, V>`, named arguments, std types for contracts, policies, probes, deployment and CLI; T1 a failure ends its task, with file, line, column and operation on both targets; substrate items R2 to R5 | `ignite check examples/terminal/terminal.design.ignite` exit 0; both examples in component form with unchanged behaviour; `terminald` survives a failing handler | in progress |
| 11 | F2 plan and explain | `ignite plan` emits the versioned system model (D7, deterministic JSON), `ignite explain`, assembly bindings resolved, pure evaluator with fuel (D5) | model byte-identical across runs and golden-tested; every property value evaluated | next |
| 12 | F3 run and control | `ignite run <assembly>`: probes as protocol clients, SLI windows and contract evaluation, `Restart` with backoff, `DrainRollout`, event journal, `event("…")` consequences; signals and references get runtime behaviour | the terminal system runs under `ignite run`; a killed `terminald` restarts within the policy's backoff; a probe timeout is in the journal and the contract's standing | |
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
