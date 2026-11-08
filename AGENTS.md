# Ignite

Intent-oriented language for specifying, building, deploying and operating systems. Design:
`docs/design.md` (version 0.3, 2026-11-01). Direction and the current slice's contract:
`docs/slice.md`. This repository holds the compiler (Rust), the standard library (Ignite) and
the IO substrate (Rust) that compiled programs link.

## Layout

| path | role |
|---|---|
| `crates/ignite-syntax/` | lexer, parser, AST, diagnostics |
| `crates/ignite-check/` | name resolution across modules, effect and protocol rules; produces `Symbols` |
| `crates/ignite-types/` | type checker: a type for every expression, E040 to E046 (E046 match exhaustiveness, list columns split by length for rest patterns, `docs/slice.md` 20.5); `moves.rs`, the last-use moves analysis (20.3): `TypedProgram::moves` (a place in a move position taken without a copy) and `consumed` (the parameters a callee takes by value, a per-module fixpoint, `ModuleTypes::consumed`), read by both backends |
| `crates/ignite-rustgen/` | Rust target: the `ignite-std` crate and the program crate |
| `crates/ignite-llvm/` | LLVM target: IR text for std, program and one main per executable |
| `crates/ignite-rt/` | runtime of the LLVM target, `libignite_rt.a`: C ABI adapter over `ignite-io` (value conversion, handle table), arenas and regions, the spawn handshake; no IO of its own |
| `crates/ignite-io/` | IO substrate of both targets, blocking: TCP listener and stream (urgent data inline since story `telnet-rfc854-conformance`: `write_urgent`, `urgent_pending`, `SO_OOBINLINE` on every stream, a read stops before the urgent byte; `docs/slice.md` 19.21), the in-process pipe (`Pipe.pair()`, `PipeStream`, coalescing as TCP, bounded at `PIPE_CAPACITY`, `received_batches`; `docs/slice.md` 18.4, 18.8) and the `ByteStream` trait both streams implement, stdin, stdout, terminal mode, tasks (one thread each), `run`, process arguments and exit, the reports of `assert` and `?`, the test runner (feature `runner`); since slice 17 (`docs/slice.md` 21.4 to 21.8) `fs` (`Directory`: files under a root, an `openat` walk that refuses every symlink, atomic replace, bounds), `child` (`Launcher`, `ChildProcess`: one watcher thread per child, `Invocation.input` written to the child's stdin by a `child-input` thread, timeout and kill by process group, bounded output, `close_range` and `PR_SET_PDEATHSIG` in `pre_exec`; the test runner's `run_child` uses the same watcher), `entropy` (`/dev/urandom`), `args::flag_from_args` (settings and directory flags) and the `at_exit` hooks that kill live children; `limits`, the open-files soft limit raised to the hard limit at the entry of both targets (`docs/slice.md` 27) |
| `crates/ignite-eval/` | compile-time evaluator (`docs/slice.md` section 15): interprets property expressions over the typed modules with fuel (E115, E117, E118), builds the system model `ignite plan` prints, resolves assemblies through `std.deploy`'s `admits` and `includes` (E110 to E114), renders `ignite explain`; steps the reducers of `std.control` for `ignite run` and `ignite replay` (`docs/slice.md` 16.2, O27); used by `check`, `plan`, `explain`, `run` and `replay` only, linked by no backend and no compiled program |
| `crates/ignite/` | `ignite` CLI: `check`, `emit`, `build`, `test`, `plan`, `explain`, `run`, `replay`, `host`, `apply`, `inspect`, `rfc import`, `rfc check`, `conformance` (`src/conformance.rs`: the matrix and verdicts of `docs/slice.md` 19.10); `emit --typed-json` (`src/typed_json.rs`: the typed program as one `ignite-typed/0.1` document, `docs/slice.md` 24); end-to-end tests of the example in `tests/terminal.rs`, model and explain goldens in `tests/golden/` |
| `crates/ignite/src/run/` | the driver of `ignite run` and `ignite replay` (`docs/slice.md` 16.3): `engine` steps the `std.control` reducers in `ignite-eval`, `live` runs the adapters `process`, `probe`, `clock` and `control` that turn effects into processes, attempts, timers and stdin commands and feed events back, `journal` records every event, effect, signal, standing and consequence (16.5), `replay` re-steps a journal and compares (E127) |
| `crates/ignite/src/rfc/` | `ignite rfc import` and `ignite rfc check` (`docs/slice.md` 19.8): `xml` the hand-written RFCXML reader (no dependency), `extract` one requirement per sentence with a `<bcp14>` mark, `write` the catalogue module; never opens a network connection |
| `crates/ignite/src/apply/` | the local host and source-bound saved plans (`docs/slice.md` 17): `plan` owns input/artifact identities and action manifests, `host` owns the state lock, socket queue, durable receipts and host journal, `revision` owns retained source/type/model contexts and calls the existing `run` driver; implementation-only server replacements use `std.control`'s existing rollout through a pinned artifact builder; since slice 18 the host journal is checked at start (a torn last record repaired as `journalRepaired`, an altered record, a cut before the `status.json` anchor or a differing revision journal refused with exit 3; 22.7), the host records its working directory in `<state>/workdir`, and `host/recovery` carries the operator's `recover` (22.9) |
| `crates/ignite-load/` | `ignite-load`: load generator for `terminald` (workloads `rr`, `pipe`, `connect`; `ignite-load matrix` for `task bench-throughput`); not linked into compiled programs |
| `tools/ignite-perf/` | `ignite-perf`: the performance budget harness (modes `self-check`, `recover-baseline`, `smoke`, `prepare`, `cold`, `warm`, `programs`, `cli`, `all`, `summarize`; records `ignite-performance/1`, `docs/performance/README.md`) and `verify-llvm-readiness`; replaced `scripts/*.py`; not linked into compiled programs |
| `std/` | standard library in Ignite; module `std.a.b` is `std/a/b.ignite` |
| `std/console.ignite` | `Console`, `Reply`, `Action`: a line-oriented console application, independent of the protocol that carries it; `CommandConsole` (the declared form: `implementation: Module<ConsoleCommands>`, `command_schema`), `console_of`, `command_names` |
| `std/text.ignite` | text operations called by UFCS (`split`, `contains`, `to_lower`, `is_digit`, …) |
| `std/option.ignite`, `std/result.ignite` | UFCS helpers on `Option` and `Result` (`unwrap_or`, `map`, `and_then`, `ok_or`, `map_err`, `ok`, …); the shared names make a glob import of both E041 at use |
| `std/json.ignite` | `Json` (RFC 8259): integers that fit `Int` as `Number(Int)` (`-0` excepted), every other number as `Decimal(Text)` of its exact text, `decimal(text)` the checked constructor (`docs/slice.md` 19.20); `encode`, `parse` |
| `std/net.ignite` | `Endpoint { address, port }`: the client-side target a bare `TcpListener` resource name gives where `Endpoint` is expected (slice 10); the module is also the substrate builtins `TcpListener`, `TcpStream`, `Exposure` and, since slice 14, the interface `ByteStream`, the test-only `Pipe.pair()` and its ends `PipeStream` (`docs/slice.md` 18.3, 18.4); `Endpoint.of(ref)` is the substrate builtin that resolves a `Ref` through `ignite run`'s `--ref` arguments (16.9); since the network builtins story a `TcpStream` has `peer()` (an `Endpoint`), `timed_out()` (the last read ended by the idle timeout) and `TcpStream.connect_within(address, port, timeout)` (`docs/slice.md` 26) |
| `std/fs.ignite` | `std.fs`, the capability `Directory` (`docs/slice.md` 21.4): its builtin methods (`read_file`, `write_file`, `read_directory`, `create_directory`, `remove`, `remove_tree`, `absolute_path`), the helpers `read_text_file`, `write_text_file`, `join_path`, the test-only `Directory.temporary()`; base layer |
| `std/process.ignite` | `std.process`, the capabilities `Launcher` and `ChildProcess` (`execute`, `start`, `id`, `wait`, `kill`; `Invocation` with its stdin `input`, `ChildExit`, `Ending`, `outcome`), `Entropy` (`random_bytes`), `Setting` (`value`), `CommandLine` (`arguments`) and `Environment` (`get`) (`docs/slice.md` 21.5 to 21.7, 21.19, 21.20; the statics `Clock.sleep`, `Clock.now` and `Process.flush` are documented there too); test-only `Launcher.new()`, `Entropy.new()`, `Setting.of(flag, value)`, `CommandLine.of(arguments)`, `Environment.of(entries)`; base layer |
| `std/rfc.ignite` | `std.rfc`, the vocabulary of requirements (`docs/slice.md` 19.3): `Level`, `Party`, `Source`, `Requirement`, `Spec`; a leaf, like every `std.rfc.*` module; declares no fn |
| `std/rfc/` | the RFC catalogues, one module per RFC (`std.rfc.rfc854`, `rfc855`, `rfc1143`, `rfc9110`, `rfc9112`), and the catalogues of specifications that are no RFC, named by a text `id` (`std.rfc.jsonrpc_2`, `language_server_3_17`; `docs/slice.md` 19.19): one `Spec` constant and one `Requirement` constant per requirement, all literal data (E091); `rfc9110` and `rfc9112` written by `ignite rfc import`, the three older RFCs quoted by hand (`source = Prose`); a protocol names them in `conforms` |
| `std/sli.ignite` | the Telnet probe's outcome `ProbeAttempt` and its predicates `prompt_received_before_timeout` and `prompt_received_within_threshold` (slice 18, `docs/slice.md` 22.3), `Sample` (one emission: its slot and the printed line, `None` on a timeout) and `Verdict` (the slot and whether a contract's `classify` held), windows, missing data and the meets check; the leaf module that `std.control`, `std.contract` and `std.protocol.telnet` import |
| `std/time.ignite` | civil time for calendar windows, a leaf: `month_start` and `next_month_start` of an instant in `UTC` or `Europe/Berlin` (EU summer-time rule), `zone_known` |
| `std/control.ignite` | the pure reducers `ignite run` steps (`docs/slice.md` 16.5 to 16.12): `Machine` (`Supervise`, `Schedule`, `Evaluate`, `Roll`), `Event`, `Effect` (since slice 18 `AttemptDone` carries the printed line, `Emit` a `std.sli.Sample` and `Emitted` a `Verdict`; the driver classifies at routing, 22.3), `Standing`, `Fired`, `step(machine, event) -> Step`, `companions(machine)`; imports only the leaves `std.sli`, `std.policy`, `std.time` (O45), tested inline with synthetic event lists; `crates/ignite/tests/fixtures/control_trace.ignite` steps the same reducers in the evaluator and on both targets (`control_trace::reducers_agree_across_the_evaluator_and_both_targets`) and times A7 (`control_trace::a7_evaluator_steps_10000_emissions_into_a_30_day_window`) |
| `std/protocol/telnet.ignite` | `protocol Telnet`: declares `conforms [RFC854, RFC855]` (`docs/slice.md` 19.5; `ignite conformance` prints the matrix and whether each claim holds; both hold since 19.21: IAC GA after every prompt and AYT answer, the Synch sent on Ctrl+C and honoured by the server through the stream's urgent data, Ctrl+D sends a partial line); server role `serve(stream, limits: SessionLimits, application)`, client role `connect`, both generic over `S: ByteStream` (`docs/slice.md` 18.6); its session tests run over a pipe, one stays on TCP; the executable kinds `TelnetServer` (accept loop, `SessionLimits`, `TelnetProfile`), `TelnetClient` (exit 3 and 4) and the probe `TelnetPrompt` (`main` runs one attempt, the probe process of `docs/slice.md` 16.8; `machine` its scheduler) |
| `std/protocol/http.ignite` | `protocol Http`: declares `conforms [RFC9112, RFC9110]`, scoped to the parties a server is; server role `serve(stream: T, limits: HttpLimits, application: HttpApp<S>)`, generic over `T: ByteStream`; `Request`, `Response` (a `Content` that is `Inline` text, `Binary` bytes or `Streamed`: pieces from `HttpApp.piece`, chunked for HTTP/1.1, `docs/slice.md` 19.23), `HttpLimits`; the executable kind `HttpServer<S>` with the hooks `machine` (supervisor, `recovery`, `rollout`) and `validate` of `docs/slice.md` 16.16, so the standard `Deployment` runs it as a `nativeProcess`; its session tests run over a pipe, one also on TCP |
| `std/protocol/jsonrpc.ignite` | `protocol JsonRpc`: declares `conforms [JSONRPC_2]` for the parties a server is, batches unsupported (`docs/slice.md` 19.19); an id that is a `Decimal` comes back as sent (`DecimalId`); Content-Length framing generic over `T: ByteStream` (`frame`, `write_message`, `read_message` returning `Frame`, `split_frame` the same framing over bytes already read for a reader that is no `ByteStream`, typed `FrameError`, bounded by `JsonRpcLimits`; a body that is not UTF-8 is the parse error, `is_utf8`, `decode_body`), JSON-RPC 2.0 messages through `std.json` (`Message`: `Call(Request)`, `Notify(Notification)`, `Reply(Response)`; `Id`, `ErrorObject`, the standard codes; `encode_message`, `decode`); server role `serve(stream, limits, application: JsonRpcApp<S>)`, whose `call` handler returns the outcome a request is always answered with and whose `notify` handler returns none; tests over a pipe |
| `std/contract.ignite`, `std/policy.ignite`, `std/deploy.ignite`, `std/cli.ignite` | the declaration vocabulary of design section 11.3 (`SLA`, `Availability`, `Restart`, `DrainRollout`, `Deployment`, `Application`, …; the Telnet probe `TelnetPrompt` is in `std/protocol/telnet.ignite`), checked and tested; `std.deploy`'s `Member`, `admits` and `includes` are what the compiler calls to resolve an assembly (`docs/slice.md` 15.6); since slice 12 each runnable kind has the hooks `machine` and `validate` of 16.16 (`ignite run` steps the machines, `ignite check` reports `validate`'s reasons as E120) and `std.deploy.entries` names the entries a binding needs (E129); since slice 18 a contract kind is generic over its probe's outcome type, `good` is a predicate `fn(T) -> Bool` and the hook `classify(self, sample: T) -> Bool` applies it (22.3; E150, E151) |
| `examples/terminal/terminal.ignite` | the terminal system in component form (`CommandConsole`, `TelnetServer`, `TelnetClient`); `ignite build` emits `terminald` and `tn` |
| `examples/terminal/commands.ignite`, `command_schema.ignite` | sibling modules of the terminal: the console's commands (`respond`) and their names (`commands`), filled into the declaration by `module(..)`; `commands.ignite` ends with the test "the terminal transcript over a pipe", slice 9 item 4's transcript through `Telnet.serve` over `Pipe.pair()` (`docs/slice.md` 18.0 row 3); tests elsewhere rewrite its `lines = ["ready"]`, so that test writes `ready` as bytes |
| `examples/terminal/terminal.design.ignite` | the full declaration from design section 11.3; `ignite check` passes it (`cli::check_design_file_is_silent`); `ignite run` builds it; `task compile` builds `terminal.ignite` |
| `examples/todo/todo.ignite` | the todo system, durable since slice 18 (`docs/slice.md` 22.4): an HTTP JSON backend whose memory `Store<Todo>` is the authority inside a process and whose every change is written as `<shard>/<id>/<version>.json` under the `Directory` `todo-data` (relative to the working directory of the process that starts `todod`; a deletion is a tombstone file; the first request loads the newest versions); `TodoService` with the assembly `local`, the `Availability` contract `reachable` over the probe `health`; known limit: two processes over one directory during a rollout's drain window can lose or hide a write acknowledged by the old one (22.2), and writes do not `fsync`; `ignite build` emits `todod` and `health` |
| `examples/todo/health.ignite` | `examples.todo.health`, the HTTP probe kind `HttpHealth` declared outside `std/` (22.6): its outcome type `HealthCheck { answered, status, latency }`, the predicate `healthy` a contract names in `good`, `encode_check` (one line per attempt in the model encoding); the request is written by hand (D13) |
| `github.com/ignite-lang/ignite-lsp` | the language server `ignite-lsp` (written in Ignite), its generated LSP types, their generator `ignite-lsp-types`, the vendored LSP `metaModel.json` and its end-to-end test live there since story:lsp-repository; it builds against a released `ignite` |
| `examples/typed/` | `sample.ignite`, a small program, and `reader.ignite`, which reads the typed document of `sample.ignite` (`ignite emit --typed-json`, `docs/slice.md` 24) with `std.json` in an inline test; the document is checked in as the raw string `SAMPLE`, kept equal to the compiler's output by `typed_json::the_reader_fixture_is_the_sample_document` (`IGNITE_BLESS=1` rewrites it) |
| `dist/toolchain/` | `Dockerfile` of the toolchain image `ghcr.io/ignite-lang/ignite-toolchain:<version>`, one per release (AGENTS.md § Releases) |
| `docs/rfc/` | the RFCXML sources of RFC 9110 and RFC 9112, vendored unmodified for `ignite rfc check` in the gate; `README.md` states their licence (`docs/slice.md` 19.9, D5) |
| `docs/vision.md` | direction: target architecture, sequencing of waves and slices, contradictions between documents, open and taken decisions |
| `PLAN.md` | the roadmap in outline: slices, waves, state; reasoning stays in `docs/vision.md` |
| `docs/` | design document, slice contract, `llvm.md` (LLVM target); `friction.md` (language friction backlog); research: `research/lean.md`, `research/ess.md` (comparisons), `research/review-2026-11-04.md` and `research/review-2026-11-04-ecosystem.md` (external reviews and what the plan does with them); future designs, not implemented: `serde.md` (data formats: one value model, compiler-derived codecs, JSON, YAML, TOML), `web.md` (the web framework `x.web`: the web app as a system declaration with derived routes, JSON, OpenAPI, live pages, caching and contracts; markup literals and the other language extensions it needs; a second root `x` beside `std`), `wasm.md` (WebAssembly target, runtime in Ignite, hot reload), `actors.md` (typed actors, pub/sub, supervision), `protocols.md` (RFC-bound protocols, pluggable transports) |
| `.engineering/` | AEP planning store (`aep plan artifact list`): epics and stories of live work, from slice 13 on; slices 1 to 12c stay in `PLAN.md` |
| `site/` | the website: `index.html` and `assets/` (the landing page, static HTML, CSS and vanilla JS) and `book/` (mdBook 0.5.4: guide, language design, reference; theme in `book/theme/`, Ignite highlighting in `book/theme/highlight.js`); every `ignite` block checks or is marked `fragment`, `fails=Ennn` or `dir=PATH`, and no page names `target/debug/ignite` (`site::` in `crates/ignite/tests/site.rs`); deployed to GitHub Pages by `.github/workflows/pages.yml` on push to `main` |
| `ux/` | the website's UI loop (story `docs-site-ui-loop`, `ux/README.md`): `adapter.ts` (viewports, themes, always-visible controls), `scenarios/` (the page list and the tasks a reader does from the landing page), Playwright TypeScript specs `tests/` (L2 layout contract with axe-core, L3 screen-only user, break-tests), `baseline.json` (the ratchet, checked by `site::ux::ratchet` in `crates/ignite/tests/site/ux.rs`), `findings/UXF-nnn.md`; rounds: `scenarios/cards/` (two anchor cards and one rotating card per round), the trial `round/sessions.spec.ts` with `round.config.ts`, `rounds/R-nn/` (`report.md`, `round.json`, `shots/`, written by `site::round::write` in `crates/ignite/tests/site/round.rs`; the reviews and `triage.md` beside them) and `rounds/trend.md`; `node_modules/` is ignored, a link to one shared offline install per lockfile (`task ux-install` is the one networked step) |

