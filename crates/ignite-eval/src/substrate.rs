//! The model of the substrate under a test (`docs/slice.md` 33.5): stores,
//! the data capabilities, pipes, tasks, channels and a virtual clock, in
//! memory and deterministic. [`crate::Evaluator::run_test`] makes one
//! [`World`] per test; property expressions, `ignite check` and `ignite
//! plan` have none, so there these builtins still have no value at compile
//! time.
//!
//! Tasks. Every task of the test (the body is task 0) runs on a thread of
//! its own, but only the task holding the turn runs: a task runs until it
//! blocks (a read of an empty pipe, a channel operation that must wait,
//! `Clock.sleep`) or ends, and then the turn goes to the first task in
//! spawn order that is ready. So a run is the same every time. When no task
//! is ready, time is virtual: it advances to the earliest deadline a blocked
//! task has (an idle timeout, `receive_within`, a sleep), never by reading a
//! clock. When no task is ready and none has a deadline the model is
//! deadlocked, and the test is one the evaluator cannot run. A task that
//! fails ends only itself, as on the targets; one the evaluator cannot run
//! (a limit, an operation it has no rule for) ends the test with that
//! reason. Once the test body has ended, the other tasks are dropped.
//!
//! Handles. A store, a pipe end, a channel end or a capability is a
//! [`Handle`] value; its clones are its shares, as on the targets, and when
//! the last share is dropped the model drops the object: a pipe end delivers
//! what was written and ends its peer's stream, the last sender or receiver
//! closes its channel. Drops are queued and applied before the next
//! operation of the model, by the task that dropped the share, so they are
//! ordered like everything else.
//!
//! Pipes deliver as `crates/ignite-io/src/pipe.rs` does: a write is kept in
//! the end's send buffer and registered with the writing task, which
//! delivers it at its flush points (before a read, before a channel
//! operation or a sleep that waits, `Process.flush`, at the task's end), at
//! [`SEND_BUFFER`] bytes, on `close` and on the drop of the end; each
//! delivery is one batch of the peer's `received_batches`.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use ignite_types::QualName;

use crate::Value;
use crate::eval::EvalError;

/// Unread bytes a pipe end's receive queue holds before a delivery into it
/// fails (`ignite_io::PIPE_CAPACITY`).
const PIPE_CAPACITY: usize = 16 * 1024 * 1024;

/// Bytes an end's send buffer holds before a write delivers them
/// (`ignite_io::SEND_BUFFER`).
pub(crate) const SEND_BUFFER: usize = 64 * 1024;

/// The largest read (`ignite_io::MAX_READ`).
const MAX_READ: usize = 1024 * 1024;

/// The tasks one test may spawn: each runs on a thread with the
/// evaluator's stack, so a test that spawns more is one the evaluator
/// cannot run.
pub(crate) const MAX_TASKS: usize = 64;

const QUEUE_FULL: &str = "pipe queue full";
const PEER_GONE: &str = "pipe peer is gone";
const CLOSED: &str = "write to a closed stream";

/// What a [`Handle`] is a share of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HandleKind {
    Store,
    PipeStream,
    Sender,
    Receiver,
    Setting,
    CommandLine,
    Environment,
}

impl HandleKind {
    /// The type's name, as `debug()` writes it in `<..>` on the targets.
    pub fn name(self) -> &'static str {
        match self {
            HandleKind::Store => "Store",
            HandleKind::PipeStream => "PipeStream",
            HandleKind::Sender => "Sender",
            HandleKind::Receiver => "Receiver",
            HandleKind::Setting => "Setting",
            HandleKind::CommandLine => "CommandLine",
            HandleKind::Environment => "Environment",
        }
    }
}

/// A handle of the model: one share of a store, a pipe end, a channel end
/// or a capability. Clones are shares of the same object and equal; the
/// object is dropped when its last share is (module docs).
#[derive(Clone)]
pub struct Handle {
    kind: HandleKind,
    id: usize,
    share: Arc<Share>,
}

