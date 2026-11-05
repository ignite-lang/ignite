# Ignite actors: messages, state and supervision

**Status:** Future design, proposal. Nothing here is implemented.  
**Date:** 1 November 2026  
**Scope:** Actors, typed messages and addresses, publish/subscribe, state handling, supervision, hot reload, runtime mapping on the current and planned targets, syntax, testing and a phased roadmap.  
**Companion:** `docs/wasm.md` (WebAssembly target, a runtime written in Ignite, hot reload) was written in parallel; this document was aligned with it on 1 November 2026. It takes over that document's stored encoding and type hashes (wasm §5.2), its migration rules and diagnostics E080 and E081 (wasm §5.3), its swap points, coordinated reload and rollback (wasm §5.4 to 5.6) and its one instance per task (wasm §7). It answers wasm Q6: actor migration shares one encoding and one diagnostic set with store migration. Section 7.4 lists what the wasm runtime must add for actors.

Timo's request, verbatim: "another subagent review or analysis or research should be looking at Erlang So yeah, we could have just actors in the sense that we have publishers and receivers of messages and a nice state handling."

This document decides. Each question gives the chosen design, the rejected alternatives and one line on why. Claims about Ignite cite `design.md` (*design*), `docs/slice.md` (*slice*), `docs/llvm.md`, `docs/wasm.md` (*wasm*) or a source file. External claims cite section 14 by ID (A1 to A28). A proposal that contradicts a stated Ignite decision is marked **Conflict** and collected in section 12.

## 0 Decisions at a glance

| # | Question | Decision |
|---|---|---|
| D1 | What is an actor? | A component of the library kind `Actor` in a `system`, defined by an initial state value and a pure `handle` fn. No new keyword. |
| D2 | Messages and addresses | A message type is a Ignite enum. `Address<M>` is an unforgeable typed capability; holding it is the authority to send, and sending is the effect `a.send`. |
| D3 | Request/reply | `a.call(Get(id), 500)` passes a variant without its last `ReplyTo<T>` payload and returns `Result<T, Undelivered>`. No closures needed. |
| D4 | Pub/sub | `Topic<E>` is a std actor written in Ignite; subscribers are `Sink<E>` values. Delivery is FIFO per pair and at most once, in-process only. |
| D5 | Back-pressure | Every mailbox is bounded. `send` fails fast with `Err(Full)`; `call` waits for capacity and reply within its timeout; an actor's own sends wait up to a deadline, then the actor fails as `Overloaded`. |
| D6 | State | `handle(state: S, msg: M) -> Step<S>`, pure. A `Step` carries the new state, an `Outbox` of intents and `Continue` or `Stop`. One shape for servers and state machines. |
| D7 | Store | `Store<T>` stays as the resource for keyed data without behaviour; the wasm runtime holds it across reloads (wasm §5.2). State with behaviour lives in an actor. The todo example moves to an actor because it now publishes events. |
| D8 | Failure | `Supervisor` components with `OneForOne`, `RestForOne` or `OneForAll`, intensity and period. An address and its mailbox survive restarts. Monitors deliver a typed `Down`. No user-level links. |
| D9 | Hot reload | Swap a whole actor component at a message boundary, never two versions of one actor's state at once. `migrate: fn(Old) -> New` follows the store rules (wasm §5.3: E080, E081, defaulted fields automatic). |
| D10 | Runtime | Mailboxes, one-shot reply slots, timers, `Outbox` dispatch and exit notification are substrate (`ignite-io`, written once). The dispatch loop, supervision policy, topics and `Table` are Ignite in `std/`. |

## 1 What Ignite takes from Erlang/OTP, and what it leaves

| Erlang/OTP fact | Source | Ignite |
|---|---|---|
| Processes share nothing; message data is copied, except refc binaries and literals. | A3 | Takes it. Values are copied in value positions already (slice §6, ownership rule; `docs/llvm.md` §4 "Values and memory"). |
| Signals from one sender to one receiver keep their order. | A2 | Takes it: FIFO per pair (4.3). |
| `receive` picks the first matching message from the start of the queue, so a long queue makes a selective receive expensive. | A2, A3 | Leaves it. `handle` is total over `M` and takes messages in order (3.5). |
| Mailboxes are unbounded; overload grows them until memory runs out. Akka's default mailbox is unbounded too. | A16, A22 | Leaves it. Every mailbox is bounded (4.4). |
| Messages are untyped. Gleam's `Subject` and Akka Typed's `ActorRef[T]` make the compiler reject a wrong message. | A17, A18, A21 | Takes the typed form: `Address<M>`. |
| "Let some other process do the error recovery. If you can't do what you want to do, die. Let it crash." | A1 §4.3 | Takes it, with typed exit reasons and supervision declared in `system`. |
| Supervisors `one_for_one`, `one_for_all`, `rest_for_one`, `simple_one_for_one`. More than `MaxR` restarts in `MaxT` seconds terminate the supervisor with reason `shutdown`; the defaults are 1 and 5. | A9 | Takes the three static strategies and the intensity rule. Dynamic children come in P4. |
| `gen_server:call` defaults to 5000 ms, and a timeout exits the caller. `cast` returns `ok` even when the server does not exist. | A6 | Changes both. A timeout is `Err(Timeout)`, and `send` reports `Err(Stopped)` (design §3.3 "Values and types", row Failure: failures cannot be silently discarded). |
| `gen_statem` keeps a state beside its data and offers postpone, state enter calls and three kinds of timeout. | A7 | Folds it into one `handle` whose state is an enum, like `decode` (fn `decode` over `enum Decoder` in `std/protocol/telnet.ignite`). Postpone is selective receive and is left out. |
| `gen_event` runs handlers inside the manager process and removes a handler that crashes. | A8 | Leaves it. Each subscriber is its own actor (4.1). |
| An application is "a component that can be started and stopped as a unit"; its `start/2` builds a supervision tree, and releases package applications. | A10, A12 | A `system` with its assembly plays both roles (design §5). |
| A module can have an old and a current version. A third load purges the old one and terminates the processes still in it. `code_change/3` migrates state, and an `{error, R}` from it rolls the upgrade back. | A11, A6 | Takes the migration and the rollback; leaves the coexistence of versions (7.2). |
| Appups and relups are "one of the most complex parts of OTP"; the author recommends rolling upgrades where possible. | A13, A12 | Ignite derives the upgrade from a build diff and checks it (7). |
| `pg` groups processes across nodes with strong eventual consistency; pids are transparent across nodes, the cluster is a full mesh, and traffic is clear text by default. | A14, A15 | Leaves both. Topics are in-process (4); a host is crossed only through declared queue resources (design §8.9). |
| Preemptive scheduling on reductions; per-process heaps and per-process generational copying GC. | A5, A4 | Not reproduced. OS threads now, tokio tasks in slice 5 (8). |

