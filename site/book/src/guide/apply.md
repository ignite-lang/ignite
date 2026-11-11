# Host and apply

`ignite run` runs an assembly in the foreground and stops with it. Local apply separates the
two jobs: a long-running **host** owns a state directory and supervises what runs there, and
**apply** submits a reviewed revision to it. The client may exit, disconnect or retry; the
host keeps the work and the record.

| command | what it does |
|---|---|
| `ignite host --state DIR` | owns one state directory until stopped: lock, control socket, journal, supervision, probes and recovery |
| `ignite plan FILE --for-apply --state DIR --out PLAN` | prepares a saved, source-bound plan against the host's current revision |
| `ignite apply --state DIR PLAN` | submits the plan and waits until it completes |
| `ignite inspect --state DIR` | reads the host's status, or its last durable status when no host runs |

What local apply is not: there is no automatic daemon (`apply` never starts a
host), no cloud provider, no portable bundle, no in-place update and no stateful migration.
A change replaces the owned executable, using the drain protocol of `DrainRollout`.

## The walkthrough

Start a host in one terminal and keep it open:

```sh
ignite host --state .ignite/terminal
```

```text
ignite host: state:28c11b43…d57d0ae revision 0
```

A new state starts empty, at revision 0. The host takes `<state>/lock`, restricts the
directory to its user and listens on `<state>/control.sock`, a Unix-domain socket. It uses the
real clock and runs until SIGINT, SIGTERM or `stop` on its stdin; `status` on its stdin prints
the current status.

In a second terminal, save a plan and apply it:

```sh
ignite plan examples/terminal/terminal.design.ignite --assembly local \
  --for-apply --state .ignite/terminal --out terminal.apply.json
ignite apply terminal.apply.json --state .ignite/terminal
```

`plan --for-apply` needs the running host: it reads the host's base revision and ownership,
checks the source and builds the artifacts, on the LLVM target unless `--target rust` is
given, without changing anything that runs. The saved
plan (`ignite-apply-plan/0.1`) lists one action per member. For the first apply every member
is an `add`:

| member | action | ownership |
|---|---|---|
| `Terminal.client` | add | external |
| `Terminal.console` | add | included |
| `Terminal.ingress`, `Terminal.server`, `Terminal.promptProbe` | add | owned |
| `Terminal.reachable`, `Terminal.promptFast`, `Terminal.customerAccess` | add | owned |

`apply` waits until the operation completes and prints its receipt:

```text
{"baseRevision":0,"currentRevision":1,"noop":false,"operationId":"sha256:8b58e939…","planId":"sha256:8b58e939…", … ,"revision":1,"state":"completed"}
```

The host terminal now shows the run's journal lines: the supervisor spawns `terminald`, the
server reports `Ready`, the probe starts, and the contracts publish standings, as with
`ignite run`. Applying the same file again returns the same completed operation, revision 1,
with no second dispatch.

## Inspect

```sh
ignite inspect --state .ignite/terminal
```

`inspect` prints the status as JSON (`--json` for one compact line, `--operation ID` for one
operation). Shortened, after the apply above:

```text
{
  "actionProgress": { "done": 8, "members": [ { "action": "add", "member": "Terminal.client", "progress": "done", "reason": "member added" }, … ] },
  "activeOperation": null,
  "committedRevision": 1,
  "currentOperation": { "operationId": "sha256:8b58e939…", "reason": null, "revision": 1, "state": "completed" },
  "desiredRevision": 1,
  "formatVersion": "ignite-host-journal/0.1",
  "host": { "pid": 1957070, … },
  "nextAction": "none",
  "online": true,
  "readiness": { "converged": true, "expected": [ "Terminal.customerAccess.evaluator", … ], … },
  "recovery": null,
  …
}
```

With the host stopped, `inspect` reads the last durable status and labels it offline rather
than guessing at live health:

```text
  "health": "unknown: host offline",
  "host": null,
  "nextAction": "start `ignite host --state .ignite/terminal`; this is the last durable status, not live health",
  "online": false,
  "readiness": "unknown: host offline",
```

`inspect` exits 0 when it could read a status. That says nothing about whether an operation
succeeded; read `currentOperation` for that.

## Changes

Replan after an edit and apply again. The saved plan shows `add`, `replace`, `remove`,
`detach` or `unchanged` per member, with reasons and the dependents it affects.

- **Same source, replanned:** an operational no-op. Nothing is replaced and the revision does
  not move.
- **Changed implementation:** the plan pins the content digests of every source, compiler and
  artifact input, so a changed command implementation is a `replace` even when the model's
  properties are identical. Existing connections drain under `DrainRollout`, then the new
  implementation becomes ready.
- **Removal:** remove the server, its listener, the included console, the probe and the three
  contracts that depend on it together. The host drains the server, stops the probes and
  stays running with nothing hosted.
- **External members** are observed and detached, never killed or deleted.

A plan is bound to its sources: if an input changed between `plan` and `apply`, the plan is
rejected and must be made again. Apply never rebuilds behind your back.

## When something fails

- The retry key is the state identity plus the plan id. A client that lost its connection
  should `inspect` or retry the same plan, not assume a rejection. A retry returns the same
  operation.
- One apply runs at a time per state. A second distinct apply is rejected as busy.
- A failed operation keeps the previous committed revision. When `inspect` reports
  `recovery_required`, follow its `nextAction`; [Recover a host](../howto/recover.md) covers
  the case no corrective plan resolves.

What the host syncs, commits and reconciles is in
[The run-time model](../design/runtime.md#the-host). The exit statuses of `host`, `plan
--for-apply`, `apply` and `inspect` are in the
[command-line reference](../reference/cli.md#ignite-apply). The complete rules, including the
known gaps, are in
[`docs/slice.md`](https://github.com/ignite-lang/ignite/blob/main/docs/slice.md) section 17.
