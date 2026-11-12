//! Readiness scheduler (`docs/readiness-io.md` 4.1, 4.3, 4.5, 4.6; phase
//! R1): in the IO mode `readiness` every task [`crate::spawn`] starts is a
//! stackful task on one of a few worker threads, and a task that waits
//! parks itself instead of its thread.
//!
//! A worker is an OS thread (named `worker`) running a loop: run each
//! runnable task until it parks or ends, fire the timers that are due,
//! then wait in its reactor ([`crate::reactor::EpollReactor`], reached
//! only through the [`Reactor`] interface) until a descriptor is ready, a
//! timer is due or another thread wakes it. A worker creates its reactor
//! (an epoll and an eventfd descriptor) when a task first waits for a
//! descriptor through it (a lone task that may block the worker does not,
//! `wait_blocking`) and parks its thread until then. Workers start lazily,
//! one per spawn up to [`worker_limit`] (`IGNITE_WORKERS`, default
//! `std::thread::available_parallelism`), and a worker without a task for
//! [`crate::LINGER`] since its last task ended ends, so an idle server runs
//! `main` and the drain's thread only (`docs/readiness-io.md` section 6
//! row 6).
//!
//! A task is a stack (2 MiB reserved as a thread's, a guard page below it,
//! touched pages only resident; stacks are pooled: a few stay with the
//! worker their task ran on, for the next task placed there, the rest in
//! one shared pool), a coroutine on it
//! (`corosensei`), and its [`Waiter`]. [`park`] switches to the worker's
//! loop; [`schedule`] makes a parked task runnable on its worker, from any
//! thread: on the worker itself it joins the run queue, from elsewhere it
//! is handed over and the worker's reactor is woken when it sleeps.
//! Scheduling is cooperative: a task runs until it waits.
//!
//! Placement (`choose`): a spawn from a task stays on its worker while
//! that worker is at most one task busier than the least loaded one;
//! otherwise a spawn goes to a worker without tasks, else to a new worker
//! while there are fewer than the limit, else to the worker with the
//! fewest tasks. Per-task thread-locals of another crate (the LLVM
//! runtime's arena, failure handlers and owned handles) and of this one
//! (the pending writes, the open `catch`es) are exchanged whenever a
//! different task resumes, through [`TaskLocals`].
//!
//! The move (`story:readiness-task-stealing`, experiment; `IGNITE_STEAL=0`
//! switches it off): a task that waits in its first read while its worker
//! has another task waits for that read on the shared reactor
//! ([`shared`]) instead of its worker's. Its worker gives it up on its own
//! thread once the task parked ([`Worker::give_up`]: its locals swapped
//! out, its timer forgotten, its slot freed, its coroutine put into the
//! task; it counts among that worker's tasks until it is adopted), and
//! the worker whose wait returns the
//! readiness event, a timer of the task's deadline or a wake adopts it
//! ([`Worker::adopt`]). A task moves at most once; its later parks are on
//! the worker that adopted it. No thread reaches into another worker's
//! slots, timers or thread-locals. How sleeping workers wait on the shared
//! instance is [`watch_shared`].
//!
//! Stack overflow: a task's guard page is recognised by the `SIGSEGV`
//! handler (`crate::signal`) installed with the first worker, which runs
//! on the worker's alternate signal stack (std's on the Rust target, the
//! LLVM runtime's otherwise: `signal::give_workers_an_alternate_stack`),
//! prints `thread 'worker' has overflowed its stack` and aborts, as a
//! thread's overflow does; any other fault goes to the handler that was
//! installed before.
//!
//! A descriptor a task waits for (a socket, a listener, standard input)
//! is registered once per worker, edge-triggered, through a
//! [`Readiness`], by the first wait there that parks its task (the only
//! task of a worker waits on the thread instead, `wait_alone`): an event
//! wakes the tasks waiting to read or to write it, and a sequence number
//! per direction, taken before each attempt, keeps a readiness edge
//! between the attempt and the park from being lost
//! (`docs/readiness-io.md` K4).

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use corosensei::stack::{DefaultStack, Stack};
use corosensei::{Coroutine, CoroutineResult, Yielder};
use nix::sys::eventfd::EventFd;

use crate::fresh;
use crate::reactor::{EpollReactor, Event, Interest, Reactor, Token};
use crate::wait::{DescriptorWait, Limit, WaitList, Waiter, Woken, wake_ready};

/// How the substrate runs tasks (`docs/readiness-io.md` 3.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoMode {
    /// One OS thread per running task (`crate::task`).
    Threads,
    /// Stackful tasks on a few epoll workers (this module).
    Readiness,
}

/// The IO mode of the process: `IGNITE_IO=threads` or
/// `IGNITE_IO=readiness` when the process starts, `readiness` otherwise.
/// Read once; `story:io-mode-setting` makes it a build setting.
pub fn io_mode() -> IoMode {
    static MODE: OnceLock<IoMode> = OnceLock::new();
    *MODE.get_or_init(|| match std::env::var("IGNITE_IO").as_deref() {
        Ok("threads") => IoMode::Threads,
        _ => IoMode::Readiness,
    })
}

/// Whether tasks run on workers.
pub(crate) fn readiness() -> bool {
    io_mode() == IoMode::Readiness
}

/// Whether a task may move at its first read (module docs): not with
/// `IGNITE_STEAL=0` when the process starts.
pub(crate) fn steal() -> bool {
    static STEAL: OnceLock<bool> = OnceLock::new();
    *STEAL.get_or_init(|| std::env::var("IGNITE_STEAL").as_deref() != Ok("0"))
}

/// The most workers the process starts: `IGNITE_WORKERS` when it is a
/// positive number, else the parallelism available to the process (its
/// CPU affinity and cgroup quota), at least 1.
pub fn worker_limit() -> usize {
    static LIMIT: OnceLock<usize> = OnceLock::new();
    *LIMIT.get_or_init(|| {
        std::env::var("IGNITE_WORKERS")
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|count| *count > 0)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(usize::from)
                    .unwrap_or(1)
            })
    })
}

/// What a process whose tasks run on workers does once, however many
/// tasks it starts, done now: the worker limit (without `IGNITE_WORKERS`,
/// `std::thread::available_parallelism` reads the process' cgroup files),
/// the handler of a task's stack overflow and the eventfd that ends the
/// lone waits ([`escalation`]). Called when the process starts to listen
/// (`crate::net`), so that its first session does not wait for 30 system
/// calls no later session makes (`story:readiness-session-cost`). A
/// process that spawns without listening does each at its first use.
pub(crate) fn prepare() {
    if readiness() {
        worker_limit();
        crate::signal::install_overflow_handler();
        escalation();
    }
}

/// Stack reserved per task: a thread's default, so recursion depth does not
/// change with the IO mode.
pub const TASK_STACK: usize = 2 * 1024 * 1024;

/// Free stacks kept for the next tasks.
const STACK_POOL: usize = 64;

/// Free stacks a worker keeps in front of the pool's lock: a stack stays
/// with the worker its task ran on, and a spawn placed there takes it
/// back, from a task or from `main` alike, without the pool's lock
/// (`story:readiness-connect-latency`).
const WORKER_STACK_CACHE: usize = 2;

/// Tasks a worker runs before it looks at its reactor again although more
/// are runnable.
const BUDGET: usize = 128;

type Job = Box<dyn FnOnce() + Send + 'static>;

/// What the scheduler did since the process started, counted
/// (`story:readiness-connect-latency`): the cost of the connection path
/// as counts, read by tests instead of a timing. Each count is a relaxed
/// atomic bumped next to a lock or a system call the event costs anyway.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SchedulerCounts {
    /// Tasks started: given a stack and handed to a worker; a spawn that
    /// fails is not counted, nor is the stack it found.
    pub spawns: u64,
    /// Stacks mapped fresh because no free one was found.
    pub stacks_allocated: u64,
    /// Stacks a spawn took from a worker's cache: the one it placed the
    /// task on, or another when it starts a worker (which has none).
    pub stacks_from_cache: u64,
    /// Stacks a spawn took from the shared pool, under its lock.
    pub stacks_from_pool: u64,
    /// Stacks a task left that went to the shared pool (its worker's cache
    /// was full), under its lock.
    pub stacks_to_pool: u64,
    /// Spawns handed to a running worker other than the spawning thread's
    /// own (`hand_over` and `notify`).
    pub spawns_handed_over: u64,
    /// Workers started.
    pub workers_started: u64,
    /// Wakes of a parked task from a thread other than its worker's
    /// (`enqueue`: a hand-over and a notify).
    pub wakes_across: u64,
    /// Notifies that found the worker asleep and woke it (an eventfd write
    /// or an unpark): the system calls of the hand-overs above.
    pub notifies_sent: u64,
    /// Waits for a connection made around an `accept`: a `poll` off a
    /// worker, a wait through the reactor on one. Counted in both IO modes.
    pub accept_waits: u64,
    /// Clock reads of the scheduler, its reactors and the waits (`sched`,
    /// `reactor`, `wait`): every one they make, [`read_clock`]. A read of a
    /// stream that waits alone on its thread makes none for its idle
    /// timeout (`crate::wait::Limit`). Counted in both IO modes.
    pub clock_reads: u64,
    /// Tasks a worker gave up at their first read (module docs).
    pub tasks_given_up: u64,
    /// Tasks given up that a worker adopted, and of those, the ones a
    /// worker other than the one that gave them up adopted.
    pub tasks_adopted: u64,
    pub tasks_moved: u64,
    /// Waits of a worker on the shared reactor ([`watch_shared`]) that
    /// found no event.
    pub shared_polls_empty: u64,
}

/// The counts behind [`scheduler_counts`].
struct Counters {
    spawns: AtomicU64,
    stacks_allocated: AtomicU64,
    stacks_from_cache: AtomicU64,
    stacks_from_pool: AtomicU64,
    stacks_to_pool: AtomicU64,
    spawns_handed_over: AtomicU64,
    workers_started: AtomicU64,
    wakes_across: AtomicU64,
    notifies_sent: AtomicU64,
    accept_waits: AtomicU64,
    clock_reads: AtomicU64,
    tasks_given_up: AtomicU64,
    tasks_adopted: AtomicU64,
    tasks_moved: AtomicU64,
    shared_polls_empty: AtomicU64,
}

static COUNTERS: Counters = Counters {
    spawns: AtomicU64::new(0),
    stacks_allocated: AtomicU64::new(0),
    stacks_from_cache: AtomicU64::new(0),
    stacks_from_pool: AtomicU64::new(0),
    stacks_to_pool: AtomicU64::new(0),
    spawns_handed_over: AtomicU64::new(0),
    workers_started: AtomicU64::new(0),
    wakes_across: AtomicU64::new(0),
    notifies_sent: AtomicU64::new(0),
    accept_waits: AtomicU64::new(0),
    clock_reads: AtomicU64::new(0),
    tasks_given_up: AtomicU64::new(0),
    tasks_adopted: AtomicU64::new(0),
    tasks_moved: AtomicU64::new(0),
    shared_polls_empty: AtomicU64::new(0),
};

fn count(counter: &AtomicU64) {
    // Relaxed: a statistic, ordered with nothing.
    counter.fetch_add(1, Ordering::Relaxed);
}

/// An `accept` waits for a connection outside its `accept4`
/// ([`SchedulerCounts::accept_waits`]).
pub(crate) fn count_accept_wait() {
    count(&COUNTERS.accept_waits);
}

/// The instant now: the one way the scheduler, its reactors and the waits
/// read the clock, counted ([`SchedulerCounts::clock_reads`]).
#[inline]
pub(crate) fn read_clock() -> Instant {
    count(&COUNTERS.clock_reads);
    Instant::now()
}