Rules of the layout:

- The only hand-written Rust a compiled program runs is `crates/ignite-io` (Rust target) or
  `crates/ignite-rt` over `ignite-io` (LLVM target). Protocols,
  consoles and anything else a program needs are Ignite in `std/` or in the program.
- The substrate is implemented once, in one language, and every target calls the same
  functions. A target differs only in how it suspends a task (Rust futures, LLVM coroutines),
  not in how it does IO. State since slice 4: `crates/ignite-io` is the only substrate, blocking
  (`std::net`, one OS thread per task, no tokio anywhere in the workspace); the Rust target
  calls it directly with synchronous generated code, and `crates/ignite-rt` is only its C ABI
  adapter (no socket, stdin or stdout code of its own). Slice 5 makes it async:
  tokio is the event loop inside `crates/ignite-io`, Rust-target futures run on it directly,
  LLVM-target coroutines run on it as tokio tasks (Timo, 2026-11-01; replaces the libuv
  candidate of design section 8.6).
- `examples/` holds `.ignite` files only.
- Nothing named telnet lives under `crates/`.
- A general layer knows no application protocol: the layers of std and the rule for
  `crates/*/src` are in `docs/layering.md`, enforced by `layering::` in
  `crates/ignite-check/tests/layering.rs`; change the table in both together. The substrate knows sockets and TLS sessions, not application protocols: a TLS engine (rustls) may live in `crates/ignite-io`, the `Tls<T>` layer is Ignite (Timo, 2026-11-01).

## Commands