impl Handle {
    pub fn kind(&self) -> HandleKind {
        self.kind
    }

    pub(crate) fn id(&self) -> usize {
        self.id
    }
}

impl PartialEq for Handle {
    fn eq(&self, other: &Handle) -> bool {
        self.kind == other.kind && Arc::ptr_eq(&self.share, &other.share)
    }
}

impl Eq for Handle {}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{} {}>", self.kind.name(), self.id)
    }
}

/// The object behind a handle; its drop queues the object's drop.
struct Share {
    kind: HandleKind,
    id: usize,
    dropped: Arc<Mutex<Vec<(HandleKind, usize)>>>,
}

impl Drop for Share {
    fn drop(&mut self) {
        lock(&self.dropped).push((self.kind, self.id));
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The test is over (its body ended, a task the evaluator cannot run ended
/// it, or the model deadlocked): the calling task unwinds.
#[derive(Debug)]
pub(crate) struct Over;

/// What a blocked task waits for, besides its deadline.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Wait {
    /// Bytes or the end of the stream in this pipe end.
    Read(usize),
    /// A value in this channel, or its close.
    Receive(usize),
    /// Room in this channel, or its close.
    Send(usize),
    /// Only the deadline.
    Sleep,
}

/// What a spawned task calls.
pub(crate) enum Call {
    Fn(QualName),
    Role { protocol: QualName, role: String },
    Value(Value),
}

/// A spawned task's call: what it calls and the arguments, evaluated by
/// the spawning task.
pub(crate) struct Job {
    pub(crate) call: Call,
    pub(crate) args: Vec<Value>,
}

#[derive(Debug)]
enum TaskState {
    Ready,
    Running,
    Blocked(Wait, Option<i64>),
    Done,
}

#[derive(Debug)]
struct Task {
    state: TaskState,
    /// The pipe ends the task wrote to since its last flush.
    pending: Vec<usize>,
}

#[derive(Default)]
struct Store {
    last: i64,
    values: BTreeMap<i64, Value>,
}

/// One pipe end: what it reads and what it has written and not delivered.
#[derive(Default)]
struct End {
    peer: usize,
    incoming: VecDeque<u8>,
    batches: i64,
    /// The peer closed its write half or is gone.
    ended: bool,
    /// This end is gone: a delivery into it fails.
    gone: bool,
    urgent: Option<usize>,
    buffer: Vec<u8>,
    failed: Option<String>,
    closed: bool,
    idle_ms: i64,
}

struct Channel {
    queue: VecDeque<Value>,
    capacity: usize,
    closed: bool,
}

struct State {
    now: i64,
    running: usize,
    units: u64,
    tasks: Vec<Task>,
    spawns: Vec<(usize, Job)>,
    over: Option<Result<(), EvalError>>,
    stores: Vec<Store>,
    ends: Vec<End>,
    channels: Vec<Channel>,
    capabilities: Vec<Value>,
}

struct Shared {
    state: Mutex<State>,
    turn: Condvar,
    dropped: Arc<Mutex<Vec<(HandleKind, usize)>>>,
}

/// The model of one test, as the task `me` sees it.
#[derive(Clone)]
pub(crate) struct World {
    shared: Arc<Shared>,
    pub(crate) me: usize,
}

impl std::fmt::Debug for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "World(task {})", self.me)
    }
}

/// The result of a pipe read that did not wait.
pub(crate) enum Read {
    Data(Vec<u8>),
    Eof,
}

