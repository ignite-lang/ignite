# Building for Rust and LLVM

Ignite has two native targets. Both compile the same checked program, both call the same IO
substrate, and both must behave the same, down to the text of an error.

| | LLVM target (default) | Rust target |
|---|---|---|
| select it with | `--target llvm` or nothing | `--target rust` |
| the compiler emits | LLVM IR text: `std.ll`, `program.ll` and one `main_<exe>.ll` per executable | a Rust project: the program crate and an `ignite-std` crate |
| built by | clang, linked against `libignite_rt.a` | cargo |
| the substrate | `ignite-io` behind the C ABI adapter `ignite-rt` | the `ignite-io` crate, called directly |
| output | `built <out>/bin/<exe>` | `built <target-dir>/debug/<exe>` |

## The LLVM target

The LLVM target needs clang and the runtime archive `libignite_rt.a`, which is built once
in the clone (the commands below run from it):

```sh
cargo build -p ignite-rt --features runner
ignite build examples/todo/todo.ignite --runtime target/debug/libignite_rt.a
```

```text
built target/ignite/todo/bin/todod
```

`ignite build` writes into `--out` (default `target/ignite/<package>`) and prints one
`built …` line per executable. Without `--runtime`, `ignite` looks for `libignite_rt.a` next
to its own binary, then in that directory's parent; when it finds none, the error names the
command that builds it. The feature `runner` adds the test runner that `ignite test` links;
programs link against the same archive. `--clang PATH` names another clang. A program build
on this target never runs cargo or rustc.

What happens:

1. the IR is written to `<out>/llvm/`: `std.ll` (only the standard-library functions the
   program reaches), `program.ll` and `main_<exe>.ll`;
2. clang compiles each with `-O0 -g` (`-O2` with `--release`);
3. clang links each executable with the runtime archive and `--gc-sections`, so code that no
   executable reaches is dropped; `--release` also strips the debug sections.

Every module carries its target triple. `--triple` pins one (default: clang's own target),
and a runtime archive built for another target is refused before clang runs. Dev builds carry
debug metadata, so a debugger shows Ignite source lines.

Exit statuses of `ignite build`: 0 built, 1 diagnostics, 2 the backend build failed (clang's
or cargo's output is passed through), 3 a file could not be read or written, the runtime
archive included.

## The Rust target

```sh
ignite build examples/todo/todo.ignite --target rust
```

```text
built target/ignite/todo/target/debug/todod
```

`ignite build --target rust` writes the generated project into `--out` (default
`target/ignite/<package>`), runs `cargo build` on it with `--target-dir` (default
`<out>/target`) and prints one `built …` line per executable. `--release` builds with the
release profile. It needs cargo on the `PATH` and no runtime archive.

`ignite emit FILE` prints the generated Rust instead of building it: the program, then the
standard library. Generated Rust is an artifact; never edit it and never commit it.

The repository's tasks build the examples on both targets:

```sh
task compile              # terminald and tn with --target rust, into target/ignite/terminal
task compile-llvm         # terminald and tn into target/ignite-llvm/terminal/bin/
task compile-todo-llvm    # todod into target/ignite-llvm/todo/bin/
task server TARGET=llvm
```

## The same behaviour on both

- Integer overflow is a failure on both targets, not a wrap-around:
  `task ended with error: + failed at <file>:<line>:<column>: 9223372036854775807 + 9`.
- A failed `assert`, a failed `?` and an out-of-range index end the task with the same report
  on both.
- Inline tests run on both: `ignite test FILE --target all`.
- The end-to-end tests of the repository run the examples' binaries from both targets over
  real sockets and real stdin and stdout.

## Build times, as measured

The repository records program build times at every step. At the v0.3.0 close,
building `todod` from a fresh output and target directory took about 3.1 s (Rust, dev)
and 4.2 s (Rust, release), and 0.29 s (LLVM, dev) and 0.73 s (LLVM, release), over three
measured pairs. The method and every run are in
[`AGENTS.md`](https://github.com/ignite-lang/ignite/blob/main/AGENTS.md) (the section "in-process streams") and
[`docs/performance/`](https://github.com/ignite-lang/ignite/tree/main/docs/performance).
Whether one target produces faster or smaller programs is, by the design, a question for
benchmarks, not an assumption.

## Which one to use

Start with the default, LLVM: it builds a program in a fraction of the Rust target's time,
and build time is the budget Ignite is designed around. Both targets stay: the Rust target
is the differential check, every feature lands on both, and `--target rust` selects it on
every command that builds. The reasoning is in
[One substrate, two targets](../design/substrate.md).
