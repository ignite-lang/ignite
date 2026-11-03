# Draft: `docs/slice.md` section 14, slice 10: the system declaration (F1)

Status: draft for the steering review, 2026-11-01. Not in `docs/slice.md` yet. Sources:
vision 4.1 row 10, T1, T4, T5, T6, section 6 (X5, X21, X22), 10.3 (R2 to R5); design 3.2,
3.3, 4, 5, 11.3; `examples/terminal/terminal.design.telos`. Open points are marked **O1** to
**O12**, each with the recommended option, and collected in 14.13.

## 14.0 Acceptance

1. `telos check examples/terminal/terminal.design.telos` exits 0. Edits to that file are
   allowed only as listed in O5, mirrored in design §11.3 in the same change.
2. `examples/terminal/terminal.telos` and `examples/todo/todo.telos` are in the component form
   (14.4). `telos build` emits `terminald`, `tn` and `todod`. The terminal suite (64 cases
   across both targets), the todo suite and the 78 inline tests per target pass unchanged.
3. T1: a failure ends its task. `terminald` survives a session whose handler fails an
   `assert`, overflows or indexes out of range, on both targets. Every failure report names
   the Telos file, line, column and operation, identical on both targets.
4. R2 to R5 are fixed, and `docs/audit/2026-11-01/io/iotest` `drop` and `cross` behave as the
   audit README's "expected after the fix" column says.
5. Invariant 9: the `task gate` wall time and the `todod` build time (both targets) are
   measured before and after, and recorded in the `AGENTS.md` "Gate time" section. A rise of
   more than 10 % is a deviation and has to be explained.
6. Every new diagnostic (E100 to E109) has a fixture.

## 14.1 Quantity literals and quantity types

- Syntax: an Int literal or a decimal literal, then a unit name on the same line:
  `4096 bytes`, `16 KiB`, `15 minutes`, `100 ms`, `99.9%`.
  - A unit name is an identifier from the table below, not a keyword.
  - The pair binds tighter than any binary operator.
  - A decimal literal (`99.9`) exists only directly before `%` or a unit. There is no
    `Decimal` type in slice 10.

| type | units (singular and plural accepted) | representation |
|---|---|---|
| `Duration` | `ns`, `us`, `ms`, `second(s)`, `minute(s)`, `hour(s)`, `day(s)` | Int nanoseconds |
| `ByteSize` | `byte(s)`, `KiB`, `MiB`, `GiB` (1024-based), `KB`, `MB`, `GB` (1000-based) | Int bytes |
| `Ratio` | `%` | Int parts per million; at most 4 decimals, so `99.9%` is 999,000 |

- The name is `ByteSize`, not design §3.3's `Bytes`, because `Bytes` is `List<Byte>` today.
  Design §3.3 gets a one-line note.
- Operations:
  - same-type `+`, `-` and comparison;
  - `* Int` and `/ Int`;
  - `Duration.to_ms()`, `.to_ns()` and `ByteSize.to_int()`;
  - `Ratio.ppm()`;
  - `debug()` prints the literal form in the largest exact unit.
- Overflow follows T6. A literal out of range is E107.
- A unit applied to the wrong kind of value, or mixed units (`1 ms + 1 bytes`), is E101.
- `Rate<Event>`, `CPU`, `Money` and `Decimal` are not in this slice.

## 14.2 Named arguments (T5 a)

- `f(a, b, name = e)`: positional arguments first, then named ones.
- Allowed only for a direct call of a fn declared in Telos, including associated fns and
  UFCS. Not through fn values, closures or protocol roles.
- Each named argument must name a parameter that no positional argument covers.
- An unknown name, a duplicate, a positional argument after a named one, or use through a fn
  value is E103.
- Lowering: the arguments are reordered at the call, and evaluation order stays source order
  (each argument goes into a temporary first when any argument has an effect).

## 14.3 Brace literals and `Map<K, V>` (T5 b, c)

