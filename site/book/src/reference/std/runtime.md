# std.runtime

The census of the runtime and the soak. The census counts the live objects of the process per
kind and the heap bytes it holds, on both targets. A soak runs a body many times and reports
the kinds that grew, so a test can assert that its code leaks nothing.

- Layer: base.
- Source: [`std/runtime.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/runtime.ignite)
  declares `Census`, `Growth`, `Soak`, `Body` and `soak`. The statics `Runtime.census()` and
  `Runtime.settle(..)` are builtins of the substrate, specified in
  [`docs/slice.md`](https://github.com/ignite-lang/ignite/blob/main/docs/slice.md) section 33.
- Import: `import std.runtime.{Census, Soak, soak}`.

## The census

```ignite,fragment
Runtime.census() -> Census
Runtime.settle(tasks: Int, within: Duration) -> Bool
```

| static | what it does |
|---|---|
| `Runtime.census` | the counters now, without waiting |
| `Runtime.settle` | waits until at most `tasks` tasks are live; false at the deadline |

Both are pure and have no value at compile time. A copy of a handle is not a second object:
each object is counted once, however many handles share it.

```ignite,fragment
type Census {
  tasks: Int
  workers: Int
  channels: Int
  listeners: Int
  streams: Int
  pipes: Int
  children: Int
  directories: Int
  capabilities: Int
  handles: Int
  handle_slots: Int
  heap_copies: Int
  stores: Int
  heap_bytes: Int
  arena_bytes: Int
  resident_bytes: Int
}
```

| field | counts | in a soak |
|---|---|---|
| `tasks` | spawned tasks that have not returned | exact |
| `workers` | worker threads, idle or running | reported |
| `channels` | channels | exact |
| `listeners`, `streams` | `TcpListener` and `TcpStream` sockets | exact |
| `pipes` | `Pipe.pair()` pairs, one per pair | exact |
| `children` | started child processes | exact |
| `directories` | `Directory` roots | exact |
| `capabilities` | `Launcher`, `Entropy`, `Setting`, `CommandLine`, `Environment` | exact |
| `handles` | filled slots of the handle table; LLVM only | exact |
| `handle_slots` | slots ever issued; LLVM only | reported |
| `heap_copies` | values held in stores and channels; LLVM only | exact |
| `stores` | stores; on LLVM a store lives until the process exits | reported |
| `heap_bytes` | heap bytes allocated and not freed | 64 KiB slack |
| `arena_bytes` | arena blocks of every task but the caller; LLVM only | 64 KiB slack |
| `resident_bytes` | the resident set the kernel reports | reported |

A kind marked LLVM only is 0 on the Rust target.

## The soak

```ignite,fragment
type Growth {
  kind: Text
  before: Int
  after: Int
}

type Soak {
  runs: Int
  warmup: Int
  growth: List<Growth>
}

type Body = fn(run: Int) -> Unit requires [run.*, channel.*, net.*, clock.*, process.*, directory.*, launcher.*, entropy.*, setting.*, command_line.*, environment.*]

fn soak(runs: Int, body: Body) -> Soak
```

`soak` runs `body(i)` for `i` in `0..runs`. After each run it waits up to 5 s for the tasks
the run spawned. The first `max(10, runs / 10)` runs are the warm-up. The census after the
warm-up is compared with the census after the last run:

- every kind marked exact must not grow;
- `heap_bytes` and `arena_bytes` must not grow by more than 64 KiB;
- the kinds marked reported are not compared.

`growth` names each kind that grew, empty when nothing leaked. `Body` lists the effects a body
may use, so an effectful function is a valid body (E017 otherwise).

A debug build of the substrate reads `IGNITE_DEBUG_LEAK=<kind>` and skips one release per
object of that kind, which seeds a leak a soak must find. Release builds ignore it.

## Example

```ignite
import std.runtime.{soak}

test fn channel_made(run: Int) -> Unit {
  let (tx, rx) = Channel.bounded(1)
  let sent = tx.try_send(run)
  let got = rx.receive_within(0 ms)
}

test "a channel made and dropped in every run leaks nothing" {
  let result = soak(200, channel_made)
  assert result.warmup == 20
  assert result.growth == []
}

test "the census counts a channel while it lives" {
  let before = Runtime.census()
  let (tx, rx) = Channel.bounded(1)
  let sent = tx.try_send(1)
  assert Runtime.census().channels == before.channels + 1
  tx.close()
  rx.close()
}
```