impl World {
    /// A model whose task 0, the test body, holds the turn with `units` of
    /// fuel.
    pub(crate) fn new(units: u64) -> World {
        World {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    now: 0,
                    running: 0,
                    units,
                    tasks: vec![Task {
                        state: TaskState::Running,
                        pending: Vec::new(),
                    }],
                    spawns: Vec::new(),
                    over: None,
                    stores: Vec::new(),
                    ends: Vec::new(),
                    channels: Vec::new(),
                    capabilities: Vec::new(),
                }),
                turn: Condvar::new(),
                dropped: Arc::new(Mutex::new(Vec::new())),
            }),
            me: 0,
        }
    }

    /// The same model as the task `me` sees it.
    pub(crate) fn task(&self, me: usize) -> World {
        World {
            shared: Arc::clone(&self.shared),
            me,
        }
    }

    fn handle(&self, kind: HandleKind, id: usize) -> Handle {
        Handle {
            kind,
            id,
            share: Arc::new(Share {
                kind,
                id,
                dropped: Arc::clone(&self.shared.dropped),
            }),
        }
    }

    /// The state, with the drops queued since the last operation applied.
    fn state(&self) -> Result<MutexGuard<'_, State>, Over> {
        let mut state = lock(&self.shared.state);
        if state.over.is_some() {
            return Err(Over);
        }
        self.apply_drops(&mut state);
        Ok(state)
    }

    fn apply_drops(&self, state: &mut State) {
        loop {
            let dropped = std::mem::take(&mut *lock(&self.shared.dropped));
            if dropped.is_empty() {
                return;
            }
            for (kind, id) in dropped {
                // Values a dropped object held are dropped after the
                // state's own borrow ends, so their shares queue again.
                let mut released: Vec<Value> = Vec::new();
                match kind {
                    HandleKind::Store => {
                        released.extend(std::mem::take(&mut state.stores[id].values).into_values());
                    }
                    HandleKind::PipeStream => drop_end(state, id),
                    HandleKind::Sender | HandleKind::Receiver => {
                        state.channels[id].closed = true;
                    }
                    HandleKind::Setting | HandleKind::CommandLine | HandleKind::Environment => {
                        released.push(std::mem::replace(&mut state.capabilities[id], Value::Unit));
                    }
                }
                drop(released);
            }
        }
    }

    // ---- tasks ----------------------------------------------------------

    /// Register a task for `job`, ready after every task spawned before it;
    /// the supervisor starts its thread. `Err` names why it cannot start.
    pub(crate) fn spawn(&self, job: Job) -> Result<Result<(), String>, Over> {
        let mut state = self.state()?;
        if state.tasks.len() > MAX_TASKS {
            return Ok(Err(format!("a test spawns more than {MAX_TASKS} tasks")));
        }
        let id = state.tasks.len();
        state.tasks.push(Task {
            state: TaskState::Ready,
            pending: Vec::new(),
        });
        state.spawns.push((id, job));
        drop(state);
        self.shared.turn.notify_all();
        Ok(Ok(()))
    }

    /// The supervisor's wait for tasks to start: `None` once the test is
    /// over.
    pub(crate) fn next_spawns(&self) -> Option<Vec<(usize, Job)>> {
        let mut state = lock(&self.shared.state);
        loop {
            if state.over.is_some() {
                return None;
            }
            if !state.spawns.is_empty() {
                return Some(std::mem::take(&mut state.spawns));
            }
            state = self
                .shared
                .turn
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// End the test with `outcome` when nothing ended it before (a thread
    /// that cannot start).
    pub(crate) fn end_with(&self, outcome: EvalError) {
        let mut state = lock(&self.shared.state);
        if state.over.is_none() {
            state.over = Some(Err(outcome));
        }
        drop(state);
        self.shared.turn.notify_all();
    }

    /// Wait for the turn of task `me`: the fuel left, or `Over`.
    pub(crate) fn begin(&self) -> Result<u64, Over> {
        let mut state = lock(&self.shared.state);
        loop {
            if state.over.is_some() {
                return Err(Over);
            }
            if state.running == self.me {
                return Ok(state.units);
            }
            state = self
                .shared
                .turn
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// The task `me` ended with `units` of fuel left: its writes are
    /// delivered and the turn passes on. The body's end (task 0) is the
    /// test's outcome; another task's end is the test's only when the
    /// evaluator cannot run it.
    pub(crate) fn end(&self, ended: Result<(), EvalError>, units: u64) {
        let mut state = lock(&self.shared.state);
        if state.over.is_some() {
            return;
        }
        self.apply_drops(&mut state);
        flush(&mut state, self.me);
        state.units = units;
        state.tasks[self.me].state = TaskState::Done;
        if self.me == 0 {
            state.over = Some(ended);
        } else if let Err(error) = ended {
            state.over = Some(Err(error));
        } else {
            schedule(&mut state);
        }
        drop(state);
        self.shared.turn.notify_all();
    }

    /// The test's outcome, once it is over, and the fuel left.
    pub(crate) fn outcome(&self) -> (Option<Result<(), EvalError>>, u64) {
        let state = lock(&self.shared.state);
        (state.over.clone(), state.units)
    }

    /// Block the task `me` on `wait` until it is ready or `deadline` (in
    /// virtual nanoseconds) has come; `units` is the fuel it hands on and
    /// gets back.
    pub(crate) fn block(
        &self,
        wait: Wait,
        deadline: Option<i64>,
        units: &mut u64,
    ) -> Result<(), Over> {
        let mut state = self.state()?;
        state.units = *units;
        state.tasks[self.me].state = TaskState::Blocked(wait, deadline);
        schedule(&mut state);
        self.shared.turn.notify_all();
        loop {
            if state.over.is_some() {
                return Err(Over);
            }
            if state.running == self.me {
                *units = state.units;
                return Ok(());
            }
            state = self
                .shared
                .turn
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// The virtual time in nanoseconds since the test began.
    pub(crate) fn now(&self) -> Result<i64, Over> {
        Ok(self.state()?.now)
    }

    /// Deliver what the task `me` wrote (`Process.flush`, and every
    /// operation that waits).
    pub(crate) fn flush(&self) -> Result<(), Over> {
        let mut state = self.state()?;
        flush(&mut state, self.me);
        Ok(())
    }

    /// The nodes the model holds: stored values, queued values and bytes.
    pub(crate) fn nodes(&self) -> u64 {
        let state = lock(&self.shared.state);
        let stored: u64 = state
            .stores
            .iter()
            .flat_map(|store| store.values.values())
            .map(Value::nodes)
            .sum();
        let queued: u64 = state
            .channels
            .iter()
            .flat_map(|channel| &channel.queue)
            .map(Value::nodes)
            .sum();
        let bytes: u64 = state
            .ends
            .iter()
            .map(|end| ((end.incoming.len() + end.buffer.len()) / 64) as u64)
            .sum();
        stored + queued + bytes
    }

    // ---- stores ---------------------------------------------------------

    pub(crate) fn store_new(&self) -> Result<Handle, Over> {
        let mut state = self.state()?;
        let id = state.stores.len();
        state.stores.push(Store::default());
        Ok(self.handle(HandleKind::Store, id))
    }

    /// The next id of the store: 1, 2, 3, ..
    pub(crate) fn store_next_id(&self, store: usize) -> Result<i64, Over> {
        let mut state = self.state()?;
        let store = &mut state.stores[store];
        store.last += 1;
        Ok(store.last)
    }

    pub(crate) fn store_get(&self, store: usize, id: i64) -> Result<Option<Value>, Over> {
        Ok(self.state()?.stores[store].values.get(&id).cloned())
    }

    pub(crate) fn store_put(&self, store: usize, id: i64, value: Value) -> Result<(), Over> {
        let replaced = self.state()?.stores[store].values.insert(id, value);
        drop(replaced);
        Ok(())
    }

    pub(crate) fn store_remove(&self, store: usize, id: i64) -> Result<Option<Value>, Over> {
        Ok(self.state()?.stores[store].values.remove(&id))
    }

    /// Every value in ascending id order.
    pub(crate) fn store_values(&self, store: usize) -> Result<Vec<Value>, Over> {
        Ok(self.state()?.stores[store]
            .values
            .values()
            .cloned()
            .collect())
    }

    // ---- capabilities ---------------------------------------------------

    /// A capability of `kind` holding `value` (`Setting.of`,
    /// `CommandLine.of`, `Environment.of`).
    pub(crate) fn capability(&self, kind: HandleKind, value: Value) -> Result<Handle, Over> {
        let mut state = self.state()?;
        let id = state.capabilities.len();
        state.capabilities.push(value);
        Ok(self.handle(kind, id))
    }

    pub(crate) fn capability_value(&self, id: usize) -> Result<Value, Over> {
        Ok(self.state()?.capabilities[id].clone())
    }

    // ---- pipes ----------------------------------------------------------

    /// Two connected ends (`Pipe.pair()`).
    pub(crate) fn pipe_pair(&self) -> Result<(Handle, Handle), Over> {
        let mut state = self.state()?;
        let first = state.ends.len();
        state.ends.push(End {
            peer: first + 1,
            ..End::default()
        });
        state.ends.push(End {
            peer: first,
            ..End::default()
        });
        Ok((
            self.handle(HandleKind::PipeStream, first),
            self.handle(HandleKind::PipeStream, first + 1),
        ))
    }

    /// The start of a read of `end`: the task's writes delivered, a kept
    /// failure taken, and the read's deadline from the idle timeout.
    pub(crate) fn read_start(&self, end: usize) -> Result<Result<Option<i64>, String>, Over> {
        let mut state = self.state()?;
        flush(&mut state, self.me);
        if let Some(failed) = state.ends[end].failed.take() {
            return Ok(Err(failed));
        }
        let idle = state.ends[end].idle_ms;
        Ok(Ok((idle > 0).then(|| {
            state.now.saturating_add(idle.saturating_mul(1_000_000))
        })))
    }

    /// One try of a read of at most `maximum` bytes: `None` when it must
    /// wait.
    pub(crate) fn read_try(
        &self,
        end: usize,
        maximum: i64,
        deadline: Option<i64>,
    ) -> Result<Option<Read>, Over> {
        let mut state = self.state()?;
        let now = state.now;
        let size = usize::try_from(maximum.max(1))
            .unwrap_or(MAX_READ)
            .min(MAX_READ);
        let end = &mut state.ends[end];
        if !end.incoming.is_empty() {
            let mut count = size.min(end.incoming.len());
            if let Some(mark) = end.urgent.filter(|mark| *mark > 0) {
                count = count.min(mark);
            }
            end.urgent = end.urgent.and_then(|mark| mark.checked_sub(count));
            let bytes: Vec<u8> = end.incoming.drain(..count).collect();
            return Ok(Some(Read::Data(bytes)));
        }
        if end.ended || deadline.is_some_and(|deadline| now >= deadline) {
            return Ok(Some(Read::Eof));
        }
        Ok(None)
    }

    /// `write_all`: kept in the send buffer and registered with the task,
    /// or delivered at [`SEND_BUFFER`] bytes.
    pub(crate) fn write(&self, end: usize, data: &[u8]) -> Result<Result<(), String>, Over> {
        let mut state = self.state()?;
        let me = self.me;
        let state = &mut *state;
        if state.ends[end].closed {
            return Ok(Err(CLOSED.to_string()));
        }
        if let Some(failed) = state.ends[end].failed.take() {
            return Ok(Err(failed));
        }
        state.ends[end].buffer.extend_from_slice(data);
        if state.ends[end].buffer.len() >= SEND_BUFFER {
            return Ok(deliver(state, end, false));
        }
        let pending = &mut state.tasks[me].pending;
        if !pending.contains(&end) {
            pending.push(end);
        }
        Ok(Ok(()))
    }

    /// `write_urgent`: what was written and `data` delivered at once, the
    /// last byte of `data` marked urgent in the peer's queue.
    pub(crate) fn write_urgent(&self, end: usize, data: &[u8]) -> Result<Result<(), String>, Over> {
        let mut state = self.state()?;
        let state = &mut *state;
        if state.ends[end].closed {
            return Ok(Err(CLOSED.to_string()));
        }
        if let Some(failed) = state.ends[end].failed.take() {
            return Ok(Err(failed));
        }
        state.ends[end].buffer.extend_from_slice(data);
        Ok(deliver(state, end, !data.is_empty()))
    }

    /// `close`: deliver what was written, then end the peer's stream.
    pub(crate) fn close(&self, end: usize) -> Result<Result<(), String>, Over> {
        let mut state = self.state()?;
        let state = &mut *state;
        state.ends[end].closed = true;
        let closed = match state.ends[end].failed.take() {
            Some(failed) => Err(failed),
            None => deliver(state, end, false),
        };
        if closed.is_err() {
            state.ends[end].buffer = Vec::new();
        }
        let peer = state.ends[end].peer;
        state.ends[peer].ended = true;
        Ok(closed)
    }

    pub(crate) fn set_idle_timeout(&self, end: usize, ms: i64) -> Result<(), Over> {
        self.state()?.ends[end].idle_ms = ms.max(0);
        Ok(())
    }

    pub(crate) fn urgent_pending(&self, end: usize) -> Result<bool, Over> {
        Ok(self.state()?.ends[end].urgent.is_some())
    }

    pub(crate) fn received_batches(&self, end: usize) -> Result<i64, Over> {
        Ok(self.state()?.ends[end].batches)
    }

    // ---- channels -------------------------------------------------------

    /// `Channel.bounded(capacity)` for a capacity of at least 1.
    pub(crate) fn channel(&self, capacity: usize) -> Result<(Handle, Handle), Over> {
        let mut state = self.state()?;
        let id = state.channels.len();
        state.channels.push(Channel {
            queue: VecDeque::new(),
            capacity,
            closed: false,
        });
        Ok((
            self.handle(HandleKind::Sender, id),
            self.handle(HandleKind::Receiver, id),
        ))
    }

    /// One try of `send`: `Ok(Ok(queued))`, or the value back when it
    /// must wait (`wait` false: `try_send`, which never does). The task's
    /// writes are delivered first when it would wait.
    pub(crate) fn send(
        &self,
        channel: usize,
        value: Value,
        wait: bool,
    ) -> Result<Result<bool, Value>, Over> {
        let mut state = self.state()?;
        let full = {
            let channel = &state.channels[channel];
            channel.queue.len() >= channel.capacity && !channel.closed
        };
        if full {
            if !wait {
                drop(state);
                drop(value);
                return Ok(Ok(false));
            }
            flush(&mut state, self.me);
            return Ok(Err(value));
        }
        let channel = &mut state.channels[channel];
        if channel.closed {
            drop(state);
            drop(value);
            return Ok(Ok(false));
        }
        channel.queue.push_back(value);
        Ok(Ok(true))
    }

    /// One try of `receive`: `Some(value)`, `Some(None)` once closed and
    /// empty or past `deadline`, `None` when it must wait. The task's
    /// writes are delivered first when the channel is empty and open.
    pub(crate) fn receive(
        &self,
        channel: usize,
        deadline: Option<i64>,
        first: bool,
    ) -> Result<Option<Option<Value>>, Over> {
        let mut state = self.state()?;
        let now = state.now;
        let empty = {
            let channel = &state.channels[channel];
            channel.queue.is_empty() && !channel.closed
        };
        if empty && first {
            flush(&mut state, self.me);
        }
        let channel = &mut state.channels[channel];
        if let Some(value) = channel.queue.pop_front() {
            return Ok(Some(Some(value)));
        }
        if channel.closed || deadline.is_some_and(|deadline| now >= deadline) {
            return Ok(Some(None));
        }
        Ok(None)
    }

    pub(crate) fn close_channel(&self, channel: usize) -> Result<(), Over> {
        self.state()?.channels[channel].closed = true;
        Ok(())
    }

    pub(crate) fn channel_closed(&self, channel: usize) -> Result<bool, Over> {
        Ok(self.state()?.channels[channel].closed)
    }
}

/// Deliver `end`'s send buffer into its peer's queue as one batch, or
/// nothing; the buffer is empty afterwards either way. With `urgent` the
/// last byte delivered becomes the peer's urgent byte.
fn deliver(state: &mut State, end: usize, urgent: bool) -> Result<(), String> {
    let buffer = std::mem::take(&mut state.ends[end].buffer);
    if buffer.is_empty() {
        return Ok(());
    }
    let peer = state.ends[end].peer;
    let peer = &mut state.ends[peer];
    if peer.gone {
        return Err(PEER_GONE.to_string());
    }
    if peer.incoming.len() + buffer.len() > PIPE_CAPACITY {
        return Err(QUEUE_FULL.to_string());
    }
    if urgent {
        peer.urgent = Some(peer.incoming.len() + buffer.len() - 1);
    }
    peer.incoming.extend(buffer);
    peer.batches += 1;
    Ok(())
}

/// Deliver what the task `task` wrote; a failed delivery is kept with its
/// end for the end's next read, write or close.
fn flush(state: &mut State, task: usize) {
    for end in std::mem::take(&mut state.tasks[task].pending) {
        if state.ends[end].gone {
            continue;
        }
        if let Err(error) = deliver(state, end, false) {
            state.ends[end].failed = Some(error);
        }
    }
}

/// The last share of a pipe end is gone: deliver what was written, end the
/// peer's stream and fail every later delivery into this end.
fn drop_end(state: &mut State, end: usize) {
    let _ = deliver(state, end, false);
    let peer = state.ends[end].peer;
    state.ends[peer].ended = true;
    let end = &mut state.ends[end];
    end.gone = true;
    end.incoming = VecDeque::new();
    end.urgent = None;
}

/// Whether a blocked task's wait is over.
fn ready(state: &State, wait: Wait, deadline: Option<i64>) -> bool {
    if deadline.is_some_and(|deadline| state.now >= deadline) {
        return true;
    }
    match wait {
        Wait::Read(end) => {
            let end = &state.ends[end];
            !end.incoming.is_empty() || end.ended
        }
        Wait::Receive(channel) => {
            let channel = &state.channels[channel];
            !channel.queue.is_empty() || channel.closed
        }
        Wait::Send(channel) => {
            let channel = &state.channels[channel];
            channel.queue.len() < channel.capacity || channel.closed
        }
        Wait::Sleep => false,
    }
}

/// Give the turn to the first task in spawn order that can run, advancing
/// the virtual time to the earliest deadline when none can; with none and
/// no deadline the test is over as a deadlock.
fn schedule(state: &mut State) {
    loop {
        let next = state.tasks.iter().position(|task| match task.state {
            TaskState::Ready => true,
            TaskState::Blocked(wait, deadline) => ready(state, wait, deadline),
            TaskState::Running | TaskState::Done => false,
        });
        if let Some(next) = next {
            state.tasks[next].state = TaskState::Running;
            state.running = next;
            return;
        }
        let earliest = state
            .tasks
            .iter()
            .filter_map(|task| match task.state {
                TaskState::Blocked(_, deadline) => deadline,
                _ => None,
            })
            .min();
        match earliest {
            Some(deadline) if deadline > state.now => state.now = deadline,
            _ => {
                state.over = Some(Err(EvalError::Failed {
                    report: "evaluation failed: every task of the model waits and none has a deadline (a deadlock)".to_string(),
                }));
                return;
            }
        }
    }
}
