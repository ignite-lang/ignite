# std.sync

Bounded channels between the tasks of one process. A channel is the waitable signal of the
substrate: a long poll, a heartbeat or a drain window waits on a channel with a deadline. No
descriptor, thread or socket is held per channel or per waiter.

- Layer: base (substrate).
- Source: [`std/sync.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/sync.ignite)
  declares nothing. It holds the API's comment and the inline tests. `Channel`, `Sender<T>` and
  `Receiver<T>` are builtins of the substrate, specified in
  [`docs/slice.md`](https://github.com/ignite-lang/ignite/blob/main/docs/slice.md) section 30.
- Import: none needed.

## Channel

```ignite,fragment
Channel.bounded(capacity: Int) -> (Sender<T>, Receiver<T>)
fn send(sender: Sender<T>, value: T) -> Bool
fn try_send(sender: Sender<T>, value: T) -> Bool
fn close(sender: Sender<T>) -> Unit
fn receive(receiver: Receiver<T>) -> Option<T>
fn receive_within(receiver: Receiver<T>, timeout: Duration) -> Option<T>
fn closed(receiver: Receiver<T>) -> Bool
fn close(receiver: Receiver<T>) -> Unit
```

| operation | effect | what it does |
|---|---|---|
| `Channel.bounded` | none | a channel that holds at most `capacity` values; a capacity below 1 fails the task |
| `send` | `tx.send` | waits while the channel is full; true once the value is queued, false when the channel is closed or closes meanwhile |
| `try_send` | `tx.send` | never waits; false when full or closed |
| `close` on a sender | `tx.send` | closes the channel |
| `receive` | `rx.receive` | waits for a value; `None` once the channel is closed and empty |
| `receive_within` | `rx.receive` | as `receive`, and `None` at the deadline; zero or less only looks |
| `closed` | `rx.receive` | whether the channel is closed, queued values or not |
| `close` on a receiver | `rx.receive` | closes the channel |

`tx` and `rx` stand for each parameter the end comes from. `Channel.bounded` in a constant or a
field default is E010. See [Effects](../effects.md).

## Behaviour

- `close` on either end closes the channel. So does the last handle of all senders, or of all
  receivers, going away: the end of its scope, or the end of the task it was handed to.
- A closed channel ends every waiter. Each waiting `send` returns false and drops its value.
  Each `receive` returns the values sent before the close, in order, then `None`.
- A value already queued is returned even when the deadline of `receive_within` has passed.
- A task sends what it wrote to its streams before it waits on a channel, as before
  `Clock.sleep`. It does not when the operation does not wait.

## Element type

`T` comes from the context: an expected type, a parameter or a return type, or for
`let (tx, rx) = Channel.bounded(n)` the first later use that fixes it, such as `tx.send(1)`.
An element type nothing fixes is E042. `Channel.bounded` where a type other than a pair of
ends is expected is E040. `Sender` and `Receiver` have no `==` (E045), and `debug()` writes
`<Sender>` and `<Receiver>`.

## Ownership

- Both ends are handles: a copy of a binding shares the end. Passed to `spawn` at its last
  use, an end moves into the task.
- `send` moves its value into the channel when the value is not used later, and copies it
  otherwise. The receiver gets its own value either way: nothing is shared between tasks.
- An end is released with its last owner, wherever it was moved: a second binding, a record
  field, a `Store`.

## Example

```ignite
test fn ring_after(bell: Sender<Text>, delay: Duration) -> Result<Unit, IoError> {
  Clock.sleep(delay)
  bell.send("ring")
  Ok(())
}

test "values arrive in order and a full channel refuses try_send" {
  let (tx, rx) = Channel.bounded(2)
  assert tx.try_send(1)
  assert tx.try_send(2)
  assert !tx.try_send(3)
  assert rx.receive() == Some(1)
  assert rx.receive() == Some(2)
  assert rx.receive_within(10 ms) == None
}

test "a send from another task wakes a waiting receiver" {
  let (bell, rings) = Channel.bounded(1)
  spawn ring_after(bell, 50 ms)
  assert rings.receive_within(5 seconds) == Some("ring")
}

test "a closed channel still gives what was sent before" {
  let (tx, rx) = Channel.bounded(4)
  assert tx.send(1)
  tx.close()
  assert rx.closed()
  assert rx.receive() == Some(1)
  assert rx.receive() == None
  assert !tx.send(2)
}
```
