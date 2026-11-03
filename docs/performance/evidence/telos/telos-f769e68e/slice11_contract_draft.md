# Draft: `docs/slice.md` section 15, slice 11: plan and explain (F2)

Status: draft for the steering review, 2026-11-02. Not in `docs/slice.md` yet. Sources:
vision 4.1 row 11, invariant 9, T4, D5, D7, section 6 (X2, X3); design 3.1, 3.4, 3.5, 5, 6.1
to 6.3, 10.1, 11.3; `docs/slice.md` 14.4 to 14.7 and 14.14; `examples/terminal/terminal.design.ignite`;
`std/contract.ignite`, `std/policy.ignite`, `std/deploy.ignite`, `std/probe.ignite`. Open points
are **O13** to **O26** (numbering continues slice 10's O1 to O12), each with the default taken,
collected in 15.12. Every one is "default, steering may change".

## 15.0 Acceptance

Carry-overs from slice 10 first. The measuring agent P0 measures and attributes them on the
committed tree `60aef38`, in its own copy of that tree with its own target dir, before any
slice 11 edit lands and in parallel with P1's skeleton; P0 fixes nothing. The coordinator
assigns the fixes by P0's numbers when P2 and P3 are dispatched (15.1, 15.11). Each item ends
in a fix or in a new baseline the steering session accepts in writing.

1. Gate warm time: `task gate` after `touch crates/ignite-check/src/lib.rs`, re-measured at
   load average under 5, attributed per test file of `crates/ignite/tests/`. Slice 10 recorded
   45.5 s against 25.9 s after slice 9b (+76 %; 39.0 s of e2e test execution at load 21).
2. Program build time: `todod` re-measured at load average under 5 on both targets, attributed
   per compile step. Slice 10 recorded LLVM release 0.9 s to 1.2 s and Rust dev 3.0 s to 3.3 to
   4.0 s.
3. The flake `failover::a_failing_session_does_not_end_the_service` is reproduced and fixed, or
   bounded: 0 failures in 30 runs alone and 30 runs beside a full `cargo test --workspace`.

Slice 11 proper:

4. `ignite plan examples/terminal/terminal.design.ignite` exits 0 and prints the system model
   (15.5). Two runs in separate processes, started from two different working directories,
   give byte-identical output, equal to the golden `crates/ignite/tests/golden/terminal.design.model.json`.
5. Every property value in that model is evaluated: the golden contains no Ignite expression
   text (none of `exponential(`, `rolling(`, `calendar_month(`, `event(`, `module(`,
   `localSocket(`, `nativeProcess(`, `endpoint(`, and no quantity written with a unit), and a
   test walks the JSON and checks every value against the encoding of 15.4.
6. The assembly `local` is resolved: each of the five resources and components of `Terminal`
   appears exactly once in `assembly.resolution` (15.6). `ignite check` on the design file
   still exits 0, now with the evaluation and resolution rules of this slice applied.
7. `ignite explain` on the design file prints the golden texts for `Terminal.reachable`,
   `Terminal.server.recovery` and `local` (15.8).
8. The evaluator agrees with both targets: every fn of the fixture
   `crates/ignite/tests/fixtures/eval_equivalence.ignite` gives the same debug text in the
   evaluator, in a Rust-target build and in an LLVM-target build (15.3).
9. Builds are unchanged: every Rust golden and every LLVM golden is byte-identical, except
   LLVM goldens changed by the reachable-only std emission if carry-over 2 adopts it; each
   such golden is re-blessed with the diff explained in P3's report. The only behaviour change
   of an existing program is E116 for effectful property expressions (O16).
10. Invariant 9: `task gate` wall time (cold and warm), the `todod` build time on both targets,
    and `ignite check` and `ignite plan` of the design file are measured before and after and
    recorded in `AGENTS.md`. A rise of more than 10 % is a deviation and has to be explained.
11. Every diagnostic E110 to E119 has a fixture.

## 15.1 Carry-overs: how they are measured

- Who and when: P0 measures on its own copy of commit `60aef38` (`git archive 60aef38 | tar
  -x`), with its own target dir, while P1 builds the skeleton; it reports numbers and
  attributions to the coordinator and changes no file of the repository. The fixes below are
  candidates; the coordinator assigns them by P0's numbers: the LLVM reachable-only std
  emission to P3 only if `clang` on `std.ll` is confirmed as the cause; the e2e shared-build
  or parallel-test fix and the flake test rewrite to P3, the owner of `crates/ignite/tests/`.
  P2 keeps the evaluator only.
- Load: read `/proc/loadavg` before and after each timed run; a run whose 1-minute load
  average is 5 or more at either point is discarded and repeated. Report median and range of
  at least 3 accepted runs.
- Item 1: each file of `crates/ignite/tests/` runs alone as `cargo test -p ignite --test main
  <file>::` after the warm touch, timed; the sum is compared with the whole step. The fix
  candidates are named in the order to try: one shared build of each example program per test
  binary run instead of one per test, then parallel e2e tests where they now serialise on a
  lock, then removal of a test that repeats another's coverage (named and agreed). If the warm
  gate stays above 28.5 s (25.9 s + 10 %) after the fix, the measured number becomes the new
  baseline only with the steering session's written yes.
- Item 2: LLVM release split into emit, `clang -O2` of `std.ll`, `clang -O2` of `program.ll`
  and link; Rust dev into the `ignite-std` crate and the program crate. If `clang` on `std.ll`
  is the cause (inferred in slice 10: 18,120 lines, 0.84 s), the LLVM target emits only the std
  fns reachable from the program's executables and tests (a call-graph walk over the typed
  modules before emission; glue for types no reachable fn uses is dropped too). The Rust-target
  part is measured and attributed; a fix there is a labelled later milestone unless the
  low-load number is more than 10 % over 3.0 s.