## 2 How actors fit Ignite's model

### 2.1 An actor is a component kind

**Decision.** `Actor` is a library component kind, like `Service` and `Client` (slice §6.1). Its properties:

| property | type | meaning |
|---|---|---|
| `init` | `S` | Initial state, evaluated in the system's scope. Resource names and actor names are values in it. |
| `handle` | `fn(state: S, msg: M) -> Step<S>`, pure | One message, one step. A named module-level fn, so it can be a wasm cut point (wasm §3.1, E082). |
| `mailbox` | `Int`, default 1024 | Capacity of the bounded mailbox. |
| `send_deadline_ms` | `Int`, default 1000 | How long the actor's own sends may wait (4.4). |
| `restart` | `Permanent` (default), `Transient` or `Temporary` | Restart type under its supervisor (6.1). |
| `persist` | `Persist`, optional | Journal and snapshot hooks (5.3). |
| `migrate` | `fn(old: S0) -> S`, optional | Hot-reload migration (7.1). |

The checker infers `S` and `M` from `handle`. A wrong shape is the proposed E061.

**Rejected.**
- A keyword `actor`. Domain nouns are library symbols (design §3.1 "Design rules"), and the design counts twelve declaration forms (design §3.2 "Declaration primitives").
- An actor as a protocol role. A role is a task over one TCP stream with fixed signatures (slice §4.2, E030). It is the edge that turns bytes into messages, not a holder of state.
- An actor as a resource. A resource is an "owned or bound external lifecycle entity" (design §3.2, row Resource); an actor is a "computational unit" (design §3.2, row Component).

### 2.2 Relation to the other forms

| form | relation |
|---|---|
| `system` | Declares actors, topics and supervisors and their wiring; the topology stays static and inspectable (design §2, §5). |
| `Service` and `Client` components | Their connection tasks hold `Address` values through `application` and call actors. They are not actors and stay imperative (the `serve` role of `protocol Telnet` in `std/protocol/telnet.ignite`). |
| `protocol` roles | Unchanged: the role is the byte-level edge, the actor the state behind it. |
| `resource` | `Topic<E>` is declared as a resource with type arguments (slice §11.4 grammar) and implemented by a std actor. `Store<T>` and `TcpListener` are unchanged. |
| assembly (design §5) | Places actors: in one process, in wasm instances or in separate executables. Until assemblies exist, an executable hosts every actor its component's properties reach. |

Actor component names become values of type `Address<M>` in property expressions, as resource names are (slice §11.4, §11.7 "Systems"). This extends that rule; it does not conflict with it.

### 2.3 `Store<T>` and actor state

**Decision.** Both stay, with a rule for which to use.

| use | when |
|---|---|
| `Store<T>` (slice §11.3) | Keyed data with no behaviour beyond `get`, `put`, `remove`, `values` and a pure `update`. On wasm the runtime holds it in the stored encoding across reloads (wasm §5.2). |
| actor state | Anything that decides: invariants over several keys, events published on change, a protocol of requests, a state machine. |

The todo service of section 9 moves to a `Todos` actor because it publishes change events and owns id assignment. Its state uses `Table<T>`, a pure std value written in Ignite with `Store`'s method names and id order. `Store` keeps its effects: a pure `handle` cannot call it, so an actor that needs a store holds its data in its own state.

**Rejected.**
- Retiring `Store<T>`. It is the store that wasm's reload rests on (wasm §5.2, §9 "reload: state"), and plain CRUD needs no actor.
- A `handle` with effects on a `Store` in its state. Its writes are then neither intents nor replayable, so 5.2 and 5.3 break.

## 3 Typed messages and addresses

### 3.1 Message types are enums

**Decision.** `M` is an ordinary Ignite enum, generic where needed (slice §11.1); E046 then guarantees that `handle` covers every message. A request carries its reply channel as its last payload: `Get(Int, ReplyTo<Option<Todo>>)`.

**Rejected.** A per-message interface. Ignite has no traits, and a closed enum is what makes exhaustiveness possible.

### 3.2 Address, Sink, ReplyTo

| type | what holding it allows | constructed by |
|---|---|---|
| `Address<M>` | send `M`, call, monitor, adapt | System wiring only in P1 to P3; P4 adds dynamic actors |
| `Sink<T>` | push `T` (one way, no monitor) | `a.sink()`, or `a.adapt(f)` with a named pure `f: fn(T) -> M` |
| `ReplyTo<T>` | one reply of type `T` | The runtime inside `a.call(..)`; `ReplyTo.test(n)` in `test` blocks only (proposed E064) |

None of these types can be built from `Int` or `Text`, so they cannot be forged. At run time an address is a handle with a kind and a generation, as in `ignite-rt`'s handle table (`docs/llvm.md` §5) and the per-instance handles of wasm §8: a stale or wrongly kinded handle is an error. The `f` of a `Sink` runs in the receiving actor's component, before its `handle`.

**Rejected.**
- Subjects owned by the receiving process (A18). A pure `handle` has no `self`, and replies go to a `ReplyTo`, not to a subject the caller owns.
- Untyped pids, the gap Gleam and Akka Typed close (A17, A21).

### 3.3 Request and reply

**Decision.** `a.call(V(x, y), timeout_ms) -> Result<T, Undelivered>`, where the variant `V` of `M` has the payload `(X, Y, ReplyTo<T>)`. The caller writes the variant without its last payload, and the runtime supplies a fresh one-shot `ReplyTo<T>`. The call waits up to `timeout_ms` in total for mailbox capacity and for the reply. The typing rule is local to `call`'s first argument; any other argument is the proposed E060. `enum Undelivered { Full, Timeout, Stopped }`: `Full` comes only from `send` (a full mailbox makes `call` time out instead); `Timeout` means no reply in time; `Stopped` means the actor stopped or crashed on this request, so the runtime dropped its `ReplyTo` and the caller learns at once. A late reply, or a second reply on the same `ReplyTo`, is dropped and counted (R5).

**Rejected.**
- Gleam's `call(subject, timeout, fn(reply) -> msg)` (A17). It needs a closure capturing `id`. Closures capture locals by copy since slice 9 (`docs/slice.md` 13.3), so T3's finding no longer holds. What P4 still needs before it adds this as the general form, with `call(V(..))` as sugar, is `docs/slice.md` 20.13 items 1 and 2: an effectful closure passed as a fn value (E017 today), and an unannotated closure parameter inferred from the closure body (`call(|r| Get(id, r))` is E042 twice; `call(|r: Reply<Text>| Get(id, r))` passes).
- `let (reply, pending) = ReplyTo.pair()`. A pattern `let` takes no annotation (slice §3), so `T` would be E042.
- Crashing the caller on timeout (A6). A Ignite failure is a `Result` (design §3.3).

### 3.4 Effects and authority

