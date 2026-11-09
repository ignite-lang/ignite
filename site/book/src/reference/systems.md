# System declarations

This page is the reference for the system half of a program: `system`, its members, the
kinds they use, the edges between them, `assembly`, the hooks a kind declares, the system
model `ignite plan` prints and the journal `ignite run` writes. The syntax is in the
[Grammar](grammar.md). Why systems and assemblies work this way is in
[Systems and assemblies](../design/systems.md).

## Declarations

```ignite,fragment
system Name {
  resource ingress: TcpListener { address = "127.0.0.1", port = 7080 }
  component server: Kind { listener = ingress }
  contract reachable: Availability { … }
  policy access: PolicyKind { … }
}

assembly local: Deployment { system = Name, bindings = { … } }
```

| member | kind | meaning |
|---|---|---|
| `resource name: K { .. }` | one of the resource kinds below | something the substrate creates: a socket, storage, a directory, a capability |
| `component name: K { .. }` | a record type, from std or the program | a part of the system. With `K.main` it is an executable |
| `contract name: K { .. }` | a contract kind, such as those of [`std.contract`](std/contract.md) | an objective over a signal |
| `policy name: K { .. }` | a record type | a rule that applies to members. The terminal design states its policies as properties instead, such as `recovery = Restart { .. }` from [`std.policy`](std/policy.md) |
| `assembly name: K { .. }` | a top-level declaration, its kind usually [`std.deploy.Deployment`](std/deploy.md) | where each member of one system runs |

Every member has the identity `<System>.<name>`. The braces of a member hold a record literal
of its kind: fields with a default may be left out, a field without one is E104. A `resource`
may leave out its braces when its kind has no required property (`resource todos:
Store<Todo>`). Members are separated by line breaks. A `system` in a standard-library module
is E020. A property expression is pure (E116) and evaluated at compile time.

## Resource kinds

The resource kinds are known to the compiler. Any other kind is E020. Each property value of a
capability kind must be a literal (E104).

