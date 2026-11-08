# Layering of the standard library and the crates

A general layer knows no application protocol and no application. The rule extends two
layout rules of `AGENTS.md` ("The substrate knows sockets and TLS sessions, not application
protocols", "Nothing named telnet lives under `crates/`") and O45 (`docs/slice.md` 16.2) to
all of std. The test `layering::` in `crates/ignite-check/tests/layering.rs` enforces it and
holds the same layer table; change both together. The table of rule 6 it reads from this file.

## Layers

| layer | modules | may import |
|---|---|---|
| base | `std.io`, `std.net`, `std.fs`, `std.process`, `std.sync` (substrate), `std.option`, `std.result`, `std.text`, `std.json`, `std.data` (imports nothing; `std.json` and every later format module import it) | base |
| leaf | `std.sli`, `std.time`, `std.policy` (O45), `std.rfc` and every `std.rfc.*` | base, leaf |
| general | `std.control`, `std.contract`, `std.deploy`, `std.console`, `std.cli` | base, leaf, general |
| protocol | every `std.protocol.*` | every layer |

Rules, each a failure of the test that names the file, line and identifier or import:

1. A module imports only modules of its own layer or a lower one.
2. No module outside `std.protocol.*` imports a protocol module.
3. No module outside `std.protocol.*` declares an item, owner, field, variant or role whose
   name contains a protocol name. Protocol names are read from std: each `protocol`
   declaration and each file name under `std/protocol/` (today `telnet`, `http`, `jsonrpc`).
   Exempt: a catalogue `std.rfc.*` may declare names that contain the name of a protocol
   whose module imports it, because it quotes that protocol's specification (`JSONRPC_2`
   in `std.rfc.jsonrpc_2`, which `std.protocol.jsonrpc` imports). No other module is
   exempt, `std.rfc` itself included.
4. Every std module outside the prefixes `std.protocol.` and `std.rfc` (`std.rfc` itself and
   every `std.rfc.*`) has a row in the table above.
5. No file under `crates/*/src` contains a protocol name, in code or in comments, except
   `crates/ignite-load` (below).
6. No string literal in the code of a file under `crates/*/src` names a std module or an item
   of one (`std.` and a lowercase segment, as `"std.sli"` or `"std.control.Machine"`) unless
   the table "Std names in the crates" below lists that name for that file. Comments are not
   read. A row whose file no longer contains one of its names fails as well, so the list
   holds only what is used. The test reads the table from this file.

A component kind that speaks one protocol (a server, a client, a probe) lives in that
protocol's module: `TelnetServer`, `TelnetClient` and `TelnetPrompt` are in
`std.protocol.telnet`, `HttpServer` in `std.protocol.http`.

`crates/ignite-load` is exempt from rule 5. It is a load client of the example servers
(terminal and todo), linked into neither the compiler nor a compiled program, and it has to speak Telnet and HTTP
to measure the server. The layout rule names `crates/` in its wording; its purpose is that the
compiler and the substrate know no protocol, which `ignite-load` does not touch.

`crates/ignite-eval/src` mirrors the std vocabulary `std.rfc` (`Level`, `Party`, `Source`,
and the fields of `Requirement` and `Spec`) in its claim model (`claims.rs`, `docs/slice.md`
19.7), as it already names `std.sli.Window.Rolling` (the audit row of
`crates/ignite-eval/src/validate.rs` below). No protocol name is involved, so rule 5 holds.

## Std names in the crates

The std modules the compiler crates know by name, for rule 6. Each module has one home, a
constant or a function, and every other crate uses that home: `ignite_check::RFC_MODULE`,
`ignite_check::SUBSTRATE_MODULES`, `ignite_types::CONTROL_MODULE`, `ignite_types::endpoint`
and `child_exit`, `ignite_eval::SLI_MODULE` and `DEPLOY_MODULE`. A new name needs a row here
with its reason, or a derivation from the program.