| task | command |
|---|---|
| build everything | `task build` |
| all tests | `task test` |
| format and lint | `task check` |
| merge gate | `task gate` |
| pack the release binary | `task dist` (release `ignite` for Linux x86_64, glibc linked statically, with `std/` as `target/dist/ignite-VERSION-x86_64-linux.tar.gz`: `bin/ignite`, `share/ignite/std`; a driver at `<prefix>/bin/ignite` reads `<prefix>/share/ignite/std` first when it exists) |
| build and check the toolchain image | `task image VERSION=X.Y.Z` (`dist/toolchain/Dockerfile`, tags `ghcr.io/ignite-lang/ignite-toolchain:X.Y.Z` from release vX.Y.Z's archive and the tag's source), `task image-check VERSION=X.Y.Z [IMAGE=REF]` (`image::` in `crates/ignite/tests/image.rs`, ignored elsewhere: needs docker) |
| compile the example | `task compile` (`ignite build --target rust`; writes `target/ignite/terminal/`, prints `built …/terminald` and `built …/tn`) |
| run the compiled server | `task server` |
| run the compiled client | `task client` |
| compile the example with the LLVM target | `task compile-llvm` (writes `target/ignite-llvm/terminal/bin/`) |
| build a program | `target/debug/ignite build FILE [--target llvm\|rust] [--runtime ARCHIVE] [--out DIR] [--release] [--static]` (LLVM by default, `docs/vision.md` D2 B: clang links `libignite_rt.a`, the first of `--runtime`, the archive next to the binary, the one in its parent directory, and the one the binary carries (built by `crates/ignite/build.rs` with `--features runner`, written once to `$XDG_CACHE_HOME/ignite/runtime/<hash>/`, else `~/.cache`); exit 3 when that write fails; `--target rust` runs cargo; every command with `--target` defaults to `llvm`; `--static` links against musl on either target, `docs/slice.md` 23) |
| run the LLVM-built binaries | `task server TARGET=llvm`, `task client TARGET=llvm` |
| check a program | `target/debug/ignite check FILE [--json]`; an unsaved buffer on stdin: `target/debug/ignite check --stdin-for FILE [--json]` (`docs/slice.md` 1.1: `{"diagnostics":[{path, code, message, start, end}]}` on stdout, each position `line`, `column`, byte `offset`; exit 0 clean, 1 diagnostics) |
| print the typed program | `target/debug/ignite emit --typed-json FILE [--std DIR]` (`docs/slice.md` 24: one compact JSON document, format `ignite-typed/0.1`, on stdout; exit 1 diagnostics, 3 a file could not be read) |
| run the inline Ignite tests | `task test-ignite` (std/ and examples/, both targets; `FILTER=text` selects tests); one file: `target/debug/ignite test FILE [FILTER]` (LLVM, the default target, with `--runtime` a `libignite_rt.a` built with `--features runner`; `--target rust` needs none) |
| build the website | `task site` (landing page at `target/site/index.html`, book at `target/site/book/`; needs `mdbook` 0.5.4) |
| serve the website | `task site-serve` (`task site`, then `miniserve` on `http://127.0.0.1:8765/`) |
| run the website's UI loop | `task ux` (builds the site, runs the Playwright L2 and L3 specs against it on a free loopback port, then the ratchet against `ux/baseline.json`; `task gate` runs it; `IGNITE_BLESS=1` writes improved values into the baseline) |
| write a UI round | `task ux-round ROUND=R-nn` (not in `task gate`; the gating suite, then the trial of the scenario cards at every viewport with the screen-only user as the driver, then `site::round::write` into `ux/rounds/R-nn/` and `ux/rounds/trend.md`) |
| print the system model of a program | `target/debug/ignite plan FILE [--assembly NAME] [--out FILE] [--fuel N]` (canonical JSON, `docs/slice.md` 15.5; the design file's model is the golden `crates/ignite/tests/golden/terminal.design.model.json`) |
| explain one system, member, property or assembly | `target/debug/ignite explain FILE PATH [--assembly NAME]` (`Terminal.reachable`, `Terminal.server.recovery`, `local`; `docs/slice.md` 15.8) |
| run a system in the foreground | `target/debug/ignite run FILE [--assembly NAME] [--state DIR] [--build DIR] [--target llvm\|rust] [--release] [--clock real\|manual] [--epoch RFC3339] [--port RESOURCE=PORT] [--json] [--fresh]` (`docs/slice.md` 16.3; stdin takes `status`, `rollout <member>`, `stop`, `advance <quantity>`; state defaults to `target/ignite-run/<assembly>/`; exit 0 on a requested stop, 1 when a machine gave up, 2 on a diagnostic; `--fuel N` bounds each reducer step only, the plan inside `run` and `replay` keeps the default fuel) |
| replay a run's journal | `target/debug/ignite replay FILE [--assembly NAME] [--state DIR]` (re-steps the reducers over the journaled events, compares every effect and standing record, E127, prints the final standings) |
| host a state directory | `target/debug/ignite host --state DIR` (`docs/slice.md` 17; owns the state lock and the control socket until stopped; exit 2 when another host or run holds the state, 3 on a host or storage failure) |
| save a plan for apply | `target/debug/ignite plan FILE --for-apply --state DIR [--assembly NAME] [--target llvm\|rust] [--release] [--runtime ARCHIVE] --out PLAN` (needs the running `ignite host` of DIR, exit 3 without one; source-bound plan, `ignite-apply-plan/0.1`) |
| apply a saved plan | `target/debug/ignite apply --state DIR PLAN` (waits for completion; exit 0 completed, 1 failed or recovery required, 2 rejected, 3 host or storage failure) |
| inspect a host | `target/debug/ignite inspect --state DIR [--operation ID] [--json]` (live status, or the durable snapshot labelled offline; since slice 18 offline also reports a damaged host journal and its `nextAction`, `docs/slice.md` 22.7) |
| recover a host no corrective resolves | `target/debug/ignite recover --state DIR --operation ID [--json]` (`docs/slice.md` 22.9: needs the running `ignite host`; stops every instance of the state and restarts the committed revision, the operation becomes `abandoned`; exit 0 `recovered <id>: …`, 1 refused (an unaccounted process) or not ready within 60 s, 2 when the operation requires no recovery or is being recovered) |
| import an RFC catalogue | `target/debug/ignite rfc import FILE [--sections LIST] [--package NAME] [--url URL] [--out FILE]` (a local RFCXML v3 file, no network; `docs/slice.md` 19.8; exit 2 on a usage error or malformed XML, 3 on a file error); each catalogue's header names the command that wrote it |
| check a catalogue against its source | `target/debug/ignite rfc check CATALOGUE [SOURCE]` (exit 0 silent when the digests match, 1 when they differ; without SOURCE prints `<CATALOGUE>: unchecked`) |
| print a conformance matrix | `target/debug/ignite conformance FILE [--target llvm\|rust] [the build flags of ignite test]` (`docs/slice.md` 19.10: runs FILE's tests once, one block per protocol and spec, a claim line each; exit 0 every claim holds, 1 one does not, 2 a diagnostic or no conforming protocol, 3 a test build or run failure) |

`task` is go-task. Rust toolchain: stable, edition 2024, `rustfmt` defaults. Every cargo
invocation in automation sets `CARGO_INCREMENTAL=0`. `task test-ignite` honours
`CARGO_TARGET_DIR` (binary, runtime and outputs under it); `task compile-llvm` and
`task compile-todo-llvm` build and pass the runtime under it too, and write their programs to
`target/ignite-llvm/` regardless.

## Gate time

The budget of `docs/vision.md` invariant 9; each slice re-measures and adds its row. 2026-11-01,
slice 9b (one test executable per target, one integration-test binary per crate), 20 cores
shared with other sessions' builds (load average 2 to 18), sccache on, a fresh
`CARGO_TARGET_DIR` after `cargo clean`. Before and after are two copies of one tree that differ
only in 9b. Gate = `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo test --workspace --no-fail-fast`, `task test-ignite`, run one after another.

| measurement | command | before 9b | after 9b |
|---|---|---|---|
| gate, cold | the gate with `SCCACHE_RECACHE=1` after `cargo clean` | 129.9 s | 86.5 s |
| gate, warm | the gate after `touch crates/ignite-check/src/lib.rs` | 35.4 s | 25.9 s |
| test executables | `cargo test --workspace --no-run` | 49, 666 MB | 18, 202 MB |
| integration-test binaries | same | 39 | 8, one per crate |
| `--no-run`, dependencies cached | same, after `cargo clean` (3 runs before, 2 after) | 3.5 to 18.9 s | 4.6 to 7.0 s |
| `--no-run`, warm | same, repeated | 0.0 to 0.4 s | 0.0 s |
| `--no-run` after a touch | same, after `touch crates/ignite-check/src/lib.rs` | 1.8 to 2.2 s | 1.3 to 1.4 s |
| `task test-ignite` in the cold gate | `task test-ignite` | 12.3 s, 6 executables | 18.4 s, 2 executables |
| `ignite test` of std/ and examples/, cold, Rust | fresh `--out` and `--target-dir`, `SCCACHE_RECACHE=1`, 2 runs | 5.2, 8.4 s | 4.0, 4.1 s |
| same, LLVM | same with `--target llvm` | 3.8, 4.1 s | 2.7, 2.3 s |
| `task test-ignite`, warm | nothing changed | 1.3 s | 0.6 s |

- Test counts, both trees: 520 cargo tests run (518 passed, 2 LLVM goldens failing on slice
  9c's work in progress), 3 ignored; 78 inline tests per target.
- The cold-gate `task test-ignite` row also holds `cargo build -p ignite` and the runtime build;
  how its 6.1 s rise splits between them and load was not measured. Inferred, not measured:
  before 9b the step ran a stale `target/debug/ignite` and skipped the compiler build; it now
  builds `ignite` and the runtime first, so the rise is not a regression of the test build.
- Program builds (steering session, 2026-11-01 18:35, the todo example, cold): Rust target
  3.0 s dev, 3.9 s release; LLVM target 0.8 s dev, 0.9 s release (emit, clang, link);
  `libignite_rt.a` once 3.0 s dev, 4.7 s release.

Slice 10 (component form, 2026-11-02 01:01, P4), one tree, 20 cores shared with other
sessions (load average 11 to 23), sccache on, own `CARGO_TARGET_DIR`; "before" is the
9b column above.

| measurement | command | before (9b) | slice 10 |
|---|---|---|---|
| gate, cold | `cargo clean`, `SCCACHE_RECACHE=1`, the gate | 86.5 s | 72.3 s (fmt 0.9, clippy 7.4, test 55.5, test-ignite 8.5) |
| gate, warm | after `touch crates/ignite-check/src/lib.rs` | 25.9 s | 45.5 s (fmt 0.4, clippy 2.1, test 41.7, test-ignite 1.3) |
| test executables | `cargo test --workspace --no-run` | 18, 202 MB | 18, 210 MB |
| `--no-run`, warm | same, repeated | 0.0 s | 0.1 s |
| `--no-run` after a touch | same, after the touch | 1.3 to 1.4 s | 3.5 s |
| todod, Rust | `ignite build examples/todo/todo.ignite`, fresh `--out` and `--target-dir`, `SCCACHE_RECACHE=1`, 2 runs | 3.0 s dev, 3.9 s release | 4.0 and 3.3 s dev, 4.4 and 4.4 s release |
| todod, LLVM | same with `--target llvm`, 2 runs | 0.8 s dev, 0.9 s release | 0.4 and 0.4 s dev, 1.2 and 1.2 s release |

- Test counts: 552 cargo tests passed, 0 failed, 3 ignored; 97 inline tests per target (78 before;
  the 19 new ones are std/policy 4, contract 3, telnet 3, probe 2, deploy 2, cli 2, console 2,
  examples/terminal/command_schema 1).
- The warm gate rose 76 %: its `cargo test` step is 41.7 s, of which 39.0 s is test execution
  (`crates/ignite` end-to-end binary 33.2 s, `ignite-rustgen` 3.4 s, read from the harness'
  `finished in` lines), not compilation. The end-to-end suite grew in slice 10 (failover, slice10
  crash, llvm_memory, syntax on both targets, siblings, cli, the todo suite), and several of its
  tests build a program with cargo or clang inside the test. Measured at load 21; the 9b column
  was measured at load 2 to 18.
- LLVM release rose 33 %: `clang -O2` on the generated `std.ll` (18,120 lines, every std module)
  takes 0.84 s of the 1.2 s, measured on the build's output; `program.ll` and the main take 0.08 s
  each. Inferred, not measured against a "before" IR (the slice-9 std does not compile with the
  slice-10 compiler): the rise is the std modules slice 10 added (`contract`, `policy`, `probe`,
  `deploy`, `cli`, the kinds in `console`, `telnet`, `http`), which the std module carries whether
  the program uses them or not; `--gc-sections` drops them from the executable but after
  optimisation.
- Rust release rose 13 % and dev 10 to 33 %, with the same inferred cause: the generated
  `ignite-std` crate is 2,126 lines and holds every std module.
- `abi::release_closes_the_socket_and_invalidates_the_handle` (`ignite-rt`) failed once in six
  runs of the suite, in the warm gate: rebinding the released listener's port gave `Address
  already in use`. Inferred, not verified: another process on the host took the ephemeral port
  between release and rebind.

Slice 11 (plan and explain, 2026-11-02 03:00 to 03:10, P4), 20 cores, load average 0.8 to 4.7
throughout, sccache on. Method (`docs/slice.md` 15.1): "before" is a `git archive 60aef38` copy,
"after" a copy of the finished slice-11 tree, each with its own `CARGO_TARGET_DIR`; the cold gate
once per tree in a fresh target dir with `SCCACHE_RECACHE=1`, then three interleaved passes, each
running every warm unit once on the before copy and once on the after copy; the 1-minute load is
recorded per run, a pair whose two loads differ by more than 5 is discarded (none was), and the
ratio is the median of the pair ratios. Warm = after `touch crates/ignite-check/src/lib.rs`.
Scripts and raw logs: `docs/performance/evidence/telos/telos-f769e68e/p4-scripts/` and `p4-logs/`.

| measurement | command | before (60aef38) | slice 11 | ratio |
|---|---|---|---|---|
| gate, cold | fresh target dir, `SCCACHE_RECACHE=1`, the gate (1 pair) | 64.8 s (fmt 0.4, clippy 4.2, test 51.9, test-ignite 8.2) | 54.5 s (0.5, 4.3, 41.6, 8.2) | 0.84 |
| gate, warm | the gate after the touch (3 pairs) | 25.9 s (25.9 to 26.2) | 16.8 s (16.8 to 17.1) | 0.65 |
| warm `cargo test --workspace` step | same (3 pairs) | 22.9 to 23.1 s | 13.5 to 13.8 s | 0.59 |
| warm fmt, clippy, test-ignite steps | same | 0.38, 1.98, 0.65 s | 0.45, 2.19, 0.68 s | 1.16, 1.11, 1.03 |
| test executables | `cargo test --workspace --no-run`, `Executable` lines | 19, 220 MB | 21, 246 MB | |
| `--no-run`, warm / after the touch | same, repeated / after the touch (3 pairs) | 1.01 / 1.45 s | 1.03 / 1.61 s | 1.02 / 1.21 |
| `crates/ignite/tests`, per file | `cargo test -p ignite --test main <file>::` warm (3 pairs) | slice9 17.5, ignite_test 9.1, terminal 2.1, llvm_build 2.1, slice10_llvm 0.7, cli 0.6, failover 0.6 s; the other 7 files 0.6 s together | slice9 3.7, ignite_test 8.6, terminal 2.1, llvm_build 1.5, slice10_llvm 1.0, cli 0.6, failover 0.6; eval 0.4, plan 0.7 (new) | 0.21, 0.95, 1.00, 0.70, 1.22, 0.95, 1.03 |
| todod, Rust dev | `ignite build examples/todo/todo.ignite`, fresh `--out` and `--target-dir`, `SCCACHE_RECACHE=1` (3 pairs); split = separate builds of `ignite-io`, `ignite-std`, the program crate in one fresh target dir | 3.19 s (io 2.37, std 0.37, program 0.26) | 3.19 s (2.36, 0.36, 0.26) | 1.00 |
| todod, Rust release | same with `--release` | 4.12 s (io 2.79, std 0.65, program 0.51) | 4.19 s (2.83, 0.66, 0.51) | 1.02 |
| todod, LLVM dev | same with `--target llvm`; split = the build's clang and link commands rerun by hand | 0.35 s (clang std 0.08, program 0.02, main 0.02, link 0.11) | 0.33 s (0.06, 0.02, 0.02, 0.11) | 0.94 |
| todod, LLVM release | same with `--release` | 1.10 s (clang -O2 std 0.78, program 0.08, main 0.06, link 0.06) | 0.80 s (0.48, 0.08, 0.06, 0.05) | 0.73 |
| `ignite check` of the design file | `ignite check examples/terminal/terminal.design.ignite` (15 pairs) | 0.05 s | 0.06 s | 1.2 |
| `ignite plan`, `ignite explain local` of the design file | `ignite plan …` with `--out`, `ignite explain … local` (15 runs each, load 2.2 to 4.0) | | 0.06 s (0.06 to 0.07), 0.06 s | |

- Test counts, cargo tests plus the inline tests of both targets: 746 passed (552 cargo + 2 × 97
  inline), 3 ignored before; 784 passed (580 cargo + 2 × 102 inline), 4 ignored after
  (`#[ignore]`: the two 100,000-term chain tests, the LLVM `bench`, the new 100,000-term
  lowering); 97 inline tests per target before, 102 after (std/deploy +5). `cargo test
  --workspace` on the working tree at the close: 580 passed, 0 failed, 4 ignored, 13.98 s at load
  3.3; `task gate` 13.4 s warm.
- Slice 10's carry-overs (P0 on 60aef38, raw logs in `docs/performance/evidence/telos/telos-f769e68e/p0-logs/`): the warm
  gate is 25.98 s at load under 5 (fmt 0.40, clippy 2.05, test 22.88, test-ignite 0.65) and 75.4 s
  at load 18 to 44, so slice 10's 45.5 s was load, not a regression. Of the warm test step the
  `crates/ignite` end-to-end binary was 18.69 s (82 %): `slice9.rs` 17.4 s serial behind its
  `BUILDS` mutex, of which the inline test `boxed values carried across releases and through the
  store` (`fixtures/slice9/regions.ignite`) was 9.1 s (6.8 s Rust dev, 2.3 s LLVM -O0). todod LLVM
  release 1.16 s: `clang -O2` on `std.ll` 0.834 s (70.5 %), emit 0.113, `program.ll` 0.084, main
  0.064, link 0.063. todod Rust dev 3.04 s (the 3.0 s baseline); release 4.56 s against 3.9 s: libc
  and nix 52 % (recompiled every run under `SCCACHE_RECACHE=1`), `ignite-std` 16 %, the program
  crate 12 %, accepted as the new baseline. The flake
  `failover::a_failing_session_does_not_end_the_service`: 0 failures in 30 runs alone and 30
  beside a full workspace run (P0 on 60aef38); after P3's rewrite (a deadline instead of one read
  after `kill`) again 30/30 alone (P3) and 30/30 beside a workspace run (P4, load 2.5).
- Fixes in slice 11: `slice9.rs` 17.5 s to 3.7 s (the boxed test's sizes 2,000 inserts and 5,000
  links to 500 and 1,000, 6.8 s to 0.31 s Rust and 2.3 s to 0.09 s LLVM with the same assertions;
  `long_chains_lower_quickly` 100,000 to 10,000 terms, 3.5 s to 0.4 s, the 100,000-term lowering
  now `#[ignore]` as `long_chains_100k_lower_quickly`; the 14 audit probes built as two `ignite
  test` groups per target instead of 28 subprocesses, 7.2 s to 1.9 s). `llvm_build.rs` 2.1 to 1.5 s
  and todod LLVM release 1.10 to 0.80 s from P3's reachable-only std emission (`std.ll` 222 to 107
  defines; `clang -O2` on it 0.78 to 0.48 s). The steering target of a warm test step under 8 s is
  not met: 13.5 s at load 3, of which `ignite_test.rs` is 8.6 s (the end-to-end files run in
  parallel, so the binary's wall is its longest file; `timeout_kills_a_test` 2.6 s,
  `overflow_fails_the_same_everywhere` 1.7 s, each spawning `ignite test` per target and profile),
  the rebuild after the touch 1.6 s, the other crates' binaries about 3 s.