- A brace literal `{ k = e, … }` without a type name takes its type from the expected type:
  - a record type makes it a record literal (the slice-9 rules, defaults and `..base`);
  - `Map<K, V>` makes it a map literal.
    - Identifier keys give `K = Text`.
    - Qualified component references (`Terminal.ingress`) give `K = Ref<C>` for a common
      kind, or the kind-erased `Ref` when the kinds differ (O2).
- There is no type named `Any`. `Ref<K>` coerces to `Ref` wherever a `Ref` is expected.
- A brace literal with no expected type is E100.
- `Map<K, V>`: `Map.empty()`, `get(k) -> Option<V>`, `put(k, v)`, `remove(k)`, `keys()`,
  `len()`, `contains(k)`, and `for (k, v) in m` in key order.
  - Value semantics, as for `List`.
  - Structural `==` and debug text are `{k: v, …}` in key order.
- Key types: `Int`, `Byte`, `Text`, `Bool`, enums without payloads (in declaration order), and
  `Ref<C>`. Any other key type is E106 (O11).
- Rust: `std::collections::BTreeMap`. LLVM: a sorted array of pairs with binary search, plus
  copy and drop glue (O11).

## 14.4 System declarations: resources, components, references

The grammar is unchanged from `slice.md` section 3. What changes is what a kind is: a component
kind is an ordinary Telos record type (D6), declared in std or in the program.

- `component name: K { props }`: `K` is a record type. `props` is a record literal of `K` with
  the slice-9 rules, so fields with defaults may be left out. A field without a default that is
  left out is E104.
- `resource name: K { props }`: in slice 10 the resource kinds stay the two compiler-known
  ones, `TcpListener` and `Store<T>`. `TcpListener` gains `exposure: Exposure`
  (`Loopback | Private | Public`, default `Loopback`) and `backlog: Int` (default 128).
  `exposure = Loopback` with a non-loopback address is E104.
- A qualified enum variant `Type.Variant` (`Exposure.Loopback`) is accepted wherever an enum
  value is expected, when `Type` names an enum. The bare form stays valid.
- Names inside a `system` (O2):
  - A bare resource or component name used in a property expression is a value of its
    declared type, as resources are today. `application = console` passes the console's
    record value.
  - The qualified form `System.name` is a reference, `Ref<K>`, and is used as an assembly key
    and as a probe `target`.
  - A cycle of bare-name uses among components (`a` uses `b`, `b` uses `a`) is E105.
- Output fields (O3): a field of a kind whose type is `Signal<T>` is an output.
  - Setting it in a declaration is E104.
  - `promptProbe.attempts` is a value of type `Signal<T>` that names the component and the
    field.
  - Signals carry no runtime behaviour in slice 10; slice 12 gives them one.
- `module("p")` (O4): its type is `Module<I>`, where `I` is the expected record type whose
  fields are all fn types.
  - `p` names a module of the program's package (`examples.terminal.commands` is
    `examples/terminal/commands.telos`) or a std module.
  - The literal becomes a record `I` whose fields are filled by name from the module's public
    fns, and their types must match.
  - A module that is not found, or a missing or mismatched fn, is E102.
  - This adds sibling modules of the program's package to the module loader. Package
    manifests and lockfiles stay wave H.

## 14.5 Executable components (O1)

- A kind `K` produces an executable when it has the associated fn
  `fn K.main(self: K) -> Result<(), IoError>`, declared in K's module, with an inferred
  `requires`.
- The generated `main`, on both targets:
  1. runs under `telos_io::run`;
  2. creates the `Store` resources;
  3. binds each `TcpListener` resource the component uses, with the `--port` override and the
     `listening on` log as today (section 6.1);
  4. builds the component's record value, with resource handles and the values of embedded
     components substituted;
  5. calls `K.main(value)`;
  6. maps an `Err` to its exit code.
- The accept loop and the per-connection `spawn` move from generated code into Telos:
  - `fn TelnetServer.main` and `fn TelnetClient.main` in `std/protocol/telnet.telos`;
  - `fn HttpServer.main` in `std/protocol/http.telos`.

  The compiler-known kinds `Service` and `Client` are removed.