**Decision.** Sending is an effect on the address, written in slice §5.1's notation:

| operation | effect |
|---|---|
| `a.send(m) -> Result<Unit, Undelivered>` | `a.send` for each root `a` of the receiver |
| `a.call(V(..), ms) -> Result<T, Undelivered>` | `a.send` |
| `s.send(v)` on a `Sink<T>` | `s.send` |
| `a.sink()`, `a.adapt(f)`, every `Outbox` method | none (pure) |

An HTTP handler `fn show(store: Address<TodoMsg>, id: Int) -> Response` has the inferred set `[store.send]`, which fits `HttpApp`'s `fn(state: S, request: Request) -> Response requires [state.*]` (slice §11.2). On wasm, `a.send` maps to an import `ignite:actor/send` exactly as store effects map to `ignite:store` (wasm §3.2). A pure actor component therefore imports only `ignite:actor` and `log`, so it cannot open a socket. Authority is passed explicitly (design §3.3 "Values and types", row Capability; §9.3): a component can reach only the addresses it was given in its properties, in a message or in a reply. Inside an actor, sends are intents in the `Outbox`; the dispatcher is a root of the effect graph, like system mains (slice §5.1).

**Rejected.** A registry of named processes (`register/2`): it is ambient authority.

### 3.5 No selective receive

**Decision.** An actor takes messages strictly in arrival order, and `handle` must accept every variant (E046). A blocking caller waits on its own one-shot slot, never on a mailbox, which gives without any scan what Erlang's reference optimisation gives (A3).

**Rejected.** Selective receive and `postpone`. Their cost grows with queue length (A3), they hide unhandled messages, and they make the replay order differ from the arrival order.

## 4 Publishers and receivers

### 4.1 Typed topics

**Decision.** `type Topic<E> = Address<TopicMsg<E>>`, with `enum TopicMsg<E> { Publish(E), Subscribe(Sink<E>) }`. The topic is an actor in `std/actor.ignite`, written in Ignite. A system declares `resource changes: Topic<TodoEvent> { subscribers = [audit.adapt(on_change)] }`. Static subscriptions come first; dynamic `Subscribe` comes in P4.

**Rejected.**
- `pg`-style groups of raw addresses (A14). They have no element type, so every member would need the same `M`.
- `gen_event` (A8). Handlers in one process share its failure and its back-pressure.
- A substrate pub/sub primitive. Beyond IO, what a program needs is Ignite in `std/` (`AGENTS.md`, "Rules of the layout").

### 4.2 Fan-out

The topic's `handle` pushes each event to every `Sink` in subscription order through its `Outbox`. Value semantics gives each subscriber its own copy.

### 4.3 Delivery guarantees

| path | ordering | loss |
|---|---|---|
| sender to actor, same process or same wasm runtime | FIFO per (sender, receiver) pair, as in Erlang (A2) | An accepted message is handled, unless the receiver crashes on it. Queued messages survive a restart (6.1). |
| publisher to topic to subscriber | FIFO per (publisher, subscriber) pair; publishers interleave | As above, plus declared `Drop` overflow, which is counted (4.4). |
| across processes or hosts | not provided by actors | Through declared queue resources only, each stating its delivery (design §8.9 "NATS resource semantics and durable effects"). Ignite does not "create exactly-once effects from a delivery label" (design §1.4 "Scope boundaries"). |

In-process delivery is at most once across a crash. Recovering state is the job of the journal (5.3).

### 4.4 Back-pressure

**Decision.** Every mailbox is bounded. When it is full:

| sender | full mailbox |
|---|---|
| effectful code, `a.send(m)` | `Err(Full)` at once (fail fast, A16) |
| effectful code, `a.call(..)` | waits within its timeout, then `Err(Timeout)` (back-pressure through synchronous calls, A16) |
| an actor's `Outbox` | the dispatcher waits up to `send_deadline_ms`; then the actor fails with `Overloaded`, and its supervisor decides |
| a topic fanning out | the topic's `overflow`: `Block` (default; the topic's mailbox fills, and publishers feel it), `DropNewest` or `DropOldest`, each drop counted as an observation |

**Open, decided inside wave C before P3.** An actor that fails as `Overloaded` keeps its mailbox across the restart (6.1), so the restarted actor meets the same backlog, blocks again and spends its supervisor's intensity on one cause. Two options: count `Overloaded` toward intensity only after the backlog shrank since the last restart, or let the supervisor drop the mailbox on `Overloaded` (each dropped message counted as an observation, waiting callers get `Err(Stopped)`).

Blocking is the default because the design forbids silent loss ("zero silent constraint relaxation", design §1.5 "Success measures") and requires back-pressure to "propagate from queues and socket writes to producers" (design §8.7 "Runtime ABI and asynchronous operations"). Dropping must be declared. This is the bounded tokio channel (A27, A28), and the effect of GenStage's demand (A19) without its protocol.

**Rejected.**
- Unbounded mailboxes (A16, A22).
- Dropping by default, as Akka's bounded mailbox does by sending overflow to dead letters (A22). That is silent loss.
- GenStage demand inside one runtime. It is more machinery than bounded queues need there; it and Broadway-style batching and acknowledgements (A20) are candidates for cross-host queues.

## 5 State handling

### 5.1 One pure shape

**Decision.**

```ignite
enum Next { Continue, Stop }
type Step<S> {
  state: S
  out: Outbox = Outbox.empty()
  next: Next = Continue
}
```

`handle(state, msg) -> Step<S>` is pure. `Outbox` is a pure builtin value that collects intents: `send(to, m)`, `push(sink, v)`, `reply(r, v)` and `offer(to, m)` (dropped when full), plus `after(ms, to, m)` and `monitor(a, wrap)` from P3. The dispatcher commits `state`, executes `out`, then continues or stops. This is the design's runtime model scaled down to one actor: "an ordered event stream enters a reducer; the reducer emits updated state and effect intents" (design §2 "Architecture and execution model"). A state machine is an enum `S` matched together with the message, the shape `decode` already has (fn `decode` over `enum Decoder` in `std/protocol/telnet.ignite`).

**Rejected.**
- An effectful `handle` that sends inline. It cannot be replayed without an effect log, or tested without a runtime.
- Separate `gen_server` and `gen_statem` behaviours (A6, A7). In Ignite they differ only in the type of `S`.
- Returning the next behaviour as a function (A21). That needs closures, and a swap could no longer locate the code.

### 5.2 Value semantics, replay and tests

State is a value: copied in, returned out, never shared (slice §6; `docs/llvm.md` §4 "Values and memory"). Given the same initial state and the same ordered messages, `handle` yields the same states and intents. That is design §6.3's replay property and §13.1's "Control: Replay, ... deterministic effect intents" (design §13.1 "Verification layers", row Control). Time enters only as timer messages, which are recorded inputs (design §6.3).