- Slice 11b (2026-11-02, the four target bugs and the warm test step; 3 interleaved passes over
  the 60aef38 copy, the slice-11 close tree and the 11b tree, load 6 to 9): warm `cargo test
  --workspace` step 10.8 s median (10.3 to 17.8 s) against 16.1 s for the slice-11 tree in the
  same passes (ratio 0.70); `ignite_test.rs` 8.6 s to 4.6 s (the two targets on two threads, one
  shared basics run). The target "under 8 s at load under 5" was not checked: the load never fell
  under 5. Remainder: the end-to-end binary 5.6 s, the rebuild after the touch 1.8 s, the
  rustgen tests 1.7 s. todod against the 60aef38 copy in the same passes: Rust dev 3.35 s
  (ratio 1.00), release 4.74 s (1.03); LLVM dev 0.35 s (0.95), release 0.88 s (0.70). Cargo
  tests 582 passed, 0 failed, 4 ignored; inline tests 102 per target.
- Rises over 10 %: fmt +16 % (70 ms) and clippy +11 % (0.2 s) from the new crate `ignite-eval`;
  `--no-run` after the touch +21 % (0.16 s) because `ignite-eval` depends on `ignite-types` and
  recompiles with it; `ignite check` of the design file +20 % (10 ms, at 10 ms resolution): O20
  evaluates every property and resolves the assembly at check time; `slice10_llvm.rs` +22 %
  (0.69 to 0.98 s, after-tree range 0.69 to 1.00 s: one slow run, load noise, inferred); test
  executables 19 to 21 (+12 % size): `ignite-eval`'s unit and integration test binaries.
- Flake seen in the measurement: `llvm_build::default_runtime_is_next_to_the_driver_or_in_its_parent`
  failed once per tree in 8 gate runs with `Text file busy` at the exec of the copied driver (the
  copy's write descriptor inherited by another test thread's fork for an instant); the test now
  retries the exec on ETXTBSY. Pre-existing on 60aef38, not a slice-11 regression.

