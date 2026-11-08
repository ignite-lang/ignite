# One substrate, two targets

Ignite compiles one checked program to native executables in two ways: through generated
Rust and cargo, or through LLVM IR and clang. Both ways call the same IO code, a Rust crate
written once, so a socket, a task or an error report behaves the same whichever target built
the program. This page explains that split, what each target does, and why both are kept.
The commands and their options are in [Building for Rust and LLVM](../guide/targets.md).

## The substrate is written once

Everything a compiled program does to the outside world goes through
[`crates/ignite-io`](https://github.com/ignite-lang/ignite/tree/main/crates/ignite-io):

| area | what `ignite-io` provides |
|---|---|
| network | TCP listener and stream, with write coalescing |
| in-process streams | `Pipe.pair()` and `PipeStream`, the test-only pipe that coalesces as TCP does |
| terminal | stdin, stdout, terminal mode |
| tasks | `spawn`, one OS thread per task, and `run`, the process entry |
| process | arguments (`--port`, `--ref`), exit, the drain on SIGTERM |
| failures | the reports of `assert`, `?`, overflow and index errors |
| tests | the runner of `ignite test` |

Two rules keep it small and shared:

- **The substrate knows no application protocol.** It knows sockets, pipes, stdio and tasks.
  Telnet, HTTP, the console and the control machines are Ignite code in `std/` or in the
  program; see [Protocols and roles](protocols.md#a-general-layer-knows-no-protocol).
- **A target differs only in how it suspends a task, not in how it does IO.** Neither target
  has IO code of its own.

In release v0.15.0 the substrate is blocking: `std::net`, one OS thread per task, no event
loop. Being built: a readiness scheduler inside `ignite-io` that runs spawned tasks as
stackful tasks on a few worker threads, one epoll loop each, with no change to either
target's code generation
([`docs/readiness-io.md`](https://github.com/ignite-lang/ignite/blob/main/docs/readiness-io.md),
[roadmap](../roadmap.md)). It replaces the earlier plan of tokio, Rust futures and LLVM
coroutines.

## The Rust target

Selected with `--target rust`. The compiler generates a Rust project and cargo builds it:

| step | output |
|---|---|
| `ignite-rustgen` | a program crate, plus an `ignite-std` crate with the std code the program reaches |
| cargo | the executables, linked against `ignite-io` as an ordinary crate |

Generated code calls `ignite-io` directly and synchronously. Protocol roles are monomorphised:
the generated std calls `Telnet::serve::<ignite_io::TcpStream>`, with no `dyn ByteStream`.
`ignite emit FILE` prints the generated Rust; it is an artifact, never edited or committed.

## The LLVM target

The default. The compiler emits LLVM IR text and clang compiles and links it. Building a
program never runs cargo or rustc.

| step | output |
|---|---|
| `ignite-llvm` | `std.ll` (the std the program reaches), `program.ll`, one `main_<exe>.ll` per executable |
| `clang -c` | object files, `-O0 -g` by default, `-O2` with `--release`, for `--triple` or clang's own target |
| clang as linker | each executable, linked with `libignite_rt.a` and `--gc-sections` |

`libignite_rt.a` is
[`crates/ignite-rt`](https://github.com/ignite-lang/ignite/tree/main/crates/ignite-rt), built
once per clone: `ignite-io` behind a C ABI. Every function is `extern "C"` with the prefix
`ignite_rt_`. The adapter converts values, keeps a table of IO handles, runs the per-task
arenas and the spawn handshake, and does no IO of its own.

The IR owns what the Rust target delegates to rustc:

- the layout of every value, and a copy function per aggregate type for value semantics;
- memory: a bump arena per task, freed at task end, with marks to release what a stretch of code allocated;
- checked arithmetic: overflow is a failure, not LLVM's wrap-around or undefined behaviour;
- debug metadata, so a debugger shows Ignite source lines.

The contract is
[`docs/llvm.md`](https://github.com/ignite-lang/ignite/blob/main/docs/llvm.md).

## The same behaviour on both

The two targets are held to one behaviour, and the tests check it rather than assume it:

- the failure reports (overflow, a failed `assert` or `?`, an index out of range) are the same
  text on both;
- `ignite test FILE --target all` runs every inline test on every target;
- the end-to-end tests run the examples' binaries from both targets over real sockets and
  real stdin and stdout;
- the fixture `eval_equivalence.ignite` gives the same `debug` text for each of its pure
  functions in the compile-time evaluator, a Rust build and an LLVM build;
- `control_trace.ignite` steps the `std.control` reducers in the evaluator and on both
  targets and compares the results.

## Why two targets

The design compares the two routes in section 8.4. Generating Rust "normally already reaches
LLVM through rustc", so the choice is about who owns the lowering, not native against
non-native code:

| concern | generated Rust | LLVM IR directly |
|---|---|---|
| first release | reuses Rust checking, libraries and tooling | needs more language and runtime machinery first |
| semantics | Ignite values and effects translated into valid Rust | Ignite layouts, calls, memory and control flow defined directly |
| safety | Rust checks, within the limits of unsafe runtime code | Ignite must enforce safety; LLVM verification is no memory-safety proof |
| async | can reuse a Rust executor | must specify suspension, scheduling and cancellation |
| dependencies | the Rust toolchain | LLVM tools, the runtime archive, a linker |

The design says: "The strongest benefits of the LLVM route are control of representation,
fewer semantic compromises through a host language, and a natural long-term backend for the
implementation subset." And: "Rust code generation remains a credible shortcut to the first
native controller." It calls faster builds, smaller binaries and higher throughput
"hypotheses to benchmark". The measured build times are in the
[targets guide](../guide/targets.md#build-times-as-measured).

## Which target is the default

LLVM. `--target llvm` is the default of every command that builds (`build`, `test`, `run`,
`plan --for-apply`, `conformance`; their `--help` says `[default: llvm]`), and `--target rust`
selects the Rust target. Both targets stay, and every feature lands on both. The sources:

- [`docs/vision.md`](https://github.com/ignite-lang/ignite/blob/main/docs/vision.md),
  decision D2 in section 8, taken with option B, "LLVM primary": LLVM becomes the default
  `--target` and the Rust target stays the test oracle. Contradiction X25 recorded that the
  code still defaulted to Rust; it was resolved on 2026-11-04 (build time is why Ignite exists,
  section 1) and carried out on 2026-11-05.
- [`docs/llvm.md`](https://github.com/ignite-lang/ignite/blob/main/docs/llvm.md): the LLVM
  target is the default; the Rust target stays as the differential check.
- [`docs/vision.md`](https://github.com/ignite-lang/ignite/blob/main/docs/vision.md),
  invariant 7: "Every language feature lands on both native targets with a golden and a
  fixture, until D2 says otherwise."

The measured reason is in the [targets guide](../guide/targets.md#build-times-as-measured):
`todod` builds in 1.30 s on LLVM against 4.12 s on Rust (dev, release v0.15.0, the dedicated
timing host).

## Where the code lives

| path | role |
|---|---|
| `crates/ignite-io` | the substrate, both targets |
| `crates/ignite-rustgen` | the Rust target |
| `crates/ignite-llvm` | the LLVM target |
| `crates/ignite-rt` | the LLVM target's runtime archive, the C ABI over `ignite-io` |
| `crates/ignite` | the CLI that drives cargo or clang |

The rule that keeps it this way, from the repository's `AGENTS.md`: the only hand-written Rust
a compiled program runs is `ignite-io` (Rust target) or `ignite-rt` over `ignite-io` (LLVM
target). Install the compiler as described in [Install](../guide/install.md).

Next: [The compile-time evaluator](evaluator.md).