- Item 3: P0 runs the test 30 times alone and 30 times beside a full workspace test run.
  Slice 10's failure lacked the third `task ended with error` line in a log read after `kill`.
  The rewrite (P3, whether or not P0 reproduces the failure): the test waits for the line with
  a deadline instead of reading the log once after `kill`.

## 15.2 Property expressions are pure (E116)

- A property expression is the right-hand side of a property in a `resource`, `component`,
  `contract`, `policy` or `assembly` declaration, and the default expression of every kind
  field a declaration leaves out.
- Its effect set must be empty. A call of a fn with a non-empty `requires`, `spawn`, an
  operation on a handle (`TcpListener`, `TcpStream`, `Store<T>`), or an ambient name (`stdin`,
  `stdout`, `process`) inside it is E116, reported by the type checker at the expression.
- A bare member name stays a value of its type (14.4), and a resource name stays legal: naming
  a handle is not an operation on it.
- Audit probe `docs/audit/2026-11-01/effects/a21_system_property_effect.ignite` changes from
  "accepted" to E116; the audit README row and the `.expected` file change with it (O16).
  Vision 10.2 names a21 a hole (property expressions not walked by the checker), and E116
  closes it.
- The generated `main` of 14.5 still evaluates property expressions at run time on both
  targets; this slice does not fold them into the binaries (O19).

## 15.3 The evaluator (D5)

- New crate `crates/ignite-eval`: an interpreter over the typed program, used by `ignite
  check`, `ignite plan` and `ignite explain`. Compile time only: no backend and no compiled
  program depends on it. Both targets share it because neither links it.
- It evaluates every property expression of every declaration of a program, with the fns,
  constants and default field values it reaches in any module (program, siblings, std): `let`,
  `var`, assignment, `if`, `match`, `while`, `loop`, `for`, `break`, `return`, closures applied
  inside the evaluation, records, enums, tuples, lists, maps, quantities, text, string
  interpolation, `debug()`, named arguments (in source order, 14.2), and every builtin whose
  effect set is empty.