Slice 12 (`ignite run`, 2026-11-02 06:51 to 06:58, P4), 20 cores shared with other sessions'
test runs (load average 10 to 40 throughout; 3 to 6 for the before tree's cold gate at 05:43),
sccache on. Method as slice 11 (`docs/slice.md` 16.1): "before" is the coordinator's
`s12-before` copy of the slice-12 start tree, "after" the snapshot `s12-p4-after2` of the closed
tree (every test green), each with its own `CARGO_TARGET_DIR`; one cold gate per tree, then three
interleaved passes; a pair whose two 1-minute loads differ by more than 5 is discarded. No run
of either tree happened under load 5. Scripts and raw logs:
`docs/performance/evidence/telos/telos-f769e68e/s12-p4-scripts/` and `s12-p4-logs/measure/`; the P2 and P3 paths below (`p3-measure/`, `s12-p2-bench-unit1cold/`) are in the same directory (`pairs2.txt` is the summary; `pairs.txt` the
earlier set over the snapshot that still held 4 failing tests, see the note below).

| measurement | command | before (s12-before) | slice 12 | ratio |
|---|---|---|---|---|
| gate, cold | fresh target dir, SCCACHE_RECACHE=1, the gate (1 pair, discarded by the load rule: loads 3.1 and 14.6) | 57.1 s (fmt 0.47, clippy 4.61, test 43.54, test-ignite 8.45) | 78.1 s (0.52, 6.54, 60.43, 10.57) | 1.37, not valid |
| gate, warm | the gate after the touch (pairs 2 and 3 kept, loads 27 to 35; pair 1 discarded, loads 11 and 34) | 23.5 s (19.8, 27.3) | 33.1 s (29.5, 36.7) | 1.42 (1.35, 1.49) |
| warm `cargo test --workspace` step | same (pairs 2 and 3) | 17.5 s (14.4, 20.6) at load 28 to 31 | 26.3 s (23.1, 29.5) at load 27 to 31 | 1.52 (1.43, 1.61) |
| warm fmt, clippy steps | same (pairs 2 and 3) | 0.86, 4.25 s | 0.81, 4.37 s | 0.95, 1.04 |
| warm `task test-ignite` step | same; every warm pair discarded by the load rule (loads 28 to 41), the cold pair (loads 6.0 and 10.6) shown | 8.45 s cold | 10.57 s cold | 1.25 (1 pair) |
| test executables | `cargo test --workspace --no-run`, `Executable` lines | 21, 247 MB | 21, 265 MB | |
| `--no-run`, warm / after the touch | same (3 pairs) | 1.13 / 1.89 s | 1.29 / 2.30 s | 1.03 / 1.26 |
| `crates/ignite/tests`, per file | `cargo test -p ignite --test main <file>::` warm (3 pairs, loads 15 to 27, from the earlier set over the snapshot whose `terminal` tests failed; the other files were not affected, `pairs.txt`) | ignite_test 6.6, slice9 3.9, llvm_build 2.5, terminal 2.1, slice10_llvm 1.4, plan 1.0, failover 0.9, cli 0.7, llvm_memory 0.6, eval 0.5 s | ignite_test 9.8, slice9 6.0, llvm_build 3.6, terminal failed, slice10_llvm 1.5, plan 2.3, failover 1.7, cli 0.7, llvm_memory 0.9, eval 0.8; new: run 3.0 (2.9 to 3.1), drain 1.9 (1.7 to 2.2), control_trace 0.7, decode 0.2 | 1.47, 1.50, 1.45, , 1.52, 2.46, 1.39, 0.90, 1.53, 1.54 |
| todod, Rust dev / release | `ignite build examples/todo/todo.ignite`, fresh `--out` and `--target-dir`, `SCCACHE_RECACHE=1` (3 pairs, loads 29 to 38) | 5.55 s (5.0 to 6.6) / 6.84 s (5.4 to 7.5) | 6.02 s (5.2 to 7.0) / 7.57 s (5.3 to 8.9) | 1.06 (0.93 to 1.21) / 1.20 (0.77 to 1.40) |
| todod, LLVM dev / release | same with `--target llvm` | 0.42 s (0.42 to 0.64) / 1.11 s (0.91 to 1.46) | 0.87 s (0.56 to 0.90) / 1.40 s (1.24 to 1.83) | 1.41 (1.33 to 2.07) / 1.26 (1.25 to 1.36) |
| same, P2 after unit 1 at load 1 to 4 | `s12-p2-bench-unit1cold/todod.log`, 3 pairs | Rust 3.32 / 4.25 s, LLVM 0.33 / 0.81 s | Rust 3.50 / 4.76 s, LLVM 0.50 / 0.98 s | 1.07 / 1.12, 1.52 / 1.23 |
| `ignite check`, `ignite plan` of the design file | 15 pairs each, loads 27 to 32 | 0.10 s (0.07 to 0.13), 0.10 s | 0.20 s (0.13 to 0.37), 0.19 s (0.13 to 0.23) | 1.86, 1.75 |
| `ignite run` to the first `ready` | P3, `p3-measure/run-ready-*`, warm build, manual clock, 5 runs | | 127 ms | |
| `ignite replay` of a 10,071-record journal | P3, `p3-measure/replay-*`, 5 runs | | 2.54 to 2.63 s | |
| A7: 10,000 emissions into a 30-day rolling window, evaluator | `control_trace::a7_evaluator_steps_10000_emissions_into_a_30_day_window` (`#[ignore]`; the gate runs the 1,000-step variant) | | 12.5 s, 1.25 ms per step | |

- Test counts: before 786 passed (582 cargo + 2 × 102 inline), 0 failed, 4 ignored; after 932
  passed (636 cargo + 2 × 148 inline), 0 failed, 5 ignored (A7 10,000 added), in the after2 cold
  gate and in the close gate below. Inline tests per target 102 to 148 (std/control 28, time 5,
  sli +1, contract +2, telnet +4, probe, deploy +1, policy +2). Test executables 21 before and
  after, 247 to 265 MB (+7 %).
- The measured snapshot's 4 failures were deterministic (3 of 3 reruns alone at load 13.6), no
  load flake, and are fixed at the close. `terminal::idle_terminald_runs_one_thread` and
  `terminal::{rust,llvm}::a_task_that_cannot_start_closes_only_its_connection`: the drain's
  `sigwait` thread (comm `drain`, `crates/ignite-io/src/drain.rs`, 16.13) is a second idle
  thread and counts against systemd's `TasksMax=2`, so the first session's thread got EAGAIN
  (P2, verified); the tests now expect 2 threads idle and 3 with a session and start the server
  under `TasksMax=3` (`crates/ignite/tests/terminal.rs` only, no timed path).
  `terminal::terminal_checks_clean_against_the_standard_library`
  (`crates/ignite-check/tests/terminal.rs:100`) listed the terminal program's functions before
  the 16.16 hooks existed; it now expects `TelnetServer.machine` and `TelnetServer.validate`
  beside the two mains. The working tree's gate at the close (06:47, load 10 to 24): fmt 0.51 s
  exit 0, clippy 0.84 s exit 0, `cargo test --workspace --no-fail-fast` 34.5 s exit 0 (636 passed,
  0 failed, 5 ignored), `task test-ignite` 2.30 s exit 0 (148 per target), `task gate` 28.1 s
  exit 0, `task compile` 0.47 s, `compile-llvm` 1.00 s, `compile-todo` 0.43 s, `compile-todo-llvm`
  0.47 s, all exit 0 (`s12-p4-logs/measure/finalgate.log.summary`).
- The warm test step's target (under 10.8 s, slice 11b at load 6 to 9) is not met: 23.1 and
  29.5 s at load 27 to 31, where the before tree took 14.4 and 20.6 s in the same pairs; the pair
  ratio 1.52 (1.43 to 1.61) is the slice's own growth, the rest of the absolute rise is load. The
  growth, measured per file in the earlier set at matched loads: `ignite_test.rs` 6.6 to 9.8 s
  sets the end-to-end binary's wall (its files run in parallel), `slice9.rs` 3.9 to 6.0,
  `llvm_build.rs` 2.5 to 3.6, `plan.rs` 1.0 to 2.3, the new `run.rs` 3.0 s and `drain.rs` 1.9 s
  (under O41's 6 s together), the rebuild after the touch 1.9 to 2.3 s. Their causes are in the
  next bullet.
- Rises over 10 %, measured causes: `ignite check` and `ignite plan` of the design file 1.86 and
  1.75 (0.10 to 0.20 s and 0.19 s at load 27 to 32; 0.06 to 0.12 s in the earlier set; P3: the std
  the CLI loads grew from 3,033 to 5,135 lines and model construction stays about 1 ms in both
  trees, `p3-measure/plan-*.log`). Every test that spawns the CLI pays that per spawn, which is
  the growth of the short per-file rows (plan, siblings, syntax, todo, llvm, eval: 0.2 to 1.3 s
  more each) and of `task test-ignite` (cold pair 8.45 to 10.57 s, 1.25, also 102 to 148 inline
  tests); the split between the two is inferred. Program builds pay the larger generated std:
  `ignite-std` 2,191 to 3,792 lines (P2, measured) for the Rust rows (todod Rust dev 1.06 and
  release 1.20 here, 1.07 and 1.12 at P2's load 1 to 4; ignite_test, slice9, llvm_build 1.45 to
  1.50, which build programs inside the tests, inferred); for LLVM (todod dev 1.41, release 1.26
  here, 1.52 and 1.23 at P2's load) the frontend on the larger std (P2: check 0.058 to 0.102 s
  measured, `clang -O2` on `std.ll` unchanged at 0.50 s and 107 defines measured, IR emission not
  measured, so the LLVM dev remainder is inferred). fmt 0.95 and clippy 1.04 in the valid pairs
  (the earlier set's 1.13 and 1.25 were load, inferred); `--no-run` after the touch 1.26 (0.4 s,
  earlier set): the new `crates/ignite/src/run/` (9 modules), `ignite-io` `drain` and `clock`,
  and the new test files (inferred from the source growth, not split).
- Cold gate, the one pair invalid by the load rule (3.1 against 14.6): 60.4 s against 43.5 s in
  the test step holds 146 more tests, among them the end-to-end `run` and `drain` suites that
  build the design file once per target with cargo inside the test (P3: builds shared per
  target, O41), at load 14.0 to 10.6 against 3.8 to 6.0 (inferred split); cold `task test-ignite`
  8.5 to 10.6 s (148 tests, the compiler build inside the step; inferred).
- Earlier set (06:30 to 06:43, `pairs.txt`, over the snapshot in which 4 tests still failed, loads
  6 to 28): gate cold 80.6 s (1.41), gate warm 30.8 s (2.22, 2 pairs), warm test step 14.9, 15.7,
  35.3 s after against 10.2, 10.7, 20.9 s before (all pairs void by the failure), fmt, clippy,
  test-ignite 1.13, 1.25, 1.67, todod 1.02 / 1.15 Rust and 1.64 / 1.20 LLVM, check and plan 0.06
  to 0.12 s (1.83, 1.85). Replaced by the rows above, kept for the record.

Slice 12b (2026-11-02, completed after recovering the interrupted batch). The 11b baseline
stays unchanged; the remaining timing exceptions are recorded rather than accepted as a new
baseline. Full evidence, named-test timings and attribution:
`docs/performance/12b/report.md`, `summary-final.md`, `records.jsonl` and the restart journals;
the Cargo timing HTML/JSON was not retained (`docs/performance/evidence/README.md`).

One exact-final cold before/final/slice-12 pass uses fresh owned targets and
`SCCACHE_RECACHE=1` (start loads 3.71/4.66/0.98); three interleaved warm passes compare `s12-before`, final and
`s12-p4-after2` (loads 5.1–7.0). A unit with start-load delta over 5 or a failed exit is
excluded. Absolute medians and medians of pair ratios are shown below. Final gate/cache
samples include the minimal ownership test and journal fix; unchanged program/CLI paths use
three program-build pairs and 15 check/plan pairs measured before those two changes. Earlier
sources, invalid cold comparisons and failed samples remain separately versioned; the final
cold comparison against each baseline meets the start-load rule and has no failed step.

| measurement | 11b → 12b; ratio | slice 12 → 12b; ratio |
|---|---|---|
| gate, cold | 57.141 → 66.111 s; ×1.157 (1 kept/0 discarded) | 68.727 → 66.111 s; ×0.962 (1 kept/0 discarded) |
| gate, warm after touch | 13.374 → 16.786 s; ×1.255 (3 kept/0 discarded) | 18.705 → 16.627 s; ×0.889 (2 kept/1 discarded) |
| warm cargo test --workspace | 10.142 → 12.413 s; ×1.233 (3 kept/0 discarded) | 14.594 → 12.313 s; ×0.844 (2 kept/1 discarded) |
| warm fmt | 0.447 → 0.523 s; ×1.169 (3 kept/0 discarded) | 0.523 → 0.523 s; ×1.009 (3 kept/0 discarded) |
| warm clippy | 2.166 → 2.713 s; ×1.271 (3 kept/0 discarded) | 2.508 → 2.713 s; ×1.042 (3 kept/0 discarded) |
| warm task test-ignite | 0.663 → 1.140 s; ×1.709 (3 kept/0 discarded) | 1.099 → 1.140 s; ×1.047 (3 kept/0 discarded) |
| todod Rust dev | 4.952 → 3.998 s; ×0.990 (3 kept/0 discarded) | 4.836 → 3.998 s; ×0.892 (3 kept/0 discarded) |
| todod Rust release | 5.611 → 4.740 s; ×0.843 (3 kept/0 discarded) | 5.371 → 4.740 s; ×0.934 (3 kept/0 discarded) |
| todod LLVM dev | 0.379 → 0.309 s; ×0.892 (3 kept/0 discarded) | 0.528 → 0.309 s; ×0.585 (3 kept/0 discarded) |
| todod LLVM release | 0.862 → 0.820 s; ×0.946 (3 kept/0 discarded) | 1.052 → 0.820 s; ×0.791 (3 kept/0 discarded) |
| check design file | 0.062 → 0.090 s; ×1.447 (15 kept/0 discarded) | 0.115 → 0.090 s; ×0.769 (15 kept/0 discarded) |
| plan design file | 0.063 → 0.091 s; ×1.437 (15 kept/0 discarded) | 0.117 → 0.091 s; ×0.778 (15 kept/0 discarded) |
| --no-run warm | 0.029 → 0.030 s; ×0.961 (3 kept/0 discarded) | 0.032 → 0.030 s; ×0.931 (3 kept/0 discarded) |
| --no-run after touch | 2.233 → 2.594 s; ×1.162 (3 kept/0 discarded) | 2.751 → 2.594 s; ×0.901 (3 kept/0 discarded) |
| ignite_test.rs file | 7.735 → 2.185 s; ×0.399 (3 kept/0 discarded) | 8.233 → 2.185 s; ×0.259 (3 kept/0 discarded) |

- Validation: 647 cargo tests pass, 0 fail, 5 ignored; 148 inline tests per target. All nine
  parent close steps pass, followed by three normal final warm gates and the exact-final
  cold gate. Slice 12's first final warm pass hit its existing shared-ARGS race and is
  discarded (2 valid gate/test pairs); that baseline was not patched. Actual executable
  lines count 20 per tree, about 247 MB before and 266 MB after; the earlier count 21 also
  matched Cargo's `target(s)` summary text.
- Generated todo `ignite-std`: 2,191 lines (11b), 3,792 (slice 12), 993 (12b). All four
  program-build ratios meet 1.10 against 11b. `ignite_test.rs` is 2.185 s median (range
  2.131–3.875), under 4.6 s; `report.md` names every isolated test timing and the filter-test
  rename. Isolated cases reset shared fixture setup, so their times are not additive.
- Frontend exception, separate disposable profiling (15 pairs): std parse/check/type is
  10.12/11.97/24.55 → 13.71/16.20/36.65 ms. Required `std.control` alone costs
  6.65/7.74/21.41 ms; unused HTTP/JSON disappear, but the design's kind hooks/imports retain
  reducers. Program parse/check/type stays about 0.58/0.72/1.85 ms; evaluation check is
  3.68 → 6.73 ms. Normal CLI output contains no profiling instrumentation.
- Warm workspace attribution (second final pair): Ignite E2E execution 4.93 → 4.91 s,
  rustgen integration 1.60 → 3.13 s (its helper still loads/types the full std per program),
  new driver unit tests 0 → 0.25 s. Inline tests grow 102 → 148 per target. Cold test/inline
  steps are 44.37 → 51.26 s and 8.05 → 9.68 s. These remain explicit exceptions; a finer
  causal split beyond the measured crate/phase figures is not claimed.
- Attribution before removal of redundant cache-test cases: package fmt `ignite`
  0.087 → 0.146 s, evaluator 0.070 → 0.087 s; runtime/rustgen unchanged. Warm clippy
  2.26 → 2.95 s; timed driver library 0.17 → 0.48 s, test library 0.19 → 0.50 s, E2E check
  0.52 → 0.72 s, evaluator 0.40 → 0.57 s, LLVM 0.67 → 0.80 s. Parallel unit durations
  are not additive; see retained Cargo timing artifacts.
- Five frontend tests and the cache regression cover transitive/module-literal roots,
  unused broken std and project ownership. Four E031/E032/E003 CLI fixtures explicitly
  import their broken modules; the emit fixture expects unused nested std absent. All 21
  std/example files check silently. Recovered `consts.std.rs`/`variants.std.rs` goldens only
  drop unreachable items; this completion changes no goldens. Removing the build claim
  mutation-checked the cache regression (wrong third program); restored minimal A/B/A
  coverage is 0.538 s median over three normal runs.
- Pre-fix integrated restart failures (1,341.69/869.97 ms) and the shared-ARGS test race are
  retained in the evidence. Three isolated restart passes alone had not established
  readiness. The runtime now avoids separate syncs for deterministic follow records;
  every originating effect is still synced before dispatch (O30), with unchanged journal
  order/bytes and replay. Final normal suite delays are 106.72, 107.28 and 104.51 ms;
  final cold delay 104.40 ms, all inside the unchanged 100–500 ms bound with replay passing.
  `restart-samples.json` links the copied journals. A test-only mutex fixes the ARGS race;
  syscall-traced HTTP failures are diagnostic interference, not readiness evidence.

- No comments in `Cargo.toml`, `Taskfile.yml` or any other config file.
- Tooling, checks and scripts are Rust (Timo, 2026-11-02): a `#[test]` in the crate that owns the rule, or a Rust binary. No new Python; Bash only for one-line invocations in `Taskfile.yml`. The former `scripts/*.py` are `tools/ignite-perf` and the serial `cargo fmt --all -- --check` of `task check` (`story:rust-tooling`); scripts under `docs/performance/` are recorded evidence and stay as they are.
- The repository, its releases and its packages are public (Timo, 2026-11-08): no company, customer, service or internal project names, hosts, paths, account ids or credentials in any file, commit message, PR, release note or image label. A case study is named by what it is ("a WebSocket push service").
- Commit titles follow Conventional Commits: `type(scope): summary` with `type` one of `feat`,
  `fix`, `perf`, `docs`, `test`, `refactor`, `chore`, `build`, `ci`; the scope is optional. Every
  commit has a body: a blank line after the title, then bullets on what changed and why.
- Reasoning lives in doc comments, `docs/slice.md` and commit messages, not in reports.
- `docs/slice.md` is the contract between the compiler crates. Change it and the tests together;
  do not let a crate's doc comment restate it.
- Public APIs of the compiler crates are the frozen skeletons' shapes; add, do not rename.
- Backend generation changes also update `apply::plan::CODEGEN_REVISION`; saved plans
  distinguish generation semantics from their separate exact compiler-byte pin.
- Generated Rust is an artifact: never edit it, never commit it. `target/ignite/` holds it.
- Tests exercise behaviour through public APIs: the parser through source text, the checker
  through fixtures with `.expected` diagnostics, the generator through golden files and a real
  `cargo build`, the compiled programs through a real socket and real stdin/stdout.
- Each diagnostic code in `docs/slice.md` section 8 has a fixture or test that triggers it.
- Do not add a dependency without a line in the commit message saying what it replaces.
- Ignite behaviour is tested inline with `test` blocks; Rust tests cover the compiler and end-to-end binaries.
- One integration-test binary per crate (`docs/vision.md` invariant 9): every file in a crate's
  `tests/` is a module of its `tests/main.rs` (`autotests = false`, one `[[test]] name = "main"`).
  A new test file gets a `mod` line there, never a `[[test]]` of its own. Shared helpers are
  `tests/common/mod.rs`, declared in `main.rs` and imported with `use crate::common;`. Harness
  names are `<file>::<test>`: one file runs with `cargo test -p <crate> --test main <file>::`.
- `ignite test` links one test executable per target for all files given (`docs/slice.md`
  12.7); only two programs with one module path get separate executables.

## Merging

Standing rule (Timo, 2026-11-04): finished work goes to `main` without waiting for a person.

- Every change reaches `main` through a pull request that the agent opens and merges itself with
  `gh pr merge --merge <PR>`; merges are merge commits, so a slice keeps its unit history.
- A change to code, `std/`, tests or `Taskfile.yml` has a green `task gate` on the PR's head
  before the merge; the PR body names the gate's counts and exit. A change to documents only
  needs no gate.
- `main` is protected (2026-11-05): force-push and deletion are refused, admins included; no
  status check or review is required, so direct fast-forward pushes and `gh pr merge` work. `main`
  has no CI check, so the local gate is the check and the agent merges directly. Once a CI
  workflow runs the gate as a required check on `main`, the rule becomes
  `gh pr merge --auto --merge <PR>`.
- Planning-store commits (`.engineering/` only) go to `main` directly as fast-forwards.
- No force-push to `main`, no rewrite of its history, no moved or deleted release tag.

## Releases

Standing rule (Timo, 2026-11-07): a release is cut after every third wave a coordinator closes,
without waiting for a person.

- Count: waves closed on `main` since the last release tag; the wave pages under
  `.engineering/waves/` name them. At the third, the next minor version is released.
- Steps, as `v0.12.0` (`29f656f2`): one `chore(release): version X.Y.Z` commit bumping the
  workspace crates in `Cargo.toml` and `Cargo.lock` and the version the site names
  (`site/book/src/README.md`, `faq.md`, `guide/install.md`, `design/related-work.md`,
  `design/inspirations.md`, a `roadmap.md` row, a milestone in `site/index.html`); a PR with a
  green `task gate`, merged; tag `vX.Y.Z` on the merge commit; `gh release create vX.Y.Z` with
  notes in the shape of `v0.12.0`'s: one bullet per merged PR, behaviour changes (a
  `CODEGEN_REVISION` bump means the first `ignite apply` rebuilds), what is not yet covered.
- Binary: on the tagged commit run `task dist` and upload its archive with `gh release upload vX.Y.Z
  target/dist/ignite-X.Y.Z-x86_64-linux.tar.gz`: Linux x86_64, glibc linked statically (a musl driver
  would carry a musl runtime archive that the default, dynamic LLVM link cannot use), `bin/ignite`
  with `share/ignite/std`, the standard library a driver at `<prefix>/bin/ignite` reads first;
  the Rust target still needs a clone (`--substrate`). The VS Code extension of `ignite-lsp`
  downloads it.
- Image (story `toolchain-image`), after the archive is uploaded: `task image VERSION=X.Y.Z`
  (`dist/toolchain/Dockerfile`: `rust:1.98-trixie`, clang, lld, the release archive checked
  against its `.sha256`, the tag's source in `/usr/local/src/ignite` with its crates fetched,
  `CARGO_NET_OFFLINE=true`), `task image-check VERSION=X.Y.Z` (the todo example built on both
  targets with no network, each `todod` answers), `docker push
  ghcr.io/ignite-lang/ignite-toolchain:X.Y.Z` (login: `gh auth token | docker login ghcr.io -u
  <gh user> --password-stdin`; the package is public), then add the reference with its
  `@sha256:` digest (`docker inspect --format '{{index .RepoDigests 0}}'`) to the release notes
  with `gh release edit`.
- Announce: send the release's tag, URL and notes to the `brain` session (Claude session named
  `brain`, `SendMessage`), asking it to forward the announcement to AI autodev. Send the path of
  a ready file, not the text: Slack mrkdwn (`<url|label>` links), every line at most 200
  characters; `brain` posts it as is (asked by `brain`, 2026-11-07).