/// The scheduler's counts so far ([`SchedulerCounts`]); all zero in the IO
/// mode `threads`, but for [`SchedulerCounts::accept_waits`] and
/// [`SchedulerCounts::clock_reads`].
pub fn scheduler_counts() -> SchedulerCounts {
    let read = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
    let c = &COUNTERS;
    SchedulerCounts {
        spawns: read(&c.spawns),
        stacks_allocated: read(&c.stacks_allocated),
        stacks_from_cache: read(&c.stacks_from_cache),
        stacks_from_pool: read(&c.stacks_from_pool),
        stacks_to_pool: read(&c.stacks_to_pool),
        spawns_handed_over: read(&c.spawns_handed_over),
        workers_started: read(&c.workers_started),
        wakes_across: read(&c.wakes_across),
        notifies_sent: read(&c.notifies_sent),
        accept_waits: read(&c.accept_waits),
        clock_reads: read(&c.clock_reads),
        tasks_given_up: read(&c.tasks_given_up),
        tasks_adopted: read(&c.tasks_adopted),
        tasks_moved: read(&c.tasks_moved),
        shared_polls_empty: read(&c.shared_polls_empty),
    }
}

/// Per-task state another crate keeps in thread-locals: the scheduler
/// creates one value per task with the function given to
/// [`set_task_locals`] and calls [`TaskLocals::swap`] when the task starts
/// or resumes on its worker and again when it parks or ends, so the
/// thread-locals hold the running task's state. The value is dropped on the
/// worker after the task ended. Nothing may be borrowed from those
/// thread-locals across a park.
pub trait TaskLocals {
    /// Exchange this value with the thread-locals it stands for.
    fn swap(&mut self);
}

static LOCALS: OnceLock<fn() -> Box<dyn TaskLocals>> = OnceLock::new();

/// Register the maker of every task's [`TaskLocals`]; the first call wins.
pub fn set_task_locals(make: fn() -> Box<dyn TaskLocals>) {
    let _ = LOCALS.set(make);
}

/// Running, parked, queued to run, or ended: a task's state.
const RUNNING: u8 = 0;
const PARKED: u8 = 1;
const QUEUED: u8 = 2;
const DONE: u8 = 3;

/// Where a task is (module docs): on the worker it was placed on, being
/// given up by it (set by the task before the park that gives it up, read
/// by the worker after it), given up and not adopted, or adopted.
const HOME: u8 = 0;
const LEAVING: u8 = 1;
const ADRIFT: u8 = 2;
const MOVED: u8 = 3;

/// The scheduler's part of a task's [`Waiter`].
pub(crate) struct Task {
    /// The worker the task was placed on.
    first: Arc<Shared>,
    /// The worker that adopted it, once it was given up: set once, as a
    /// task moves at most once.
    moved: OnceLock<Arc<Shared>>,
    state: AtomicU8,
    /// Readiness bits of the events since the task's last descriptor wait.
    ready: AtomicU8,
    /// The task's place in its worker's table; set again by the worker
    /// that adopts it, before it runs the task.
    slot: AtomicUsize,
    /// [`HOME`], [`LEAVING`], [`ADRIFT`] or [`MOVED`].
    phase: AtomicU8,
    /// The task has not begun its first read's wait: the one wait that
    /// may move it.
    first_read: AtomicBool,
    /// The task while it is adrift: its coroutine and locals, put here by
    /// the worker that gave it up and taken by the one that adopts it.
    carried: Mutex<Carried>,
}

/// A task between two workers ([`Task::carried`]).
struct Carried {
    local: Option<Local>,
    /// The deadline of the park it is given up in, set by the task before
    /// that park, kept in [`DRIFTING`] while it is adrift.
    deadline: Option<Instant>,
}

// SAFETY: a `Carried` holds a task that is parked and not running: its
// coroutine (`!Send` because it may hold thread-bound values on its stack),
// the yielder pointer into that stack, its `TaskLocals` and pending writes.
// One worker puts it into `Task::carried` after the coroutine yielded on
// that worker (`Worker::give_up`), and exactly one worker takes it out
// (`Worker::adopt`): the one whose `enqueue` follows the one successful
// PARKED to QUEUED exchange of the task's state (`Task::queue`), so no two
// threads hold it, and none resumes it while another does. What the
// stack keeps across that park is safe on another thread: the thread-locals
// the task uses were swapped out into its locals before (`restore_locals`,
// `swap_pending`, `crate::report::swap_catching`) and are swapped into the
// adopting worker's before it resumes the task; the substrate's and the LLVM
// runtime's code reach their thread-locals only through
// `crate::fresh`, so no frame on the stack holds a thread-local's address
// from the thread it left; the locks it holds are `wait::Lock`s, which
// hold no thread-bound guard; and the coroutine's stack and its guard page
// are memory of the process. The frames that can be on the stack at that
// park, and why each is safe, are listed in the module docs of
// `crate::fresh`.
unsafe impl Send for Carried {}

impl Task {
    /// The worker the task runs on, or ran on last.
    fn worker(&self) -> &Arc<Shared> {
        self.moved.get().unwrap_or(&self.first)
    }

    /// The task's slot on [`Task::worker`].
    fn slot(&self) -> usize {
        // Relaxed: changed only by the adopting worker before it resumes
        // the task, which then reads it on that worker; a waker acts on the
        // task only after the SeqCst exchange of `state` that follows.
        self.slot.load(Ordering::Relaxed)
    }

    /// A readiness event named this task (`wait::wake_ready`).
    pub(crate) fn note_ready(&self, ready: Interest) {
        self.ready.fetch_or(ready.bits(), Ordering::SeqCst);
    }