- Exit statuses stay those of section 6.1 (1 to 4). New builtin `IoError.with_exit_code(n)`
  (O12). Bind failure (1) and `--port` errors (2) stay in the generated part.
- `executable` is a field of each executable kind, defaulting to the component name. The
  examples set `executable = "terminald"`, `"tn"` and `"todod"`.
- Components without `K.main` (`CommandConsole`, `TelnetPrompt` and `Application` in slice
  10) produce no executable. They are values, embedded where a bare name uses them.

Example, the buildable terminal (`examples/terminal/terminal.telos`):

```telos
system Terminal {
  resource ingress: TcpListener { address = "127.0.0.1", port = 2323 }
  component console: CommandConsole {
    implementation = module("examples.terminal.commands")
    prompt = "terminal> "
    maximum_line = 4096 bytes
  }
  component server: TelnetServer { listener = ingress, application = console, executable = "terminald" }
  component client: TelnetClient { target = ingress, executable = "tn" }
}
```

Whether the buildable example keeps its fns in one file (with `module("examples.terminal")`
naming itself) or splits out `commands.telos` is the implementing agent's choice. Both must
work.

## 14.6 Std types (Telos, no runtime behaviour beyond 14.5)

| module | types and fns |
|---|---|
| `std/console.telos` | `CommandConsole { implementation: Module<ConsoleCommands>, prompt, maximum_line: ByteSize, command_schema: Option<Module<CommandSchema>> = None }`, `ConsoleCommands { respond: fn(Text) -> Reply }`; the existing `Console` stays as the runtime value built from it |
| `std/protocol/telnet.telos` | `TelnetServer`, `TelnetClient`, `TelnetProfile`, `SessionLimits { maximum, idle_timeout: Duration, input_buffer: ByteSize, output_buffer: ByteSize, slow_reader_deadline: Duration }`; `idle_timeout` replaces `IDLE_TIMEOUT_MS` |
| `std/protocol/http.telos` | `HttpServer { listener, application: HttpApp<S>, executable }` |
| `std/contract.telos` | `Availability`, `ThresholdCompliance`, `SLA`, `Window` (`rolling(Duration)`, `calendar_month(Text)`), `Consequence` (`event(Text)`), `MissingData` |
| `std/policy.telos` | `Restart { backoff: Backoff, maximum_attempts, interval: Duration }`, `Backoff` (`exponential(Duration, Duration)`, `fixed(Duration)`), `DrainRollout { deadline: Duration, on_deadline: Text }` |
| `std/probe.telos` | `TelnetPrompt { target: Ref<TelnetServer>, interval, timeout, expected_prompt, close_after_probe, attempts: Signal<ProbeAttempt> }`, `ProbeAttempt` |
| `std/deploy.telos` | `Deployment { system: SystemRef, bindings: Map<Ref, Binding> }`, `Binding` (`localSocket()`, `nativeProcess(includes: List<Ref>)`, `workstationExecutable()`, `scheduledProbeHost()`) |
| `std/cli.telos` | `Application { executable, commands: Map<Text, Command> }`, `Command`, `Argument { name, kind: ArgKind }`, `ArgKind` (`endpoint(Protocol)`, `text()`, …) (O6, O9) |

- Every closed-set field that the design writes as a string stays `Text` in slice 10, with no
  exceptions: `good`, `missing_data`, `on_deadline`, `terminal`, `baseline`, `negotiation`,
  `unsupported_options` and `character_encoding`. Slice 12 turns each into an enum when it
  gives it behaviour.

## 14.7 `contract`, `policy`, `assembly` declarations

- `contract name: K { … }` and `policy name: K { … }` are declarations inside a `system`, with
  the same rules as `component` but never executable.
- `assembly name: K { … }` is a top-level declaration whose kind is a record type
  (`Deployment`). `system = Terminal` is a `SystemRef`.