- Semantics are those of the targets: T6 overflow, 14.8 failures, 14.3 map order, 14.1
  quantity arithmetic and debug text, slice-9 structural `==`.
- Fuel (O14): one unit per expression evaluated, per loop iteration and per call. The limit is
  1,000,000 units per property expression, and `--fuel N` on `check`, `plan` and `explain`
  changes it. A value larger than 1,000,000 nodes (list items, map entries, record fields,
  counted recursively) also stops the evaluation. Either is E115 at the property expression,
  naming the limit and the fn that was running.
- A failure during evaluation (`assert`, overflow, index, `match`, `?` on `Err`, `slice`) is
  E117 at the property expression, with the 14.8 report text as its detail:
  `E117 evaluating Terminal.server.sessions: + failed at <file>:<line>:<column>: <detail>`.
- A result that the model cannot encode is E118 (15.4): a closure value, a fn value that is not
  a named top-level fn, a handle produced by anything other than a member name.
- Equivalence (acceptance 8): the fixture holds at least 40 pure fns, at least one per builtin
  family (Int, Byte, Text, List, Map, Option, Result, quantities, records, enums, closures,
  interpolation, `debug`). The test evaluates each with `ignite-eval` and runs one compiled
  program per target that prints `debug(f())` for each; the three texts must be equal.