### 5.3 Persistence hooks

**Decision.** `persist = Persist { journal = todolog, snapshot_every = 1000 }`, where `todolog` is a file or volume resource. The dispatcher appends each message to the journal before `handle` and snapshots the state every N messages. Recovery loads the last snapshot and replays the later messages through `handle`, discarding their `Outbox`, so no write is replayed blindly (design §8.9 "NATS resource semantics and durable effects"; durable intents §7.1). A `ReplyTo` inside a replayed message is dead. Journal entries and snapshots use wasm's stored encoding, tagged with the type hash (wasm §5.2). That is one encoding for stores, actor swaps and journals (wasm Q6).

A message that crashes `handle` is journaled before it runs, so plain replay would crash every restart on it and spend the supervisor's intensity on one poison message. Before the restart, the dispatcher therefore appends a skip mark for that message to the journal (or snapshots the state without it), and recovery replays every journaled message except the skipped ones. The crashing message is lost once, as 6.1 states, and never replayed.

**Rejected.**
- Snapshot after every message as the only mode. It is O(state) per message; it remains available as `snapshot_every = 1`.
- Explicit state-write calls from the handler, the Orleans model (A23). They are an effect inside `handle`.

### 5.4 How this removes shared mutable state

Actor state has exactly one owner. Messages are copies, connection tasks hold only addresses, and user code takes no locks. `Store<T>` remains the one shared mutable resource, and its operations are serialized by its implementation (slice §11.3; on wasm by the runtime, wasm §5.4). No Ignite place is ever shared.

## 6 Failure

### 6.1 Supervision declared in the system

**Decision.** `component root: Supervisor { strategy = RestForOne, intensity = 3, period_ms = 5000, children = [store, audit] }`.

- Strategies `OneForOne`, `RestForOne` and `OneForAll` mean what they mean in OTP (A9). Children start in list order and stop in reverse order.
- More than `intensity` restarts within `period_ms` stop the children and fail the supervisor (A9). A system with actors and no `Supervisor` gets an implicit root `OneForOne` with OTP's defaults: 1 restart in 5000 ms.
- An actor restarts from `init`, or from its journal when it has `persist`. **Its address and mailbox survive the restart**: holders keep a valid `Address`, and queued messages wait. The address is a stable logical identity (design §3.4), like an Orleans grain reference (A24). Only the message it crashed on is lost; with `persist` the crash is journaled as a skip before the restart, so replay does not crash again (5.3). An `Overloaded` actor keeps its backlog too; how that counts toward intensity is open (4.4).
- `children` names components and is resolved by the checker; it is not a list value, since the children have different `M`. A child under two supervisors, or a child that is neither `Actor` nor `Supervisor`, is the proposed E062.
- The decision logic is a pure Ignite reducer, `supervise(state, ChildExit) -> Step<SupervisorState>` in `std/actor.ignite`, run by the same dispatcher. Its restart intents are privileged `Outbox` entries that only a `Supervisor`'s dispatcher executes.

**Rejected.** `simple_one_for_one` before P4, because it needs dynamic actors. Backoff inside the process: in-process restarts are cheap, and backoff belongs to the process-level `Restart` (6.2).

### 6.2 Mapping onto recovery and restart limits

| tier | owner | rule |
|---|---|---|
| actor | in-process `Supervisor` | intensity and period, as above |
| component or process | the controller's "Server supervisor" machine: start, await readiness, running, backoff, failed, stopping (design §11.8 "Lowered control behavior"); on wasm, the component's failure rule (wasm §8, Containment) | `recovery = Restart { backoff, maximum_attempts, interval }` (design §11.3 "Full system declaration", component `server`) |

When the root supervisor gives up, a native executable ends with the new exit status 5 ("supervision gave up"; slice §6.1 uses 0 to 4). The controller counts that against `Restart`. On wasm the actor component is reported failed and, when the failures began after a swap, rolled back (wasm §5.6). The terminal acceptance row "Recovery: server crash, restart limit" (design §11.10 "Terminal acceptance tests") then has an actor half and a process half.

### 6.3 Links and monitors

