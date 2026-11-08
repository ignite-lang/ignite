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
//! descriptor and parks its thread until then. Workers start lazily, one per
//! spawn up to [`worker_limit`] (`IGNITE_WORKERS`, default
//! `std::thread::available_parallelism`), and a worker without a task for
//! [`crate::LINGER`] ends, so an idle server runs `main` and the drain's
//! thread only (`docs/readiness-io.md` section 6 row 6).
//!
//! A task is a stack (2 MiB reserved as a thread's, a guard page below it,
//! touched pages only resident; stacks are pooled), a coroutine on it
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
//! fewest tasks. A task never moves to another
//! worker (no stealing in R1), so the thread-local state a task keeps
//! across a park stays on its thread; per-task thread-locals of another
//! crate (the LLVM runtime's arena, failure handlers and owned handles)
//! are exchanged whenever a different task resumes, through [`TaskLocals`].
//!
//! Stack overflow: a task's guard page is recognised by the `SIGSEGV`
//! handler installed with the first worker (on an alternate signal stack
//! each worker sets up), which prints `thread 'worker' has overflowed its
//! stack` and aborts, as a thread's overflow does; any other fault goes to
//! the handler that was installed before.
//!
//! A descriptor a task waits for (a socket, a listener, standard input)
//! is registered once per worker, edge-triggered, through a
//! [`Readiness`]: an event wakes the tasks waiting to read or to write it,
//! and a sequence number per direction, taken before each attempt, keeps a
//! readiness edge between the attempt and the park from being lost
//! (`docs/readiness-io.md` K4).

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::ffi::c_void;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Once, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use corosensei::stack::{DefaultStack, Stack};
use corosensei::{Coroutine, CoroutineResult, Yielder};

use crate::reactor::{EpollReactor, Event, Interest, Reactor, Token};
use crate::wait::{DescriptorWait, WaitList, Waiter, Woken, wake_ready};

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

/// Stack reserved per task: a thread's default, so recursion depth does not
/// change with the IO mode.
pub const TASK_STACK: usize = 2 * 1024 * 1024;

/// Free stacks kept for the next tasks.
const STACK_POOL: usize = 64;

/// Tasks a worker runs before it looks at its reactor again although more
/// are runnable.
const BUDGET: usize = 128;

type Job = Box<dyn FnOnce() + Send + 'static>;

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

/// The scheduler's part of a task's [`Waiter`].
pub(crate) struct Task {
    worker: Arc<Shared>,
    state: AtomicU8,
    /// Readiness bits of the events since the task's last descriptor wait.
    ready: AtomicU8,
    /// The task's place in its worker's table.
    slot: usize,
}

