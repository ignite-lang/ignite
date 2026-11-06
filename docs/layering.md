# Layering of the standard library and the crates

A general layer knows no application protocol and no application. The rule extends two
layout rules of `AGENTS.md` ("The substrate knows sockets and TLS sessions, not application
protocols", "Nothing named telnet lives under `crates/`") and O45 (`docs/slice.md` 16.2) to
all of std. The test `layering::` in `crates/ignite-check/tests/layering.rs` enforces it and
holds the same table; change both together.

## Layers

| layer | modules | may import |
|---|---|---|
| base | `std.io`, `std.net`, `std.fs`, `std.process` (substrate), `std.option`, `std.result`, `std.text`, `std.json` | base |
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

A component kind that speaks one protocol (a server, a client, a probe) lives in that
protocol's module: `TelnetServer`, `TelnetClient` and `TelnetPrompt` are in
`std.protocol.telnet`, `HttpServer` in `std.protocol.http`.

`crates/ignite-load` is exempt from rule 5. It is a load client of the terminal example's
server, linked into neither the compiler nor a compiled program, and it has to speak Telnet
to measure the server. The layout rule names `crates/` in its wording; its purpose is that the
compiler and the substrate know no protocol, which `ignite-load` does not touch.

`crates/ignite-eval/src` mirrors the std vocabulary `std.rfc` (`Level`, `Party`, `Source`,
and the fields of `Requirement` and `Spec`) in its claim model (`claims.rs`, `docs/slice.md`
19.7), as it already names `std.sli.Window.Rolling` (the audit row of
`crates/ignite-eval/src/validate.rs` below). No protocol name is involved, so rule 5 holds.

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
