# Replace a server with a rollout

A server component replaces its process under its `rollout` policy. `TelnetServer` and
`HttpServer` default to `DrainRollout {}`, a 2-minute deadline. The terminal design file
declares it explicitly:

```ignite,fragment
component server: TelnetServer {
  listener = ingress
  application = console
  executable = "terminald"
  rollout = DrainRollout {
    deadline = 2 minutes
    on_deadline = "disconnect_remaining"
  }
}
```

The examples come from the [install page](../guide/install.md). The steps are the same for
`ignite run` and for a [host](../guide/apply.md).

## Under ignite run

Start the run on a port of your choice:

```sh
ignite run examples/terminal/terminal.design.ignite --port Terminal.ingress=41733
```

Open a session in a second terminal with `telnet 127.0.0.1 41733` and leave it open. Then
type this line on the run's stdin:

```text
rollout Terminal.server
```

The journal on stderr, shortened to the rollout and supervisor records:

```text
34 4588965721 ns event Terminal.server.rollout: Control("rollout")
35 4588965721 ns effect Terminal.server.rollout: Build("Terminal.server")
36 4804679088 ns event Terminal.server.rollout: Built(2)
37 4804679088 ns effect Terminal.server.rollout: Kill("Terminal.server", 1707783, 15)
38 4804679088 ns effect Terminal.server.rollout: AwaitRelease("Terminal.server")
40 5058080590 ns event Terminal.server.rollout: Released
41 5058080590 ns effect Terminal.server.rollout: Spawn("Terminal.server", 2)
46 5367264927 ns event Terminal.server.supervisor: Ready(1723593)
49 5367264927 ns effect Terminal.server.rollout: Note("disruption 309184337 ns")
51 8595903236 ns event Terminal.server.supervisor: Exited(1707783, 0)
53 8595903236 ns effect Terminal.server.rollout: Note("retired")
```

| records | what happens |
|---|---|
| 35, 36 | generation 2 is built; a failed build aborts here and the old process keeps serving |
| 37 | the old process gets SIGTERM and starts to drain |
| 38, 40 | the old process closes its listener and reports the release on stderr |
| 41, 46 | the new process binds the port and is ready |
| 49 | the measured disruption, from the release to the new process's readiness |
| 51, 53 | the old process exits once its last session ends, here when the telnet session closed |

New connections are refused between records 40 and 46. The model declares that interval
instead of claiming zero downtime:

```sh
ignite explain examples/terminal/terminal.design.ignite Terminal.server.rollout
```

```text
Terminal.server.rollout: machine Roll of Terminal.server
  …
disruption: connections refused between listener close and rebind
```

A session still open at `deadline` is ended: `disconnect_remaining` sends SIGKILL and the
journal records `retired forced`.

### With no open session

An idle old process exits on SIGTERM before it reports the release. The supervisor's restart
is then the new process, and the disruption counts from the exit:

```text
38 4627456952 ns effect Terminal.server.rollout: AwaitRelease("Terminal.server")
ignite run: Terminal.server has no live process to release
40 4651538987 ns event Terminal.server.supervisor: Exited(1664482, 0)
41 4651538987 ns effect Terminal.server.supervisor: Note("restart attempt 1 delay 100 ms")
44 4651538987 ns effect Terminal.server.rollout: Note("retired")
47 4751538987 ns effect Terminal.server.supervisor: Spawn("Terminal.server", 1)
52 4784448774 ns event Terminal.server.supervisor: Ready(1669911)
55 4784448774 ns effect Terminal.server.rollout: Note("disruption 132909787 ns")
```

## Through ignite apply

On a host, a changed implementation is the trigger. Change the reply of `status` in
`commands.ignite`, then plan against the host:

```sh
ignite plan examples/terminal/terminal.design.ignite --for-apply --state .ignite/terminal --out r2.json
```

The saved plan marks the two members whose executables changed:

| member | kind | reason |
|---|---|---|
| `Terminal.server`, `Terminal.promptProbe` | `replace` | implementation, target, runtime contract or codegen revision changed |
| every other member | `unchanged` | operationally unchanged |

`ignite apply --state .ignite/terminal r2.json` exits 0 when the new revision is ready. The
host's journal shows the same records as above, from `Control("rollout")` to the
`disruption` note, and the server answers with the new reply.

Flags and stdin lines: [`ignite run`](../reference/cli.md#ignite-run),
[`ignite apply`](../reference/cli.md#ignite-apply). The model behind the steps:
[Rollout and its declared disruption](../design/runtime.md#rollout-and-its-declared-disruption).
