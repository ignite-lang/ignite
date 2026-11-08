# Command line

`ignite` is the one command-line tool. Every subcommand that reads a program parses and
checks it, with the standard library, before it does anything else.

```text
Usage: ignite <COMMAND>

Commands:  check  emit  build  test  plan  host  apply  inspect  explain  run  replay
Options:   -h, --help     Print help
           -V, --version  Print version
```

`ignite <command> --help` prints the options of one command; the tables below follow it.

`ignite --version` prints `ignite X.Y.Z` for a clean build of the release tag `vX.Y.Z`, and
`ignite X.Y.Z+g<commit>` (the first 12 hex digits, `.dirty` appended when a tracked source
of the compiler's crates was modified) for any other build from a git checkout. A build from a source archive without git
names the commit in the archive's `COMMIT` file, as the toolchain image's source has it, or
prints `ignite X.Y.Z+unknown`.

## Options shared by several commands

| option | commands | meaning |
|---|---|---|
| `--std DIR` | all that read a program | root of the standard library (default: the `std/` of the source tree `ignite` was built from) |
| `--assembly NAME` | `plan`, `explain`, `run`, `replay` | the assembly to select (default: the program's only one) |
| `--fuel N` | `plan`, `explain`, `run`, `replay` | `plan`, `explain`: fuel per property expression; `run`, `replay`: fuel per reducer step only, the plan inside them keeps the default; default 1,000,000 units |
| `--target llvm\|rust` | `build`, `test`, `run`, `plan --for-apply`, `conformance` | native target, default `llvm`; `test` also takes `all` |
| `--release` | `build`, `test`, `run`, `plan --for-apply` | optimised build (release profile on Rust, clang `-O2` on LLVM) |
| `--runtime FILE` | `build`, `test`, `run`, `plan --for-apply`, `conformance` | the LLVM runtime archive `libignite_rt.a` (default: next to `ignite`, then in its parent directory, then the archive `ignite` carries, written once to `$XDG_CACHE_HOME/ignite/runtime/`) |
| `--clang PATH`, `--triple T` | `build`, `test` | clang for the LLVM target (default `clang` on `PATH`) and its target triple (default: clang's own) |
| `--static` | `build`, `test`, `conformance` | link against musl: no dynamic loader, no shared library, on either target; needs the Rust target `<arch>-unknown-linux-musl`, exit 2 naming it when missing; with `--triple` a musl triple ([Static executables](../guide/targets.md#static-executables)) |
| `--target-dir DIR` | `build`, `test`, `run` | cargo target directory of the Rust target |

## ignite check

```text
ignite check [--std DIR] [--json] FILE
ignite check [--std DIR] [--json] --stdin-for FILE
```

Parse and check a program and the standard library. Prints nothing on success. Diagnostics
go to stderr, one per line, as `file:line:column: error[Ennn]: message`
([Diagnostics](diagnostics.md)).

| option | meaning |
|---|---|
| `--json` | print `{"diagnostics":[...]}` on stdout instead: per diagnostic `path`, `code`, `message`, and `start` and `end`, each with 1-based `line` and `column` and the byte `offset`; a clean program prints `{"diagnostics":[]}` |
| `--stdin-for FILE` | check the text on stdin as the program `FILE` (an unsaved editor buffer); its sibling modules and the standard library come from disk, `FILE` is not read |

| exit | meaning |
|---|---|
| 0 | clean |
| 1 | diagnostics |
| 3 | a file could not be read |

## ignite emit

```text
ignite emit [--substrate PATH] [--std DIR] FILE
```

Print the generated Rust projects: the program, then `std/`. `--substrate` is the path of the
`ignite-io` crate written into the generated `Cargo.toml`. Generated Rust is an artifact;
read it, never edit it.

## ignite build

```text
ignite build [--target llvm|rust] [--out DIR] [--target-dir DIR] [--release]
             [--clang PATH] [--runtime FILE] [--triple T] [--static] [--substrate PATH] [--std DIR] FILE
```

Build the program's executables: LLVM IR with clang (the default), or generated Rust with
cargo (`--target rust`). Prints one `built <path>` line
per executable. See [Building for Rust and LLVM](../guide/targets.md).

| option | meaning |
|---|---|
| `--out DIR` | directory of the generated project (default `target/ignite/<package>`); `build` owns and overwrites it |
| `--target-dir DIR` | cargo target directory of `--target rust` (default `<out>/target`) |
| `--substrate PATH` | path of the `ignite-io` crate |

| exit | meaning |
|---|---|
| 0 | built |
| 1 | diagnostics |
| 2 | cargo or clang failed (their output is passed through) |
| 3 | a file could not be read or written |

## ignite test

```text
ignite test [OPTIONS] FILE.ignite... [FILTER]
```

Build the inline tests of the named files into one test executable per target and run them.
Arguments ending in `.ignite` are files; at most one other argument is the filter (a
substring of the test name). See [Testing with ignite test](../guide/testing.md).

| option | meaning |
|---|---|
| `--target llvm\|rust\|all` | default `llvm`; `all` runs every supported target with shared preparation |
| `--exact` | run only the test whose full name equals the filter |
| `--list` | list the selected tests, run nothing |
| `--jobs N` | tests run at once (default: the CPU count) |
| `--timeout SECS` | a test still running after this is killed and fails (default 60) |
| `--out DIR` | root of the test builds (default `target/ignite-test`) |
| `--target-dir DIR` | cargo target directory for the Rust tests (default `<out>/<name>/rust/target`) |

| exit | meaning |
|---|---|
| 0 | every selected test passed, or none was selected, or `--list` |
| 1 | a test failed |
| 2 | a compile error |

## ignite plan

```text
ignite plan [--assembly NAME] [--out FILE] [--fuel N] [--std DIR] FILE
ignite plan FILE --for-apply --state DIR --out PLAN [--assembly NAME]
            [--target llvm|rust] [--release] [--runtime FILE] [--fuel N] [--std DIR]
```

Print the system model as canonical JSON (`ignite-model/0.1`), or write it to `--out`. With
`--for-apply`, prepare a source-bound saved plan (`ignite-apply-plan/0.1`) against the running
host of `--state`; this builds the artifacts but changes nothing that runs. See
[Plan, explain, run and replay](../guide/operate.md) and [Local apply](../guide/apply.md).

| exit | meaning |
|---|---|
| 0 | model or plan written |
| 1 | diagnostics |
| 3 | `--for-apply` without a running host of `DIR`, or a host or storage failure |

## ignite explain

```text
ignite explain [--assembly NAME] [--fuel N] [--std DIR] FILE PATH
```

Explain one system, member, property path or assembly of the model, for example
`Terminal`, `Terminal.reachable`, `Terminal.server.recovery` or `local`. Exits 1 on a
diagnostic, E119 among them for a path that names nothing.

## ignite run

```text
ignite run [OPTIONS] FILE
```

Run an assembly in the foreground: build its members, start them, probe them and evaluate
its contracts.

| option | meaning |
|---|---|
| `--state DIR` | journal, lock and executables (default `target/ignite-run/<assembly>/`) |
| `--build DIR` | build directory (default `<state>/build/`) |
| `--clock real\|manual` | default `real` |
| `--epoch RFC3339` | epoch of the manual clock (default `2026-01-01T00:00:00Z`) |
| `--settle DURATION` | settle time of the manual clock, a Duration literal (default `2 s`) |
| `--port RESOURCE=PORT` | bind a resource to a port; port 0 binds a free port; repeatable |
| `--json` | print the journal records on stdout |
| `--fresh` | rename an existing journal and start a new one |

Commands on stdin, one per line:

| line | effect |
|---|---|
| `status` | the standing of every contract, the state of every machine and the ports in use (one JSON line with `--json`) |
| `rollout <member>` | start the member's rollout machine |
| `stop` | stop, as SIGINT does |
| `advance <quantity>` | manual clock only: move time, for example `advance 100 ms` |

| exit | meaning |
|---|---|
| 0 | a requested stop, with no failed machine |
| 1 | a machine gave up or failed (E128) |
| 2 | a diagnostic before the start |

## ignite replay

```text
ignite replay [--assembly NAME] [--state DIR] [--fuel N] [--std DIR] FILE
```

Re-step the journal of a state directory through the reducers, compare every effect and
standing record (E127), and print the final standings. Exits 0 when the journal replays, 1 on
a diagnostic.

## ignite host

```text
ignite host --state DIR
```

Own and supervise a local state directory until stopped (SIGINT, SIGTERM or `stop` on
stdin; `status` on stdin prints the status). It holds `<state>/lock` and answers on
`<state>/control.sock`.

| exit | meaning |
|---|---|
| 0 | stopped |
| 2 | another host or run holds the state |
| 3 | a host or storage failure |

## ignite apply

```text
ignite apply --state DIR PLAN
```

Apply a saved plan through the running host of `DIR` and wait for completion. A disconnect
cancels neither the accepted operation nor the host's work.

| exit | meaning |
|---|---|
| 0 | completed, or already completed |
| 1 | failed, outcome unknown, or recovery required |
| 2 | plan, admission or revision rejected |
| 3 | a host or storage failure |

## ignite inspect

```text
ignite inspect --state DIR [--operation ID] [--json]
```

Print the host's status as JSON (`--json`: compact), or the last durable status labelled
offline when no host runs. `--operation ID` inspects one accepted operation. Exit 0 means the
status was read, not that an operation succeeded.

## Compiled executables

The generated `main` of an executable binds the `TcpListener` resources its component uses
and prints `listening on <address>:<port>` for each. `--port N` overrides the port of the
first listener, or of an `Endpoint` the component connects to; `--port 0` binds a free port
and the `listening on` line reports it. `--ref <path>=<address>:<port>` (or
`--ref=<path>=…`) binds a reference for `Endpoint.of`; `ignite run` passes it to a probe. The
first binding with the path wins; no binding with that path is the error `no binding for
<path>`.

| exit | meaning |
|---|---|
| 0 | `main` returned normally |
| 1 | binding the listener failed, `Endpoint.of` found no `--ref` binding, or `main` returned an error without an exit code of its own |
| 2 | `--port` or `--ref` malformed, or a port out of range |
| 3, 4 | the terminal client `tn`: connecting failed, or the session ended with an error |