impl Task {
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
    /// The worker sleeps (in its reactor, or parked): a hand-over must
    /// wake it.
    sleeping: AtomicBool,
    /// Tasks placed on the worker and not ended.
    tasks: AtomicUsize,
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
            sleeping: AtomicBool::new(false),
            tasks: AtomicUsize::new(0),
        }
    }

    /// The worker's reactor, created on first use; called on the worker.
    fn reactor(&self) -> io::Result<&EpollReactor> {
        if let Some(reactor) = self.reactor.get() {
            return Ok(reactor);
        }
        let _ = self.reactor.set(EpollReactor::new()?);
        Ok(self.reactor.get().expect("the reactor was just set"))
    }

    /// Wake the worker if it sleeps. The worker creates its reactor only
    /// while it runs a task, before it sets `sleeping`, so a waker that
    /// sees `sleeping` also sees how the worker sleeps.
    fn notify(&self) {
        if self.sleeping.swap(false, Ordering::SeqCst) {
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
    /// Deadlines of parked tasks, with a sequence number that keeps the
    /// keys apart.
    static TIMERS: RefCell<BTreeMap<(Instant, u64), Waiter>> = const { RefCell::new(BTreeMap::new()) };
    static TIMER_SEQUENCE: Cell<u64> = const { Cell::new(0) };
    /// The timer of each task slot of this worker ([`SlotTimer`]).
    static SLOT_TIMERS: RefCell<Vec<SlotTimer>> = const { RefCell::new(Vec::new()) };
    /// The guard page of the running task's stack, `start..end`; read by
    /// the `SIGSEGV` handler, so a `const` cell without destructor.
    static GUARD: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

/// The waiter of the running task; `None` off a worker or between tasks.
pub(crate) fn running() -> Option<Waiter> {
    RUNNING_TASK
        .try_with(|running| running.borrow().clone())
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
    if HERE.with(Cell::get) == Arc::as_ptr(&task.worker) {
        RUN.with(|run| run.borrow_mut().push_back(waiter.clone()));
        return;
    }
    lock(&task.worker.incoming).woken.push(waiter.clone());
    task.worker.notify();
}

/// Park the running `task` (`wait::park`): switch to the worker's loop
/// until a wake or `deadline`.
pub(crate) fn park(waiter: &Waiter, task: &Task, deadline: Option<Instant>) -> Woken {
    debug_assert!(
        HERE.with(Cell::get) == Arc::as_ptr(&task.worker),
        "a task parks on its own worker"
    );
    loop {
        if waiter.take_wake() {
            return Woken::Notified;
        }
        if let Some(deadline) = deadline {
            if Instant::now() >= deadline {
                return Woken::Deadline;
            }
            arm(task.slot, deadline, waiter);
        }
        suspend();
        if deadline.is_some() {
            with_timer(task.slot, |timer| timer.wanted = None);
        }
    }
}

/// The timer of the task in one slot of this worker. A task has at most one
/// entry in `TIMERS` armed for it; a park with a deadline arms one only when
/// none is armed at or before that deadline, and a wake leaves it in place.
/// When it fires, a task that wants a later deadline by then is armed again
/// for it ([`Worker::fire_timers`]). A read with an idle timeout therefore
/// costs no timer operation per wait, only one per timeout period.
#[derive(Clone, Copy, Default)]
struct SlotTimer {
    /// The deadline of the task's current park, if it has one.
    wanted: Option<Instant>,
    /// The key of the task's entry in `TIMERS`.
    armed: Option<(Instant, u64)>,
}

fn with_timer<T>(slot: usize, f: impl FnOnce(&mut SlotTimer) -> T) -> T {
    SLOT_TIMERS.with(|timers| {
        let mut timers = timers.borrow_mut();
        if timers.len() <= slot {
            timers.resize(slot + 1, SlotTimer::default());
        }
        f(&mut timers[slot])
    })
}

/// Insert a timer entry for `waiter` at `at`; its key.
fn insert_timer(at: Instant, waiter: &Waiter) -> (Instant, u64) {
    let sequence = TIMER_SEQUENCE.with(|n| {
        n.set(n.get() + 1);
        n.get()
    });
    let key = (at, sequence);
    TIMERS.with(|timers| timers.borrow_mut().insert(key, waiter.clone()));
    key
}

/// The task in `slot` parks until `deadline` at the latest.
fn arm(slot: usize, deadline: Instant, waiter: &Waiter) {
    let insert = with_timer(slot, |timer| {
        timer.wanted = Some(deadline);
        timer.armed.is_none_or(|(at, _)| at > deadline)
    });
    if insert {
        let key = insert_timer(deadline, waiter);
        with_timer(slot, |timer| timer.armed = Some(key));
    }
}

/// The task in `slot` ended: remove its armed entry.
fn forget_timer(slot: usize) {
    if let Some(key) = with_timer(slot, std::mem::take).armed {
        TIMERS.with(|timers| timers.borrow_mut().remove(&key));
    }
}

/// Switch from the running task to its worker's loop.
fn suspend() {
    let yielder = YIELDER.with(Cell::get);
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
    descriptor: RawFd,
    interest: Interest,
    deadline: Option<Instant>,
) -> io::Result<DescriptorWait> {
    let reactor = task.worker.reactor()?;
    let token = reactor.register(descriptor, interest, waiter.clone())?;
    task.ready.store(0, Ordering::SeqCst);
    let woken = park(waiter, task, deadline);
    let _ = reactor.deregister(token);
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
    install_overflow_handler();
    let stack = match lock(&STACKS).pop() {
        Some(stack) => stack,
        None => DefaultStack::new(TASK_STACK)?,
    };
    let here = HERE.with(Cell::get);
    let mut workers = lock(&WORKERS);
    if let Some(shared) = choose(&workers, here) {
        shared.tasks.fetch_add(1, Ordering::SeqCst);
        lock(&shared.incoming).new.push((job, stack));
        drop(workers);
        if Arc::as_ptr(&shared) != here {
            shared.notify();
        }
        return Ok(());
    }
    let shared = Arc::new(Shared::new());
    shared.tasks.store(1, Ordering::SeqCst);
    lock(&shared.incoming).new.push((job, stack));
    let started = Arc::clone(&shared);
    match std::thread::Builder::new()
        .name(String::from("worker"))
        .spawn(move || work(started))
    {
        Ok(_) => {
            workers.push(shared);
            Ok(())
        }
        Err(error) => {
            let (job, stack) = lock(&shared.incoming)
                .new
                .pop()
                .expect("the job handed to the worker");
            drop(workers);
            give_stack(stack);
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
/// loaded.
fn choose(workers: &[Arc<Shared>], here: *const Shared) -> Option<Arc<Shared>> {
    let load = |shared: &Arc<Shared>| shared.tasks.load(Ordering::SeqCst);
    let elsewhere = |shared: &Arc<Shared>| Arc::as_ptr(shared) != here;
    if let Some(local) = workers.iter().find(|shared| !elsewhere(shared)) {
        let least = if workers.len() < worker_limit() {
            0
        } else {
            workers.iter().map(load).min().unwrap_or(0)
        };
        if load(local) <= least + 1 {
            return Some(Arc::clone(local));
        }
    }
    if let Some(idle) = workers
        .iter()
        .filter(|shared| load(shared) == 0)
        .min_by_key(|shared| elsewhere(shared))
    {
        return Some(Arc::clone(idle));
    }
    if workers.len() < worker_limit() {
        return None;
    }
    workers
        .iter()
        .min_by_key(|shared| (load(shared), elsewhere(shared)))
        .cloned()
}

fn give_stack(stack: DefaultStack) {
    let mut stacks = lock(&STACKS);
    if stacks.len() < STACK_POOL {
        stacks.push(stack);
    }
}

/// A task on this worker.
struct Local {
    coroutine: Option<Coroutine<(), (), (), DefaultStack>>,
    yielder: *const Yielder<(), ()>,
    locals: Option<Box<dyn TaskLocals>>,
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
        let Incoming { woken, new } = std::mem::take(&mut *lock(&self.shared.incoming));
        if !woken.is_empty() {
            RUN.with(|run| run.borrow_mut().extend(woken));
        }
        for (job, stack) in new {
            self.start(job, stack);
        }
    }

    fn start(&mut self, job: Job, stack: DefaultStack) {
        // A new task's locals are made from the worker's thread-locals
        // (the LLVM runtime's spare arena chunks).
        self.restore_locals();
        let slot = self.free.pop().unwrap_or_else(|| {
            self.slots.push(None);
            self.slots.len() - 1
        });
        let start = stack.limit().get();
        let guard = (start, start + page_size());
        let waiter = Waiter::for_task(Task {
            worker: Arc::clone(&self.shared),
            state: AtomicU8::new(QUEUED),
            ready: AtomicU8::new(0),
            slot,
        });
        let coroutine = Coroutine::with_stack(stack, move |yielder: &Yielder<(), ()>, ()| {
            YIELDER.with(|cell| cell.set(yielder));
            if let Err(report) = crate::report::catch(job) {
                crate::report::task_failed(&report);
            }
            crate::flush();
        });
        self.slots[slot] = Some(Local {
            coroutine: Some(coroutine),
            yielder: std::ptr::null(),
            locals: LOCALS.get().map(|make| make()),
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
        let slot = task.slot;
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
        task.state.store(RUNNING, Ordering::SeqCst);
        YIELDER.with(|cell| cell.set(local.yielder));
        GUARD.with(|cell| cell.set(local.guard));
        RUNNING_TASK.with(|running| *running.borrow_mut() = Some(waiter.clone()));
        let result = coroutine.resume(());
        RUNNING_TASK.with(|running| *running.borrow_mut() = None);
        GUARD.with(|cell| cell.set((0, 0)));
        let local = self.slots[slot].as_mut().expect("the resumed task's slot");
        local.yielder = YIELDER.with(|cell| cell.replace(std::ptr::null()));
        match result {
            CoroutineResult::Yield(()) => {
                local.coroutine = Some(coroutine);
                task.state.store(PARKED, Ordering::SeqCst);
                if waiter.has_wake() && task.queue() {
                    RUN.with(|run| run.borrow_mut().push_back(waiter.clone()));
                }
            }
            CoroutineResult::Return(()) => {
                task.state.store(DONE, Ordering::SeqCst);
                self.restore_locals();
                forget_timer(slot);
                give_stack(coroutine.into_stack());
                let ended = self.slots[slot].take();
                drop(ended);
                self.free.push(slot);
                self.live -= 1;
                self.shared.tasks.fetch_sub(1, Ordering::SeqCst);
                crate::task::task_finished();
            }
        }
    }

    /// Queue the tasks whose deadline passed, arm again those that want a
    /// later one by now ([`SlotTimer`]); the next deadline. Reads the clock
    /// only when there is a timer.
    fn fire_timers(&self) -> Option<Instant> {
        if TIMERS.with(|timers| timers.borrow().is_empty()) {
            return None;
        }
        let now = Instant::now();
        loop {
            let first = TIMERS.with(|timers| {
                let mut timers = timers.borrow_mut();
                match timers.first_key_value() {
                    Some((&(at, _), _)) if at > now => Err(Some(at)),
                    Some(_) => Ok(timers.pop_first().expect("the first timer")),
                    None => Err(None),
                }
            });
            let (key, waiter) = match first {
                Ok(entry) => entry,
                Err(next) => return next,
            };
            let Some(task) = waiter.task() else {
                continue;
            };
            let wanted = with_timer(task.slot, |timer| {
                if timer.armed != Some(key) {
                    return None;
                }
                timer.armed = None;
                Some(timer.wanted)
            });
            match wanted {
                None | Some(None) => {}
                Some(Some(later)) if later > now => {
                    let key = insert_timer(later, &waiter);
                    with_timer(task.slot, |timer| timer.armed = Some(key));
                }
                Some(Some(_)) => {
                    if task.queue() {
                        RUN.with(|run| run.borrow_mut().push_back(waiter.clone()));
                    }
                }
            }
        }
    }

    /// End the worker if no task was placed on it meanwhile; whether it
    /// ended.
    fn retire(&self) -> bool {
        let mut workers = lock(&WORKERS);
        if self.shared.tasks.load(Ordering::SeqCst) != 0 || !lock(&self.shared.incoming).is_empty()
        {
            return false;
        }
        workers.retain(|shared| !Arc::ptr_eq(shared, &self.shared));
        true
    }
}

fn runnable() -> bool {
    RUN.with(|run| !run.borrow().is_empty())
}

fn next_runnable() -> Option<Waiter> {
    RUN.with(|run| run.borrow_mut().pop_front())
}

fn dispatch(events: &mut Vec<Event>) {
    for event in events.drain(..) {
        wake_ready(&event.waiter, event.ready);
    }
}

/// The loop of a worker thread (module docs).
fn work(shared: Arc<Shared>) {
    crate::task::mark_task_thread();
    alternate_signal_stack();
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
        let mut ran = 0;
        while ran < BUDGET {
            let Some(waiter) = next_runnable() else {
                break;
            };
            worker.resume(waiter);
            ran += 1;
        }
        let next_timer = worker.fire_timers();
        if runnable() {
            if ran >= BUDGET
                && let Some(reactor) = worker.shared.reactor.get()
            {
                let _ = reactor.wait(Some(Instant::now()), &mut events);
                dispatch(&mut events);
            }
            continue;
        }
        let deadline = if worker.live == 0 {
            let now = Instant::now();
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
            Some(next_timer.map_or(end, |timer| timer.min(end)))
        } else {
            idle_since = None;
            next_timer
        };
        worker.shared.sleeping.store(true, Ordering::SeqCst);
        if !lock(&worker.shared.incoming).is_empty() {
            worker.shared.sleeping.store(false, Ordering::SeqCst);
            continue;
        }
        match worker.shared.reactor.get() {
            Some(reactor) => {
                let _ = reactor.wait(deadline, &mut events);
            }
            None => match deadline {
                Some(deadline) => {
                    std::thread::park_timeout(deadline.saturating_duration_since(Instant::now()));
                }
                None => std::thread::park(),
            },
        }
        worker.shared.sleeping.store(false, Ordering::SeqCst);
        dispatch(&mut events);
    }
}

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

/// The handlers of `SIGSEGV` and `SIGBUS` before [`on_fault`].
static PREVIOUS: OnceLock<[nix::sys::signal::SigAction; 2]> = OnceLock::new();

/// Install [`on_fault`] for `SIGSEGV` and `SIGBUS` once, keeping the
/// handlers before it for every fault outside a task's guard page.
fn install_overflow_handler() {
    use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let ours = SigAction::new(
            SigHandler::SigAction(on_fault),
            SaFlags::SA_ONSTACK | SaFlags::SA_SIGINFO,
            SigSet::empty(),
        );
        // SAFETY: `on_fault` is async-signal-safe: it reads a `const`
        // thread-local cell, writes to stderr with `write`, aborts, or
        // hands the signal to the handler that was installed before.
        let previous = unsafe {
            [
                sigaction(Signal::SIGSEGV, &ours),
                sigaction(Signal::SIGBUS, &ours),
            ]
        };
        if let [Ok(segv), Ok(bus)] = previous {
            let _ = PREVIOUS.set([segv, bus]);
        }
    });
}

/// A worker's alternate signal stack: the overflow handler cannot run on
/// the stack that overflowed. A thread that has one already (Rust's
/// runtime gives one to the threads it starts when it handles overflow
/// itself) keeps it.
fn alternate_signal_stack() {
    use nix::libc;
    const SIZE: usize = 64 * 1024;
    // SAFETY: `sigaltstack` reads and writes the `stack_t` values given;
    // the new stack is a fresh private mapping of `SIZE` bytes that stays
    // mapped for the life of the process (a worker's stack is never given
    // back, so a signal on it can never land in unmapped memory).
    unsafe {
        let mut current: libc::stack_t = std::mem::zeroed();
        if libc::sigaltstack(std::ptr::null(), &mut current) != 0
            || current.ss_flags & libc::SS_DISABLE == 0
        {
            return;
        }
        let memory = libc::mmap(
            std::ptr::null_mut(),
            SIZE,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        );
        if memory == libc::MAP_FAILED {
            return;
        }
        let stack = libc::stack_t {
            ss_sp: memory,
            ss_flags: 0,
            ss_size: SIZE,
        };
        libc::sigaltstack(&stack, std::ptr::null_mut());
    }
}

/// The `SIGSEGV` and `SIGBUS` handler (module docs).
extern "C" fn on_fault(signal: i32, info: *mut nix::libc::siginfo_t, context: *mut c_void) {
    use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};
    // SAFETY: the kernel passes a valid `siginfo_t` to a handler installed
    // with `SA_SIGINFO`; `si_addr` is defined for SIGSEGV and SIGBUS.
    let address = unsafe { (*info).si_addr() } as usize;
    let (start, end) = GUARD.try_with(Cell::get).unwrap_or((0, 0));
    if start <= address && address < end {
        let message = b"\nthread 'worker' has overflowed its stack\nfatal runtime error: stack overflow, aborting\n";
        let _ = nix::unistd::write(std::io::stderr().as_fd(), message);
        std::process::abort();
    }
    let which = usize::from(signal != Signal::SIGSEGV as i32);
    match PREVIOUS.get().map(|previous| previous[which].handler()) {
        Some(SigHandler::SigAction(previous)) => previous(signal, info, context),
        Some(SigHandler::Handler(previous)) => previous(signal),
        _ => {
            let default = SigAction::new(SigHandler::SigDfl, SaFlags::empty(), SigSet::empty());
            if let Ok(signal) = Signal::try_from(signal) {
                // SAFETY: restoring the default action is async-signal-safe;
                // the fault repeats on return and ends the process with it.
                let _ = unsafe { sigaction(signal, &default) };
            }
        }
    }
}

/// The readiness of one descriptor for tasks on workers (module docs): a
/// socket's or a listener's. Off a worker [`Readiness::wait`] is
/// [`crate::wait::wait_descriptor`]. Deregistered from every worker when
/// dropped, so it must be dropped before the descriptor closes.
pub(crate) struct Readiness {
    waiter: Waiter,
}

/// The [`Waiter`] a descriptor is registered with (`wait::Kind`).
pub(crate) struct Descriptor {
    /// Readiness events for reading (input, urgent data, hang-up, error).
    read_events: AtomicU64,
    /// Readiness events for writing (room, hang-up, error).
    write_events: AtomicU64,
    waiting: Mutex<Waiting>,
    /// The workers the descriptor is registered with, and its token there.
    registered: Mutex<Vec<(Arc<Shared>, Token)>>,
    /// The worker it was registered with first, compared only, never
    /// dereferenced: a wait there skips `registered`. The `Arc` in
    /// `registered` keeps that worker's address from being reused until
    /// the descriptor is deregistered, which clears this first.
    home: AtomicPtr<Shared>,
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

fn reads(ready: Interest) -> bool {
    ready.intersects(Interest::READ | Interest::URGENT | Interest::HANGUP | Interest::ERROR)
}

fn writes(ready: Interest) -> bool {
    ready.intersects(Interest::WRITE | Interest::HANGUP | Interest::ERROR)
}

impl Descriptor {
    /// A readiness event (`wait::wake_ready`): count it and wake the
    /// tasks waiting for its direction.
    pub(crate) fn ready(&self, ready: Interest) {
        let (read, write) = (reads(ready), writes(ready));
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
            waiter: Waiter::for_descriptor(Descriptor {
                read_events: AtomicU64::new(0),
                write_events: AtomicU64::new(0),
                waiting: Mutex::new(Waiting::default()),
                registered: Mutex::new(Vec::new()),
                home: AtomicPtr::new(std::ptr::null_mut()),
            }),
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

    /// Wait until `descriptor` may be ready for `interest`, after an
    /// attempt begun at `mark` found it not ready, or until `deadline`.
    /// `Woken` and `Ready` both mean: attempt again. The first wait of a
    /// task on a worker registers the descriptor there and returns at once.
    /// A worker that cannot create its reactor (no descriptor free for its
    /// epoll and eventfd) looks at the descriptor without waiting and parks
    /// the task for [`RETRY`], until it can: a session keeps running while
    /// the process is out of descriptors.
    pub(crate) fn wait(
        &self,
        descriptor: BorrowedFd<'_>,
        interest: Interest,
        mark: u64,
        deadline: Option<Instant>,
    ) -> io::Result<DescriptorWait> {
        let me = Waiter::current();
        let Some(task) = me.task() else {
            return crate::wait::wait_descriptor(descriptor.as_raw_fd(), interest, deadline);
        };
        let state = self.descriptor();
        let here = Arc::as_ptr(&task.worker).cast_mut();
        if state.home.load(Ordering::Acquire) != here {
            let mut registered = lock(&state.registered);
            if !registered
                .iter()
                .any(|(shared, _)| Arc::ptr_eq(shared, &task.worker))
            {
                let Ok(reactor) = task.worker.reactor() else {
                    drop(registered);
                    return without_reactor(&me, descriptor, interest, deadline);
                };
                let token = reactor.register(
                    descriptor.as_raw_fd(),
                    Interest::READ | Interest::WRITE | Interest::URGENT | Interest::HANGUP,
                    self.waiter.clone(),
                )?;
                registered.push((Arc::clone(&task.worker), token));
                if registered.len() == 1 {
                    state.home.store(here, Ordering::Release);
                }
                return Ok(DescriptorWait::Woken);
            }
        }
        if let Some(waited) = wait_alone(task, state, descriptor, interest, mark, deadline)? {
            return Ok(waited);
        }
        {
            let mut waiting = lock(&state.waiting);
            if state.events(interest).load(Ordering::SeqCst) != mark {
                return Ok(DescriptorWait::Woken);
            }
            waiting.of(interest).push(me.clone());
        }
        let woken = crate::wait::park(&me, deadline);
        let still_queued = lock(&state.waiting).of(interest).remove(&me);
        Ok(if still_queued && woken == Woken::Deadline {
            DescriptorWait::Deadline
        } else {
            DescriptorWait::Ready(interest)
        })
    }
}

/// [`Readiness::wait`] of the only task of its worker, with nothing else
/// runnable or handed over: the task waits for the descriptor on the
/// worker's thread with [`EpollReactor::poll_one`], as a blocked thread
/// would, instead of switching to the worker's loop. One `poll` of the
/// descriptor and the worker's eventfd measured cheaper per reply than the
/// worker's `epoll_wait` called from the task. The worker counts as
/// sleeping meanwhile, so a hand-over ([`Shared::notify`]) ends the wait
/// and the task then parks as usual. `None` when the worker has other work.
fn wait_alone(
    task: &Task,
    state: &Descriptor,
    descriptor: BorrowedFd<'_>,
    interest: Interest,
    mark: u64,
    deadline: Option<Instant>,
) -> io::Result<Option<DescriptorWait>> {
    let shared = &task.worker;
    if shared.tasks.load(Ordering::SeqCst) != 1 || runnable() {
        return Ok(None);
    }
    let Some(reactor) = shared.reactor.get() else {
        return Ok(None);
    };
    shared.sleeping.store(true, Ordering::SeqCst);
    if !lock(&shared.incoming).is_empty() {
        shared.sleeping.store(false, Ordering::SeqCst);
        return Ok(None);
    }
    if state.events(interest).load(Ordering::SeqCst) != mark {
        shared.sleeping.store(false, Ordering::SeqCst);
        return Ok(Some(DescriptorWait::Woken));
    }
    let ready = reactor.poll_one(descriptor, interest, deadline);
    shared.sleeping.store(false, Ordering::SeqCst);
    Ok(Some(if ready? {
        DescriptorWait::Ready(interest)
    } else if deadline.is_some_and(|end| Instant::now() >= end) {
        DescriptorWait::Deadline
    } else {
        DescriptorWait::Woken
    }))
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
    let retry = Instant::now() + RETRY;
    crate::wait::park(me, Some(deadline.map_or(retry, |end| end.min(retry))));
    Ok(match deadline {
        Some(end) if Instant::now() >= end => DescriptorWait::Deadline,
        _ => DescriptorWait::Woken,
    })
}

impl Readiness {
    /// Deregister the descriptor from every worker and wake its waiters;
    /// before the descriptor closes. A later wait registers it again.
    pub(crate) fn deregister(&self) {
        let registered = {
            let mut registered = lock(&self.descriptor().registered);
            self.descriptor()
                .home
                .store(std::ptr::null_mut(), Ordering::Release);
            std::mem::take(&mut *registered)
        };
        for (shared, token) in registered {
            if let Some(reactor) = shared.reactor.get() {
                let _ = reactor.deregister(token);
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

impl Drop for Readiness {
    fn drop(&mut self) {
        self.deregister();
    }
}

impl std::fmt::Debug for Readiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Readiness").finish_non_exhaustive()
    }
}
