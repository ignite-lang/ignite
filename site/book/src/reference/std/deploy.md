# std.deploy

Where the members of a system run. An assembly binds each resource and component of a system
to a place: a local socket, a native process, a workstation executable or the host that
schedules probes. `ignite plan`, `ignite check` and `ignite run` resolve an assembly by calling
this module's `admits`, `includes` and `entries`. The compiler knows those function names and
the record `Member`, not the variants of `Binding`.

- Layer: general.
- Source: [`std/deploy.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/deploy.ignite).
- Import: `import std.deploy.{Deployment, localSocket, nativeProcess, workstationExecutable, scheduledProbeHost}`.

## Deployment and bindings

```ignite,fragment
type Deployment {
  system: SystemRef
  bindings: Map<Ref, Binding>
}

enum Binding {
  LocalSocket,
  NativeProcess(List<Ref>),
  WorkstationExecutable,
  ScheduledProbeHost
}

fn localSocket() -> Binding
fn nativeProcess(includes: List<Ref>) -> Binding
fn workstationExecutable() -> Binding
fn scheduledProbeHost() -> Binding
```

`Deployment` is the kind of an `assembly` declaration: a system and the binding of each of its
members, keyed by reference (`Terminal.server`).

| binding | hosts | runs code |
|---|---|---|
| `localSocket()` | a `TcpListener` resource on this machine | no |
| `nativeProcess(includes)` | an executable component, run as a native process that also hosts the members in `includes` | yes |
| `workstationExecutable()` | any component, installed as an executable on a workstation | yes, built but not run by `ignite run` |
| `scheduledProbeHost()` | a component with at least one output signal, run by the host that schedules probes | yes |

Every resource and component must be a key or included by exactly one binding (E110, E111),
and a binding's `includes` must equal the components its member embeds (E113).

## Resolution hooks

```ignite,fragment
type Member {
  declaration: Text
  kind: Text
  executable: Bool
  outputs: List<Text>
}

fn admits(binding: Binding, member: Member) -> Option<Text>
fn includes(binding: Binding) -> List<Ref>
fn entries(binding: Binding) -> List<Text>
fn is_process(binding: Binding) -> Bool
```

| function | what it does |
|---|---|
| `admits` | why `binding` cannot host `member`, or `None` when it can; the reason is the text E112 prints |
| `includes` | the members a binding hosts beside the one it binds; only a native process has any |
| `entries` | the functions the hosted kind must declare for `ignite run` to run it: `["main", "machine"]` for a native process and a scheduled probe host, none otherwise (E129) |
| `is_process` | whether the binding runs code of its own |

`Member` is what the compiler tells `admits` about a member: its declaration (`resource`,
`component`, `contract` or `policy`), the qualified name of its kind, whether it is an
executable (its kind has `main`), and the names of the `Signal` outputs it leaves unset. No
binding admits a contract or a policy.

## In a declaration

From [`terminal.design.ignite`](https://github.com/ignite-lang/ignite/blob/main/examples/terminal/terminal.design.ignite):

```ignite,fragment
assembly local: Deployment {
  system = Terminal
  bindings = {
    Terminal.ingress = localSocket()
    Terminal.server = nativeProcess(
      includes = [Terminal.console]
    )
    Terminal.client = workstationExecutable()
    Terminal.promptProbe = scheduledProbeHost()
  }
}
```

`ignite plan --assembly local` and `ignite explain FILE local` show the resolution; see
[Plan, explain, run and replay](../../guide/operate.md).

## Example

```ignite
import std.deploy.*

test "bindings and what they host" {
  assert is_process(nativeProcess(includes = []))
  assert !is_process(localSocket())
  assert entries(scheduledProbeHost()) == ["main", "machine"]
  assert entries(workstationExecutable()) == []
  let listener = Member { declaration = "resource", kind = "TcpListener", executable = false, outputs = [] }
  assert admits(localSocket(), listener) == None
  assert admits(nativeProcess(includes = []), listener) == Some("a native process hosts an executable component, not a resource")
  assert admits(workstationExecutable(), listener) == Some("a workstation executable hosts a component, not a resource")
}
```
