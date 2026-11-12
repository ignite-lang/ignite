# Command line

`ignite` is the one command-line tool. Every subcommand that reads a program parses and
checks it, with the standard library, before it does anything else. This page is the home of
every flag and every exit status. The guide links here.

```text
Usage: ignite <COMMAND>

Commands:  check  emit  build  test  plan  host  apply  inspect  recover  explain  run  replay
           rfc  conformance
Options:   -h, --help     Print help
           -V, --version  Print version
```

| command | what it does |
|---|---|
| [`check`](#ignite-check) | parse and check a program |
| [`emit`](#ignite-emit) | print the generated Rust, or the typed program as JSON |
| [`build`](#ignite-build) | build the program's executables |
| [`test`](#ignite-test) | build and run inline tests |
| [`plan`](#ignite-plan) | print the system model, or save a plan for `apply` |
| [`explain`](#ignite-explain) | explain one part of the model |
| [`run`](#ignite-run) | run an assembly in the foreground |
| [`replay`](#ignite-replay) | re-step a run's journal |
| [`host`](#ignite-host) | own a local state directory |
| [`apply`](#ignite-apply) | apply a saved plan through a host |
| [`inspect`](#ignite-inspect) | read a host's status |
| [`recover`](#ignite-recover) | abandon an operation no plan can resolve |
| [`rfc import`](#ignite-rfc-import), [`rfc check`](#ignite-rfc-check) | write an RFC catalogue, check it against its source |
| [`conformance`](#ignite-conformance) | print the conformance matrix of a file's protocols |

`ignite <command> --help` prints the options of one command.

`ignite --version` prints `ignite X.Y.Z` for a clean build of the release tag `vX.Y.Z`. Any
other build from a git checkout prints `ignite X.Y.Z+g<commit>`: the first 12 hex digits of
the commit, with `.dirty` appended when a tracked source of the compiler's crates was
modified. A build from a source archive without git names the commit in the archive's
`COMMIT` file, as the toolchain image's source does. Without that file it prints
`ignite X.Y.Z+unknown`.

## Options shared by several commands

| option | commands | meaning |
|---|---|---|
| `--std DIR` | all that read a program | root of the standard library. Default: `PREFIX/share/ignite/std` for a driver at `PREFIX/bin/ignite` when that directory exists, else the `std/` of the source tree `ignite` was built from ([the standard library root](modules.md#the-standard-library-root)) |
| `--assembly NAME` | `plan`, `explain`, `run`, `replay` | the assembly to select. Default: the program's only one |
| `--fuel N` | `plan`, `explain`, `run`, `replay` | `plan` and `explain`: fuel per property expression. `run` and `replay`: fuel per reducer step, while the plan inside them keeps the default. Default 1,000,000 units |
| `--target llvm\|rust` | `build`, `test`, `run`, `plan --for-apply`, `conformance` | native target, default `llvm`. `test` also takes `all` |
| `--release` | `build`, `test`, `run`, `plan --for-apply`, `conformance` | optimised build: the release profile on Rust, clang `-O2` on LLVM |
| `--runtime FILE` | `build`, `test`, `run`, `plan --for-apply`, `conformance` | the LLVM runtime archive `libignite_rt.a` ([the runtime archive](#the-runtime-archive)) |
| `--clang PATH` | `build`, `test`, `conformance` | clang for the LLVM target. Default: `clang` on `PATH` |
| `--triple T` | `build`, `test`, `conformance` | target triple passed to clang. Default: clang's own. With `--static` a musl triple, on either target |
| `--static` | `build`, `test`, `conformance` | link against musl ([static executables](#static-executables)) |
| `--target-dir DIR` | `build`, `test`, `run`, `conformance` | cargo target directory of the Rust target |
| `--out DIR` | `build`, `test`, `conformance` | `build`: the generated project. `test`, `conformance`: root of the test builds |
| `--substrate PATH` | `build`, `test`, `emit` | the `ignite-io` crate of the Rust target ([the substrate crate](#the-substrate-crate)) |

### The substrate crate

The Rust target's generated project depends on the IO substrate crate `ignite-io`. It is the
first of:

1. `--substrate PATH`, an absolute path; a relative one is resolved against the generated
   project;
2. `PREFIX/share/ignite/substrate/crates/ignite-io` for a driver at `PREFIX/bin/ignite`, when
   it holds a `Cargo.toml`. The release archive installs it there from v0.17.0 on, with a
   workspace manifest and a `Cargo.lock` two directories up;
3. the `crates/ignite-io` of the source tree `ignite` was built from. The release archive's
   driver records no source tree.

A project without a `Cargo.lock` gets a copy of the lock two directories above the crate, so
cargo takes the locked versions from its cache.

### The runtime archive

The LLVM target links every executable against `libignite_rt.a`. The archive is the first
of:

1. `--runtime FILE`;
2. `libignite_rt.a` next to the `ignite` binary;
3. `libignite_rt.a` in the parent directory of the binary's directory;
4. the archive the `ignite` binary carries, written once to
   `$XDG_CACHE_HOME/ignite/runtime/<hash>/` (`~/.cache` when `XDG_CACHE_HOME` is unset).

The carried archive holds the test runner, so programs and tests link against the same one.
An archive of your own is built in a clone with `cargo build -p ignite-rt --features runner`.
`ignite test` and `ignite conformance` need an archive with the feature `runner`. A failed
write of the carried archive is exit 3. An archive built for another target than the
program's triple is refused before clang runs.

### Static executables

`--static` links every executable against musl, on either target: no dynamic loader and no
shared library. It needs the Rust target `<arch>-unknown-linux-musl` (`rustup target add
x86_64-unknown-linux-musl` on x86_64). An `ignite` built where that target is installed
carries the musl runtime and libc, so a static LLVM build needs only clang. The Rust target
builds with `cargo build --target <arch>-unknown-linux-musl`. Without the musl target the
command exits 2 and names what to install. It never falls back to a dynamic build. With
`--static`, `--triple` must name a musl target.

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
| `--json` | print `{"diagnostics":[...]}` on stdout instead. Each diagnostic has `path`, `code`, `message`, `start` and `end`. A position has 1-based `line` and `column` and the byte `offset`. A clean program prints `{"diagnostics":[]}` |
| `--stdin-for FILE` | check the text on stdin as the program `FILE`, for an unsaved editor buffer. Sibling modules and the standard library come from disk. `FILE` is not read |

| exit | meaning |
|---|---|
| 0 | clean |
| 1 | diagnostics |
| 3 | a file could not be read |

## ignite emit

```text
ignite emit [--substrate PATH] [--std DIR] FILE
ignite emit --typed-json [--std DIR] FILE
```

Without `--typed-json`, print the generated Rust projects: the program, then the standard
library. `--substrate` is the path of the `ignite-io` crate written into the generated
`Cargo.toml`. Generated Rust is an artifact: read it, do not edit it.

With `--typed-json`, print the parsed and typed program of `FILE` as one compact JSON
document, format `ignite-typed/0.1`, followed by a newline. `FILE` is loaded as `ignite plan`
loads it. The evaluator does not run. The document covers `FILE` alone: a sibling module or a
standard-library module is a document of its own. The same inputs give the same bytes. A
change that removes or reinterprets a key bumps the format version. An added key does not, so
a reader ignores keys it does not know. `--typed-json` conflicts with `--substrate`. The full
format is
[`docs/slice.md` section 24](https://github.com/ignite-lang/ignite/blob/main/docs/slice.md#24-the-typed-program-ignite-emit---typed-json).

| exit | meaning |
|---|---|
| 0 | printed |
| 1 | diagnostics, nothing on stdout |
| 3 | a file could not be read |

## ignite build

```text
ignite build [--target llvm|rust] [--out DIR] [--target-dir DIR] [--release]
             [--clang PATH] [--runtime FILE] [--triple T] [--static]
             [--substrate PATH] [--std DIR] FILE
```

Build the program's executables and print one `built <path>` line per executable, in
declaration order. `build` owns the directory `--out` and overwrites it.

| option | meaning |
|---|---|
| `--target llvm\|rust` | `llvm` (default): LLVM IR compiled and linked by clang. `rust`: a generated Rust project built by cargo |
| `--out DIR` | directory of the generated project. Default `target/ignite/<package>` |
| `--target-dir DIR` | cargo target directory of `--target rust`. Default `<out>/target` |
| `--release` | optimised build, below |
| `--clang PATH`, `--runtime FILE`, `--triple T` | LLVM only: clang, the [runtime archive](#the-runtime-archive) and the triple |
| `--static` | link against musl ([static executables](#static-executables)) |
| `--substrate PATH` | path of the `ignite-io` crate for `--target rust` ([the substrate crate](#the-substrate-crate)) |

On the LLVM target:

1. the IR goes to `<out>/llvm/`: `std.ll` with the standard-library functions the program
   reaches, `program.ll`, and `main_<exe>.ll` per executable;
2. clang compiles each file with `-O0 -g`, or `-O2` with `--release`;
3. clang links each executable with the runtime archive and `--gc-sections`. `--release`
   also strips the debug sections.

The executables are `<out>/bin/<exe>`. A program build on this target runs no cargo and no
rustc. Dev builds carry debug metadata, so a debugger shows Ignite source lines. With
`--release` and none of `--runtime`, `--triple` or `--static`, the units are compiled for
ThinLTO and `ld.lld` links them with the bitcode runtime the binary carries. That needs a
clang of the LLVM major version of the rustc that built `ignite`, and `ld.lld` on `PATH`.
Otherwise `--release` builds at `-O2` as above.

On the Rust target the project is built with `cargo build`, with the release profile under
`--release`. The executables are `<target-dir>/debug/<exe>`, or `release/` with `--release`.
It needs cargo on `PATH` and no runtime archive.

| exit | meaning |
|---|---|
| 0 | built |
| 1 | diagnostics |
| 2 | cargo or clang failed, with their output passed through, or the musl target is missing |
| 3 | a file could not be read or written, the runtime archive included |

## ignite test

```text
ignite test [OPTIONS] FILE.ignite... [FILTER]
```

Build the inline tests of the named files into one test executable per target and run them.
Arguments ending in `.ignite` are files. At most one other argument is the filter, a
substring of the test's full name `<module>/<test name>`. What a test is and how it fails is
in [Tests and assertions](tests.md).

| option | meaning |
|---|---|
| `--target llvm\|rust\|all` | default `llvm`. `all` runs every supported target with shared preparation |
| `--exact` | run only the test whose full name equals the filter |
| `--list` | print `<full name>: test` per selected test and run nothing |
| `--jobs N` | tests run at once. Default: the CPU count |
| `--timeout SECS` | a test still running after this is killed and fails. Default 60 |
| `--release` | the release profile on Rust, clang `-O2` on LLVM |
| `--out DIR` | root of the test builds. Default `target/ignite-test` |
| `--target-dir DIR` | cargo target directory of the Rust tests. Default `<out>/<name>/rust/target` |
| `--clang PATH`, `--runtime FILE`, `--triple T` | LLVM only: clang, the [runtime archive](#the-runtime-archive) with the test runner, and the triple |
| `--static` | link the test executables against musl |
| `--substrate PATH` | the `ignite-io` crate of the Rust tests ([the substrate crate](#the-substrate-crate)) |

The output follows `cargo test`:

```text
running 1 test
test hello/greet puts the name in the sentence ... ok

test result: ok. 1 passed; 0 failed; 0 filtered out; finished in 0.01s
```

| exit | meaning |
|---|---|
| 0 | every selected test passed, or none was selected, or `--list` |
| 1 | a test failed |
| 2 | a compile error or an invalid argument, with diagnostics as `ignite check` prints them |

## ignite plan

```text
ignite plan [--assembly NAME] [--out FILE] [--fuel N] [--std DIR] FILE
ignite plan FILE --for-apply --state DIR --out PLAN [--assembly NAME]
            [--target llvm|rust] [--release] [--runtime FILE] [--fuel N] [--std DIR]
```

Evaluate every property at compile time and print the system model as canonical JSON, format
`ignite-model/0.1` ([System declarations](systems.md#the-system-model)). Ordinary `plan`
builds nothing.

With `--for-apply`, prepare a saved plan, format `ignite-apply-plan/0.1`, against the running
host of `--state`. The plan is bound to the source and to the host's current revision. It
builds the artifacts but changes nothing that runs. The members build as in `ignite build`:
LLVM by default, the dev profile unless `--release`. A `Setting` without a `default` in a
member the plan would start is E142, reported before the host is contacted.

| option | meaning |
|---|---|
| `--assembly NAME` | the assembly to select |
| `--out FILE` | write the model, or the saved plan, to this file and overwrite it. Required with `--for-apply` |
| `--fuel N` | fuel per property expression |
| `--for-apply` | write a saved plan instead of the model. Needs `--state` and `--out` |
| `--state DIR` | state directory of the running host. Only with `--for-apply` |
| `--target`, `--release`, `--runtime` | how the members are built. Only with `--for-apply` |

| exit | meaning |
|---|---|
| 0 | model or plan written |
| 1 | diagnostics |
| 3 | `--for-apply` without a running host of `DIR`, or a host or storage failure |

## ignite explain

```text
ignite explain [--assembly NAME] [--fuel N] [--std DIR] FILE PATH
```

Print one system, member, property path or assembly of the model for people. `PATH` is, for
example, `Terminal`, `Terminal.reachable`, `Terminal.server.recovery` or `local`.

| exit | meaning |
|---|---|
| 0 | explained |
| 1 | diagnostics, E119 among them for a path that names nothing |

## ignite run

```text
ignite run [OPTIONS] FILE
```

Run an assembly in the foreground: build its members, start them, probe them and evaluate
its contracts. Each event goes to the journal of the state directory before reducers of
[`std.control`](std/control.md) step it. The journal's records are in
[System declarations](systems.md#the-journal).

| option | meaning |
|---|---|
| `--assembly NAME` | the assembly to run |
| `--state DIR` | journal, lock and executables. Default `target/ignite-run/<assembly>/` |
| `--build DIR` | build directory. Default `<state>/build/` |
| `--target llvm\|rust`, `--release`, `--runtime FILE`, `--target-dir DIR` | how the members are built, as in `ignite build`. `--target-dir` defaults to `<build>/target` |
| `--clock real\|manual` | the clock the reducers see. Default `real` |
| `--epoch RFC3339` | epoch of the manual clock. Default `2026-01-01T00:00:00Z` |
| `--settle DURATION` | settle time of the manual clock, a Duration literal. Default `2 s` |
| `--port RESOURCE=PORT` | bind a resource to a port. Port 0 binds a free port. Repeatable |
| `--json` | print the journal records on stdout |
| `--fresh` | rename an existing journal and start a new one |
| `--fuel N` | fuel per reducer step |

`ignite run` passes no setting values to the members it starts. A `Setting` takes its
`default`, and a `Setting` without one is E142 before anything starts. A relative `Directory`
path resolves against the working directory of `ignite run`.

Commands on stdin, one per line:

| line | effect |
|---|---|
| `status` | the standing of every contract, the state of every machine and the ports in use. One JSON line with `--json` |
| `rollout <member>` | start the member's rollout machine |
| `stop` | stop, as SIGINT does |
| `advance <quantity>` | manual clock only: move time, for example `advance 100 ms` |

`stop`, SIGINT and SIGTERM send SIGTERM to every process, wait up to the member's drain
deadline, kill what is left and exit.

| exit | meaning |
|---|---|
| 0 | a requested stop, with no failed machine |
| 1 | a machine gave up or failed (E128) |
| 2 | a diagnostic before the start, E125, E126 and E142 among them |

## ignite replay

```text
ignite replay [--assembly NAME] [--state DIR] [--fuel N] [--std DIR] FILE
```

Re-step the journal of a state directory through the reducers, compare every effect and
standing record (E127), and print the final standing of each contract, one per line.
`--state` defaults to `target/ignite-run/<assembly>/`.

| exit | meaning |
|---|---|
| 0 | the journal replays |
| 1 | a diagnostic: E126 for a journal of another model, E127 for a journal that does not replay |

## ignite host

```text
ignite host --state DIR
```

Own and supervise a local state directory until SIGINT, SIGTERM or `stop` on stdin. `status`
on stdin prints the status. The host holds `<state>/lock` and answers on
`<state>/control.sock`. It uses the real clock. `apply` never starts a host. A restarted host
resumes accepted work and reconciles its last committed revision. At start it checks its
journal. A torn last record is repaired. An altered record refuses the start with exit 3.

| exit | meaning |
|---|---|
| 0 | stopped |
| 2 | another host or run holds the state |
| 3 | a host or storage failure, a damaged host journal among them |

## ignite apply

```text
ignite apply --state DIR PLAN
```

Apply a saved plan through the running host of `DIR` and wait for completion. The receipt is
printed on stdout as compact JSON. A disconnect cancels neither the accepted operation nor
the host's work: inspect or retry rather than assume a rejection.

| exit | meaning |
|---|---|
| 0 | completed, or already completed |
| 1 | failed, outcome unknown, or recovery required |
| 2 | plan, admission or revision rejected, or a plan that [`recover`](#ignite-recover) abandoned |
| 3 | a host or storage failure |

## ignite inspect

```text
ignite inspect --state DIR [--operation ID] [--json]
```

Print the host's status as JSON. When no host runs, print the last durable status of
`<state>/status.json`, labelled offline. An offline status also reports a damaged host
journal and its `nextAction`. `--operation ID` inspects one accepted operation. `--json`
prints compact JSON.

| exit | meaning |
|---|---|
| 0 | the status was read, which says nothing about whether an operation succeeded |
| 3 | no host answers and no durable status can be read |

## ignite recover

```text
ignite recover --state DIR --operation ID [--json]
```

Abandon an operation that the host holds as `recovery_required` and that no corrective plan
resolves. Needs the running host of `DIR`. The host stops the run of every revision, drains
and stops every instance it owns, marks the operation `abandoned` and starts the committed
revision afresh. It prints `recovered <id>: stopped <n> instances; committed revision <r>
restarted`, or the host's answer as compact JSON with `--json`. A plan of an abandoned
operation is refused by `apply` with exit 2: prepare a new plan. `recover` is repeatable: a
host killed during a recovery restarts with the operation still `recovery_required`.

| exit | meaning |
|---|---|
| 0 | recovered: the committed revision is ready |
| 1 | refused, because a process under `<state>/revisions/` is not accounted for, or the revision was not ready within its readiness timeout |
| 2 | the operation requires no recovery, or is being recovered |
| 3 | a host failure |

## ignite rfc import

```text
ignite rfc import FILE [--sections LIST] [--package NAME] [--url URL] [--out FILE]
```

Write the catalogue module of a local RFCXML v3 file: one `Spec` constant and one
`Requirement` constant per sentence that holds a `<bcp14>` keyword ([`std.rfc`](std/rfc.md)).
The command never opens a network connection. A lowercase "must" outside `<bcp14>` yields
nothing. The party of each requirement is guessed from its sentence, so review every `party`
line. The output starts with a comment that names the command that wrote it. It is
deterministic: no time and no path beyond what was given.

| option | meaning |
|---|---|
| `--sections LIST` | comma-separated section numbers to keep. A requirement is kept when its section equals one or starts with one followed by `.`. Ordinals are counted before the filter, so ids do not depend on it. Default: every section |
| `--package NAME` | package of the module, a dotted module path. Default `std.rfc.rfc<number>` |
| `--url URL` | URL of the `Spec`. Default: the root's first `<link rel="alternate">` |
| `--out FILE` | write the module to this file and overwrite it. Default: stdout |

| exit | meaning |
|---|---|
| 0 | written |
| 2 | a usage error, malformed XML or a root without a `number` attribute. The error is `<file>:<line>:<col>: <message>` |
| 3 | `FILE` could not be read or `--out` could not be written |

## ignite rfc check

```text
ignite rfc check CATALOGUE [SOURCE]
```

Compare the `digest` of the catalogue's single `Spec` constant, `sha256:` and lowercase hex,
with the digest of `SOURCE`'s bytes. Any source file works, a `.txt` too.

```text
$ ignite rfc check std/rfc/rfc9110.ignite docs/rfc/rfc9110.xml
$ ignite rfc check std/rfc/rfc854.ignite
std/rfc/rfc854.ignite: unchecked
```

| exit | meaning |
|---|---|
| 0 | the digests match, and nothing is printed. Or no `SOURCE` was given, and it prints `<CATALOGUE>: unchecked` |
| 1 | the digests differ. One line on stderr names the catalogue and both digests |
| 2 | the catalogue has no single `Spec` constant with a literal `digest` |
| 3 | a file could not be read |

## ignite conformance

```text
ignite conformance FILE [--target llvm|rust] [--out DIR] [--target-dir DIR] [--release]
                   [--clang PATH] [--runtime FILE] [--triple T] [--static] [--std DIR]
```

Print the conformance matrix of every protocol in `FILE` that declares `conforms`. When a row
names a test, `FILE`'s tests are built as `ignite test` builds them and run once. The options
are those of `ignite test`. Test output is not echoed. One block per protocol and spec:

```text
rfc854 Telnet Protocol Specification, protocol Telnet, parties all, sections all
RP2_1   p2   MAY         none            -                                                           unclassified
RP2_2   p2   MUST NOT    satisfied       decode_within; test "DO is answered WONT and WILL is answered DONT" passed  pass
…
claim conformance of Telnet to rfc854: yes (0 gaps, 0 failures, 0 inconclusive)
```

That is `ignite conformance std/protocol/telnet.ignite`, shortened. A row has six columns: id,
section, level, status, evidence and verdict.

| status | meaning |
|---|---|
| `satisfied` | declarations name the requirement in `satisfies` |
| `limit` | satisfied by a limit field with a minimum |
| `unsupported`, `not_applicable` | the protocol declares it so, with a reason |
| `none` | no status |

| verdict | when, first rule that applies |
|---|---|
| `out of scope` | the requirement is outside the `scope` of `conforms` |
| `n/a` | `not_applicable`, or `unsupported` at MAY class |
| `gap` | `unsupported` at MUST class |
| `deviation` | `unsupported` at SHOULD class |
| `unclassified` | no status |
| `fail` | satisfied, and a verifying test failed or a limit is below its minimum |
| `inconclusive` | satisfied, and a verifying test did not run, a limit has no value, or there is neither a test nor a limit |
| `pass` | satisfied, and every verifying test passed, or every limit meets its minimum |

A claim holds when no MUST-class row in scope is `fail`, `gap` or `inconclusive`. The counts
on the claim line are over every row in scope.

| exit | meaning |
|---|---|
| 0 | every claim holds |
| 1 | a claim does not hold |
| 2 | a diagnostic, a usage error, or no protocol of `FILE` conforms |
| 3 | a test build or run failure, or an IO error |

## Compiled executables

The generated `main` of an executable reads its flags, binds the `TcpListener` resources its
component uses and prints `listening on <address>:<port>` for each.

| flag | meaning |
|---|---|
| `--port N` | the port of the first listener, or of an `Endpoint` the component connects to. `--port 0` binds a free port, and the `listening on` line reports it |
| `--ref <path>=<address>:<port>` | bind a reference for `Endpoint.of`. `ignite run` passes it to a probe. The first binding with the path wins. No binding with that path is the error `no binding for <path>` |
| the `flag` of a `Setting` or a `Directory` | the setting's value, or the directory's path. No `--flag` given: the `default`, or the declared `path` |

Each flag is read as `--flag V` or `--flag=V`, and the first one wins. Arguments that no read
asks for are ignored, so one argument list serves every read. A value after `--flag` that
starts with `--` is no value. `--flag=V` takes `V` as it is. A value that is not UTF-8 is
refused with exit 2. A `Setting` without a `default` whose flag is absent ends the program with
`error: <flag>: required argument missing` and exit 2.

| exit | meaning |
|---|---|
| 0 | `main` returned normally |
| 1 | binding the listener failed, `Endpoint.of` found no `--ref` binding, or `main` returned an error without an exit code of its own |
| 2 | `--port`, `--ref` or a flag malformed or missing, a value not UTF-8, or a port out of range |
| 3, 4 | the Telnet client of `TelnetClient`, such as `tn`: connecting failed, or the session ended with an error |
