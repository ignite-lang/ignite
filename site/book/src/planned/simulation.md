# Fault injection and deterministic simulation <span class="badge design">design</span>

Planned: making the failure paths of a program run under `ignite test`. Faults are injected
where the program meets the network, the disk and the clock. A simulated world is driven by
one seed, so a failing run repeats exactly. Nothing on this page is implemented, and the
design is a draft: it lists options with a recommendation, and the maintainers have not
decided yet. The code below follows the recommendations and does not compile today. The full
proposal is
[`docs/fault-injection.md`](https://github.com/ignite-lang/ignite/blob/main/docs/fault-injection.md).

## Why

A WebSocket push service was implemented in five languages against one specification, and
every implementation passed the conformance suite. One was still wrong. After a reconnect to
its message broker it delivered stale messages from a reconnect buffer, and it never echoed a
close frame. The suite simulated broker outages by pausing the broker. A paused process keeps
its connections open, so the reconnect path never ran. A test only exercises the faults its
world can produce.

Ignite sees every IO operation, every task and every clock read, because they all go through
one substrate. So it can produce the faults from the inside and make a failing run repeat.

## Faults at the seam

Faults are injected where the program calls the substrate:

| fault | what the program sees |
|---|---|
| connect refused, reset | the error at the next operation, queued bytes gone |
| silent peer, partition | bytes accepted and never delivered, until an idle timeout or the heal |
| partial reads and writes | deliveries split where a seeded generator chooses |
| latency and jitter | delivery later, in virtual time |
| DNS failure, disk full, I/O error | the error from the call |
| clock jump | a node's monotonic clock jumps forward |
| crash and restart | a node loses what it had not written atomically |

A place in your own code where something can go wrong that no IO fault reaches is a named
fault point. Outside test builds it compiles to nothing:

```ignite,fragment
fn publish(broker: Broker, message: Message) -> IoResult<Unit> {
  broker.send(message)?
  Fault.point("broker.ack")?
  broker.await_ack()
}
```

## Generated schedules, minimal reproductions

```ignite,fragment
import std.sim.{Sim, Nemesis, Reset, Partition, Latency, check}

test "stale buffered messages are dropped after a reconnect" {
  let report = check(Nemesis {
    nodes = ["broker"]
    kinds = [Reset, Partition(2 seconds), Latency(300 ms)]
    faults_per_run = 3
    runs = 200
  }, fn(sim: Sim) -> IoResult<History> = scenario(sim), invariants)
  assert report.passed
}
```

- Each run draws a fault schedule from its seed. Invariants are ordinary Ignite functions over
  what the scenario observed, such as "no message delivered more than 1 s after it was
  published".
- A failing run is shrunk: faults are removed and moved earlier while the invariant still
  fails. The minimal schedule is printed as Ignite source, ready to paste into a regression
  test.
- `ignite test FILE --sim --seed 0x…` reruns a failure exactly, on the same target and build.
- Recovery time reuses the contracts a system already declares: the time until an
  `Availability` standing is met again after a fault.

## Plan

| phase | content |
|---|---|
| 1 | the seam in the substrate with the real world: forced resets, refused connects, silent peers, disk errors; fault points; a fault trace |
| 2 | a simulated world for one node: a deterministic scheduler, virtual time, seeded entropy, `--sim` and `--seed` |
| 3 | several nodes: crash and restart, partitions, latency, a disk with a crash model |
| 4 | the nemesis, invariants over histories, shrinking |
| 5 | whole systems: the members of a `system` as simulated nodes |

Release builds pay nothing, and both targets get the seam unchanged because it lives in the
one substrate. The [roadmap](../roadmap.md) lists it with the other designs.
