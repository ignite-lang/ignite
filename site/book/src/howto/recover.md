# Recover a host

A failed operation on a [host](../guide/apply.md) becomes `recovery_required`. The host keeps
the committed revision serving and refuses new plans until the operation is resolved. This
page resolves it, first with a corrective plan, then with `ignite recover` when no plan can.

## Read what the host asks for

Here an apply added a second server whose port was taken, so its supervisor gave up:

```text
$ ignite apply --state .ignite/terminal r3.json
error: gave_up
```

`ignite apply` exits 1. `ignite inspect --state .ignite/terminal` shows the state, shortened:

```text
  "activeOperation": "sha256:3b229e20…",
  "committedRevision": 2,
  "nextAction": "apply a corrective plan prepared against the committed revision; then prepare and apply a corrective plan",
  "recoveryReason": "gave_up",
```

## Apply a corrective plan

When `nextAction` asks for a corrective plan, fix the source, then plan and apply again:

```sh
ignite plan examples/terminal/terminal.design.ignite --for-apply --state .ignite/terminal --out corrective.json
ignite apply --state .ignite/terminal corrective.json
```

The corrective resolves the failed operation. `ignite recover` refuses an operation that a
corrective can resolve:

```text
$ ignite recover --state .ignite/terminal --operation sha256:3b229e20…
error: operation sha256:3b229e20… requires no recovery
```

It exits 2.

## Recover when no plan resolves it

Some reasons recorded after the first one cannot be resolved by a plan. One example is
`ready process artifact differs from the admitted desired artifact`: the run's copy of an
executable no longer matches the plan. `inspect` then shows another next action, and the
receipt keeps the later reasons:

```text
  "nextAction": "inspect retained ownership and journal before recovery",
  "recoveryReason": "gave_up",
```

Every apply is refused:

```text
$ ignite apply --state .ignite/terminal corrective.json
error: host has an active or unresolved operation
$ ignite apply --state .ignite/terminal r3.json
error: operation requires recovery; inspect its durable status
```

The first exits 2, the retry of the failed plan exits 1. Abandon the operation:

```text
$ ignite recover --state .ignite/terminal --operation sha256:3b229e20…
recovered sha256:3b229e20…: stopped 0 instances; committed revision 2 restarted
```

`recover` needs the running host. It stops the run of every revision, drains and stops every
instance the state owns, and restarts the committed revision. Here the failed server was not
running, so it stopped 0 instances. It exits 0 once the committed revision is ready.

`inspect` afterwards:

```text
  "activeOperation": null,
  "committedRevision": 2,
  "nextAction": "none",
  "recovery": null,
  "recoveryReason": null,
```

The operation's receipt now has the state `abandoned` and keeps its `reason` and
`laterReasons`. Its plan cannot be applied again:

```text
$ ignite apply --state .ignite/terminal r3.json
error: operation sha256:3b229e20… was abandoned by ignite recover; prepare a new plan
```

Prepare a new plan and apply it as usual.

## When recover refuses

| exit | what to do |
|---|---|
| 1 | a process under `<state>/revisions/` is not accounted for: the message gives its pid and path, and nothing was signalled. `recover` proceeds once no such process is left. Exit 1 also means the committed revision was not ready within its readiness timeout. |
| 2 | `requires no recovery`: apply a corrective plan. `is being recovered`: wait for the running `recover` to finish. |

`recover` is repeatable. A host killed during a recovery restarts with the operation still
`recovery_required`, and a second `ignite recover` completes it.

Options and exits: [`ignite recover`](../reference/cli.md#ignite-recover). Why the host refuses
to guess: [The host](../design/runtime.md#the-host).