- Pure `test` blocks at `ignite check` and constant folding in the backends are not in this
  slice (labelled later milestone, D5 option A's remaining parts).

## 15.4 Value encoding in the model

Every value is a JSON value; where JSON alone is ambiguous it is an object tagged by `"type"`.

| Ignite value | JSON |
|---|---|
| `Unit` | `null` |
| `Bool` | `true`, `false` |
| `Int` | a JSON integer, the exact i64 (O17) |
| `Byte` | `{"type": "Byte", "value": 7}` |
| `Text` | a JSON string |
| `List<T>` | an array, in list order |
| tuple | `{"items": [...], "type": "Tuple"}` |
| record | `{"fields": {...}, "type": "<qualified type name>"}`, every field present |
| enum variant | `{"type": "<qualified enum name>", "variant": "<name>"}`, plus `"fields": [...]` when it has a payload; `Option` and `Result` are enums named `Option` and `Result` |
| `Map<K, V>` | `{"entries": [[k, v], ...], "type": "Map"}`, in the map's key order (14.3) |
| `Duration` | `{"ns": 100000000, "type": "Duration"}` |
| `ByteSize` | `{"bytes": 16384, "type": "ByteSize"}` |
| `Ratio` | `{"ppm": 999000, "type": "Ratio"}` |
| `Ref<K>`, `Ref` | `{"ref": "Terminal.server", "type": "Ref"}` |
| `Signal<T>` | `{"ref": "Terminal.promptProbe.attempts", "type": "Signal"}` |
| `SystemRef` | `{"ref": "Terminal", "type": "SystemRef"}` |
| `Protocol` | `{"ref": "std.protocol.telnet.Telnet", "type": "Protocol"}` |
| `module("p")` record (14.4) | `{"fns": {"respond": "examples.terminal.commands.respond"}, "module": "examples.terminal.commands", "type": "Module"}` |
| named top-level fn | `{"fn": "std.text.trim", "type": "Fn"}` |
| a member name used as a value (`listener = ingress`, `application = console`) | `{"ref": "Terminal.ingress", "type": "Ref"}`, plus a `uses` edge (15.5); the embedded value is the member's own entry |
| a `TcpListener` name where `Endpoint` is expected (14.14 item 5) | the evaluated `std.net.Endpoint` record |

Qualified names are `module.Name` as the typer prints them; builtin types use their bare name.

## 15.5 The system model (D7)

`ignite plan` prints one JSON document, the system model of one system and at most one
assembly. Canonical form, so that equal inputs give equal bytes:

- UTF-8, object keys in byte order at every level, two-space indentation, one key or array item
  per line, `[]` and `{}` for empty containers, `\n` line ends and a final newline.
- Strings escape only `"`, `\` and control characters (`\n`, `\t`, `\r`, `\b`, `\f`, others as
  `\u00XX`); other characters are written as UTF-8.
- Arrays of members, edges and resolution entries are sorted by `id` (or by `from`, `kind`,
  `to`); declaration order in the source does not show (O15).
- Paths are relative: a program file and its siblings relative to the program file's
  directory, std files as `std/<path>`. No absolute path, timestamp, host name or compiler
  version appears (design 6.3).

Top-level keys. Where design 6.2's envelope has a key, the model keeps its name; `assembly` and
`edges` are additions (O13):

| key | content |
|---|---|
| `assembly` | the selected assembly (15.6), or `null` when the program declares none; design 6.2's `bindings` is `assembly.resolution` |
| `components` | every `component` of the system |
| `contracts` | every `contract` |
| `edges` | `{"from", "kind", "to"}`: `uses` (a bare member name in a property), `references` (a `Ref` value), `observes` (a `Signal` value), `hosts` (from the resolution) |
| `formatVersion` | `"ignite-model/0.1"`; a change that removes or reinterprets a key bumps it, an added key does not |
| `policies` | every `policy` |
| `resources` | every `resource` |
| `sourceMap` | object from every member id and property path (`Terminal.server.recovery`) to `file:line:column` of its declaration or expression; a defaulted property maps to the default's position in the kind's module. Slice 13's plan diff compares the model without `sourceMap`; a reformat of the source changes only that key |
| `systemId` | the system's name |

Each member entry:

| key | content |
|---|---|
| `declaration` | `"resource"`, `"component"`, `"contract"` or `"policy"` |
| `defaults` | names of the properties that took the kind's default, sorted |
| `executable` | the executable name when the kind has `K.main` (14.5), else `null` |
| `id` | `<System>.<name>`, the stable logical identity (design 3.4) |
| `kind` | qualified name of the kind |
| `outputs` | object from each `Signal<T>` field the declaration does not set to its element type's name |
| `properties` | object from every other field of the kind to its evaluated value (15.4), set or defaulted |

Excerpt for `Terminal.promptProbe` (the emitted form puts every key on its own line):

```json
{
  "declaration": "component", "defaults": [], "executable": null,
  "id": "Terminal.promptProbe", "kind": "std.probe.TelnetPrompt",
  "outputs": {"attempts": "std.probe.ProbeAttempt"},
  "properties": {
    "close_after_probe": true,
    "expected_prompt": "terminal> ",
    "interval": {"ns": 10000000000, "type": "Duration"},
    "target": {"ref": "Terminal.server", "type": "Ref"},
    "timeout": {"ns": 2000000000, "type": "Duration"}
  }
}
```

The design 6.2 envelope's `machines`, `observations`, `artifacts`, `stateSchema` and
`admissionChecks` are not emitted; slices 12 and 13 add them as new keys under the same
`formatVersion` rule. P4 adds a note to design 6.2 (15.11) saying:

- the executable specification is the system model of `docs/slice.md` §15 (D7's rename, X3);
- its `bindings` is the model's `assembly.resolution`;
- `machines`, `observations`, `artifacts`, `stateSchema` and `admissionChecks` arrive in slices
  12 and 13;
- until wave H the public schema is the 15.4 table plus the golden
  `crates/ignite/tests/golden/terminal.design.model.json`; there is no JSON Schema file in this
  slice.

## 15.6 Assembly resolution

- `ignite plan` selects one assembly: the one named by `--assembly NAME`, else the only one in
  the program. Several assemblies and no `--assembly`, or a name that is not an assembly, is
  E114. Without assemblies, the model's system is the program's only `system`; several systems
  and no assembly is E114 as well.
- An assembly kind is resolvable when it has a field `bindings: Map<Ref, B>` and `B`'s module
  declares `fn admits(binding: B, member: std.deploy.Member) -> Option<Text>` and `fn
  includes(binding: B) -> List<Ref>` (O18). The compiler knows these two fn names and the
  record `std.deploy.Member`, not the variants of `std.deploy.Binding`. An assembly of another
  kind is evaluated and emitted with `"resolution": null`.
- `std/deploy.ignite` gains `type Member { declaration: Text, kind: Text, executable: Bool,
  outputs: List<Text> }` and `admits`: `LocalSocket` admits a `TcpListener` resource;
  `NativeProcess` a component that is executable; `WorkstationExecutable` any component;
  `ScheduledProbeHost` a component with at least one output. Each refusal returns the reason
  text that E112 prints. Inline tests cover each variant.
- Rules, checked by `ignite check` for every assembly and by `ignite plan` for the selected
  one, after evaluation:

| rule | diagnostic |
|---|---|
| every resource and component of the system is a `bindings` key or in exactly one binding's `includes` | E110 when neither |
| no member is both a key and included, or included by two bindings | E111 |
| `admits(binding, member)` returns `Some(reason)` for a key | E112 with that reason |
| a binding's `includes` equals the components its member embeds (`ExecutableInfo.embedded`, 14.14 item 12) | E113, naming the missing or extra member |
| contracts and policies are never keys | E112 (`admits` is not consulted) |

- `assembly.resolution` lists one entry per resource and component, sorted by member id:
  `{"binding": <value>, "member": "Terminal.console", "via": "Terminal.server"}`, where `via`
  is the key whose binding hosts the member (the member itself when it is a key).
- The assembly entry otherwise follows the member form: `id` (the assembly's name), `kind`,
  `properties` (evaluated, so `system` and `bindings` appear as values), `defaults`.
- `ignite build` still ignores assemblies (O8 stays); slice 12's `ignite run` reads the
  resolution.

## 15.7 `ignite plan`

```text
ignite plan <file.ignite> [--assembly NAME] [--out FILE] [--fuel N] [--std PATH]
```

- Prints the model on stdout, or writes it to `FILE` (overwriting it).
- Exit statuses as `ignite check`: 0 model written, 1 diagnostics, 2 invalid arguments, 3 a
  file could not be read or written.
- Nothing is planned against a live system in this slice: no admission, no diff, no effects
  (slice 13).

## 15.8 `ignite explain`

```text
ignite explain <file.ignite> <path> [--assembly NAME] [--fuel N] [--std PATH]
```

- `path` is a system (`Terminal`), a member (`Terminal.reachable`), a property path
  (`Terminal.server.recovery`, `Terminal.server.sessions.idle_timeout`) or an assembly
  (`local`). A path that names none of these is E119.
- Output on stdout, plain text, deterministic like the model:
  - line 1: `<path>: <declaration> <kind> (<file:line:column>)`;
  - then one line per property in the kind's field order: `  <name> = <value>`, with
    ` (default)` after a defaulted one. Values print as slice-9 debug text with quantities in
    their literal form (14.1); `Ref`, `Signal`, `SystemRef` and `Protocol` print their name
    unquoted;
  - then the member's edges, one per line (`uses Terminal.ingress`, `observed by
    Terminal.reachable`), and its resolution line when an assembly is selected;
  - a contract ends with `standing: not evaluated before slice 12`.
- Exit statuses as `ignite plan`.

Golden for `Terminal.reachable`:

```text
Terminal.reachable: contract std.contract.Availability (terminal.design.ignite:81:3)
  signal = Terminal.promptProbe.attempts
  good = "prompt_received_before_timeout"
  target = 99.9%
  window = Rolling(30 days)
  missing_data = "insufficient_evidence"
observes Terminal.promptProbe.attempts
standing: not evaluated before slice 12
```

Golden for `local`:

```text
local: assembly std.deploy.Deployment of Terminal (terminal.design.ignite:108:1)
  Terminal.client = WorkstationExecutable
  Terminal.console via Terminal.server = NativeProcess([Terminal.console])
  Terminal.ingress = LocalSocket
  Terminal.promptProbe = ScheduledProbeHost
  Terminal.server = NativeProcess([Terminal.console]), executable terminald
```

## 15.9 Typed API (add-only)

- `TypedProgram::systems() -> &[SystemInfo]`: per system its members, each a `MemberInfo {
  id, declaration, kind: Ty, properties: Vec<(field, PropertySource)>, outputs, offset }`, with
  `PropertySource::Set(&Expr) | Default(QualName)` naming the default's constant
  (`<Record>.<field>`, slice 9 item 6).
- `TypedProgram::assemblies() -> &[AssemblyInfo]`: name, kind, system, the property
  expressions, offset.
- `TypedProgram::effects(expr) -> &EffectSet`: the inferred effect set of an expression (used
  by E116 and asserted empty by the evaluator).
- `TypedModules` exposes each module's typed program and AST to `ignite-eval`, so that fns of
  std and siblings can be evaluated.
- `ignite-eval`: `Evaluator::new(&TypedModules, Fuel)`, `evaluate(&TypedProgram, &Expr) ->
  Result<Value, EvalError>`, `Value` (15.4), `model(&TypedProgram, assembly: Option<&str>) ->
  Result<Model, Vec<Diagnostic>>`, `Model::to_json() -> String`, `explain(&Model, path) ->
  Result<String, Diagnostic>`.
- `ignite` library: `plan(file, &PlanOptions) -> Result<String, Error>` and `explain(file,
  path, &PlanOptions) -> Result<String, Error>`.

## 15.10 Diagnostics E110 to E119

| code | rule | fixture |
|---|---|---|
| E110 | a resource or component the assembly neither binds nor includes | `e110_unbound_member` |
| E111 | a member bound twice: key and included, or included by two bindings | `e111_bound_twice` |
| E112 | binding not admitted for the member (reason from `admits`), or a contract or policy as a key | `e112_binding_not_admitted` |
| E113 | a binding's `includes` differs from the components its member embeds | `e113_includes_mismatch` |
| E114 | no assembly or system selectable: several and no `--assembly`, or an unknown name | `e114_assembly_selection` |
| E115 | evaluation limit exceeded: fuel or value size | `e115_fuel_exhausted` |
| E116 | effectful property expression | `e116_effectful_property` |
| E117 | failure during evaluation, with the 14.8 report text | `e117_evaluation_failed` |
| E118 | value the model cannot encode | `e118_unencodable_value` |
| E119 | `ignite explain` path names no system, member, property or assembly | `e119_unknown_path` |

Fixtures live in `crates/ignite-types/tests/fixtures/` (E116) and
`crates/ignite/tests/fixtures/` (the rest, run through `ignite check`, `plan` or `explain`).
E119 is the last code of this slice; nothing from E120 on (T4).

## 15.11 Work split

| agent | owns |
|---|---|
| P0 measuring | its own copy of commit `60aef38` and its own target dir; measures and attributes 15.0 items 1 to 3 per 15.1 and reports the numbers to the coordinator; edits no file of the repository and fixes nothing |
| P1 frontend | `ignite-syntax`, `ignite-check`, `ignite-types`: 15.2 (E116), 15.9 typed API, the `crates/ignite-eval` skeleton with its placeholders and the `plan` and `explain` subcommand stubs; `docs/slice.md` for the whole slice |
| P2 evaluator | `crates/ignite-eval` `eval.rs` and `value.rs`: 15.3, E115, E117, E118, the equivalence fixture and test (acceptance 8); nothing else |
| P3 model, resolution, CLI, tests | `crates/ignite-eval` `model.rs`, `resolve.rs`, `explain.rs`; `crates/ignite` `plan`, `explain` and their CLI; `crates/ignite/tests/`, including `main.rs` and `common/mod.rs`; 15.4 to 15.8, E110 to E114, E119; goldens for the design file; the carry-over fixes the coordinator assigns by P0's numbers (15.1), including the flake rewrite |
| P4 std, examples, acceptance | `std/deploy.ignite` `Member`, `admits` and `includes` with inline tests (15.6, first, so that P3 can resolve); model goldens for `examples/terminal/terminal.ignite` and `examples/todo/todo.ignite`; the design 6.2 note of 15.5 and a 10.1 note on `plan` and `explain`; audit README row a21; 15.0 items 4 to 11 verified with commands; `AGENTS.md` layout row for `ignite-eval` and the slice-11 gate-time row; `docs/friction.md`. P4 does not edit `docs/vision.md` |

- Order: P0 and P1 in parallel; then P2, P3 and P4 (P4 starting with `std/deploy.ignite`);
  P4's acceptance last.
- Shared files: `crates/ignite/tests/main.rs` and `tests/common/mod.rs` are P3's. P2's
  equivalence test gets its `mod` line by message to P3, as in slice 10.

## 15.12 Open points for the steering review

| # | default taken |
|---|---|
| O13 | Design 6.2 key names are kept where they exist (`formatVersion` with the value `"ignite-model/0.1"`, `systemId`, `resources`, `components`, `contracts`, `policies`, `sourceMap`); `assembly` and `edges` are additions; additive keys keep the version (steering amendment 3) |
| O14 | Fuel: 1,000,000 units per property expression (expression, iteration, call), value size 1,000,000 nodes; `--fuel` overrides |
| O15 | Member arrays sorted by id, not in declaration order, so that reordering declarations changes nothing (slice 13's plan diff) |
| O16 | Property expressions must be pure (E116); audit probe a21 flips from accepted to E116 |
| O17 | `Int` is a JSON integer even beyond 2^53; consumers parse it as i64. No string fallback |
| O18 | Resolution is defined by `admits` and `includes` in the binding type's module, not by compiler-known variants of `std.deploy.Binding` (invariant 6: domain nouns are library symbols) |
| O19 | Builds do not use the evaluator: constants and property values stay run-time code; goldens unchanged |
| O20 | `ignite check` evaluates every property expression and resolves every assembly, so E110 to E118 appear at check time |
| O21 | Paths in the model and in `explain` are relative to the program file's directory; std files are `std/<path>` |
| O22 | No source hashes and no compiler version in the model, so the golden changes only when the model does; provenance beyond `sourceMap` waits for packages (wave H) |
| O23 | `ScheduledProbeHost` admits a component with at least one `Signal` output; `WorkstationExecutable` any component (`Application` has no `main`, O9) |
| O24 | Explain prints properties in the kind's field order; the model uses byte order |
| O25 | Edge kinds are the syntactic `uses`, `references`, `observes`, `hosts`; design 3.4's provisioning, readiness and recovery classes are slice 12's |
| O26 | Carry-over fixes that leave a number more than 10 % over its baseline need the steering session's written yes before the baseline moves |

## 15.13 Placeholder arms

P1's skeleton adds `crates/ignite-eval` and the `plan` and `explain` subcommands so that the
workspace builds with `--all-targets`. Each placeholder is `unimplemented!("slice 11: …")`,
replaced by its owner:

| owner | file | fn | covers |
|---|---|---|---|
| P2 | `ignite-eval/src/eval.rs` | `Evaluator::evaluate` | 15.3 |
| P2 | `ignite-eval/src/value.rs` | `Value::debug_text` | equivalence, explain values |
| P3 | `ignite-eval/src/model.rs` | `model`, `Model::to_json` | 15.4, 15.5 |
| P3 | `ignite-eval/src/resolve.rs` | `resolve` | 15.6 |
| P3 | `ignite-eval/src/explain.rs` | `explain` | 15.8 |
| P3 | `ignite/src/lib.rs` | `plan`, `explain` | 15.7, 15.8 |