- Every `bindings` key must be a component or resource of that system, otherwise E108.
- `telos build` does not read assemblies in slice 10 (O8); slice 11 resolves them.
- `system`, `component` and `resource` are keywords already. `contract`, `policy` and
  `assembly` become keywords, which takes the keyword count from 29 to 32. Check `std/` and
  `examples/` for identifiers with these names.

## 14.8 T1: a failure ends its task

- A panic, a failed `assert` or a failed `?` outside a test ends the task that ran it, on
  every target. So does T6 overflow, an index out of range, a division by zero or a
  non-exhaustive match.
- The substrate logs `task ended with error: <report>`.
- The process exits 101 only when the failing task is the main task or a test.
- Report format, identical on both targets:
  `error: <operation> failed at <file>:<line>:<column>: <detail>`.
  - Example: `error: Int + overflowed at examples/todo/todo.telos:41:17: 9223372036854775807 + 1`.
  - The operations are `assert`, `?`, `+`, `-`, `*`, `/`, `%`, `index`, `match`, `unwrap` and
    `slice`.
  - For `assert a == b`, the detail is the slice-9 left and right debug text.
- Rust target:
  - Each failure site calls `telos_io::fail(op, file, line, col, detail)`. Arithmetic uses
    `checked_*`, so rustc's panic text is gone.
  - `telos_io::spawn` wraps the task in `catch_unwind`. The main task's panic exits 101.
- LLVM target (O10):
  - `setjmp` never sits in a Rust frame, because `returns_twice` is undefined in Rust. It
    lives either in the emitted IR's task-entry wrapper (a `setjmp` call with the
    `returns_twice` attribute, then the task body) or in a small C file built by `telos-rt`.
  - `telos_rt_fail` prints the report, releases the task's arena regions and handles, then
    `longjmp`s to that wrapper, which calls `task_end`.
  - No landing pads in the emitted IR.
  - A Rust panic inside `telos-io` or `telos-rt` is caught by `catch_unwind` at the task
    thread and reported in the same format. It is never `longjmp`ed over.
- `slice.md` §7 and §12.5 and `llvm.md` §4 change to match. X5 and X22 are resolved.

## 14.9 Substrate items R2 to R5 (vision 10.3)

| item | change | acceptance |
|---|---|---|
| R2 | a weak reference in the pending list; send plus close in the stream's `Drop` | `iotest drop`: the peer reads the bytes without waiting for a flush |
| R3 | `close` sets a `closed` flag, and `write_all` after it returns `Err` on the writer's side | `iotest cross`: the error reaches the writer and not the other task's `read` |
| R4 | `transient` also covers EPROTO, ENETDOWN, EHOSTUNREACH, ENETUNREACH and ENONET | a unit test of `transient` |
| R5 | remove the flush in `exit` | a unit test: exit with a stalled writer returns |

## 14.10 Typed API (add-only)

- `TypedProgram::quantity(expr) -> Option<(QuantityKind, i64)>`.
- `TypedProgram::named_args(call) -> Option<&[usize]>`, the permutation.
- `TypedProgram::brace_kind(expr) -> BraceKind { Record(TypeId), Map(TypeId, TypeId) }`.
- `TypedProgram::executables() -> &[ExecutableInfo]`: component, kind, main fn, resources
  used and embedded components.
- `TypedProgram::module_record(expr) -> &ModuleRecord`: the fn per field.
- `TypedProgram::failure_site(expr) -> Option<FailureSite { op, file, line, col }>` for every
  expression that can fail.
- `Symbols`: `contract`, `policy` and `assembly` declarations, kept for slice 11.

## 14.11 Diagnostics E100 to E109

| code | rule |
|---|---|
| E100 | brace literal with no expected type |
| E101 | unit on the wrong kind of value, or unit mismatch in an operation |
| E102 | `module("…")`: module not found, or a fn missing or of the wrong type |
| E103 | named argument: unknown name, duplicate, positional after named, or used through a fn value |
| E104 | component or resource property; three distinct message texts and three fixtures: `required field <f> of <K> is missing`, `output field <f> of <K> cannot be set`, `exposure Loopback needs a loopback address, got <a>` |
| E105 | cycle among components through bare-name uses |
| E106 | `Map` key type not orderable |
| E107 | quantity literal out of range, or more than 4 decimals |
| E108 | assembly binding key not a member of the named system |
| E109 | executable kind's `main` has the wrong signature |