| file | names | reason |
|---|---|---|
| `crates/ignite-check/src/lib.rs` | `std.net`, `std.io`, `std.fs`, `std.process` | `SUBSTRATE_MODULES`, the home modules of the builtins; `std.net` is also the example of the E031 hint |
| `crates/ignite-check/src/lib.rs` | `std.rfc` | `RFC_MODULE`, the clause vocabulary the checker, the type checker and the evaluator read by name (`docs/slice.md` 19.3) |
| `crates/ignite-types/src/builtins.rs` | `std.process`, `std.net` | `Invocation`, `ChildExit` and `Endpoint`, record types in builtin signatures (14.14, 21.5) |
| `crates/ignite-types/src/lib.rs` | `std.control` | `CONTROL_MODULE`: the hook `K.machine` returns `std.control.Machine`, and the run driver calls its reducers (16.6) |
| `crates/ignite-eval/src/model.rs` | `std.sli` | `SLI_MODULE`: `Window`, which E123 compares with a probe interval, and the `Sample` and `Verdict` the run driver builds (22.3) |
| `crates/ignite-eval/src/model.rs` | `std.deploy` | `DEPLOY_MODULE`: `admits`, `includes` and `entries` resolve an assembly and are asked about its `Member` (15.6, 16.17) |
| `crates/ignite/src/run/live.rs` | `std.sli.ProbeAttempt` | unit-test data: a probe line of the terminal example, whose outcome type this is |

The run driver decodes a probe's line by the element type of the observed signal, which the
model names (22.3); the fallback to `std.sli.ProbeAttempt` that the architecture review of
2026-11-05 found went with the typed probe outcomes (`f648922`), and no code of the driver
names that type.

## Audit of 2026-11-02

The defect that started it: `std/probe.ignite` (slice 10, `60aef38`), the general probe
module, held only the Telnet probe `TelnetPrompt` and imported
`std.protocol.telnet.{TelnetServer, await_prompt}`. `TelnetPrompt` moved to
`std/protocol/telnet.ignite` with its hooks and tests; `std/probe.ignite` is gone. The model
names the kind `std.protocol.telnet.TelnetPrompt`.

Fixed in the same change: every doc-comment example under `crates/*/src` that named
Telnet now uses a neutral example (`Echo.serve`, `std.a.b`, `std.json`, a server kind's
`main`), so rule 5 holds without exceptions inside the compiler crates. Before the change the
mentions were all doc comments; no compiler, generator or runtime code branched on a protocol
or kind name.

Still open, with the proposed fix:

| where | class | what | proposed fix |
|---|---|---|---|
| `std/sli.ignite` (`ProbeAttempt`, `prompt_received_before_timeout`, `prompt_received_within_threshold`) | vocabulary in a leaf | the prompt probe's outcome type and its two predicates live in the leaf; since slice 18 (`docs/slice.md` 22.3) no closed outcome set remains: a contract's `good` is any predicate over its probe's outcome type, checked by the type checker and applied by the kind's `classify` | move `ProbeAttempt` and its predicates beside `TelnetPrompt` once the design file may import them from there |
| `std/deploy.ignite:127`, `:131`, `:140`, `:147`, `:160` | test data | the `admits` fixtures use the kind names `std.protocol.telnet.TelnetServer` and `TelnetPrompt` as text; no import | neutral kind names (`app.Server`, `app.Probe`) |
| `std/text.ignite:273` | test data | an HTTP request line as input of `lines()` | any CR LF text |
| `std/cli.ignite:6` | doc comment | names `std.protocol.telnet.TelnetClient` as the buildable client | keep while it is the only one, or say "a protocol's client kind" |
| `std/console.ignite:6` | doc comment | "A protocol such as std.protocol.telnet" | acceptable as an example |
| `std/control.ignite:26`, `:75` | doc comment | ids of the terminal example (`Terminal.promptProbe`) | neutral ids (`App.probe`) |
| `crates/ignite-types/src/lib.rs:814` | doc comment | `Terminal.promptProbe.attempts` as a reference example | neutral path |
| `crates/ignite-eval/src/validate.rs:350` | domain noun in the evaluator | E123 recognises `std.sli.Window.Rolling` by name to compare a window with a probe interval | acceptable: `std.sli` is a leaf the compiler already reads; revisit if the window moves |
| `crates/ignite-load/src/main.rs` | protocol client under `crates/` | 21 Telnet mentions, a Telnet filter of its own | exempt (above); move to `tools/ignite-load/` if the layout rule should hold by its letter |

No leaf imports a non-leaf: `std.sli`, `std.time`, `std.policy` and `std.rfc` import nothing,
and a catalogue `std.rfc.*` imports only `std.rfc`.

## Later milestone, not built

A declared layer on `package` (`package std.sli layer leaf`) would let `ignite-check` report a
crossing as a diagnostic for every program, not only std. It needs a syntax decision and a
diagnostic code in the slice-13 range or later.