## Lifecycle ownership

- `ignite build` owns the generated project directory it was given; it overwrites it.
- `ignite-io::run` owns the process entry and `ignite-io::spawn` the task threads; generated `main`
  never starts a thread itself.

## Slice 12c release decision (2026-11-02)

The user chose to release the tested improvements with documented performance gaps
and stop further optimization for this release. Performance recovery is **not complete**.
Frozen implementation: `815cf741b359af1bb6df16fe7f48b7a5b1e304ba`; full evidence,
raw samples and exclusions: `docs/performance/12c/release-815cf74/README.md`.

- Correctness: 660 Rust tests, five ignored, 148 inline cases per backend; full
  format/Clippy/workspace/inline gate passes; four example builds pass.
- LLVM readiness: manual restart exactly 100 ms, real restart 104.162 ms, probe
  failure/recovery and standings, stop and journal replay pass. Rust lifecycle
  and both-backend draining rollouts are covered by the full suite.
- Fifteen matched check/plan pairs pass at 1.0647/1.0589 of frozen 11b.
- Formatting, inline, generator-file and CLI-file paired budgets pass.
- Clippy remains 1.1135; checker-touch no-run remains 1.2814 (limit 1.10).
- Warm workspace median 14.333 s misses 10.8 s; comparable low-load evidence
  was not established. Cold and final program timing guards are uncollected;
  published-comparison workspace evidence is insufficient. No baseline moved.