| kind | properties, default | what the substrate creates | page |
|---|---|---|---|
| `TcpListener` | `address` and `port` required; `exposure` (`Loopback`), `backlog` (128), `keep_through_drain` (none) | a bound listening socket. `exposure = Loopback` with a non-loopback address is E104 | [`std.net`](std/net.md) |
| `Store<T>` | none | shared in-memory storage keyed by `Int` | [`std.io`](std/io.md#store) |
| `Directory` | `path` required; `create` (`false`), `flag` (`""`) | the files under one root, opened at launch | [`std.fs`](std/fs.md) |
| `Launcher` | `maximum_children` (16, 1 to 1024) | starts child processes | [`std.process`](std/process.md) |
| `Entropy` | none | random bytes | [`std.process`](std/process.md) |
| `Setting` | `flag` required; `default` (none) | one command-line value | [`std.process`](std/process.md) |
| `CommandLine` | none | the process arguments | [`std.process`](std/process.md) |
| `Environment` | none | the environment variables | [`std.process`](std/process.md) |

A `flag` is `--` followed by lowercase letters, digits or `-`. Two resources with one flag, or
the flag `--port` or `--ref`, is E141. How the generated `main` reads flags is in
[Compiled executables](cli.md#compiled-executables). A `Setting` without a `default` makes its
flag required at launch, and `ignite run` refuses it (E142).

## Names inside a system

| written | where | means |
|---|---|---|
| `ingress` | a property that expects a value | the member's value: a resource handle or a component's record. The member is embedded |
| `server` or `Name.server` | a property that expects a `Ref<K>` or `Ref` | a reference: the member is named, not embedded |
| `Name.server` | an assembly key | the member to bind |
| `probe.attempts` | a property that expects a `Signal<T>` | the output signal of a component |
| `module("examples.terminal.commands")` | a property of type `Module<I>` | a record of fns filled from a module ([Module bindings](modules.md#module-bindings)) |

A field of a kind whose type is `Signal<T>` is an output. Setting it in a declaration is E104.
A cycle of embedding uses among components is E105. Two members that only reference each
other are no cycle.

## Edges

The compiler derives the edges of the system graph from the names above, the assembly and the
machines. `ignite plan` prints them in the model, and `ignite explain` prints those of one
member.

| edge | from | terminal example |
|---|---|---|
| `uses` | a member name where a value is expected: the value is embedded | `Terminal.server` uses `Terminal.ingress` |
| `references` | a member name where a `Ref` is expected | `Terminal.promptProbe` references `Terminal.server` |
| `observes` | a `Signal` value | `Terminal.reachable` observes `Terminal.promptProbe.attempts` |
| `hosts` | the assembly: one member runs inside another | `Terminal.server` hosts `Terminal.console` |
| `provisioning` | a resource and the component that binds it | `Terminal.ingress` to `Terminal.server` |
| `readiness` | what a probe or an evaluator waits for | `Terminal.promptProbe` to `Terminal.server` |
| `recovery` | a member and its supervisor machine | `Terminal.server` to `Terminal.server.supervisor` |

## Assemblies

An assembly binds every resource and component of one system to a place. The compiler knows
no binding variants. It calls three functions of the binding type's module: `admits`,
`includes` and `entries` ([`std.deploy` resolution hooks](std/deploy.md#resolution-hooks)). A
binding type without them gets `"resolution": null` in the model. `ignite check` applies these
rules to every assembly, `ignite plan` to the selected one:

| rule | diagnostic |
|---|---|
| an assembly key is a member of the system, as `System.member` | [E108](diagnostics.md#e108) |
| every resource and component is a key or included by exactly one binding | [E110](diagnostics.md#e110) |
| no member is both a key and included, or included twice | [E111](diagnostics.md#e111) |
| `admits` accepts the member, and no contract or policy is a key | [E112](diagnostics.md#e112) |
| a binding's `includes` equals what its member embeds | [E113](diagnostics.md#e113) |
| one assembly or system can be selected: one only, or `--assembly` names it | [E114](diagnostics.md#e114) |
| every contract's signal has a producer the assembly runs | [E121](diagnostics.md#e121) |
| a run-time reference names a member the assembly runs and resolves to one listener | [E122](diagnostics.md#e122) |
| a hosted member's kind declares every hook its binding's `entries` names | [E129](diagnostics.md#e129) |

A capability resource such as a `Directory` or a `Setting` needs no binding of its own: it is
placed with the component that uses it, as `via` in the model shows.

## Hooks

A kind `K` gives its members behaviour through associated fns in its module. Any other
signature, or an effect in a pure hook, is [E109](diagnostics.md#e109).

| hook | signature | used by |
|---|---|---|
| `main` | `fn K.main(self: K) -> Result<Unit, IoError>`, effects inferred | `ignite build`: one executable per component of kind `K`. `main` is an [effect root](effects.md) |
| `validate` | `fn K.validate(self: K) -> List<Text>`, pure | `ignite check`: each text is one [E120](diagnostics.md#e120) reason. Called for every member whose kind has it and every record value inside a property whose type has it |
| `machine` | `fn K.machine(self: K, id: Text) -> std.control.Machine`, pure | `ignite run`: the member's control machine, from [`std.control`](std/control.md). Called at plan time, so the initial machines are in the model |
| `classify` | `fn K.classify(self: K, sample: T) -> Bool`, pure | `ignite run`: whether one sample of the contract's signal of `Signal<T>` was good. Required when the contract's machine subscribes to a signal ([E150](diagnostics.md#e150)) |

## Rules by code

Every rule of system declarations, in code order. Each code links its entry with an example.

| code | rule |
|---|---|
| [E020](diagnostics.md#e020) | unsupported or malformed declaration |
| [E100](diagnostics.md#e100) | brace literal without an expected type |
| [E101](diagnostics.md#e101) | quantity units do not fit |
| [E102](diagnostics.md#e102) | `module(..)` cannot be filled |
| [E103](diagnostics.md#e103) | named argument misuse |
| [E104](diagnostics.md#e104) | component or resource property: a required one missing, an output set, a value refused, or no literal where one is needed |
| [E105](diagnostics.md#e105) | component cycle |
| [E106](diagnostics.md#e106) | map key type |
| [E107](diagnostics.md#e107) | quantity literal out of range |
| [E108](diagnostics.md#e108) | assembly binding key |
| [E109](diagnostics.md#e109) | hook signature |
| [E110](diagnostics.md#e110) | member neither bound nor included |
| [E111](diagnostics.md#e111) | member bound twice |
| [E112](diagnostics.md#e112) | binding not admitted |
| [E113](diagnostics.md#e113) | includes differ from the embedded components |
| [E114](diagnostics.md#e114) | no assembly or system selectable |
| [E115](diagnostics.md#e115) | evaluation limit exceeded |
| [E116](diagnostics.md#e116) | effectful property expression |
| [E117](diagnostics.md#e117) | evaluation failed |
| [E118](diagnostics.md#e118) | value the model cannot encode |
| [E119](diagnostics.md#e119) | unknown explain path |
| [E120](diagnostics.md#e120) | declaration refused by `validate` |
| [E121](diagnostics.md#e121) | signal without a producer |
| [E122](diagnostics.md#e122) | unresolvable run-time reference |
| [E123](diagnostics.md#e123) | probe timing |
| [E124](diagnostics.md#e124) | contract window too short |
| [E125](diagnostics.md#e125) | nothing to run |
| [E126](diagnostics.md#e126) | journal of another model, or a held lock |
| [E127](diagnostics.md#e127) | journal does not replay |
| [E128](diagnostics.md#e128) | reducer failed at run time |
| [E129](diagnostics.md#e129) | hosted member lacks an entry |
| [E141](diagnostics.md#e141) | flag used twice |
| [E142](diagnostics.md#e142) | setting without a default |
| [E150](diagnostics.md#e150) | contract without `classify` |
| [E151](diagnostics.md#e151) | signal element is no outcome type |

## Example

```ignite
import std.control.{Machine, Supervisor}
import std.deploy.{Deployment, localSocket, nativeProcess}
import std.policy.Restart

type Archive {
  listener: TcpListener
  files: Directory
  mode: Setting
  executable: Text
  recovery: Restart = Restart {}
}

fn Archive.main(self: Archive) -> Result<Unit, IoError> {
  loop {
    let stream = self.listener.accept()?
    stream.write_all(self.mode.value().to_bytes())?
    stream.close()?
  }
}

fn Archive.machine(self: Archive, id: Text) -> Machine = Machine.Supervise(Supervisor {
  id = "{id}.supervisor"
  member = id
  restart = self.recovery
})

system Vault {
  resource ingress: TcpListener {
    address = "127.0.0.1"
    port = 7080
  }
  resource data: Directory { path = "archive-data", create = true, flag = "--root" }
  resource mode: Setting { flag = "--mode", default = "read-only" }

  component server: Archive {
    listener = ingress
    files = data
    mode = mode
    executable = "archived"
  }
}

assembly local: Deployment {
  system = Vault
  bindings = {
    Vault.ingress = localSocket()
    Vault.server = nativeProcess(includes = [])
  }
}
```

```text
$ ignite explain vault.ignite local
local: assembly std.deploy.Deployment of Vault (vault.ignite:43:1)
  Vault.data via Vault.server = NativeProcess([])
  Vault.ingress = LocalSocket
  Vault.mode via Vault.server = NativeProcess([])
  Vault.server = NativeProcess([]), executable archived
```

## The system model

`ignite plan` prints the model of one system and at most one assembly as canonical JSON,
format `ignite-model/0.1`. Equal inputs give equal bytes:

- object keys in byte order, two-space indentation, one key or item per line;
- members, edges and resolution entries sorted, so declaration order does not show;
- relative paths, and no absolute path, timestamp, host name or compiler version.

| key | content |
|---|---|
| `formatVersion` | `"ignite-model/0.1"` |
| `systemId` | the system's name |
| `resources`, `components`, `contracts`, `policies` | every member: `id`, `kind`, its evaluated `properties`, the `defaults` it left out, `outputs` and `executable` |
| `edges` | each edge as `from`, `kind`, `to` |
| `assembly` | the selected assembly, its properties and its `resolution`: per resource and component, its `binding` and `via`, the key that hosts it. `null` without an assembly |
| `machines` | the initial control machines |
| `observations` | the signals, each with its `producer`, `element` type and `interval` |
| `sourceMap` | every member and property path to `file:line:column` |

Values are encoded by type. A quantity keeps its unit, for example
`{"ns": 10000000000, "type": "Duration"}`. A variant is `{"type", "variant", "fields"}`, a
reference `{"ref", "type": "Ref"}`. A change that removes or reinterprets a key bumps
`formatVersion`. An added key does not, so a reader ignores keys it does not know.
`ignite explain FILE PATH` renders the same model for one system, member, property path,
assembly or machine id such as `Terminal.server.rollout`. An unknown path is E119.

## The journal

`ignite run` writes `<state>/journal.jsonl`, and `ignite replay` reads it. The state directory
holds:

```text
target/ignite-run/local/
  bin/            the executables, one directory per generation
  build/          the generated build project
  journal.jsonl   one JSON record per line
  lock            held while a run or host owns the directory
```

Each line is one JSON object, keys in byte order, values in the model encoding. Line 1 is the
header:

```text
{"epoch":{"ns":…,"type":"Duration"},"formatVersion":"ignite-journal/0.2","kind":"header","model":{…the full model…},"seq":0}
```

Every other record has this shape:

```text
{"at":{"ns":…,"type":"Duration"},"kind":"<kind>","machine":"<machine id or null>","seq":<n>,"value":<value>}
```

| kind | value | written when |
|---|---|---|
| `segment` | `std.control.Segment`: start or stop, clock mode, generation, declared members that are not buildable | at run start, resume and stop |
| `event` | `std.control.Event` | before the reducers step it |
| `effect` | `std.control.Effect` | after the step, synced to disk before it is carried out |
| `signal` | `{"ref", "value"}`, an emission routed to subscribers | when a reducer emits |
| `standing` | `std.control.Standing` | at every evaluation |
| `consequence` | `std.control.Fired` | at a period close with a violation |
| `error` | the failure report and its code, E128 | at a reducer failure |

- `seq` is gap-free from 0. `at` is on the run's clock.
- A run on an existing journal with the same model and version appends a `segment` record. A
  journal of another model or version is E126.
- `--fresh` first renames the old file to `journal.<n>.jsonl`.
- A final line without a line break, from a crash while writing, is dropped on open and
  reported once on stderr. Any other line that does not parse is E127.
- There are no snapshots. Resume and replay re-step from `seq` 1.