E109 is the last code of this slice. Nothing from E110 on (T4).

## 14.12 Work split (proposal; dispatched only after review)

| agent | owns |
|---|---|
| P1 frontend | syntax, check and types: 14.1 to 14.4, 14.7, 14.10, 14.11, `docs/slice.md` §14 |
| P2 Rust target and substrate | `telos-rustgen`, `telos-io`: Map, quantities, the 14.5 mains, the T1 Rust half, R2 to R5 |
| P3 LLVM target | `telos-llvm`, `telos-rt`, `docs/llvm.md`: Map, quantities, the 14.5 mains, the T1 LLVM half (setjmp, per-task release) |
| P4 std and examples (after P1 to P3) | 14.6 std modules, both examples in the component form, the design-file edits of O5, design §11.3 and §3.3 notes, friction rows, gate-time record |

## 14.13 Open points for the steering review

| # | question | options | recommendation |
|---|---|---|---|
| O1 | How does a component kind become an executable? | A: a record type plus associated `fn K.main(self)` in Telos; the generated main binds resources and calls it, and the accept loop moves to std. B: keep the compiler-known `Service`/`Client` lowering, keyed on std type names | A: it follows the AGENTS.md rule "protocols … are Telos in std/" |
| O2 | What is a component or resource name inside a system? | A: a bare name is a value of its type, and `System.name` is `Ref<K>`. B: every name is a `Ref<K>`, read through `.value` | A: today's examples already use bare resource names as values |
| O3 | How are outputs such as `promptProbe.attempts` declared? | A: fields of type `Signal<T>` are outputs, not settable. B: an `out` keyword on fields | A: no new keyword |
| O4 | What does `module("…")` resolve to? | A: a sibling module of the program's package or a std module, giving a record of fns by name. B: check-only resolution with no codegen | A: this adds sibling modules ahead of wave H |
| O5 | Can the design file change? It cannot compile as written: `module("terminal.commands")` has no package prefix; `module("std.telnet.connect")` names a role on a stale path; `type = Endpoint<Telnet>` uses a keyword field and a type as a value | A: minimal listed edits, mirrored in design §11.3. B: language features to accept it verbatim | A |
| O6 | How is `type = Endpoint<Telnet>` handled? | A: `kind = endpoint(Telnet)`, using protocol values that exist today. B: keyword field names plus types as values | A |
| O7 | How are quantities represented? | `Duration` as Int ns, `ByteSize` as Int bytes, `Ratio` as Int ppm; decimal literals only before a unit or `%`; no `Decimal` | as stated; `Decimal` waits for a consumer |
| O8 | Does `telos build` read assemblies in slice 10? | A: no, they are typed only. B: yes | A: vision row 11 puts binding resolution in slice 11 |
| O9 | What is the client's form? The design uses `std/cli` `Application` with a command map, but "behaviour unchanged" needs plain `tn` | A: `std/cli` types are checked only, and the buildable example uses `TelnetClient` with `main`. B: implement CLI dispatch now, which changes `tn`'s argv | A |
| O10 | How does the LLVM target contain a failure to its task? | A: `setjmp` at task entry and `longjmp` from `telos_rt_fail`, with per-task arena and handle release. B: unwind with landing pads | A: no landing pads in emitted IR, smaller compile time (invariant 9) |
| O11 | What are `Map` keys, and how is a Map stored on LLVM? | keys: scalars, payload-free enums and `Ref`; LLVM storage: a sorted array | as stated; a B-tree is a later labelled milestone |
| O12 | How do executable kinds in Telos set exit codes? | A: builtin `IoError.with_exit_code(n)`. B: exit codes per kind in the compiler | A |