- `ignite test --target=all` enumerates all supported native CLI targets.

## Slice 13 `ignite apply` (2026-11-03)

Implemented: `ignite host`, `ignite plan --for-apply`, `ignite apply`, `ignite inspect`, the
`Transition` reducer, durable receipts, crash recovery and the `docs/slice.md` 17.7 matrix on
both targets (coverage table after 17.7, standing in `STATUS.md`). The precedence of recovery
reasons is in 17.5 "As landed (U5)"; no known gaps.

- Gate at the close, `task gate` on `feat/slice13-local-apply` (d93f059), 2026-11-03 03:39:49
  to 03:46:39 UTC, 410 s, exit 0, one cargo build on the host, `RUST_TEST_THREADS=4`: 746 cargo
  tests passed, 0 failed, 5 ignored; 160 inline tests per target. The `crates/ignite`
  end-to-end binary is 388.31 s of it, set by the `apply*` files (`apply::` 46 tests 448.83 s
  alone, `apply_transition::` 10 tests 273.78 s alone).
- Gate budget: slice 13 is recorded as an exception, as 12c was; no baseline moved. The
  warm-step budget of 10.8 s is not reachable while the live `apply*` suites run inside
  `cargo test --workspace`. Analysis and the proposed tiered budget:
  `docs/performance/13/verdict.md`.

## Slice 14 in-process streams (2026-11-03)