**Decision.** Monitors only, typed. `out.monitor(a, wrap)` with a named `wrap: fn(Down) -> M` delivers `Down { actor: ActorId, reason: ExitReason }` once, also when the target is already gone (Erlang's `noproc`, A2). `enum ExitReason { Normal, Failed(Text), Overloaded, Killed }`. Blocking callers need no monitor: `call` returns `Stopped`.

**Rejected.** Bidirectional links and trapped exits (A2). An exit signal that kills its receiver unless it traps exits is an untyped side channel. The supervisor relation is the only link, and its strategy states what propagates.

### 6.4 What a crash is

A Ignite fn has no exceptions. An actor fails when `handle` panics (index out of range, allocation failure; `docs/llvm.md` §4 "Values and memory", `ignite_rt_panic`), when its sends overrun `send_deadline_ms`, or when a trap ends its wasm task (wasm §8). A `Step { next = Stop }` ends it normally; `next = Fail(reason)` comes in P3. The failure must end only that actor and must be reported to its supervisor. Today, "a task that panics ends its thread" on the Rust target with no report (slice §7, Tasks), wasm follows the Rust target (wasm §5.7), and the LLVM target exits the process with status 101 (`docs/llvm.md` §4 "Values and memory", `ignite_rt_panic`). **Conflict** (C1): the LLVM target must contain a panic to its task. This answers wasm Q1 for actors: a failure ends the task and notifies its supervisor, on all three targets.

## 7 Hot reload

### 7.1 Typed migration

**Decision.** Actor state follows wasm's store rules (wasm §5.3), with the same diagnostics:

| change of `S`, by type hash (wasm §5.2) | outcome |
|---|---|
| identical | no migration |
| a record gains a field with a default (slice §4.7) | automatic: decode with the old type, fill in the default |
| anything else | the `Actor` component needs `migrate = f`, `f: fn(old: S0) -> S` pure; without one E080, and the reload is refused |

`S0` must hash to the running state's type; otherwise E081, with the running shape printed as Ignite. The compiler thereby checks the migration against what runs, not against an old source file. The type hash is computed over structure, not names, in the content-addressed spirit of Unison (A26). A pure `migrate` that returns a value cannot leave a half-migrated state. A failing `migrate` (a trap) rolls the reload back, as a `code_change` returning `{error, R}` does (A6, wasm §5.6).

**Rejected.**
- `code_change(OldVsn, State, Extra)` over untyped terms (A6). Nothing checks it before it runs.
- Appup and relup scripts (A12, A13). The driver derives the batch from type and interface hashes (wasm §5.5).
- Actor-specific diagnostics. wasm Q6 asks for one set.

### 7.2 Suspend, swap, resume at a message boundary

**Decision.** The actor swap is wasm's quiescent swap (wasm §5.4) with the message as the call boundary. For an actor component C:

1. The driver checks the build (7.1, 7.3) before anything stops.
2. Close admission. Each actor of C finishes its current message, up to wasm's deadline (default 5 s), and its next `receive` returns the substrate event `Swap` instead of a message. Senders keep filling the mailboxes and then feel back-pressure (4.4). The dispatcher calls `handle` only between messages, so this is `sys:suspend` (A12) without user code.
3. If `M` changed, drain first: the queued messages run under the old code, with admission still closed (7.3).
4. Each actor returns its state in the stored encoding, and its task ends. The runtime snapshots the states, runs `migrate` or the automatic rule in a fresh instance (wasm §5.3), and writes a journal snapshot when `persist` is set. The journal therefore never replays old-`M` messages under new code.
5. Start each actor's task in a new instance through `resume(state)`. Reopen admission.
6. During the observation window, a trap rolls back to `previous` and restores the snapshot in development mode; elsewhere it reports "recovery required" (wasm §5.6; design §7.4 "Failure defaults").

At any moment one code version handles one actor's state. Erlang's old and current versions (A11) exist because processes switch on their next fully qualified call; a Ignite dispatcher switches all of C's actors at a message boundary.

**Rejected.**
- Lazy per-actor switching, the Erlang model. Two versions then coexist, and a third load terminates the stragglers (A11).
- Drain swap for actors (wasm §5.4). An actor never ends on its own, so its state must move.

### 7.3 In-flight messages and mailbox type changes

**Decision.** Only an identical type hash of `M` and of every `T` in a `ReplyTo<T>` inside `M` counts as compatible, as wasm counts only an unchanged interface hash (wasm §5.5). On any change, the driver puts C and every component holding an `Address<M>` through system wiring into one atomic coordinated reload (wasm §5.5), and C drains its queue under the old code first (7.2, step 3). Until P4, messages carry no `Address`, only `ReplyTo` (alive for one call) and `Sink` (held by topics, which the wiring shows). The holder set is therefore exact in P1 to P3 (R4). Orleans versions grain interfaces for the same reason (A23).

**Rejected.** Accepting added variants as compatible. Senders compiled against the old `M` would lower values with the old layout (wasm §3.4).

### 7.4 Interface points the wasm runtime must meet

Hot reload is a wasm-runtime feature. A native executable restarts and recovers from its journal instead; a direct bind cannot run old and new servers side by side (design §11.8 "Lowered control behavior").

| ID | Point | wasm anchor |
|---|---|---|
| W1 | One wasm component per `Actor` component; `handle`, `init` data, `resume` and `migrate` are its exports. | §3.1, §3.3 |
| W2 | One instance per actor task. A trap discards only that actor's instance. | §7, §8 |
| W3 | Mailboxes, addresses, one-shot slots, timers, monitors and supervisor state live in the runtime, outside instances, so a swap keeps them. | §5.2 |
| W4 | An interface `ignite:actor` (`receive`, `send`, `call`, `dispatch` of an `Outbox`, `monitor`, `after`) in the `ignite:*` world, synchronous and suspending in the host. | §3.2, §3.5, §7 |
| W5 | An `Outbox` entry crosses the boundary as (handle, stored-encoding bytes, type hash); the runtime checks the hash against the target's `M` before enqueuing. | §5.2 |
| W6 | `receive` can return the substrate event `Swap`; the guest's loop then returns its state encoded, and `resume(state)` re-enters it. | §5.4 |
| W7 | A `Sink`'s adapter fn and `migrate` run inside the receiving component and are named fns. | §3.1 (E082) |
| W8 | Actor reload reuses E080, E081, the snapshot, the observation window and rollback. | §5.3, §5.6 |

## 8 Runtime mapping

### 8.1 Blocking `ignite-io` (today)

**Decision.** One task per actor through `ignite_io::spawn` (slice §7, Tasks). The task runs `std.actor.run`, a Ignite loop: `receive` from a bounded mailbox (substrate, `std::sync::mpsc::sync_channel`), `handle`, commit, `out.dispatch()`. The generated `main` first creates every mailbox, so wiring cycles are fine (design §3.4: "a bidirectional protocol is not itself a dependency-cycle error"). It then starts the supervisors, which spawn their children, and then the accept loop. Only the substrate starts threads (`AGENTS.md`, Lifecycle ownership).

**Rejected.** A pool of threads multiplexing actors. It needs non-blocking receive and a scheduler, which slice 5 brings anyway; tens of actors fit on threads.

### 8.2 Slice-5 tokio

**Decision.** An actor is a tokio task, and its mailbox is `tokio::sync::mpsc::channel(capacity)`. `send` waits for capacity (A27), `try_send` returns `Full` or `Closed`, and `send_timeout` bounds the wait (A28). These map one to one onto `call`, `send` and the `Outbox` deadline. `ReplyTo` is a oneshot channel, and timers are `tokio::time`. LLVM-target actors are coroutines run as tokio tasks (`AGENTS.md`, "Rules of the layout").

### 8.3 Wasm runtime

**Decision.** One instance per actor task (W2, wasm §7). The same `std.actor.run` loop runs inside the guest; `receive` suspends in the host (wasm §7), and state stays in the task's memory between messages. It is encoded only at a swap or a snapshot.

**Rejected.**
- Many actors per instance. It contradicts wasm §7, and one trap would take down unrelated actors.
- State held by the host between messages. Every message would encode the whole state.

### 8.4 What lives where

| piece | home | why |
|---|---|---|
| bounded mailbox, one-shot slot, timers, monotonic clock, exit notification, panic containment | `ignite-io` (`ignite-rt` C ABI; on wasm, `ignite:actor` imports) | waiting and waking are substrate work, written once (`AGENTS.md`, "Rules of the layout") |
| `Outbox` with type-erased entries and its `dispatch` | `ignite-io` (`Box<dyn Any + Send>`); `ignite-rt` with the `HeapValue` glue `Store` uses (slice §11.3); wasm per W5 | heterogeneous sends do not fit one Ignite enum |
| `run`, `Step`, `Next`, `Undelivered`, `ExitReason`, `answer`, topics, `supervise` | `std/actor.ignite` | plain Ignite, testable as pure functions |
| `Table<T>` | `std/table.ignite` | pure value |

## 9 Syntax sketches (slice-7 syntax)

New names are the builtins of section 3 (`Address`, `Sink`, `ReplyTo`, `Outbox`), the kinds `Actor` and `Supervisor`, and the std items of 8.4. Durations are `Int` milliseconds, because quantity literals are not in the slice (slice §2). `std/protocol/http.ignite` is not in the tree yet; `HttpApp<S>` is assumed to have slice §11.2's shape.

### 9.1 `std/actor.ignite` (excerpt)

```ignite
package std.actor

enum Next { Continue, Stop }
enum Undelivered { Full, Timeout, Stopped }

type Step<S> {
  state: S
  out: Outbox = Outbox.empty()
  next: Next = Continue
}

fn answer<S, T>(state: S, to: ReplyTo<T>, value: T) -> Step<S> =
  Step { state, out = Outbox.empty().reply(to, value) }

enum TopicMsg<E> { Publish(E), Subscribe(Sink<E>) }
type Topic<E> = Address<TopicMsg<E>>

fn publish<E>(out: Outbox, topic: Topic<E>, event: E) -> Outbox = out.send(topic, Publish(event))
```

The topic's own `handle`, `topic(sinks: List<Sink<E>>, msg: TopicMsg<E>)`, answers `Publish(e)` with one `push` per sink in a `for` loop over an `Outbox` variable, and `Subscribe(s)` by appending `s` to its state.

### 9.2 `examples/todo/todo.ignite` with a `Todos` actor

`Todo`, `or` and `to_json`, and the JSON and HTTP imports, are unchanged (imports, `Todo`, `or`, `to_json` in `examples/todo/todo.ignite`). `Store<Todo>` (the parameter of `list`, `create`, `show`, `patch`, `delete` and `handle`, and `resource todos` in `system TodoService`) is gone from this example.

```ignite
import std.actor.{Step, Topic, Undelivered, answer, publish}
import std.table.Table

enum TodoEvent { Created(Todo), Changed(Todo), Deleted(Int) }

type Todos {
  table: Table<Todo>
  changes: Topic<TodoEvent>
}

enum TodoMsg {
  All(ReplyTo<List<Todo>>)
  Create(Text, Bool, ReplyTo<Todo>)
  Get(Int, ReplyTo<Option<Todo>>)
  Patch(Int, Option<Text>, Option<Bool>, ReplyTo<Option<Todo>>)
  Delete(Int, ReplyTo<Bool>)
}

fn told(state: Todos, event: TodoEvent, step: Step<Todos>) -> Step<Todos> =
  Step { state = step.state, out = publish(step.out, state.changes, event) }

fn todos(state: Todos, msg: TodoMsg) -> Step<Todos> = match msg {
  All(reply) => answer(state, reply, state.table.values())
  Create(title, done, reply) => {
    let todo = Todo { id = state.table.next_id, title, done }
    let next = Todos { table = state.table.put(todo.id, todo), changes = state.changes }
    told(state, Created(todo), answer(next, reply, todo))
  }
  Get(id, reply) => answer(state, reply, state.table.get(id))
  Patch(id, title, done, reply) => match state.table.get(id) {
    None => answer(state, reply, None)
    Some(todo) => {
      let changed = Todo { id, title = or(title, todo.title), done = or(done, todo.done) }
      let next = Todos { table = state.table.put(id, changed), changes = state.changes }
      told(state, Changed(changed), answer(next, reply, Some(changed)))
    }
  }
  Delete(id, reply) => match state.table.remove(id) {
    (table, Some(_)) => {
      let next = Todos { table, changes = state.changes }
      told(state, Deleted(id), answer(next, reply, true))
    }
    (_, None) => answer(state, reply, false)
  }
}

const TIMEOUT: Int = 500

fn busy(error: Undelivered) -> Response = status_response(503)

fn create(store: Address<TodoMsg>, body: Text) -> Response = match parse(body) {
  Ok(value) => match value.text("title") {
    Some(title) => match store.call(Create(title, or(value.bool("done"), false)), TIMEOUT) {
      Ok(todo) => json(201, to_json(todo))
      Err(error) => busy(error)
    }
    None => status_response(400)
  }
  Err(_) => status_response(400)
}

fn show(store: Address<TodoMsg>, id: Int) -> Response = match store.call(Get(id), TIMEOUT) {
  Ok(Some(todo)) => json(200, to_json(todo))
  Ok(None) => status_response(404)
  Err(error) => busy(error)
}
```

`list`, `patch` and `delete` follow `show`, with `store.call(All, TIMEOUT)`, `store.call(Patch(id, value.text("title"), value.bool("done")), TIMEOUT)` and `store.call(Delete(id), TIMEOUT)`. `handle` keeps its routing (fn `handle` in `examples/todo/todo.ignite`) with `store: Address<TodoMsg>` in place of `todos: Store<Todo>`; its inferred effects are `[store.send]`. The mailbox serialises requests, so slice §11.6's "16 connections × 100 POSTs → 1600 distinct ids" holds without an atomic counter.

### 9.3 Pub/sub: an audit consumer

```ignite
type Audit { created: Int, changed: Int, deleted: Int }

enum AuditMsg { Saw(TodoEvent), Counts(ReplyTo<Audit>) }

fn on_change(event: TodoEvent) -> AuditMsg = Saw(event)

fn count(a: Audit, event: TodoEvent) -> Audit = match event {
  Created(_) => Audit { created = a.created + 1, changed = a.changed, deleted = a.deleted }
  Changed(_) => Audit { created = a.created, changed = a.changed + 1, deleted = a.deleted }
  Deleted(_) => Audit { created = a.created, changed = a.changed, deleted = a.deleted + 1 }
}

fn tally(state: Audit, msg: AuditMsg) -> Step<Audit> = match msg {
  Saw(event) => Step { state = count(state, event) }
  Counts(reply) => answer(state, reply, state)
}
```

### 9.4 The system with a supervisor

```ignite
system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource changes: Topic<TodoEvent> { subscribers = [audit.adapt(on_change)], overflow = Block }

  component store: Actor {
    init = Todos { table = Table.empty(), changes }
    handle = todos
    mailbox = 1024
  }

  component audit: Actor {
    init = Audit { created = 0, changed = 0, deleted = 0 }
    handle = tally
  }

  component root: Supervisor {
    strategy = RestForOne
    intensity = 3
    period_ms = 5000
    children = [store, audit]
  }

  component server: Service {
    listener = ingress
    protocol = Http
    application = HttpApp { state = store, handle }
    executable = "todod"
  }
}
```

With `RestForOne` and `store` listed first, a crash of `store` also restarts `audit`, so the counts restart with the table; a crash of `audit` leaves `store` alone. The topic sits under the implicit root supervisor. Component names (`store`, `audit`) are kept distinct from fn names (`todos`, `tally`) because a module's names share one namespace (slice §4, E004); whether component names join that namespace is for the checker to settle.

### 9.5 A migration

Version 2 adds `open: Int`, the number of todos not done, to `Todos`, without a default. That change needs `migrate` (7.1):

```ignite
type TodosV1 { table: Table<Todo>, changes: Topic<TodoEvent> }

fn open_count(items: List<Todo>) -> Int {
  var n = 0
  for todo in items {
    if !todo.done {
      n = n + 1
    }
  }
  n
}

fn from_v1(old: TodosV1) -> Todos =
  Todos { table = old.table, changes = old.changes, open = open_count(old.table.values()) }
```

The `store` component gains `migrate = from_v1`. `TodoMsg` is unchanged, so the reload needs neither a drain nor a coordinated batch (7.3). After the reload commits, `TodosV1` and `from_v1` can be deleted, as with stores (wasm §5.3).

## 10 Testing

Slice 8 landed: inline `test "name" { .. }` blocks, `test fn` and other test-only items, the statements `assert cond` and `assert cond, message`, and `ignite test` (`docs/slice.md` §12 "Slice 8: tests"; end-to-end in `crates/ignite/tests/ignite_test.rs`). A pure `handle` is driven directly, with no runtime and no threads:

```ignite
test "create assigns increasing ids and publishes" {
  let reply: ReplyTo<Todo> = ReplyTo.test(1)
  let empty = Todos { table = Table.empty(), changes = Topic.test(2) }
  let first = todos(empty, Create("milk", false, reply))
  let second = todos(first.state, Create("eggs", false, reply))
  match second.out.replied(reply) {
    [todo] => { assert todo.id == 2 }
    _ => { assert false, "exactly one reply" }
  }
  assert second.out.sent(second.state.changes).len() == 1
}
```

- `ReplyTo.test(n)` and `Topic.test(n)` make fake capabilities, and only inside tests and test-only items (E064); forging is harmless there. This is the rule slice 8 already applies to `Store.new()` (slice §12.3, E051); P2 decides whether E064 folds into E051. `out.replied(r)` and `out.sent(a)` are pure readers of an `Outbox`. Supervision policy is tested the same way, through `supervise`.
- Concurrency: `ignite test` runs a system's actors under `std.actor.sim`, a single-threaded dispatcher that picks the next ready mailbox with a seeded generator and records the delivery order as a trace. A failing seed is printed and replays exactly ("replay equivalence compares the same facts and ordered events", design §6.3). Timers advance a virtual clock. Connection tasks are not simulated; simulation tests talk to addresses.
- On wasm, `ignite test --target wasm` runs the same blocks (wasm §9).

## 11 Phased roadmap

Sizes: S is about a third of slice 7, M about slice 7 (slice §11), L about two slices. "wasm Pn" is a phase of wasm §10.

| Phase | Deliverable | Acceptance test | Size | Needs first |
|---|---|---|---|---|
| P1 Actors and calls | Builtins `Address`, `ReplyTo`, `Outbox`; the `call` rule (E060); the `Actor` kind (E061); actor names as values; mailboxes and `dispatch` in the substrate; `std.actor.run`, `Step`, `answer`; `std/table.ignite`; both native targets; the todo example of 9.2 | `crates/ignite/tests/todo.rs` passes unchanged on both targets, including 16 × 100 POSTs → 1600 distinct ids, with no `Store` in `examples/todo/todo.ignite`. A new case with `mailbox = 1` and a slow handler answers 503 from `Err(Timeout)` and does not hang. | M | Last-use moves of the loop's state (R1); type-erased `Outbox` via the `HeapValue` glue. No closures, no slice 5. |
| P2 Pure tests | `ReplyTo.test`, `Topic.test`, `out.replied`, `out.sent`, E064 | `ignite test examples/todo/todo.ignite` runs one test per `TodoMsg` variant against `todos` without starting a task | S | Slice 8 (landed, slice §12) |
| P3 Pub/sub and supervision | `Topic` and its std actor, `Sink`, overflow and drop counters; `Supervisor` (E062), strategies, intensity, restart types; panic containment on both native targets (C1); monitors and `Down`; `out.after`; exit status 5 (C2) | After 1600 POSTs, `Counts` returns `created = 1600`. A request that makes `store` panic gets 503, the next request succeeds, and ids continue from the restarted table. Four panics within 5000 ms end `todod` with status 5. | L | P1 |
| P4 Closures and dynamic actors | Capturing fn values; `call(fn(r) -> M)`; actors spawned under a `Pool` supervisor (`simple_one_for_one`); dynamic `Subscribe`; addresses inside messages | One actor per Telnet session holds the session state; 1000 sessions, each crash contained to its session | M | Closures (T3: a fn value cannot capture locals) |
| P5 Async and simulation | The dispatchers on slice-5 tokio; `std.actor.sim` with seeds and traces | 10 000 idle actors in one `todod` within a stated memory budget; a seeded simulation of `store` and `audit` replays a chosen seed with an identical trace | M | Slice 5 |
| P6 Persistence | `Persist`, journal and snapshots in the stored encoding; replay that discards the `Outbox` | `kill -9` of `todod` after 100 POSTs; after a restart `GET /todos` returns the same 100, and the next id is 101 | M | P3; wasm P4's stored encoding, or the same encoder built natively |
| P7 Actor hot reload | W1 to W8; actor swap (7.2); coordinated reload for `M` changes (7.3) | While 16 clients POST in a loop, the watcher reloads `store` with `from_v1` (9.5): no request fails, ids continue, and `open` equals the open todos at the swap. A `migrate` that traps leaves v1 serving. A change to `M` without its holders in the batch is refused. | L | P3, P6, wasm P4 |

P1 lands first. Inside P1, the move of the state through the loop comes before anything else; without it every message copies the whole state (R1).

## 12 Conflicts with stated Ignite decisions

| ID | Proposal | Stated decision |
|---|---|---|
| C1 | A panic ends only its actor's task and is reported to the supervisor, on all targets (6.4) | `docs/llvm.md` §4 "Values and memory" (`ignite_rt_panic`): a panic exits the process with status 101. Slice §7: "a task that panics ends its thread", with no report. |
| C2 | Exit status 5, "supervision gave up" (6.2) | Slice §6.1 defines statuses 0 to 4 per component kind; design §11.6 "CLI experience" proposes 0, 2, 3, 4 for `tn`. |

Diagnostic codes: actors propose E060 to E064. Earlier drafts used E050 to E054, which slice 8 took for tests (E050 to E053, slice §12) and the nesting limit (E054, slice §3).

No conflict: actors add no keyword (2.1); actor names as values extend slice §11.4 (2.2); `Store<T>` stays (2.3).

## 13 Risks and open questions

| ID | Risk or question | Handling |
|---|---|---|
| R1 | Value-semantics cost. The Rust ownership rule clones every place in a value position (slice §6), so `handle(current, msg)` in the loop and `state.table.put(..)` inside `handle` copy the state on every message: O(n) per message. | Slice 16 added last-use moves on both targets (`docs/slice.md` 20.3): one analysis in `ignite-types` (`TypedProgram::moves`, `consumed`) that both generators read. Moved without a copy: `s = handle(s, m)`, `a.state = a.handle(a.state, m)` and `Step { state = st.state, … }` with `st` dead after; a parameter the callee changes in place or moves on every path is consumed and taken by value. Still copied: an argument beside another argument that mentions the state, as in `answer(state, reply, state.table.values())` (binding `state.table.values()` first makes `state` move), a pattern binding, a `for` element and anything inside a closure body (20.13 item 4). A borrowed parameter was never the case R1 needs, because `handle` changes the state. Measured by `slice16::state_threading_is_linear` (Rust) and `slice16::llvm_state_threading_is_linear` (LLVM): 200,000 steps under 8 times 50,000. Open: a persistent `Table`; the 1600-POST measurement stays with P1 (20.14 Q1). |
| R2 | Deadlock through cycles of blocking `Outbox` sends between full mailboxes | `send_deadline_ms` turns it into an `Overloaded` crash. Open: a checker warning for a `Block` cycle in the wiring. |
| R3 | Thread-per-actor cost before slice 5 | Fine for static actors (tens); P4's dynamic actors wait for P5. |
| R4 | Addresses inside messages (P4) defeat the exact holder set of 7.3 | Conservative: a message type that ever travels in a message counts as held by every component. Open: routing by version, as Orleans does (A23). |
| R5 | Exactly-once `ReplyTo` needs linear types, which Ignite lacks | Drop and count a second reply. An arm that drops its `ReplyTo` leaves the caller waiting until its timeout; open: a lint for that. |
| R6 | One instance per actor on wasm: wasm §8 caps an instance at 64 MiB by default, and thousands of actors multiply the base cost | Measure in P7. Open: a smaller per-component cap for `Actor` components. |
| R7 | Should topics cross processes (the NATS path, design §8.9) under the same `Topic<E>`? | Leaning no: a cross-host topic is another resource kind with declared delivery, so that an in-process binding cannot stand in for a durable one (design §5 "Composition and deployment planning": "An embedded queue cannot substitute for a managed queue"). |
| R8 | The controller machines (design §11.8) have the reducer shape of actors (design §2 "Architecture and execution model"); actors could be the form for self-hosted runtime logic (M8, design §12.2 "Milestones and dependencies") | Open; not before P6, since controllers need the durable journal. |

## 14 References

| ID | Source | Used for |
|---|---|---|
| A1 | [Joe Armstrong, Making reliable distributed systems in the presence of software errors, 2003, §4.3 to 4.4](https://erlang.org/download/armstrong_thesis_2003.pdf) | "Let it crash", the error-handling slogans |
| A2 | [Erlang reference manual: Processes](https://www.erlang.org/doc/system/ref_man_processes.html) | Signal order, receive, links, trapping exits, monitors, `noproc` |
| A3 | [Erlang efficiency guide: Processes](https://www.erlang.org/doc/system/eff_guide_processes.html) | Copying of messages, the cost of selective receive, the reference optimisation |
| A4 | [ERTS: Erlang garbage collector](https://www.erlang.org/doc/apps/erts/garbagecollection.html) | Per-process generational copying GC |
| A5 | [Erik Stenman, The BEAM Book](https://blog.stenmans.org/theBeamBook/) | Preemptive scheduling, reductions, per-process heaps |
| A6 | [stdlib: gen_server](https://www.erlang.org/doc/apps/stdlib/gen_server.html) | `call` with a 5000 ms default that exits the caller; `cast`; `code_change` and rollback |
| A7 | [gen_statem behaviour](https://www.erlang.org/doc/system/statem.html) | Callback modes, postpone, state enter calls, timeouts |
| A8 | [stdlib: gen_event](https://www.erlang.org/doc/apps/stdlib/gen_event.html) | Handlers in the manager; a crashed handler is removed |
| A9 | [Supervisor behaviour](https://www.erlang.org/doc/system/sup_princ.html) | Strategies, intensity and period (defaults 1 and 5), restart types |
| A10 | [Applications](https://www.erlang.org/doc/system/applications.html) | Applications as units with a supervision tree |
| A11 | [Compilation and code loading](https://www.erlang.org/doc/system/code_loading.html) | Old and current code; a purge terminates lingering processes |
| A12 | [Release handling](https://www.erlang.org/doc/system/release_handling.html) | `sys:suspend`, `change_code`, `resume`; appups and relups |
| A13 | [Learn You Some Erlang: Relups](https://learnyousomeerlang.com/relups) | The complexity of relups; the advice to avoid them |
| A14 | [kernel: pg](https://www.erlang.org/doc/apps/kernel/pg.html) | Process groups, scopes, eventual consistency |
| A15 | [Distributed Erlang](https://www.erlang.org/doc/system/distributed.html) | Location transparency, full mesh, clear text |
| A16 | [Fred Hebert, Handling Overload](https://ferd.ca/handling-overload.html) | Unbounded mailboxes, back-pressure and load shedding, synchronous calls |
| A17 | [gleam_otp: actor](https://gleam-otp.hexdocs.pm/gleam/otp/actor.html) | Typed actors, `on_message`, `call` with a timeout |
| A18 | [gleam_erlang: process](https://gleam-erlang.hexdocs.pm/gleam/erlang/process.html) | `Subject` ownership, `send`, `receive` with a timeout, monitors |
| A19 | [GenStage](https://gen-stage.hexdocs.pm/GenStage.html) | Demand-driven back-pressure, buffer overflow |
| A20 | [Broadway](https://broadway.hexdocs.pm/Broadway.html) | Pipelines on GenStage, batching, acknowledgements |
| A21 | [Akka Typed: Introduction to actors](https://doc.akka.io/libraries/akka-core/current/typed/actors.html) | `ActorRef[T]`, state as behaviour, protocols as types |
| A22 | [Akka Typed: Mailboxes](https://doc.akka.io/libraries/akka-core/current/typed/mailboxes.html) | Unbounded default; bounded overflow goes to dead letters |
| A23 | [Orleans overview](https://learn.microsoft.com/en-us/dotnet/orleans/overview) | Virtual actors, persistence, grain versioning |
| A24 | [Orleans: Develop a grain](https://learn.microsoft.com/en-us/dotnet/orleans/grains/) | Grain references as stable logical identities |
| A25 | [Pony tutorial: Reference capabilities](https://tutorial.ponylang.io/reference-capabilities/reference-capabilities.html) | `iso`, `val`, `tag`: data-race freedom without copies; the zero-copy alternative to Ignite's value copies, not adopted |
| A26 | [Unison: The big idea](https://www.unison-lang.org/docs/the-big-idea/) | Content-addressed definitions; the basis for structural type hashes |
| A27 | [tokio::sync::mpsc](https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html) | Bounded channels give back-pressure; unbounded ones give none |
| A28 | [tokio mpsc Sender](https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.Sender.html) | `send` waits for capacity; `try_send` returns `Full` or `Closed`; `send_timeout` |