    /// Queue the parked task to run; whether this call did.
    fn queue(&self) -> bool {
        self.state
            .compare_exchange(PARKED, QUEUED, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

/// A worker's state other threads reach.
struct Shared {
    /// Created by the worker when a task first waits for a descriptor:
    /// until then the worker sleeps parked, so tasks that wait only for
    /// each other (channels, timers) cost no descriptor.
    reactor: OnceLock<EpollReactor>,
    /// The worker's thread, unparked to wake it while it has no reactor.
    thread: OnceLock<std::thread::Thread>,
    incoming: Mutex<Incoming>,
    /// `incoming` may hold something: set under its lock by a hand-over,
    /// cleared under it by the worker that takes it. The worker reads it
    /// instead of taking the lock on every turn of its loop.
    pending: AtomicBool,
    /// The worker sleeps (in its reactor, or parked): a hand-over must
    /// wake it.
    sleeping: AtomicBool,
    /// The worker's only task is in a blocking read ([`block_alone`]): a
    /// spawn goes elsewhere.
    blocked: AtomicBool,
    /// Tasks placed on the worker and not ended, and those it gave up that
    /// no worker adopted yet ([`Worker::give_up`]).
    tasks: AtomicUsize,
    /// Stacks its tasks left ([`WORKER_STACK_CACHE`]): pushed by the
    /// worker when a task ends, taken by the spawn that places the next
    /// task here, so the lock is rarely contended.
    stacks: Mutex<Vec<DefaultStack>>,
    /// The token of the shared reactor in this worker's reactor, 0 while
    /// it is not registered there ([`watch_shared`]).
    watch: AtomicU64,
}

/// What other threads hand a worker.
#[derive(Default)]
struct Incoming {
    woken: Vec<Waiter>,
    new: Vec<(Job, DefaultStack)>,
}

impl Incoming {
    fn is_empty(&self) -> bool {
        self.woken.is_empty() && self.new.is_empty()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    fn new() -> Shared {
        Shared {
            reactor: OnceLock::new(),
            thread: OnceLock::new(),
            incoming: Mutex::new(Incoming::default()),
            pending: AtomicBool::new(false),
            sleeping: AtomicBool::new(false),
            blocked: AtomicBool::new(false),
            tasks: AtomicUsize::new(0),
            stacks: Mutex::new(Vec::new()),
            watch: AtomicU64::new(0),
        }
    }

    /// Hand `put` to the worker's `incoming` (without waking it).
    fn hand_over(&self, put: impl FnOnce(&mut Incoming)) {
        let mut incoming = lock(&self.incoming);
        put(&mut incoming);
        // SeqCst: this store and the worker's `sleeping` store, then this
        // side's `sleeping` swap ([`Shared::notify`]) and the worker's
        // `pending` load, form a store-load pair each way: one side sees
        // the other's write, so the worker does not sleep on a hand-over.
        self.pending.store(true, Ordering::SeqCst);
    }

    /// Whether something was handed over and not taken yet; SeqCst for the
    /// store-load pair described at [`Shared::hand_over`].
    fn has_pending(&self) -> bool {
        self.pending.load(Ordering::SeqCst)
    }

    /// Take what was handed over, if the flag says there may be some.
    fn take(&self) -> Option<Incoming> {
        if !self.has_pending() {
            return None;
        }
        let mut incoming = lock(&self.incoming);
        // Relaxed: under the lock that every hand-over sets it under.
        self.pending.store(false, Ordering::Relaxed);
        Some(std::mem::take(&mut *incoming))
    }

    /// The worker's reactor, created on first use; called on the worker.
    /// While the move is on, a new reactor watches the shared one
    /// ([`watch_shared`]).
    fn reactor(&self) -> io::Result<&EpollReactor> {
        if let Some(reactor) = self.reactor.get() {
            return Ok(reactor);
        }
        let reactor = EpollReactor::new()?;
        if let Some(token) = watch_shared(&reactor) {
            // Relaxed: set before the reactor is published below, and read
            // only by this worker's loop.
            self.watch.store(token.0, Ordering::Relaxed);
        }
        let _ = self.reactor.set(reactor);
        Ok(self.reactor.get().expect("the reactor was just set"))
    }

    /// Wake the worker if it sleeps. The worker creates its reactor only
    /// while it runs a task, before it sets `sleeping`, so a waker that
    /// sees `sleeping` also sees how the worker sleeps.
    fn notify(&self) {
        if self.sleeping.swap(false, Ordering::SeqCst) {
            count(&COUNTERS.notifies_sent);
            match self.reactor.get() {
                Some(reactor) => {
                    let _ = reactor.wake();
                }
                None => {
                    if let Some(thread) = self.thread.get() {
                        thread.unpark();
                    }
                }
            }
        }
    }
}

/// The running workers.
static WORKERS: Mutex<Vec<Arc<Shared>>> = Mutex::new(Vec::new());

/// Free stacks.
static STACKS: Mutex<Vec<DefaultStack>> = Mutex::new(Vec::new());

thread_local! {
    /// The worker this thread runs, null off a worker.
    static HERE: Cell<*const Shared> = const { Cell::new(std::ptr::null()) };
    /// The waiter of the task running on this worker.
    static RUNNING_TASK: RefCell<Option<Waiter>> = const { RefCell::new(None) };
    /// The running task's way back to the worker's loop.
    static YIELDER: Cell<*const Yielder<(), ()>> = const { Cell::new(std::ptr::null()) };
    /// Tasks runnable on this worker.
    static RUN: RefCell<VecDeque<Waiter>> = const { RefCell::new(VecDeque::new()) };
    /// Tasks adrift that were made runnable on this worker's thread, for
    /// this worker to adopt ([`Worker::adopt`]).
    static ADOPT: RefCell<Vec<Waiter>> = const { RefCell::new(Vec::new()) };
    /// Deadlines of parked tasks, with a sequence number that keeps the
    /// keys apart, and the timer of each task slot ([`SlotTimer`]).
    static TIMERS: RefCell<Timers> = const { RefCell::new(Timers::new()) };
    /// The guard page of the running task's stack, `start..end`; read by
    /// the `SIGSEGV` handler, so a `const` cell without destructor.
    static GUARD: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

/// The worker this thread runs, null off a worker. Like every access of
/// this module's thread-locals from a task, through [`fresh`]: a task may
/// move to another thread ([`crate::fresh`]).
fn here() -> *const Shared {
    fresh::with(&HERE, Cell::get)
}

/// The waiter of the running task; `None` off a worker or between tasks.
pub(crate) fn running() -> Option<Waiter> {
    fresh::try_with(&RUNNING_TASK, |running| running.borrow().clone())
        .ok()
        .flatten()
}

/// Make the parked `task` runnable (`wait::wake`).
pub(crate) fn schedule(waiter: &Waiter, task: &Task) {
    if task.queue() {
        enqueue(waiter, task);
    }
}

fn enqueue(waiter: &Waiter, task: &Task) {
    // SeqCst: after the exchange that queued the task (`Task::queue`); the
    // worker that gave the task up stored ADRIFT before PARKED.
    if task.phase.load(Ordering::SeqCst) == ADRIFT {
        enqueue_adrift(waiter);
        return;
    }
    let worker = task.worker();
    if here() == Arc::as_ptr(worker) {
        fresh::with(&RUN, |run| run.borrow_mut().push_back(waiter.clone()));
        return;
    }
    count(&COUNTERS.wakes_across);
    worker.hand_over(|incoming| incoming.woken.push(waiter.clone()));
    worker.notify();
}

/// [`enqueue`] of a task adrift, which the caller queued: the worker of
/// this thread adopts it; off a worker, the next worker that looks at the
/// shared reactor ([`look_at_shared`]), which the shared reactor's wake
/// brings to it.
#[cold]
fn enqueue_adrift(waiter: &Waiter) {
    if !here().is_null() {
        fresh::with(&ADOPT, |adopt| adopt.borrow_mut().push(waiter.clone()));
        return;
    }
    lock(&STRAYS).push(waiter.clone());
    if let Some(shared) = shared() {
        let _ = shared.wake();
    }
}

/// Park the running `task` (`wait::park`): switch to the worker's loop
/// until a wake or `deadline`.
pub(crate) fn park(waiter: &Waiter, task: &Task, deadline: Option<Instant>) -> Woken {
    park_from(waiter, task, deadline, None)
}

/// [`park`] where `now`, if given, is the instant `deadline` was just
/// computed from: the first turn compares against it instead of reading
/// the clock. The task's slot is read again after every suspend: the task
/// may have moved there (in [`park_leaving`]'s call of this function).
fn park_from(
    waiter: &Waiter,
    task: &Task,
    deadline: Option<Instant>,
    mut now: Option<Instant>,
) -> Woken {
    debug_assert!(
        here() == Arc::as_ptr(task.worker()),
        "a task parks on its own worker"
    );
    loop {
        if waiter.take_wake() {
            return Woken::Notified;
        }
        if let Some(deadline) = deadline {
            if now.take().unwrap_or_else(read_clock) >= deadline {
                return Woken::Deadline;
            }
            arm(task.slot(), deadline, waiter);
        }
        suspend();
        if deadline.is_some() {
            with_timer(task.slot(), |timer| timer.wanted = None);
        }
    }
}

/// The park of a task's first read that gives the task up
/// ([`Readiness::wait_to_read`], module docs): the worker gives it up once it
/// yielded ([`Worker::give_up`]), and it returns on the worker that adopted
/// it, which goes on as [`park_from`] there. No timer is armed on this
/// worker: the deadline waits in [`DRIFTING`] while the task is adrift. A
/// wake already given ends the park before the task leaves.
#[inline(never)]
fn park_leaving(
    waiter: &Waiter,
    task: &Task,
    deadline: Option<Instant>,
    now: Option<Instant>,
) -> Woken {
    if waiter.take_wake() {
        return Woken::Notified;
    }
    if let Some(deadline) = deadline
        && now.unwrap_or_else(read_clock) >= deadline
    {
        return Woken::Deadline;
    }
    lock(&task.carried).deadline = deadline;
    // Relaxed: read by this worker after the yield below.
    task.phase.store(LEAVING, Ordering::Relaxed);
    suspend();
    // On the worker that adopted the task; a wake or the deadline made it
    // runnable, which `park_from` tells apart.
    park_from(waiter, task, deadline, None)
}

/// A worker's timers: a binary heap of entries keyed by deadline and
/// sequence number, the earliest on top, at most one per task slot, with
/// the position of each slot's entry in its [`SlotTimer`], so that the
/// entry of a task that ends is taken out at once and one armed earlier
/// moves up in place. No entry is ever left for a task that no longer
/// waits on it, so the worker never wakes for one. The `BTreeMap` this
/// replaces was 13 KB of code in every executable and took more
/// instructions per timer that fired (story `binary-size-back`).
struct Timers {
    heap: Vec<Timer>,
    /// The timer of each task slot of this worker, by slot: kept here, not
    /// in a vector of its own, as each allocation of a new worker costs the
    /// first connection it serves.
    slots: Vec<SlotTimer>,
    /// The sequence number of the latest entry.
    sequence: u64,
    /// The deadline of the worker's first timer: the keys count from it,
    /// so arming reads no clock.
    origin: Option<Instant>,
}

/// An entry of [`Timers`]: a copy moves it, its waiter is in its slot's
/// [`SlotTimer`].
#[derive(Clone, Copy)]
struct Timer {
    /// The deadline in nanoseconds, 2^63 at [`Timers::origin`], and the
    /// sequence number: integers order two entries by deadline, then by
    /// arming, for deadlines within 292 years of the origin.
    key: (u64, u64),
    /// The deadline.
    at: Instant,
    slot: usize,
}

/// The timer of the task in one slot of this worker. A task has at most one
/// entry in [`Timers`], whose deadline is the one armed; a park with a
/// deadline arms it only when none is armed at or before that deadline, and
/// a wake leaves it in place.
/// When it fires, a task that wants a later deadline by then is armed again
/// for it ([`Worker::fire_timers`]). A read with an idle timeout therefore
/// costs no timer operation per wait, only one per timeout period.
#[derive(Clone, Default)]
struct SlotTimer {
    /// The deadline of the task's current park, if it has one.
    wanted: Option<Instant>,
    /// The index plus one of the task's entry in the heap, 0 for none.
    position: u32,
    /// The waiter of the task's entry, while it has one.
    waiter: Option<Waiter>,
}

impl Timers {
    const fn new() -> Timers {
        Timers {
            heap: Vec::new(),
            slots: Vec::new(),
            sequence: 0,
            origin: None,
        }
    }

    /// The key of a new entry for `at` ([`Timer::key`]).
    fn key(&mut self, at: Instant) -> (u64, u64) {
        const ORIGIN: u64 = 1 << 63;
        self.sequence += 1;
        let origin = *self.origin.get_or_insert(at);
        let ticks = match at.checked_duration_since(origin) {
            Some(after) => ORIGIN.saturating_add(after.as_nanos() as u64),
            None => ORIGIN.saturating_sub(origin.duration_since(at).as_nanos() as u64),
        };
        (ticks, self.sequence)
    }

    fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// The timer of `slot`.
    #[inline]
    fn slot(&mut self, slot: usize) -> &mut SlotTimer {
        if self.slots.len() <= slot {
            self.grow(slot);
        }
        &mut self.slots[slot]
    }

    /// Make room for the timer of `slot`.
    #[cold]
    #[inline(never)]
    fn grow(&mut self, slot: usize) {
        self.slots.resize(slot + 1, SlotTimer::default());
    }

    /// Move `timer` up from `index` while it is earlier than its parent,
    /// and put it where it stops, its position recorded.
    #[inline]
    fn sift_up(&mut self, mut index: usize, timer: Timer) {
        let heap = &mut self.heap[..];
        let slots = &mut self.slots[..];
        while index > 0 {
            let parent = (index - 1) / 2;
            let above = heap[parent];
            if timer.key >= above.key {
                break;
            }
            heap[index] = above;
            slots[above.slot].position = (index + 1) as u32;
            index = parent;
        }
        heap[index] = timer;
        slots[timer.slot].position = (index + 1) as u32;
    }

    /// Take out the entry at `index`. The earliest child of each entry on
    /// the way down moves up into the gap, and the last entry fills the
    /// leaf left over and moves up from there: as deadlines are mostly
    /// armed in their order, the last entry is mostly the latest and stays,
    /// so a task that ends among many costs no comparison against it on
    /// each level.
    fn take(&mut self, index: usize) -> Timer {
        let last = self.heap.pop().expect("an entry to take");
        let heap = &mut self.heap[..];
        let slots = &mut self.slots[..];
        let len = heap.len();
        if index == len {
            slots[last.slot].position = 0;
            return last;
        }
        let taken = heap[index];
        slots[taken.slot].position = 0;
        let mut gap = index;
        loop {
            let first = 2 * gap + 1;
            if first >= len {
                break;
            }
            let second = first + 1;
            let child = if second < len && heap[second].key < heap[first].key {
                second
            } else {
                first
            };
            let moved = heap[child];
            heap[gap] = moved;
            slots[moved.slot].position = (gap + 1) as u32;
            gap = child;
        }
        self.sift_up(gap, last);
        taken
    }

    /// The task in `slot` waits until `at`: its entry, with the next
    /// sequence number, made earlier if it has a later one.
    fn arm(&mut self, slot: usize, at: Instant, waiter: &Waiter) {
        let key = self.key(at);
        match self.slot(slot).position.checked_sub(1) {
            Some(index) => {
                let index = index as usize;
                if key < self.heap[index].key {
                    self.sift_up(index, Timer { key, at, slot });
                }
            }
            None => {
                self.slots[slot].waiter = Some(waiter.clone());
                let timer = Timer { key, at, slot };
                self.heap.push(timer);
                self.sift_up(self.heap.len() - 1, timer);
            }
        }
    }

    /// Take out the entry of `slot`, if it has one.
    fn remove(&mut self, slot: usize) {
        let Some(index) = self
            .slots
            .get(slot)
            .and_then(|timer| timer.position.checked_sub(1))
        else {
            return;
        };
        self.take(index as usize);
        self.slots[slot].waiter = None;
    }

    /// The earliest entry when it is due at `now`, taken out: its slot and
    /// waiter; else the next deadline, if any.
    fn pop_due(&mut self, now: Instant) -> Result<(usize, Waiter), Option<Instant>> {
        let Some(root) = self.heap.first() else {
            return Err(None);
        };
        if root.at > now {
            return Err(Some(root.at));
        }
        let slot = self.take(0).slot;
        let waiter = self.slots[slot].waiter.take().expect("an entry's waiter");
        Ok((slot, waiter))
    }
}

fn with_timer<T>(slot: usize, f: impl FnOnce(&mut SlotTimer) -> T) -> T {
    fresh::with(&TIMERS, |timers| f(timers.borrow_mut().slot(slot)))
}

/// The task in `slot` parks until `deadline` at the latest.
fn arm(slot: usize, deadline: Instant, waiter: &Waiter) {
    fresh::with(&TIMERS, |timers| {
        let mut timers = timers.borrow_mut();
        let timer = timers.slot(slot);
        timer.wanted = Some(deadline);
        let position = timer.position;
        if position == 0 || timers.heap[position as usize - 1].at > deadline {
            timers.arm(slot, deadline, waiter);
        }
    });
}

/// The task in `slot` ended: remove its entry.
fn forget_timer(slot: usize) {
    fresh::with(&TIMERS, |timers| {
        let mut timers = timers.borrow_mut();
        timers.slot(slot).wanted = None;
        timers.remove(slot);
    });
}

/// Switch from the running task to its worker's loop.
fn suspend() {
    let yielder = fresh::with(&YIELDER, Cell::get);
    assert!(!yielder.is_null(), "a park outside a task");
    // SAFETY: `YIELDER` is set by the worker before each resume to the
    // yielder of the task it resumes, which lives as long as that task's
    // coroutine; the call runs inside that coroutine (only a running task
    // parks, `park` asserts its worker), so the yielder is live and ours.
    unsafe { (*yielder).suspend(()) }
}

/// [`crate::wait::wait_descriptor`] on a task: register `descriptor` with
/// the worker's reactor for this wait only and park.
pub(crate) fn wait_descriptor(
    waiter: &Waiter,
    task: &Task,
    descriptor: BorrowedFd<'_>,
    interest: Interest,
    deadline: Option<Instant>,
) -> io::Result<DescriptorWait> {
    let reactor = task.worker().reactor()?;
    let token = reactor.register(descriptor, interest, waiter.clone())?;
    // Relaxed: set and read on this worker only (its dispatch).
    task.ready.store(0, Ordering::Relaxed);
    let woken = park(waiter, task, deadline);
    let _ = reactor.deregister(token, descriptor);
    let ready = Interest::from_bits(task.ready.swap(0, Ordering::SeqCst));
    Ok(if !ready.is_empty() {
        DescriptorWait::Ready(ready)
    } else if woken == Woken::Deadline {
        DescriptorWait::Deadline
    } else {
        DescriptorWait::Woken
    })
}

/// Start `job` as a task on a worker (module docs); an error when no stack
/// could be reserved or no worker thread started. Called with the task
/// already counted live (`crate::task`).
pub(crate) fn spawn(job: Job) -> io::Result<()> {
    crate::signal::install_overflow_handler();
    let here = here();
    // A stack mapped while `WORKERS` was not held, because no free one
    // was found under it; the next look places the task with it.
    let mut spare: Option<DefaultStack> = None;
    'place: loop {
        let workers = lock(&WORKERS);
        let mut passed: Vec<*const Shared> = Vec::new();
        while let Some((shared, last)) = choose(&workers, here, &passed) {
            // SeqCst: this increment, then the `blocked` load, against
            // `block_alone`'s `blocked` store, then its `tasks` load: either
            // this spawn sees the worker blocked and goes elsewhere, or the
            // worker sees a second task and does not block. `last`: every
            // other worker was passed, so the task waits for this one.
            shared.tasks.fetch_add(1, Ordering::SeqCst);
            if !last && shared.blocked.load(Ordering::SeqCst) {
                shared.tasks.fetch_sub(1, Ordering::SeqCst);
                passed.push(Arc::as_ptr(&shared));
                continue;
            }
            let Some((stack, found)) = free_stack(&workers, Some(&shared), &mut spare) else {
                shared.tasks.fetch_sub(1, Ordering::SeqCst);
                drop(workers);
                spare = Some(map_stack()?);
                continue 'place;
            };
            // The task is counted on `shared`, so the worker does not
            // retire ([`Worker::retire`]) while it is handed over without
            // `WORKERS`.
            drop(workers);
            if let Some(unused) = spare.take() {
                give_stack(unused, None);
            }
            // Relaxed: a bound for `block_alone`, not an order with any data.
            escalate(LIVE.fetch_add(1, Ordering::Relaxed) + 1);
            shared.hand_over(|incoming| incoming.new.push((job, stack)));
            count(&COUNTERS.spawns);
            found.count();
            if Arc::as_ptr(&shared) != here {
                count(&COUNTERS.spawns_handed_over);
                shared.notify();
            }
            return Ok(());
        }
        let Some((stack, found)) = free_stack(&workers, None, &mut spare) else {
            drop(workers);
            spare = Some(map_stack()?);
            continue 'place;
        };
        let started = start_worker(workers, job, stack, found);
        if let Some(unused) = spare.take() {
            give_stack(unused, None);
        }
        return started;
    }
}

/// Start a worker with `job` as its first task on `stack` (`found`: where
/// it came from), and add it to `workers`.
fn start_worker(
    mut workers: MutexGuard<'_, Vec<Arc<Shared>>>,
    job: Job,
    stack: DefaultStack,
    found: Found,
) -> io::Result<()> {
    let shared = Arc::new(Shared::new());
    // Relaxed: the worker thread is not started yet; its start orders it.
    shared.tasks.store(1, Ordering::Relaxed);
    shared.hand_over(|incoming| incoming.new.push((job, stack)));
    let started = Arc::clone(&shared);
    // Counted before the worker starts, which may end the task and count
    // it out of `LIVE` before `spawn` returns here. Relaxed: as above.
    let live = LIVE.fetch_add(1, Ordering::Relaxed) + 1;
    // Named, which is a `prctl` per worker started that a thread of
    // v0.15.0 did not make: by the name the workers are told from a
    // program's other threads, in `/proc` and by the tests that watch them.
    match std::thread::Builder::new()
        .name(String::from("worker"))
        .spawn(move || work(started))
    {
        Ok(_) => {
            count(&COUNTERS.spawns);
            found.count();
            count(&COUNTERS.workers_started);
            workers.push(shared);
            escalate(live);
            Ok(())
        }
        Err(error) => {
            LIVE.fetch_sub(1, Ordering::Relaxed);
            let (job, stack) = lock(&shared.incoming)
                .new
                .pop()
                .expect("the job handed to the worker");
            drop(workers);
            give_stack(stack, None);
            drop(job);
            Err(error)
        }
    }
}

/// The worker a spawn goes to; `None`: start a new one. A spawn from a
/// task stays on its worker, with no hand-over, while that worker has at
/// most one task more than the least loaded worker (a worker not yet
/// started counts as one with none): a task and the tasks it spawns to
/// serve the same connection then wake each other without a thread
/// switch, and a task that spawns many (an accept loop on a worker)
/// still spreads them. Otherwise, and from a thread that is no worker,
/// an idle worker, else a new one up to [`worker_limit`], else the least
/// loaded. A worker whose task is in a blocking read ([`block_alone`]),
/// or that the caller `passed` as such, is busy: it is not chosen while
/// another is or a new one may start. With every worker busy and none
/// to start, the least loaded is chosen with `true` (the last resort; at
/// most `worker_limit() - 1` workers block, so it is rare).
fn choose(
    workers: &[Arc<Shared>],
    here: *const Shared,
    passed: &[*const Shared],
) -> Option<(Arc<Shared>, bool)> {
    // Relaxed: placement only weighs the counts; `spawn` orders the
    // increment it makes against `blocked`.
    let load = |shared: &Arc<Shared>| shared.tasks.load(Ordering::Relaxed);
    let elsewhere = |shared: &Arc<Shared>| Arc::as_ptr(shared) != here;
    let free = |shared: &&Arc<Shared>| {
        !passed.contains(&Arc::as_ptr(shared)) && !shared.blocked.load(Ordering::Relaxed)
    };
    if let Some(local) = workers
        .iter()
        .filter(free)
        .find(|shared| !elsewhere(shared))
    {
        let least = if workers.len() < worker_limit() {
            0
        } else {
            workers.iter().map(load).min().unwrap_or(0)
        };
        if load(local) <= least + 1 {
            return Some((Arc::clone(local), false));
        }
    }
    if let Some(idle) = workers
        .iter()
        .filter(free)
        .filter(|shared| load(shared) == 0)
        .min_by_key(|shared| elsewhere(shared))
    {
        return Some((Arc::clone(idle), false));
    }
    if workers.len() < worker_limit() {
        return None;
    }
    if let Some(least) = workers
        .iter()
        .filter(free)
        .min_by_key(|shared| (load(shared), elsewhere(shared)))
    {
        return Some((Arc::clone(least), false));
    }
    workers
        .iter()
        .min_by_key(|shared| load(shared))
        .map(|shared| (Arc::clone(shared), true))
}

/// A free stack for a task placed on `worker`, found under `WORKERS`
/// (`workers`): from that worker's cache; for a worker about to start
/// (`None`), which has none, from the first running worker's cache that
/// holds one; else from the pool, else the `spare` the spawn mapped
/// meanwhile. `None` when there is none, and the spawn maps one without
/// the lock ([`map_stack`]). A running worker's own spawns do not take
/// from another's cache: its tasks fill its own again, and taking from
/// the others' would only move their next spawns to the pool. With the
/// stack, the counter of where it came from, counted once the task is
/// handed over ([`Found::count`]).
fn free_stack(
    workers: &[Arc<Shared>],
    worker: Option<&Shared>,
    spare: &mut Option<DefaultStack>,
) -> Option<(DefaultStack, Found)> {
    let cached = match worker {
        Some(shared) => lock(&shared.stacks).pop(),
        None => workers.iter().find_map(|shared| lock(&shared.stacks).pop()),
    };
    if let Some(stack) = cached {
        return Some((stack, Found(&COUNTERS.stacks_from_cache)));
    }
    if let Some(stack) = lock(&STACKS).pop() {
        return Some((stack, Found(&COUNTERS.stacks_from_pool)));
    }
    spare
        .take()
        .map(|stack| (stack, Found(&COUNTERS.stacks_allocated)))
}

/// Where a task's stack came from: its counter, bumped only when the task
/// was handed over, so that every stack counted belongs to a spawn counted
/// in [`SchedulerCounts::spawns`].
struct Found(&'static AtomicU64);

impl Found {
    fn count(self) {
        count(self.0);
    }
}

/// A fresh stack, mapped while `WORKERS` is not held; counted as
/// allocated when a task takes it ([`free_stack`]).
fn map_stack() -> io::Result<DefaultStack> {
    DefaultStack::new(TASK_STACK)
}

/// Keep a stack that a task left: in the cache of `worker`, the worker it
/// ran on, while it has room ([`WORKER_STACK_CACHE`]), else in the pool,
/// else unmap it.
fn give_stack(stack: DefaultStack, worker: Option<&Shared>) {
    let stack = match worker {
        Some(shared) => {
            let mut stacks = lock(&shared.stacks);
            if stacks.len() < WORKER_STACK_CACHE {
                stacks.push(stack);
                return;
            }
            stack
        }
        None => stack,
    };
    let mut stacks = lock(&STACKS);
    if stacks.len() < STACK_POOL {
        stacks.push(stack);
        count(&COUNTERS.stacks_to_pool);
    }
}

/// A task on this worker.
struct Local {
    coroutine: Option<Coroutine<(), (), (), DefaultStack>>,
    yielder: *const Yielder<(), ()>,
    locals: Option<Box<dyn TaskLocals>>,
    /// The streams the task wrote to and has not flushed, while it does
    /// not run ([`crate::net::swap_pending`]).
    pending: crate::net::Pending,
    /// The `catch`es the task has open, while it does not run
    /// ([`crate::report::swap_catching`]).
    catching: usize,
    guard: (usize, usize),
}

/// A worker's own state, on its thread.
struct Worker {
    shared: Arc<Shared>,
    slots: Vec<Option<Local>>,
    free: Vec<usize>,
    live: usize,
    /// The task whose [`TaskLocals`] the thread-locals hold. A task that
    /// parks leaves them there, so resuming the same task again (one
    /// session on its worker) exchanges nothing; they go back when another
    /// task resumes, a task starts or the task ends.
    in_tls: Option<usize>,
}

impl Worker {
    /// Give the thread-locals back to the worker: exchange the locals of
    /// the task that holds them.
    fn restore_locals(&mut self) {
        if let Some(slot) = self.in_tls.take()
            && let Some(Some(local)) = self.slots.get_mut(slot)
            && let Some(locals) = local.locals.as_mut()
        {
            locals.swap();
        }
    }

    /// Take what other threads handed over.
    fn take_incoming(&mut self) {
        let Some(Incoming { woken, new }) = self.shared.take() else {
            return;
        };
        if !woken.is_empty() {
            RUN.with(|run| run.borrow_mut().extend(woken));
        }
        for (job, stack) in new {
            self.start(job, stack);
        }
    }

    /// A free slot of this worker's table.
    fn free_slot(&mut self) -> usize {
        self.free.pop().unwrap_or_else(|| {
            self.slots.push(None);
            self.slots.len() - 1
        })
    }

    fn start(&mut self, job: Job, stack: DefaultStack) {
        // A new task's locals are made from the worker's thread-locals
        // (the LLVM runtime's spare arena chunks).
        self.restore_locals();
        let slot = self.free_slot();
        let start = stack.limit().get();
        let guard = (start, start + page_size());
        let waiter = Waiter::for_task(Task {
            first: Arc::clone(&self.shared),
            moved: OnceLock::new(),
            state: AtomicU8::new(QUEUED),
            ready: AtomicU8::new(0),
            slot: AtomicUsize::new(slot),
            phase: AtomicU8::new(HOME),
            first_read: AtomicBool::new(true),
            carried: Mutex::new(Carried {
                local: None,
                deadline: None,
            }),
        });
        let coroutine = Coroutine::with_stack(stack, move |yielder: &Yielder<(), ()>, ()| {
            fresh::with(&YIELDER, |cell| cell.set(yielder));
            if let Err(report) = crate::report::catch(job) {
                crate::report::task_failed(&report);
            }
            crate::flush();
        });
        self.slots[slot] = Some(Local {
            coroutine: Some(coroutine),
            yielder: std::ptr::null(),
            locals: LOCALS.get().map(|make| make()),
            pending: crate::net::Pending::default(),
            catching: 0,
            guard,
        });
        self.live += 1;
        RUN.with(|run| run.borrow_mut().push_back(waiter));
    }

    /// Run the task of `waiter` until it parks or ends.
    fn resume(&mut self, waiter: Waiter) {
        let Some(task) = waiter.task() else {
            return;
        };
        if !Arc::ptr_eq(task.worker(), &self.shared) {
            debug_assert!(false, "a task resumed by a worker it is not on");
            return;
        }
        let slot = task.slot();
        if self
            .slots
            .get(slot)
            .is_none_or(|local| local.as_ref().is_none_or(|l| l.coroutine.is_none()))
        {
            return;
        }
        if self.in_tls != Some(slot) {
            self.restore_locals();
            if let Some(Some(local)) = self.slots.get_mut(slot)
                && let Some(locals) = local.locals.as_mut()
            {
                locals.swap();
            }
            self.in_tls = Some(slot);
        }
        let local = self.slots[slot].as_mut().expect("the resumed task's slot");
        let mut coroutine = local
            .coroutine
            .take()
            .expect("the resumed task's coroutine");
        // Relaxed: a waker acts only on PARKED, and this task was QUEUED.
        task.state.store(RUNNING, Ordering::Relaxed);
        YIELDER.with(|cell| cell.set(local.yielder));
        GUARD.with(|cell| cell.set(local.guard));
        crate::net::swap_pending(&mut local.pending);
        crate::report::swap_catching(&mut local.catching);
        RUNNING_TASK.with(|running| *running.borrow_mut() = Some(waiter.clone()));
        let result = coroutine.resume(());
        RUNNING_TASK.with(|running| *running.borrow_mut() = None);
        GUARD.with(|cell| cell.set((0, 0)));
        let local = self.slots[slot].as_mut().expect("the resumed task's slot");
        crate::net::swap_pending(&mut local.pending);
        crate::report::swap_catching(&mut local.catching);
        local.yielder = YIELDER.with(|cell| cell.replace(std::ptr::null()));
        match result {
            CoroutineResult::Yield(()) => {
                // Relaxed: stored by the task on this thread before it
                // yielded (`park_leaving`).
                if task.phase.load(Ordering::Relaxed) == LEAVING {
                    self.give_up(slot, &waiter, task, coroutine);
                    return;
                }
                local.coroutine = Some(coroutine);
                // SeqCst: this store, then the `has_wake` load, against a
                // waker's NOTIFIED set, then its QUEUED exchange: one sees
                // the other, so a wake is not lost.
                task.state.store(PARKED, Ordering::SeqCst);
                if waiter.has_wake() && task.queue() {
                    RUN.with(|run| run.borrow_mut().push_back(waiter.clone()));
                }
            }
            CoroutineResult::Return(()) => {
                // Release: a waker reading DONE does nothing with the task.
                task.state.store(DONE, Ordering::Release);
                self.restore_locals();
                forget_timer(slot);
                give_stack(coroutine.into_stack(), Some(&self.shared));
                let ended = self.slots[slot].take();
                drop(ended);
                self.free.push(slot);
                self.live -= 1;
                self.shared.tasks.fetch_sub(1, Ordering::SeqCst);
                // Relaxed: as in `spawn`.
                LIVE.fetch_sub(1, Ordering::Relaxed);
                crate::task::task_finished();
            }
        }
    }

    /// Give up the task in `slot`, which yielded in [`park_leaving`]: its
    /// locals back out of the thread-locals, its timer forgotten, its slot
    /// freed, its coroutine and locals put into the task. It is adrift from
    /// then on: the worker that makes it runnable adopts it ([`enqueue`]),
    /// this one included. It still counts among this worker's tasks
    /// ([`Shared::tasks`]) until then, so that spawns place tasks as if it
    /// were here: a task adrift belongs to no worker, and a burst of
    /// sessions that wait in their first read would otherwise all be placed
    /// on the one worker that keeps giving them up.
    #[cold]
    fn give_up(
        &mut self,
        slot: usize,
        waiter: &Waiter,
        task: &Task,
        coroutine: Coroutine<(), (), (), DefaultStack>,
    ) {
        if self.in_tls == Some(slot) {
            self.restore_locals();
        }
        forget_timer(slot);
        let mut local = self.slots[slot].take().expect("the given-up task's slot");
        local.coroutine = Some(coroutine);
        self.free.push(slot);
        self.live -= 1;
        let deadline = {
            let mut carried = lock(&task.carried);
            let deadline = carried.deadline;
            carried.local = Some(local);
            deadline
        };
        ADRIFT_TASKS.fetch_add(1, Ordering::SeqCst);
        if let Some(deadline) = deadline {
            drift_until(deadline, waiter);
        }
        count(&COUNTERS.tasks_given_up);
        // SeqCst (both): a waker that queues the task after the PARKED
        // store sees ADRIFT and the task carried ([`enqueue`]).
        task.phase.store(ADRIFT, Ordering::SeqCst);
        task.state.store(PARKED, Ordering::SeqCst);
        // A wake, or a deadline that passed, before the PARKED store found
        // the task running: the wake left its mark, and a worker that found
        // the deadline passed took it out of `DRIFTING` without queueing
        // the task ([`fire_drift`]). Either is seen here.
        let due = deadline.is_some_and(|deadline| read_clock() >= deadline);
        if (waiter.has_wake() || due) && task.queue() {
            enqueue_adrift(waiter);
        }
    }

    /// Adopt the tasks adrift made runnable on this thread ([`ADOPT`]).
    fn adopt_waiting(&mut self) {
        let adopted = ADOPT.with(|adopt| std::mem::take(&mut *adopt.borrow_mut()));
        for waiter in adopted {
            self.adopt(waiter);
        }
    }

    /// Adopt the task of `waiter`, adrift and queued by this thread: a slot
    /// here, this worker as its own, its counts here; it runs next.
    fn adopt(&mut self, waiter: Waiter) {
        let Some(task) = waiter.task() else {
            return;
        };
        let (local, deadline) = {
            let mut carried = lock(&task.carried);
            (carried.local.take(), carried.deadline.take())
        };
        let Some(local) = local else {
            debug_assert!(false, "an adopted task carries its coroutine");
            return;
        };
        if deadline.is_some() {
            drift_ended(&waiter);
        }
        let slot = self.free_slot();
        self.slots[slot] = Some(local);
        // Relaxed: read by this worker, and by wakers only after the SeqCst
        // PARKED store this worker makes once the task parked again.
        task.slot.store(slot, Ordering::Relaxed);
        let _ = task.moved.set(Arc::clone(&self.shared));
        task.phase.store(MOVED, Ordering::SeqCst);
        self.live += 1;
        // SeqCst (both), as every change of `tasks`: counted here before
        // it leaves the worker that gave it up ([`Worker::give_up`]).
        self.shared.tasks.fetch_add(1, Ordering::SeqCst);
        task.first.tasks.fetch_sub(1, Ordering::SeqCst);
        ADRIFT_TASKS.fetch_sub(1, Ordering::SeqCst);
        count(&COUNTERS.tasks_adopted);
        if !Arc::ptr_eq(&task.first, &self.shared) {
            count(&COUNTERS.tasks_moved);
        }
        RUN.with(|run| run.borrow_mut().push_back(waiter));
    }

    /// Queue the tasks whose deadline passed, arm again those that want a
    /// later one by now ([`SlotTimer`]); the next deadline. Reads the clock
    /// only when there is a timer. Tasks adrift whose deadline passed are
    /// queued for this worker to adopt ([`DRIFTING`]).
    fn fire_timers(&self) -> Option<Instant> {
        let drifting = fire_drift();
        if TIMERS.with(|timers| timers.borrow().is_empty()) {
            return drifting;
        }
        let now = read_clock();
        loop {
            let due = TIMERS.with(|timers| {
                let mut timers = timers.borrow_mut();
                let (slot, waiter) = timers.pop_due(now)?;
                let timer = timers.slot(slot);
                match timer.wanted {
                    Some(later) if later > now => {
                        timers.arm(slot, later, &waiter);
                        Ok(None)
                    }
                    wanted => Ok(wanted.map(|_| waiter)),
                }
            });
            match due {
                Ok(Some(waiter)) => {
                    if let Some(task) = waiter.task()
                        && task.queue()
                    {
                        RUN.with(|run| run.borrow_mut().push_back(waiter));
                    }
                }
                Ok(None) => {}
                Err(next) => return earlier(next, drifting),
            }
        }
    }

    /// End the worker if no task was placed on it meanwhile and none is
    /// adrift (a worker with a reactor watches the shared one for it);
    /// whether it ended.
    fn retire(&self) -> bool {
        let mut workers = lock(&WORKERS);
        // Under `WORKERS`, which every spawn holds while it counts a task:
        // a spawn that chose this worker counted its task before it lets
        // go of the lock and hands the task over, so the count is not 0.
        if self.shared.tasks.load(Ordering::Relaxed) != 0
            || !lock(&self.shared.incoming).is_empty()
            || adrift()
        {
            return false;
        }
        workers.retain(|shared| !Arc::ptr_eq(shared, &self.shared));
        // No spawn chooses this worker any more: its free stacks are
        // unmapped, as a thread's stack is when it ends.
        let freed = std::mem::take(&mut *lock(&self.shared.stacks));
        drop(workers);
        drop(freed);
        true
    }
}

/// Whether this worker has a task to run or to adopt. Called by tasks too
/// ([`wait_alone`], [`block_alone`]).
fn runnable() -> bool {
    fresh::with(&RUN, |run| !run.borrow().is_empty())
        || fresh::with(&ADOPT, |adopt| !adopt.borrow().is_empty())
}

fn next_runnable() -> Option<Waiter> {
    RUN.with(|run| run.borrow_mut().pop_front())
}

/// Wake the waiters of `events`, which `shared`'s reactor returned; its
/// registration of the shared reactor means: look at that one.
fn dispatch(events: &mut Vec<Event>, shared: &Shared) {
    // Relaxed: stored by this worker ([`Shared::reactor`]).
    let watch = shared.watch.load(Ordering::Relaxed);
    let mut look = false;
    for event in events.drain(..) {
        if watch != 0 && event.token.0 == watch {
            look = true;
            continue;
        }
        wake_ready(&event.waiter, event.ready);
    }
    if look {
        look_at_shared(events);
    }
}

/// The shared reactor (module docs): one epoll instance and its eventfd,
/// made with the first worker reactor while the move is on; `None` when
/// it could not be made, and no task moves then.
static SHARED: OnceLock<Option<EpollReactor>> = OnceLock::new();

/// [`SHARED`], made on first use.
fn shared() -> Option<&'static EpollReactor> {
    SHARED.get_or_init(|| EpollReactor::new().ok()).as_ref()
}

/// Tasks given up and not adopted yet. While there are any, no worker
/// retires, and no task waits alone on its worker's thread or blocks it,
/// so the worker that gave a task up sleeps in its reactor, which watches
/// the shared one, whenever its other tasks wait.
static ADRIFT_TASKS: AtomicUsize = AtomicUsize::new(0);

fn adrift() -> bool {
    ADRIFT_TASKS.load(Ordering::SeqCst) != 0
}

/// The deadlines of the tasks adrift, from the parks they were given up
/// in: every worker sleeps until the earliest at the latest, and the first
/// that finds it passed adopts the task ([`fire_drift`]).
static DRIFTING: Mutex<Vec<(Instant, Waiter)>> = Mutex::new(Vec::new());

/// Tasks adrift made runnable off a worker ([`enqueue_adrift`]), for the
/// next worker that looks at the shared reactor.
static STRAYS: Mutex<Vec<Waiter>> = Mutex::new(Vec::new());

/// The task of `waiter`, given up in a park until `deadline`, is adrift.
fn drift_until(deadline: Instant, waiter: &Waiter) {
    lock(&DRIFTING).push((deadline, waiter.clone()));
}

/// The task of `waiter` was adopted before its deadline.
fn drift_ended(waiter: &Waiter) {
    lock(&DRIFTING).retain(|(_, drifting)| !drifting.same(waiter));
}

/// Queue the tasks adrift whose deadline passed for this worker to adopt;
/// the earliest deadline left. Takes no lock while no task is adrift.
fn fire_drift() -> Option<Instant> {
    if !adrift() {
        return None;
    }
    let mut drifting = lock(&DRIFTING);
    if drifting.is_empty() {
        return None;
    }
    let now = read_clock();
    let mut next: Option<Instant> = None;
    drifting.retain(|(at, waiter)| {
        if *at > now {
            next = Some(next.map_or(*at, |next| next.min(*at)));
            return true;
        }
        if let Some(task) = waiter.task()
            && task.queue()
        {
            ADOPT.with(|adopt| adopt.borrow_mut().push(waiter.clone()));
        }
        // Taken out either way: a task a waker queued first is adopted
        // by that waker's worker, which finds no entry ([`drift_ended`]);
        // a task not parked yet is queued by the worker that gives it up,
        // which reads the clock after it parked it ([`Worker::give_up`]).
        false
    });
    next
}

/// How a worker waits on the shared reactor, the open design point of the
/// move (`story:readiness-task-stealing`), in this one function: called
/// when a worker makes its own reactor, it returns the token under which
/// that reactor reports the shared one, if it does.
///
/// Chosen: the shared epoll instance is registered in every worker's
/// epoll instance, edge-triggered, when the worker makes it: one
/// `epoll_ctl` per worker and none per session, and a worker that sleeps
/// in its own `epoll_wait` wakes for an event of the shared one, which it
/// takes with one `epoll_wait` of the shared instance that does not wait
/// ([`look_at_shared`]). A session alone on its worker never waits on the
/// shared instance and makes no call more. The cost: the kernel wakes
/// every worker that sleeps for each event of the shared instance
/// (`EPOLLEXCLUSIVE` is refused for an epoll descriptor), and all but one
/// of them find nothing ([`SchedulerCounts::shared_polls_empty`]): with 4
/// workers up to 3 wakes and 6 calls per moved session more.
///
/// The alternative, for a second host run: one watcher. Only one sleeping
/// worker has the shared instance registered; a worker that wakes for its
/// own work while it is the watcher takes the registration out
/// (`EPOLL_CTL_DEL`) and hands the watch to another sleeping worker, which
/// registers it (`EPOLL_CTL_ADD`) before it sleeps. One wake per event, two
/// calls and a hand-over per change of watcher, and a worker that turns
/// busy while it watches delays the events until the hand-over.
fn watch_shared(reactor: &EpollReactor) -> Option<Token> {
    if !steal() {
        return None;
    }
    let shared = shared()?;
    reactor
        .register(
            shared.as_fd(),
            Interest::READ,
            Waiter::for_descriptor(Descriptor::new()),
        )
        .ok()
}

/// Take the events of the shared reactor without waiting and wake their
/// waiters: a task adrift that one makes runnable is adopted by this
/// worker ([`enqueue_adrift`]); so are those woken off a worker
/// ([`STRAYS`]), whose wake ended the wait. `events` is empty and left so.
#[cold]
fn look_at_shared(events: &mut Vec<Event>) {
    let Some(reactor) = shared() else {
        return;
    };
    let mut found = false;
    while let Ok(taken) = reactor.poll_now(events) {
        for event in events.drain(..) {
            found = true;
            wake_ready(&event.waiter, event.ready);
        }
        if taken < crate::reactor::BATCH {
            break;
        }
    }
    let strays = std::mem::take(&mut *lock(&STRAYS));
    for waiter in strays {
        found = true;
        ADOPT.with(|adopt| adopt.borrow_mut().push(waiter));
    }
    if !found {
        count(&COUNTERS.shared_polls_empty);
    }
}

/// The earlier of two deadlines, `None` being none.
fn earlier(one: Option<Instant>, other: Option<Instant>) -> Option<Instant> {
    match (one, other) {
        (Some(one), Some(other)) => Some(one.min(other)),
        (one, other) => one.or(other),
    }
}

/// The loop of a worker thread (module docs).
fn work(shared: Arc<Shared>) {
    crate::task::mark_task_thread();
    let _worker = crate::census::Counted::new(crate::census::Kind::Workers);
    crate::signal::worker_started();
    HERE.with(|here| here.set(Arc::as_ptr(&shared)));
    let _ = shared.thread.set(std::thread::current());
    let mut worker = Worker {
        shared,
        slots: Vec::new(),
        free: Vec::new(),
        live: 0,
        in_tls: None,
    };
    let mut events: Vec<Event> = Vec::new();
    let mut idle_since: Option<Instant> = None;
    loop {
        worker.take_incoming();
        worker.adopt_waiting();
        let mut ran = 0;
        while ran < BUDGET {
            let Some(waiter) = next_runnable() else {
                break;
            };
            worker.resume(waiter);
            ran += 1;
        }
        if ran > 0 {
            // A task ran, so the worker was not without one since: a
            // session that waits alone runs from its start to its end in
            // one turn, and the worker lingers from its end on.
            idle_since = None;
        }
        let next_timer = worker.fire_timers();
        if runnable() {
            if ran >= BUDGET
                && let Some(reactor) = worker.shared.reactor.get()
            {
                let _ = reactor.wait(Some(read_clock()), &mut events);
                dispatch(&mut events, &worker.shared);
            }
            continue;
        }
        // No linger while a task is adrift: no worker retires then
        // ([`Worker::retire`]), and a worker whose tasks all moved away or
        // wait adrift sleeps until a timer, an event or a wake.
        let deadline = if worker.live == 0 && !adrift() {
            let now = read_clock();
            let since = *idle_since.get_or_insert(now);
            let end = since + crate::LINGER;
            if now >= end {
                if worker.retire() {
                    crate::task::trim_after_linger();
                    return;
                }
                idle_since = None;
                continue;
            }
            earlier(next_timer, Some(end))
        } else {
            idle_since = None;
            next_timer
        };
        // SeqCst: the store-load pair with a hand-over (`Shared::hand_over`).
        worker.shared.sleeping.store(true, Ordering::SeqCst);
        if worker.shared.has_pending() {
            // Relaxed: only this thread sets it; a waker that swapped it
            // already sends a wake that the next sleep consumes.
            worker.shared.sleeping.store(false, Ordering::Relaxed);
            continue;
        }
        match worker.shared.reactor.get() {
            Some(reactor) => {
                let _ = reactor.wait(deadline, &mut events);
            }
            None => match deadline {
                Some(deadline) => {
                    std::thread::park_timeout(deadline.saturating_duration_since(read_clock()));
                }
                None => std::thread::park(),
            },
        }
        // Relaxed: as above; a late waker's wake only ends a later sleep.
        worker.shared.sleeping.store(false, Ordering::Relaxed);
        dispatch(&mut events, &worker.shared);
    }
}

/// The page size: 4096 on x86_64, whose base page has no other size, so
/// that `nix` builds without its `feature` feature there (row 6.2 of
/// `docs/quality.md`, story `build-time-back`); `sysconf` elsewhere.
#[cfg(target_arch = "x86_64")]
fn page_size() -> usize {
    4096
}

#[cfg(not(target_arch = "x86_64"))]
fn page_size() -> usize {
    static PAGE: OnceLock<usize> = OnceLock::new();
    *PAGE.get_or_init(|| {
        nix::unistd::sysconf(nix::unistd::SysconfVar::PAGE_SIZE)
            .ok()
            .flatten()
            .and_then(|size| usize::try_from(size).ok())
            .unwrap_or(4096)
    })
}

/// The guard page of the running task's stack, `start..end`, `(0, 0)` off
/// a task: what the stack overflow handler (`crate::signal`) compares a
/// fault's address with. Reads a `const` thread-local cell only, so it is
/// async-signal-safe.
pub(crate) fn guard_page() -> (usize, usize) {
    GUARD.try_with(Cell::get).unwrap_or((0, 0))
}

/// The readiness of one descriptor for tasks on workers (module docs): a
/// socket's or a listener's. Off a worker [`Readiness::wait`] is
/// [`crate::wait::wait_descriptor`]. Forgotten by every worker when
/// dropped, as its descriptor closes ([`Readiness::deregister`] takes it
/// out of their epoll instances while it stays open).
pub(crate) struct Readiness {
    waiter: Waiter,
}

/// The [`Waiter`] a descriptor is registered with (`wait::Kind`).
pub(crate) struct Descriptor {
    /// Readiness events for reading (input, urgent data, hang-up, error).
    read_events: AtomicU64,
    /// Readiness events for writing (room, hang-up, error).
    write_events: AtomicU64,
    /// Hang-up, error and urgent data, as any event reported them; never
    /// cleared ([`Readiness::short_read_may_leave_input`]).
    lasting: AtomicU8,
    waiting: Mutex<Waiting>,
    /// The workers the descriptor is registered with, and its token there.
    registered: Mutex<Vec<(Arc<Shared>, Token)>>,
    /// The registrations report room to write ([`sides`]): set, and every
    /// registration changed, by the first wait for room that registers.
    /// Until then a registration leaves room out, so registering a
    /// socket, which has room, does not end its worker's next wait for
    /// nothing. Changed only under `registered`.
    write_armed: AtomicBool,
    /// The worker it was registered with first, compared only, never
    /// dereferenced: a wait there skips `registered`. The `Arc` in
    /// `registered` keeps that worker's address from being reused until
    /// the descriptor is deregistered, which clears this first.
    home: AtomicPtr<Shared>,
    /// The token of its registration with the shared reactor, 0 for none:
    /// made by the first read of a task that may move, for one event
    /// ([`Readiness::wait_to_read`]). Changed only under `registered`.
    shared_token: AtomicU64,
}

#[derive(Default)]
struct Waiting {
    readers: WaitList,
    writers: WaitList,
}

impl Waiting {
    fn of(&mut self, interest: Interest) -> &mut WaitList {
        if interest.intersects(READ_SIDE) {
            &mut self.readers
        } else {
            &mut self.writers
        }
    }
}

const READ_SIDE: Interest = Interest::READ;

/// What a descriptor is registered for: input, urgent data and hang-up,
/// and room to write once a task waited for it ([`Descriptor::write_armed`]).
fn sides(write: bool) -> Interest {
    let read = Interest::READ | Interest::URGENT | Interest::HANGUP;
    if write { read | Interest::WRITE } else { read }
}

fn reads(ready: Interest) -> bool {
    ready.intersects(Interest::READ | Interest::URGENT | Interest::HANGUP | Interest::ERROR)
}

fn writes(ready: Interest) -> bool {
    ready.intersects(Interest::WRITE | Interest::HANGUP | Interest::ERROR)
}

impl Descriptor {
    pub(crate) fn new() -> Descriptor {
        Descriptor {
            read_events: AtomicU64::new(0),
            write_events: AtomicU64::new(0),
            lasting: AtomicU8::new(0),
            waiting: Mutex::new(Waiting::default()),
            registered: Mutex::new(Vec::new()),
            write_armed: AtomicBool::new(false),
            home: AtomicPtr::new(std::ptr::null_mut()),
            shared_token: AtomicU64::new(0),
        }
    }

    /// A readiness event (`wait::wake_ready`): count it and wake the
    /// tasks waiting for its direction.
    pub(crate) fn ready(&self, ready: Interest) {
        let (read, write) = (reads(ready), writes(ready));
        // SeqCst, before the count: a reader whose mark already counts
        // this event sees these bits after its read.
        let lasting = ready.bits() & (Interest::HANGUP | Interest::ERROR | Interest::URGENT).bits();
        if lasting != 0 {
            self.lasting.fetch_or(lasting, Ordering::SeqCst);
        }
        if read {
            self.read_events.fetch_add(1, Ordering::SeqCst);
        }
        if write {
            self.write_events.fetch_add(1, Ordering::SeqCst);
        }
        let mut waiting = lock(&self.waiting);
        if read {
            waiting.readers.wake_all();
        }
        if write {
            waiting.writers.wake_all();
        }
    }

    fn events(&self, interest: Interest) -> &AtomicU64 {
        if interest.intersects(READ_SIDE) {
            &self.read_events
        } else {
            &self.write_events
        }
    }
}

impl Readiness {
    pub(crate) fn new() -> Readiness {
        Readiness {
            waiter: Waiter::for_descriptor(Descriptor::new()),
        }
    }

    fn descriptor(&self) -> &Descriptor {
        self.waiter.descriptor().expect("a descriptor waiter")
    }

    /// The mark of `interest` (`Interest::READ` or `Interest::WRITE`) to
    /// take before an attempt that may find the descriptor not ready, and
    /// to pass to [`Readiness::wait`] after it.
    pub(crate) fn mark(&self, interest: Interest) -> u64 {
        self.descriptor().events(interest).load(Ordering::SeqCst)
    }

    /// Whether a read that returned fewer bytes than it asked for may have
    /// left something to read that no later readiness event announces:
    /// an event reported the peer's end of stream, an error, or urgent
    /// data (a read stops before the urgent byte). Such an event raises
    /// one edge with the bytes before it, which the mark of that read
    /// already counts. Once true, it stays true. Read after the read whose
    /// mark was taken before it.
    pub(crate) fn short_read_may_leave_input(&self) -> bool {
        self.descriptor().lasting.load(Ordering::SeqCst) != 0
    }

    /// Wait until `descriptor` may be ready for `interest`, after an
    /// attempt begun at `mark` found it not ready, or until `deadline`.
    /// `Woken` and `Ready` both mean: attempt again. The only task of a
    /// worker waits on the worker's thread ([`wait_alone`]), registered or
    /// not, so a session that stays alone on its worker registers nothing.
    /// Any other wait registers the descriptor with the worker first, if it
    /// is not, and parks: the registration reports what the descriptor is
    /// ready for already as its first event. A wait for room to write
    /// makes every registration report room from then on
    /// ([`Descriptor::write_armed`]). A worker that cannot create its
    /// reactor (no descriptor free for its epoll and eventfd) looks at the
    /// descriptor without waiting and parks the task for [`RETRY`], until
    /// it can: a session keeps running while the process is out of
    /// descriptors. `limit` is the caller's across the waits of one
    /// operation ([`Limit`]): a wait on the thread itself, off a worker or
    /// alone on one, gives a duration to its `poll` as the timeout and
    /// reads no clock; a wait that parks reads it once, for the worker's
    /// timers.
    pub(crate) fn wait(
        &self,
        descriptor: BorrowedFd<'_>,
        interest: Interest,
        mark: u64,
        limit: &mut Limit,
    ) -> io::Result<DescriptorWait> {
        self.wait_in(descriptor, interest, mark, limit, false)
    }

    /// [`Readiness::wait`] of a stream's read for input, which may move
    /// the task (module docs): the first such wait of a task, if it parks
    /// on a worker that runs another task and the process may start more
    /// than one worker, parks through the shared reactor and gives the task
    /// up ([`Readiness::wait_adrift`]).
    pub(crate) fn wait_to_read(
        &self,
        descriptor: BorrowedFd<'_>,
        mark: u64,
        limit: &mut Limit,
    ) -> io::Result<DescriptorWait> {
        self.wait_in(descriptor, Interest::READ, mark, limit, true)
    }

    fn wait_in(
        &self,
        descriptor: BorrowedFd<'_>,
        interest: Interest,
        mark: u64,
        limit: &mut Limit,
        may_move: bool,
    ) -> io::Result<DescriptorWait> {
        let me = Waiter::current();
        let Some(task) = me.task() else {
            return crate::wait::wait_within(descriptor, interest, limit);
        };
        // Relaxed: only the task itself reads and clears it.
        let first = may_move && task.first_read.load(Ordering::Relaxed);
        if first {
            task.first_read.store(false, Ordering::Relaxed);
        }
        let state = self.descriptor();
        if let Some(waited) = wait_alone(task, state, descriptor, interest, mark, limit)? {
            return Ok(waited);
        }
        // Relaxed (both): a hint; the move is right as long as the task
        // ends up adopted, by whichever worker.
        if first
            && steal()
            && worker_limit() > 1
            && task.phase.load(Ordering::Relaxed) == HOME
            && task.worker().tasks.load(Ordering::Relaxed) > 1
            && let Some(waited) = self.wait_adrift(&me, task, descriptor, mark, limit)
        {
            return waited;
        }
        let here = Arc::as_ptr(task.worker()).cast_mut();
        // Relaxed: a first look; it is read again under `registered`.
        let arm =
            interest.intersects(Interest::WRITE) && !state.write_armed.load(Ordering::Relaxed);
        if arm || state.home.load(Ordering::Acquire) != here {
            let mut registered = lock(&state.registered);
            // Relaxed (here and below): stored only under `registered`.
            if arm && !state.write_armed.load(Ordering::Relaxed) {
                for (shared, token) in registered.iter() {
                    if let Some(reactor) = shared.reactor.get() {
                        reactor.rearm(*token, descriptor, sides(true))?;
                    }
                }
                state.write_armed.store(true, Ordering::Relaxed);
            }
            if !registered
                .iter()
                .any(|(shared, _)| Arc::ptr_eq(shared, task.worker()))
            {
                let Ok(reactor) = task.worker().reactor() else {
                    drop(registered);
                    let (deadline, _) = limit.deadline();
                    return without_reactor(&me, descriptor, interest, deadline);
                };
                let sides = sides(state.write_armed.load(Ordering::Relaxed));
                let token = reactor.register(descriptor, sides, self.waiter.clone())?;
                registered.push((Arc::clone(task.worker()), token));
                if registered.len() == 1 {
                    state.home.store(here, Ordering::Release);
                }
            }
        }
        {
            let mut waiting = lock(&state.waiting);
            if state.events(interest).load(Ordering::SeqCst) != mark {
                return Ok(DescriptorWait::Woken);
            }
            waiting.of(interest).push(me.clone());
        }
        let (deadline, now) = limit.deadline();
        let woken = park_from(&me, task, deadline, now);
        let still_queued = lock(&state.waiting).of(interest).remove(&me);
        Ok(if still_queued && woken == Woken::Deadline {
            DescriptorWait::Deadline
        } else {
            DescriptorWait::Ready(interest)
        })
    }

    /// The park of [`Readiness::wait_to_read`] that moves the task: the
    /// descriptor is registered with the shared reactor for one event, the
    /// task waits for input as any reader does, and its worker gives it up
    /// ([`park_leaving`]); the worker that makes it runnable adopts it: the
    /// one whose look at the shared reactor finds the event, or this one if
    /// a registration of its own or a wake comes first. `None` when it
    /// cannot: no shared reactor, or the worker does not watch it.
    #[cold]
    #[inline(never)]
    fn wait_adrift(
        &self,
        me: &Waiter,
        task: &Task,
        descriptor: BorrowedFd<'_>,
        mark: u64,
        limit: &mut Limit,
    ) -> Option<io::Result<DescriptorWait>> {
        let shared = shared()?;
        let worker = task.worker();
        // Its reactor is made first, which registers the shared one in it,
        // so this worker wakes for the event too ([`watch_shared`]).
        // Relaxed: stored by this thread when it made the reactor.
        if worker.reactor().is_err() || worker.watch.load(Ordering::Relaxed) == 0 {
            return None;
        }
        let state = self.descriptor();
        {
            let _registered = lock(&state.registered);
            // Relaxed (both): changed only under `registered`.
            let token = state.shared_token.load(Ordering::Relaxed);
            let armed = if token == 0 {
                shared
                    .register_once(descriptor, sides(false), self.waiter.clone())
                    .map(|token| state.shared_token.store(token.0, Ordering::Relaxed))
            } else {
                shared.rearm_once(Token(token), descriptor, sides(false))
            };
            if armed.is_err() {
                return None;
            }
        }
        {
            let mut waiting = lock(&state.waiting);
            if state.events(Interest::READ).load(Ordering::SeqCst) != mark {
                return Some(Ok(DescriptorWait::Woken));
            }
            waiting.readers.push(me.clone());
        }
        let (deadline, now) = limit.deadline();
        let woken = park_leaving(me, task, deadline, now);
        let still_queued = lock(&state.waiting).readers.remove(me);
        Some(Ok(if still_queued && woken == Woken::Deadline {
            DescriptorWait::Deadline
        } else {
            DescriptorWait::Ready(Interest::READ)
        }))
    }
}

/// [`Readiness::wait`] of the only task of its worker, with nothing else
/// runnable or handed over: the task waits for the descriptor on the
/// worker's thread with [`EpollReactor::poll_one`], as a blocked thread
/// would, instead of switching to the worker's loop. One `poll` of the
/// descriptor and the worker's eventfd measured cheaper per reply than the
/// worker's `epoll_wait` called from the task, and it needs no
/// registration. The worker counts as sleeping, not busy, meanwhile, so a
/// spawn may place a task here, and its hand-over ([`Shared::notify`])
/// ends the wait. `None` when the worker has other work, or the
/// hand-over ended the wait before the descriptor was ready: the task
/// then parks as usual, so the worker runs what came instead of an
/// attempt that finds nothing. A worker without a reactor waits
/// ([`wait_blocking`]) without making one while it may.
fn wait_alone(
    task: &Task,
    state: &Descriptor,
    descriptor: BorrowedFd<'_>,
    interest: Interest,
    mark: u64,
    limit: &mut Limit,
) -> io::Result<Option<DescriptorWait>> {
    let shared = task.worker();
    // Relaxed: a hint; a spawn that raises it after this also hands over,
    // which the `pending` check below sees. While a task is adrift, a task
    // parks instead, so that its worker watches the shared reactor
    // ([`ADRIFT_TASKS`]).
    if shared.tasks.load(Ordering::Relaxed) != 1 || runnable() || adrift() {
        return Ok(None);
    }
    if shared.reactor.get().is_none()
        && let Some(waited) = wait_blocking(state, descriptor, interest, mark, limit)
    {
        return waited.map(Some);
    }
    // Created here by the worker's first wait that may not block it: a
    // task runs, and `sleeping` is not set yet ([`Shared::notify`]).
    let Ok(reactor) = shared.reactor() else {
        return Ok(None);
    };
    // Before `sleeping`: every wake written until then is one the worker
    // took (it was awake since), and the hand-over check below sees what
    // came with it.
    reactor.forget_wakes();
    // SeqCst: the store-load pair with a hand-over (`Shared::hand_over`).
    shared.sleeping.store(true, Ordering::SeqCst);
    if shared.has_pending() {
        // Relaxed: only this thread sets it (see the worker loop).
        shared.sleeping.store(false, Ordering::Relaxed);
        return Ok(None);
    }
    // Acquire: pairs with the event count's increment (`Descriptor::ready`).
    if state.events(interest).load(Ordering::Acquire) != mark {
        shared.sleeping.store(false, Ordering::Relaxed);
        return Ok(Some(DescriptorWait::Woken));
    }
    // Taken right before the `poll` that uses it: a duration is spent on
    // the one `poll` it is given to (`Limit::poll_timeout`).
    let Some(timeout) = limit.poll_timeout() else {
        shared.sleeping.store(false, Ordering::Relaxed);
        return Ok(Some(DescriptorWait::Deadline));
    };
    let polled = reactor.poll_one(descriptor, interest, timeout);
    shared.sleeping.store(false, Ordering::Relaxed);
    let polled = polled?;
    Ok(if polled.ready {
        Some(DescriptorWait::Ready(interest))
    } else if polled.timed_out && limit.passed() {
        Some(DescriptorWait::Deadline)
    } else {
        None
    })
}

/// [`wait_alone`] on a worker that has no reactor: the task blocks the
/// worker's thread in one `poll` of the descriptor, under the bounds of
/// [`block_alone`], so a worker whose tasks only wait alone makes no epoll
/// instance and no eventfd (`story:readiness-syscalls-per-connection`:
/// six calls per worker started, which neither v0.15.0's threads nor a
/// `recv` that blocks make). The worker counts as busy meanwhile: a spawn
/// finds an idle worker or starts one, as for a blocking `recv`. The
/// `poll` also watches [`ESCALATION`], so that once as many tasks live as
/// workers may start ([`escalate`]), the wait ends and waits through the
/// worker's reactor instead, where a hand-over ends it: a burst of
/// sessions that wait before their first request does not leave all but
/// one worker blocked while the last takes the rest. `None` when the task
/// may not block its worker, or the escalation ended the wait before the
/// descriptor was ready.
fn wait_blocking(
    state: &Descriptor,
    descriptor: BorrowedFd<'_>,
    interest: Interest,
    mark: u64,
    limit: &mut Limit,
) -> Option<io::Result<DescriptorWait>> {
    // Relaxed: a hint that saves a `poll` that would end at once.
    if ESCALATED.load(Ordering::Relaxed) {
        return None;
    }
    let escalation = escalation()?;
    let blocking = block_alone()?;
    // Acquire: pairs with the event count's increment (`Descriptor::ready`).
    if state.events(interest).load(Ordering::Acquire) != mark {
        return Some(Ok(DescriptorWait::Woken));
    }
    // Taken right before the `poll` that uses it, as in [`wait_alone`].
    let Some(timeout) = limit.poll_timeout() else {
        return Some(Ok(DescriptorWait::Deadline));
    };
    let polled = crate::reactor::poll_beside(descriptor, interest, escalation.as_fd(), timeout);
    drop(blocking);
    Some(match polled {
        Ok(polled) if polled.ready => Ok(DescriptorWait::Ready(interest)),
        Ok(polled) if polled.woken => return None,
        Ok(polled) if polled.timed_out && limit.passed() => Ok(DescriptorWait::Deadline),
        Ok(_) => Ok(DescriptorWait::Woken),
        Err(error) => Err(error),
    })
}

/// Written once and never read ([`escalate`]): readable from then on, it
/// ends every `poll` of [`wait_blocking`]. `None` when no eventfd could be
/// made; no task then blocks its worker in a `poll`.
static ESCALATION: OnceLock<Option<EventFd>> = OnceLock::new();

/// [`ESCALATION`] was written.
static ESCALATED: AtomicBool = AtomicBool::new(false);

/// [`ESCALATION`], made by the first wait that blocks its worker in a
/// `poll`, or by [`escalate`] before it writes it.
fn escalation() -> Option<&'static EventFd> {
    ESCALATION
        .get_or_init(|| crate::reactor::eventfd().ok())
        .as_ref()
}

/// A spawn made `live` tasks live: once they are as many as workers may
/// start, end every `poll` of [`wait_blocking`], now and later. A worker
/// that blocks in it takes no task, and tasks do not move; without free
/// workers to start, a spawn that passes it would place its task on a
/// busier worker. Once per process: [`ESCALATION`] stays readable.
fn escalate(live: usize) {
    if live >= worker_limit()
        && !ESCALATED.swap(true, Ordering::SeqCst)
        && let Some(escalation) = escalation()
    {
        let _ = escalation.write(1);
    }
}

/// Workers whose only task is in a blocking read ([`block_alone`]).
static BLOCKED: AtomicUsize = AtomicUsize::new(0);

/// Tasks placed on workers and not yet ended. While fewer than
/// [`worker_limit`] live, a spawn finds an idle worker or starts one and
/// never passes a blocked worker; with more, a worker blocked in a read
/// would keep its one task while others take the rest of a burst of
/// connections, and tasks do not migrate (`get-64`: a quarter of the
/// replies per second lost).
static LIVE: AtomicUsize = AtomicUsize::new(0);

/// The permission of the running task to make one blocking system call
/// on its worker's thread, as a task of the mode `threads` would; dropped
/// when the call returned.
pub(crate) struct Blocking {
    shared: Arc<Shared>,
}

impl Drop for Blocking {
    fn drop(&mut self) {
        // Release: a spawn that sees it cleared may place a task here,
        // after everything the blocked call did.
        self.shared.blocked.store(false, Ordering::Release);
        BLOCKED.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Let the running task block its worker's thread in one system call
/// (a read with nothing to read yet), instead of waiting through the
/// reactor: one call instead of `poll` and the read. Only for the only
/// task of its worker, with nothing runnable or handed over, and while
/// fewer tasks live than [`worker_limit`] ([`LIVE`]): the worker
/// counts as busy meanwhile, so `spawn` places tasks on another worker
/// (or starts one), and nothing that comes for this worker waits on the
/// call. `None` off a worker, or when any of that does not hold.
pub(crate) fn block_alone() -> Option<Blocking> {
    let me = Waiter::current();
    let task = me.task()?;
    let shared = task.worker();
    // Relaxed: a first look; the SeqCst pair below decides.
    if shared.tasks.load(Ordering::Relaxed) != 1
        || runnable()
        || shared.has_pending()
        || LIVE.load(Ordering::Relaxed) >= worker_limit()
        || adrift()
    {
        return None;
    }
    // Relaxed: a bound on blocked workers, not an order with any data.
    if BLOCKED.fetch_add(1, Ordering::Relaxed) + 1 >= worker_limit() {
        BLOCKED.fetch_sub(1, Ordering::Relaxed);
        return None;
    }
    // SeqCst: this store, then the `tasks` load, against `spawn`'s
    // increment, then its `blocked` load (see there).
    shared.blocked.store(true, Ordering::SeqCst);
    let blocking = Blocking {
        shared: Arc::clone(shared),
    };
    if shared.tasks.load(Ordering::SeqCst) != 1 || shared.has_pending() {
        return None;
    }
    Some(blocking)
}

/// How long a task on a worker without a reactor parks between two looks
/// at the descriptor it waits for ([`Readiness::wait`]).
const RETRY: Duration = Duration::from_millis(10);

/// [`Readiness::wait`] on a worker without a reactor: `Ready` when
/// `descriptor` is ready for `interest` now, else the task parks for
/// [`RETRY`] or until `deadline`, letting the worker's other tasks run.
fn without_reactor(
    me: &Waiter,
    descriptor: BorrowedFd<'_>,
    interest: Interest,
    deadline: Option<Instant>,
) -> io::Result<DescriptorWait> {
    use nix::poll::{PollFd, poll};
    let mut fds = [PollFd::new(descriptor, interest.poll())];
    match poll(&mut fds, 0u16) {
        Ok(_) | Err(nix::errno::Errno::EINTR) => {}
        Err(error) => return Err(error.into()),
    }
    if fds[0].revents().is_some_and(|ready| !ready.is_empty()) {
        return Ok(DescriptorWait::Ready(interest));
    }
    let retry = read_clock() + RETRY;
    crate::wait::park(me, Some(deadline.map_or(retry, |end| end.min(retry))));
    Ok(match deadline {
        Some(end) if read_clock() >= end => DescriptorWait::Deadline,
        _ => DescriptorWait::Woken,
    })
}

impl Readiness {
    /// Deregister `descriptor`, the one every wait of this readiness was
    /// for, from every worker and wake its waiters; before the descriptor
    /// closes. A later wait registers it again.
    pub(crate) fn deregister(&self, descriptor: BorrowedFd<'_>) {
        self.release(Some(descriptor));
    }

    /// [`Readiness::deregister`] of `delete`; without one the workers'
    /// reactors only forget the descriptor ([`EpollReactor::forget`]): its
    /// owner closes it right after, and the close takes it out of their
    /// epoll instances without an `epoll_ctl` each.
    fn release(&self, delete: Option<BorrowedFd<'_>>) {
        let (registered, shared_token) = {
            let mut registered = lock(&self.descriptor().registered);
            self.descriptor()
                .home
                .store(std::ptr::null_mut(), Ordering::Release);
            // Relaxed: changed only under `registered`.
            let shared_token = self.descriptor().shared_token.swap(0, Ordering::Relaxed);
            (std::mem::take(&mut *registered), shared_token)
        };
        if shared_token != 0
            && let Some(reactor) = shared()
        {
            match delete {
                Some(descriptor) => {
                    let _ = reactor.deregister(Token(shared_token), descriptor);
                }
                None => reactor.forget(Token(shared_token)),
            }
        }
        for (shared, token) in registered {
            if let Some(reactor) = shared.reactor.get() {
                match delete {
                    Some(descriptor) => {
                        let _ = reactor.deregister(token, descriptor);
                    }
                    None => reactor.forget(token),
                }
            }
        }
        // A new mark under the lock a waiter compares its mark under: a
        // waiter that comes after the wakes sees it and does not park.
        let state = self.descriptor();
        let mut waiting = lock(&state.waiting);
        state.read_events.fetch_add(1, Ordering::SeqCst);
        state.write_events.fetch_add(1, Ordering::SeqCst);
        waiting.readers.wake_all();
        waiting.writers.wake_all();
    }
}

/// Its owner (a stream, a listener's slot) closes the descriptor right
/// after, and no other descriptor refers to that socket (none is ever
/// duplicated; a child process's copy closes at its `exec`), so the close
/// deregisters it.
impl Drop for Readiness {
    fn drop(&mut self) {
        self.release(None);
    }
}

impl std::fmt::Debug for Readiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Readiness").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The slots of the entries of `timers` in the order they come due at
    /// `now`, taken out.
    fn drain(timers: &mut Timers, now: Instant) -> Vec<usize> {
        std::iter::from_fn(|| timers.pop_due(now).ok())
            .map(|(slot, _)| slot)
            .collect()
    }

    /// Every entry's recorded position is where it is, no slot without an
    /// entry has one, and no parent is later than its children.
    fn check(timers: &Timers) {
        for (index, timer) in timers.heap.iter().enumerate() {
            assert_eq!(timers.slots[timer.slot].position as usize, index + 1);
            if index > 0 {
                assert!(timers.heap[(index - 1) / 2].key <= timer.key);
            }
        }
        let placed = timers.slots.iter().filter(|s| s.position != 0).count();
        assert_eq!(placed, timers.heap.len());
    }

    /// [`Timers`] hands out due entries earliest first, in the order they
    /// were armed on the same deadline, and reports the next deadline.
    #[test]
    fn timers_pop_earliest_first() {
        let waiter = Waiter::current();
        let start = Instant::now();
        let at = |n: u64| start + Duration::from_secs(n);
        let mut timers = Timers::new();
        for (slot, n) in [(3, 3), (1, 1), (4, 4), (0, 2), (5, 5), (2, 2)] {
            timers.arm(slot, at(n), &waiter);
            check(&timers);
        }
        assert_eq!(timers.pop_due(at(0)).err(), Some(Some(at(1))));
        assert_eq!(drain(&mut timers, at(2)), [1, 0, 2]);
        check(&timers);
        assert_eq!(drain(&mut timers, at(9)), [3, 4, 5]);
        assert_eq!(timers.pop_due(at(9)).err(), Some(None));
        assert!(timers.is_empty());
    }

    /// The entry of a task that ends is taken out at once, wherever it is,
    /// so its deadline is never the next one; an entry armed earlier moves
    /// up in place, a later arming leaves it.
    #[test]
    fn timers_remove_at_once_and_move_an_earlier_arming_up() {
        let waiter = Waiter::current();
        let start = Instant::now();
        let at = |n: u64| start + Duration::from_secs(n);
        let mut timers = Timers::new();
        for slot in 0..10 {
            timers.arm(slot, at(slot as u64 + 1), &waiter);
        }
        for slot in [0, 9, 4, 1] {
            timers.remove(slot);
            check(&timers);
        }
        timers.remove(4);
        assert_eq!(timers.heap.len(), 6);
        assert_eq!(timers.pop_due(at(0)).err(), Some(Some(at(3))));
        timers.arm(8, at(1), &waiter);
        check(&timers);
        timers.arm(7, at(50), &waiter);
        check(&timers);
        assert_eq!(drain(&mut timers, at(9)), [8, 2, 3, 5, 6, 7]);
        assert!(timers.is_empty());
    }
}