- Gate at the close (the slice's merge gate), `task gate` on `s14/u5b-records` (65d6cd3),
  2026-11-03 07:21:14 to 07:27:37 UTC, 383.2 s, exit 0, start load 3.14, the worktree's own
  target warm from an earlier attempt, `RUST_TEST_THREADS=4`: 819 cargo tests passed, 0 failed,
  6 ignored; 191 inline tests per target. The `crates/ignite` end-to-end binary is 359.82 s of
  it (243 passed, 4 ignored). The before tree's full `task gate`: 369.0 s, exit 0, load 3.02, 777
  passed, 5 ignored, 160 inline per target, end-to-end binary 360.32 s.
- 18.18 method (`docs/slice.md` 15.1): "before" is a `git archive 340f782` copy, "after" the
  65d6cd3 tree. One cold gate per tree, then three interleaved warm passes at load 2.9 to 6.9. No
  pair was dropped by the load rule. Medians of pair ratios; raw logs, scripts and every pair are
  in `docs/performance/14/` (`pairs.md`, `counts.txt`).

| unit | before s | after s | ratio | pairs |
|---|---|---|---|---|
| gate, cold (fmt, clippy, test without apply, test-ignite) | 77.79 (0.72, 4.94, 63.56, 8.55) | 86.84 (0.79, 6.11, 70.42, 9.50) | 1.12 (1.09, 1.24, 1.11, 1.11) | 1 |
| gate, warm after the touch | 21.34 (0.70, 3.35, 16.42, 0.80) | 23.16 (0.73, 3.50, 17.72, 1.23) | 1.09 (1.03, 1.03, 1.08, 1.51) | 3 |
| `task test-ignite`, warm | 0.750 | 1.205 | 1.62 | 3 |
| `--no-run` after the touch | 2.741 | 2.894 | 1.01 | 3 |
| `ignite check`, `ignite plan` of the design file | 0.071, 0.070 | 0.073, 0.073 | 1.03, 1.06 | 15 |
| `ignite test` telnet.ignite, cold / warm | 3.866 / 0.385 | 4.582 / 0.798 | 1.19 / 2.07 | 3 |
| `ignite test` http.ignite, cold / warm | 3.498 / 0.113 | 4.016 / 0.433 | 1.16 / 3.84 | 3 |
| todod Rust dev / release | 3.102 / 4.216 | 3.125 / 4.177 | 1.01 / 0.99 | 3 |
| todod LLVM dev / release | 0.270 / 0.718 | 0.286 / 0.733 | 1.01 / 1.00 | 3 |
| terminald Rust dev / release | 3.089 / 3.900 | 3.114 / 4.051 | 1.02 / 1.01 | 3 |
| terminald LLVM dev / release | 0.419 / 0.520 | 0.436 / 0.522 | 1.05 / 1.01 | 3 |

- Rises over 10 %:
  - `task test-ignite` (gate step 1.51, standalone 1.62): the inline tests went from 160 to 191
    per target and their execution (`finished in`) from 0.13 to 0.34 s per target, both measured;
    that is 0.21 s of the 0.45 s rise. The rest is inferred: the frontend on the larger std
    (`std/` and `examples/` +505 lines).
  - `ignite test` warm, telnet 2.07 and http 3.84: tests 25 to 37 and 23 to 41, execution 0.11
    to 0.31 s and 0.01 to 0.16 s, measured; that is 0.20 s of 0.41 s and 0.15 s of 0.32 s. That
    the new pipe idle-timeout tests set the execution time is inferred, as is the rest of each
    rise (frontend, the larger test program).
  - `ignite test` cold, telnet 1.19 and http 1.16: the same execution growth, measured; the
    compile of the larger test program is inferred.
  - Cold gate 1.12 (clippy 1.24, test 1.11, test-ignite 1.11), one pair: cargo tests without
    apply 695 to 737 passed and 5 to 6 ignored, measured; crate sources +1,218 lines in `src/` and
    +4,045 in `tests/` (`git diff --shortstat 340f782 65d6cd3`), measured. The split between
    compile and execution is inferred.
- No baseline moved. The apply suites stay the slice-13 exception and inside the full
  `task gate` only.

## Slice 15 RFC catalogues and the conformance matrix (2026-11-04)

- Gate at the close (the slice's merge gate), `task gate` on `s15/claims` (621070e),
  2026-11-04 01:37:59 to 01:45:26 UTC, 447 s, exit 0, start load 11.6, the worktree's own
  target, `RUST_TEST_THREADS=4`: 923 cargo tests passed, 0 failed, 13 ignored; 199 inline tests
  per target. The `crates/ignite` end-to-end binary is 430.29 s of it (310 passed, 8 ignored).
  Claims: Telnet `rfc854: no (4 gaps, 0 failures, 0 inconclusive)` (P2 test 3 not met,
  `docs/slice.md` 19.14 item 16), `rfc855: yes (0 gaps, 0 failures, 0 inconclusive)`; Http
  `rfc9112: no (15 gaps, 0 failures, 0 inconclusive)`, `rfc9110: no (9 gaps, 0 failures, 0
  inconclusive)`.
- 19.17 method through `tools/ignite-perf` (`cold`, `warm`, `programs`, `cli`): "before" is
  `296f849`, "after" `c20f04b`, both exported and built by the harness. Five warm passes at start
  loads 2.6 to 13.2; pairs whose loads differ by more than 5 are dropped. Medians of pair ratios;
  records, logs and scripts in `docs/performance/15/` (`README.md`, `records/summary.md`).

| unit | before s | after s | ratio | pairs |
|---|---|---|---|---|
| gate, warm after the touch | 395.39 | 393.15 | 1.02 | 3 |
| `cargo test --workspace`, warm | 403.41 | 401.10 | 0.99 | 4 |
| fmt, clippy, warm | 0.967, 4.384 | 0.798, 4.035 | 0.82, 1.00 | 3 |
| `task test-ignite`, warm | 1.257 | 1.425 | 1.16 | 5 |
| `--no-run` after the touch | 2.987 | 4.025 | 1.13 | 5 |
| `ignite_test::`, `ignite-rustgen` tests | 2.135, 1.825 | 2.363, 2.429 | 1.12, 1.33 | 5 |
| todod Rust dev / release | 3.468 / 4.537 | 3.449 / 4.582 | 0.97 / 0.98 | 3 |
| todod LLVM dev / release | 0.293 / 0.752 | 0.340 / 0.866 | 1.14 / 1.15 | 3 |
| `ignite check`, `ignite plan` of the design file | 0.079, 0.077 | 0.089, 0.084 | 1.12, 1.09 | 15 |
| `ignite check examples/todo/todo.ignite` | 0.0370 | 0.0496 | 1.35 | 15 |
| `ignite conformance` telnet, Rust / LLVM, cold and warm (after only) | | 4.545 and 0.546 / 0.805 and 0.554 | | 3 runs |
| `ignite conformance` http, Rust / LLVM, cold and warm (after only) | | 5.193 and 0.385 / 0.651 and 0.361 | | 3 runs |
| `ignite rfc import` of RFC 9110, D4's sections (after only) | | 0.177 | | 5 runs |

- Cold gate: no valid pair. Pair 1 was dropped by the load rule (12.7 against 5.4); in the retry
  the before tree failed `limits::effect_inference_is_linear_in_either_order`, a timing bound, and
  the loads differed by 9.5. The after tree passed both (448.0 s and 543.8 s).
- Rises over 10 %, measured growth (`git diff --shortstat 296f849 c20f04b`): `std/` +3,407 lines,
  3,108 of them the five catalogues; `crates/*/src` +3,954, `crates/*/tests` +7,786. Inline tests
  191 to 199 per target with execution unchanged at 0.34 s, measured. Inferred: `ignite check` and
  `plan` (and todod LLVM, the rustgen tests, `ignite_test::`, `task test-ignite`) pay the frontend
  on the catalogues that `std.protocol.http` and `std.protocol.telnet` import, the cost 19.17
  expected; `--no-run` after the touch pays the larger crates that depend on `ignite-check`.
- `ignite-perf` needed one fix to measure at all: it refused every tree since 9ad4640 for
  `docs/performance/evidence/.gitattributes` (621070e). No baseline moved. The apply suites stay
  the slice-13 exception.

## Slice 16 language gaps, wave A (2026-11-05)

- Gate at the close, `task gate` on `s16/u6-records` (426449e, the code of the slice's final
  head; later commits change documents only), 2026-11-05 01:51:29 to 02:00:36 UTC, 547 s, exit 0,
  start load 6.9, the worktree's own target, `RUST_TEST_THREADS=4`: 1009 cargo tests passed, 0
  failed, 10 ignored; 211 inline tests per target. The `crates/ignite` end-to-end binary is
  492.65 s of it (336 passed, 8 ignored).
- 20.12 method through `tools/ignite-perf`: "before" is `31b738b` (`main`, the side of the
  close's merge without slice 16), "after" `426449e`. Partly collected: one cold pair and one
  warm pair; the run stopped in warm pass 2 when the before tree's target passed the session's
  12 GB limit. Check, plan and program builds were then timed with copies of both compilers
  (`STEP` lines, same pair and load rules). Records, scripts and the uncollected units in
  `docs/performance/16/README.md`.

| unit | before s | after s | ratio | pairs | start loads |
|---|---|---|---|---|---|
| gate, cold (fmt, clippy, test, test-ignite) | 605.56 (0.85, 8.67, 579.63, 16.41) | 664.34 (0.93, 5.77, 629.57, 28.07) | 1.10 (1.10, 0.67, 1.09, 1.71) | 1 | 7.7 to 12.6 |
| gate, warm after the touch | 450.11 | 516.89 | 1.15 | 1 | 5.9, 8.7 |
| `cargo test --workspace`, warm | 440.21 | 507.38 | 1.15 | 1 | 8.9, 9.1 |
| fmt, clippy, warm | 1.063, 6.754 | 0.931, 6.430 | 0.88, 0.95 | 1 | 5.7 to 8.7 |
| `ignite check`, `ignite plan` of the design file | 0.089, 0.090 | 0.120, 0.121 | 1.32, 1.34 | 15 | 8.7 to 8.8 |
| `ignite check examples/todo/todo.ignite` | 0.056 | 0.068 | 1.21 | 15 | 8.7 to 8.8 |
| todod Rust dev / release | 4.362 / 4.890 | 4.402 / 4.613 | 1.18 / 0.91 | 3 | 6.7 to 9.4 |
| todod LLVM dev / release | 0.412 / 0.799 | 0.519 / 0.772 | 1.16 / 0.97 | 3 | 6.7 to 9.0 |
| terminald Rust dev / release | 3.727 / 4.092 | 4.151 / 3.955 | 1.02 / 1.00 | 3 | 7.6 to 9.2 |
| terminald LLVM dev / release | 0.463 / 0.594 | 0.506 / 0.662 | 1.07 / 1.19 | 3 | 7.0 to 8.5 |
| `ignite test slice16/moves_scale.ignite`, Rust / LLVM, cold and warm (after only) | | 3.345 and 0.031 / 0.181 and 0.017 | | 3 runs | 7.5 to 7.8 |

- Uncollected: warm pairs 2 and 3; the warm `task test-ignite`, `--no-run`, `ignite_test::` and
  rustgen units (pair 1 dropped by the load rule, 5.9 to 6.1 against 13.7 to 15.5). Each of the
  two `moves_scale` tests runs in 0.00 to 0.02 s on both targets, under the runner's 0.01 s
  resolution; rows 9 and 10 of 20.0 check linearity.
- Rises over 10 %, inferred unless marked: `std/` is unchanged (measured), so `ignite check` and
  `plan` (1.21 to 1.34) and the dev builds of todod (1.16, 1.18) and the LLVM release of
  terminald (1.19) pay the frontend's new work, the moves analysis over every program and
  reached std module and E046's length split; the warm workspace (1.15) runs 30 more cargo tests
  (measured), among them the `slice16::` builds on both targets; cold `task test-ignite` (1.71)
  is its compiler and runtime build, its inline execution unchanged at 0.33 to 0.35 s
  (measured). todod release falls (0.91 Rust, 0.97 LLVM).
- Known cost not measured: since wave 3 a pattern binding that is a used branch or arm tail is
  copied. Since PR 67 (`story:moves-for-pattern-bindings`) the analysis moves pattern bindings
  of a freshly created value; a binding of a borrowed scrutinee is still copied once.
- No baseline moved. The apply suites stay the slice-13 exception.

## Slice 17 playground substrate (2026-11-05)

- Gate at the close, `task gate` on `s17/u6-records` (3c5cc34f, the code of the slice's final
  head; the later commit changes documents only), 2026-11-05 08:52:12 to 09:01:03 UTC, 531 s,
  exit 0, start load 10.0, the worktree's own target, `RUST_TEST_THREADS=4`: 1089 cargo tests
  passed, 0 failed, 10 ignored; 233 inline tests per target. The `crates/ignite` end-to-end
  binary is 510.73 s of it (380 passed, 8 ignored). The first gate on the same head failed
  `slice10::executable_component_main_runs_and_maps_the_exit_code` (`ignite-rustgen`: the
  exit code matched, the stderr line was not yet in the captured log); 5 of 5 runs alone
  passed, and the second gate is the one above.
- 21.14 method through `tools/ignite-perf`: "before" is `99647132` (`main`, the merge base at
  the close; the code of the slice's start `651be04c`), "after" `884d6b19`. Partly collected:
  `smoke`, `programs`, `cli` and one `cold` attempt; check of the todo example, terminald and the
  after-only units as `STEP` lines. Loads 5.1 to 25.8, so no pair was taken under load 5.
  Records, scripts and the uncollected units in `docs/performance/17/README.md`.

| unit | before s | after s | ratio | pairs | start loads |
|---|---|---|---|---|---|
| gate, cold (fmt, clippy, test, test-ignite) | 1070.99 (1.62, 10.15, 1031.21, 28.01) | 810.17 (2.76, 24.37, 763.85, 19.17) | no valid pair | 0/1 | 21.8 against 15.8 |
| `ignite check`, `ignite plan` of the design file | 0.150, 0.144 | 0.177, 0.153 | 1.09, 1.06 | 15 | 16.2 to 16.7 |
| `ignite check examples/todo/todo.ignite` | 0.067 | 0.127 | 1.94 | 15 | 9.5 to 9.9 |
| same, after compiler, std without and with the `HttpServer` hooks | 0.068 | 0.131 | 1.89 | 15 | 9.5 to 9.9 |
| todod Rust dev / release | 3.916 / 5.145 | 4.324 / 7.096 | 1.14 / 1.42 | 4, 3 | 5.1 to 22.6 |
| todod LLVM dev / release | 0.383 / 1.368 | 0.476 / 0.999 | 1.24 / 1.05 | 3, 4 | 7.4 to 22.6 |
| terminald Rust dev / release | 3.915 / 4.615 | 5.866 / 5.556 | 1.50 / 1.19 | 2, 3 | 9.5 to 19.9 |
| terminald LLVM dev / release | 0.568 / 0.926 | 0.596 / 0.895 | 1.03 / 1.05 | 3 | 9.3 to 21.3 |
| `studio.ignite` build, Rust dev / release, LLVM dev / release (after only) | | 5.113 / 7.057, 0.498 / 0.988 | | 3 runs | 9.6 to 21.2 |
| `ignite test std/fs.ignite`, Rust / LLVM, cold and warm (after only) | | 5.512 and 0.109 / 0.361 and 0.100 | | 3 runs | 15.0 to 17.7 |
| `ignite test std/process.ignite`, Rust / LLVM, cold and warm (after only) | | 8.981 and 0.746 / 0.735 and 0.430 | | 3 runs | 17.7 to 22.5 |

- Uncollected: the `warm` mode (gate, workspace, fmt, clippy, `task test-ignite`, `--no-run`,
  `ignite_test::`, rustgen units): its passes need both trees' targets built at once (the after
  tree's was 9.0 GiB), against the disk request of 09:22 (one tree's target at a time, the
  unit's scratch under 8 GB); the cold retry, stopped for the same request.
- Rises over 10 %: the check of the todo example (1.94) is measured to `HttpServer`'s run hooks
  (the after compiler: 0.068 s with the std before them, 0.131 s with them; the before tree
  0.067 s): `std.protocol.http` imports `std.control` and `std.policy`, which every HTTP program
  loads and types (`docs/friction.md` row 43). todod Rust release 1.42 and dev 1.14: its
  generated `ignite-std` grows from 1,049 to 1,700 lines with the hooks (measured); which types
  make up the growth is not split. todod LLVM dev 1.24: the same frontend, inferred. terminald
  Rust (1.50 over 2 pairs, 1.19): its std is unchanged; `ignite-io`, which every Rust program
  build compiles, grew by 2,201 lines and the `nix` features `dir` and `poll` (measured growth,
  inferred cause). Cold clippy 2.40 in the per-step comparison, inferred: larger crates and a
  different load.
- No baseline moved. The apply suites stay the slice-13 exception.

## Slice 18 one service operated (2026-11-05)

Slice 18 gate at the close: `task gate` on `s18/u8-records` (f64adfec8, the code of the slice's
final head with `main` up to PR 48 merged; the later commit changes documents only), 2026-11-05 22:43:53 to 23:07:33 UTC,
1420 s, exit 0, start load 13, the worktree's own target, `RUST_TEST_THREADS=4`:
1190 cargo tests passed, 0 failed, 11 ignored; 322 (294 plus the LSP example's 28) inline tests per
target. The `crates/ignite` end-to-end binary is 1353.60 s of it (440 passed, 9 ignored). The first gate on the same head failed only `slice10::executable_component_main_runs_and_maps_the_exit_code` (`ignite-rustgen`, the known flake: `listening on` not yet in the captured log); 5 of 5 runs alone passed, and the second gate is the one above.

- 22.15 method through `tools/ignite-perf` (`docs/performance/18/README.md`): "before" is
  `421742b33` (the merge of slice 17 into `main`, the slice base), "after" `661e55b42` (every
  unit and `main` up to PR 47; PR 48 is not in the measured tree). Partly collected: `smoke`,
  `programs` (6 pairs), `cli`, `apply-cycle` and one `cold` attempt; `warm` uncollected. Loads
  5.3 to 29.6, so no pair was taken under load 5.

| unit | before s | after s | ratio | pairs | start loads |
|---|---|---|---|---|---|
| gate, cold (fmt, clippy, test, test-ignite) | 1246.63 (1.22, 20.27, 1182.49, 42.66) | 2366.65 (1.40, 12.76, 2313.06, 39.43), exit 1 | no valid pair | 0/1 | 17.7 against 10.2 |
| `ignite check`, `ignite plan` of the design file | 0.134, 0.137 | 0.136, 0.144 | 1.05, 1.06 | 15 | 7.1 to 7.6 |
| todo program (`todod`, after also `health`), Rust dev / release | 7.203 / 10.308 | 6.304 / 8.203 | 0.91 / 0.89 | 6 | 5.3 to 16.2 |
| todo program, LLVM dev / release | 0.513 / 0.994 | 0.856 / 1.499 | 1.53 / 1.42 | 6 | 5.3 to 16.2 |
| row 27: LLVM dev / Rust dev of the todo program, after | | 0.856 / 6.304 = 0.136 (per-pass median 0.148) | | 6 | 5.3 to 16.2 |
| apply cycle, terminal design, Rust / LLVM (after only, 5 + 5 samples) | uncollected (below) | 30.870 and 28.497 / 28.269 and 24.667 | | | 7.4 to 29.6 |
| apply cycle, todo, Rust / LLVM (after only, 5 samples) | | 31.458 / 30.821 | | | 7.4 to 29.6 |

- Rows 27 and 28 hold: the LLVM dev build of the todo program is 0.136 of the Rust one (at most
  0.15), and the todo cycle's LLVM median is below its Rust median (`todo_llvm_below_rust`
  true), by 2 %: both cycles spend 13 to 17 s in `plan --for-apply` and 12 to 15 s in the
  rollout, against a build of 0.9 s (LLVM) and 6.3 s (Rust); the split of the plan beyond its
  build is not measured.
- Rises over 10 %: the todo program on LLVM, dev 1.53 and release 1.42, measured growth: the
  after tree builds a second executable (`health`, its own main of 278 IR lines and link) and a
  larger program (`program.ll` 1,743 to 5,184 lines, `std.ll` 12,780 to 14,350); the split
  between the two is inferred. The cold gate has no valid pair: the after tree failed the slice-16 timing bound `slice16::llvm_state_threading_is_linear` (0.08 s against 0.01 s at a 0.01 s resolution, load 10 to 36), and the start loads differ by 7.5; measured, the workspace step runs 1,188 cargo tests against 1,090 and the end-to-end binary 2184.42 s (440 tests) against 1042.08 s (381), with the slice's live suites and `main`'s `rollout_repeat::` and `stream_ownership::` among the new ones; the split between tests and load is not measured; the timing test passed in the final gate. The Rust program rows fall (0.91, 0.89) although
  `ignite-std` grows from 1,700 to 2,049 lines; not attributed at their spread (4.3 to 30.6 s).
- Uncollected: `warm` (its passes need both trees' gate targets at once, against this unit's
  15 GB bound for scratch and target); the before tree's terminal apply cycle: Rust stopped
  after 1 of 5 samples and LLVM never ran, because the before tree lacks `fix/rollout-drain-stuck`
  (PR 46) and its third rollout on one host is refused as `rollout already in progress` (host
  journal in `docs/performance/18/logs/`); the before tree was not patched. `health` is not timed
  apart from `todod` (`ignite build` builds both; 22.0 row 27 as landed).
- No baseline moved. The apply suites stay the slice-13 exception.
