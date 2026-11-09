# One substrate, two targets

Ignite compiles one checked program to native executables in two ways: through LLVM IR and
clang, or through generated Rust and cargo. Both ways call the same IO code, written once in
Rust. A socket, a task or an error report behaves the same whichever target built the
program. This page explains that split, what each target does, and why both are kept. The
commands are in [Build for the Rust target and static executables](../guide/targets.md).

## The substrate is written once

Everything a compiled program does to the outside world goes through one substrate: TCP
listeners and streams, the in-process pipe of the tests, stdin and stdout and the terminal
mode, files under a directory, child processes, tasks, the process entry and exit, the drain
on SIGTERM, and the reports of a failed `assert`, `?`, overflow or index. The test runner of
`ignite test` is part of it too.

Two rules keep it small and shared:

- **The substrate knows no application protocol.** It knows sockets, pipes, stdio, files and
  tasks. Telnet, HTTP, the console and the control machines are Ignite code in `std/` or in
  the program ([Protocols and roles](protocols.md#a-general-layer-knows-no-protocol)).
- **No target suspends a task itself.** The substrate does, as it does IO, so neither target
  has IO code of its own and both get the same scheduling.

Release v0.15.0 runs one OS thread per task, with blocking IO. After it, the substrate runs
spawned tasks as stackful tasks on a few worker threads, each waiting on its sockets with
one epoll loop. Neither target's code generation changes. `IGNITE_IO=threads` selects one
thread per task again. The decision is in
[`docs/readiness-io.md`](https://github.com/ignite-lang/ignite/blob/main/docs/readiness-io.md),
the state on the [roadmap](../roadmap.md).

## The LLVM target

The default. The compiler emits LLVM IR text: the part of the standard library the program
reaches, the program, and one entry per executable. clang compiles and links it with the
runtime archive `libignite_rt.a`, which is the substrate behind a C ABI. Building a program
never runs cargo or rustc. The `ignite` binary carries the runtime archive, so a build needs
only clang.

The IR owns what the Rust target delegates to rustc:

- the layout of every value, and a copy function per aggregate type for value semantics;
- memory: a bump arena per task, freed at task end, with marks to release what a stretch of
  code allocated;
- checked arithmetic: overflow is a failure, not LLVM's wrap-around or undefined behaviour;
- debug metadata, so a debugger shows Ignite source lines.

The contract is
[`docs/llvm.md`](https://github.com/ignite-lang/ignite/blob/main/docs/llvm.md).

## The Rust target

Selected with `--target rust`. The compiler generates a Rust project, the program and the
part of the standard library it reaches, and cargo builds it against the substrate as an
ordinary crate. Generated code calls the substrate directly and synchronously. Protocol
roles are monomorphised: the generated std calls `Telnet::serve::<ignite_io::TcpStream>`,
with no `dyn ByteStream`. `ignite emit FILE` prints the generated Rust. It is an artifact,
never edited.

## The same behaviour on both

The two targets are held to one behaviour, and the tests check it rather than assume it:

- integer overflow is a failure on both, not a wrap-around:
  `task ended with error: + failed at <file>:<line>:<column>: 9223372036854775807 + 9`;
- a failed `assert`, a failed `?` and an out-of-range index end the task with the same
  report on both;
- `ignite test FILE --target all` runs every inline test on every target;
- the repository's end-to-end tests run the examples' binaries from both targets over real
  sockets and real stdin and stdout;
- the compile-time evaluator, a Rust build and an LLVM build give the same result for the
  same pure functions and the same control reducers.

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
"hypotheses to benchmark". Whether one target produces faster or smaller programs is a
question for those benchmarks, not an assumption.

## Which target is the default

LLVM, on every command that builds: `build`, `test`, `run`, `plan --for-apply` and
`conformance`. Build time is the budget Ignite is designed around, and LLVM builds a program
in a fraction of the Rust target's time. A dev build of the todo example took 1.30 s on LLVM
against 4.12 s on Rust at release v0.15.0, on the dedicated timing host. The current values
are on the [Quality](../quality.md) page.

Both targets stay. The Rust target is the differential check, and every language feature
lands on both. The decision and its history are in
[`docs/vision.md`](https://github.com/ignite-lang/ignite/blob/main/docs/vision.md) and
[`docs/llvm.md`](https://github.com/ignite-lang/ignite/blob/main/docs/llvm.md).
[Install](../guide/install.md) says what each target needs on your machine.

Next: [the roadmap](../roadmap.md) for what is designed and not built yet.
