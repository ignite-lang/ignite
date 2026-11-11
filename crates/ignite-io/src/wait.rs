//! One waiter for every wait of the substrate (`docs/readiness-io.md` 4.2).
//!
//! Every place `ignite-io` waits for something another thread or task does
//! (a value in a channel, bytes in a pipe or a socket, room to send, a
//! child's end, a free slot in a `Store`, the end of the live tasks, an
//! idle worker's next task, a pause, a listener's next connection) parks a
//! [`Waiter`] with [`park`] and is made runnable by [`wake`]. A
//! [`WaitList`] holds the waiters of one condition; it lives in the state
//! the condition is about, under that state's lock, so a waiter queued
//! before the lock is released cannot miss a wake given after it.
//!
//! The pattern, as [`park_in`] runs it: lock the state, test the condition,
//! queue [`Waiter::current`] on the list, unlock, [`park`], lock again, take
//! the waiter off the list if it is still there, test again. The waker
//! changes the state under the lock, takes waiters off the list and wakes
//! them, after unlocking where it can. A park may return without a wake
//! meant for it (a wake that raced with a deadline is kept for the next
//! park), so every caller tests its condition again; the condition, not
//! the return value, says whether to go on.
//!
//! A waiter is one of two kinds, and callers of this module do not know
//! which they park. An OS thread (`main`, the drain, a watcher, every task
//! in the IO mode `threads`) blocks on its futex (`std::thread::park`). A
//! stackful task on a worker (the IO mode `readiness`, `crate::sched`)
//! switches to its worker's loop, which runs other tasks until a wake, a
//! readiness event or the deadline makes it runnable again. The IO mode
//! chooses between them (`crate::sched::readiness`); both pass the same
//! tests.
//!
//! A wait for a descriptor goes through the reactor interface
//! ([`crate::reactor::Reactor`]): off a worker, [`wait_descriptor`]
//! waits in the thread's own [`crate::reactor::PollReactor`] (one `poll`
//! of the descriptor and the thread's `eventfd`, no registration), so a
//! [`wake`] from any thread ends the wait as it
//! ends a [`park`]; on a worker it registers with the worker's reactor and
//! parks the task. That is how the drain wakes `main` in `accept`.
//!
//! A lock held across a park must be a [`Lock`], which parks a task that
//! waits for it: a `std::sync::Mutex` would block the worker's thread, and
//! with it the task that holds the lock and runs on the same worker.

use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::ops::{Deref, DerefMut};
use std::os::fd::BorrowedFd;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, TryLockError};
use std::thread::Thread;
use std::time::{Duration, Instant};

use crate::reactor::{Interest, PollReactor, Reactor};
use crate::sched;

/// Why a [`park`] returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Woken {
    /// A [`wake`] reached the waiter (possibly one meant for an earlier
    /// park; the caller tests its condition again).
    Notified,
    /// The deadline passed first.
    Deadline,
}

/// A parked thread or task; clones name the same waiter.
#[derive(Clone)]
pub struct Waiter {
    inner: Arc<Inner>,
}

struct Inner {
    /// [`NOTIFIED`] and [`POLLING`].
    state: AtomicU8,
    kind: Kind,
}

/// The waiter kinds.
enum Kind {
    /// An OS thread: parked on its futex, or in `poll` on its own reactor
    /// while it waits for a descriptor.
    Thread {
        thread: Thread,
        reactor: OnceLock<PollReactor>,
    },
    /// A stackful task on a worker (`crate::sched`).
    Task(sched::Task),
    /// A descriptor registered with workers' reactors (`crate::sched`): a
    /// readiness event wakes the tasks waiting to read or to write it. It
    /// never parks.
    Descriptor(sched::Descriptor),
}

/// A wake arrived that no park has consumed yet.
const NOTIFIED: u8 = 1;
/// The waiter waits in its reactor ([`wait_descriptor`]): a wake must
/// also write the reactor's `eventfd`.
const POLLING: u8 = 2;

thread_local! {
    static CURRENT: Waiter = Waiter::for_thread(std::thread::current());
}

impl Waiter {
    fn for_thread(thread: Thread) -> Waiter {
        Waiter::of(Kind::Thread {
            thread,
            reactor: OnceLock::new(),
        })
    }

    fn of(kind: Kind) -> Waiter {
        Waiter {
            inner: Arc::new(Inner {
                state: AtomicU8::new(0),
                kind,
            }),
        }
    }

    /// The waiter of a task the scheduler starts.
    pub(crate) fn for_task(task: sched::Task) -> Waiter {
        Waiter::of(Kind::Task(task))
    }

    /// The waiter a descriptor is registered with.
    pub(crate) fn for_descriptor(descriptor: sched::Descriptor) -> Waiter {
        Waiter::of(Kind::Descriptor(descriptor))
    }

    /// The waiter of the calling task, or of the calling thread off a
    /// task. Every call in one task, or on one thread outside a task,
    /// gives the same waiter.
    pub fn current() -> Waiter {
        if let Some(task) = sched::running() {
            return task;
        }
        CURRENT
            .try_with(Waiter::clone)
            .unwrap_or_else(|_| Waiter::for_thread(std::thread::current()))
    }

    /// The thread a thread waiter parks; `None` for a task.
    pub fn thread(&self) -> Option<std::thread::ThreadId> {
        match &self.inner.kind {
            Kind::Thread { thread, .. } => Some(thread.id()),
            Kind::Task(_) | Kind::Descriptor(_) => None,
        }
    }

    /// Whether the waiter is a task on a worker.
    pub fn is_task(&self) -> bool {
        matches!(self.inner.kind, Kind::Task(_))
    }

    /// Whether `self` and `other` are the same waiter.
    pub fn same(&self, other: &Waiter) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// Take a pending wake; whether there was one.
    pub(crate) fn take_wake(&self) -> bool {
        self.inner.state.fetch_and(!NOTIFIED, Ordering::SeqCst) & NOTIFIED != 0
    }

    /// Whether a wake is pending, without taking it.
    pub(crate) fn has_wake(&self) -> bool {
        self.inner.state.load(Ordering::SeqCst) & NOTIFIED != 0
    }

    /// The task of a task waiter.
    pub(crate) fn task(&self) -> Option<&sched::Task> {
        match &self.inner.kind {
            Kind::Task(task) => Some(task),
            _ => None,
        }
    }

    /// The descriptor of a descriptor waiter.
    pub(crate) fn descriptor(&self) -> Option<&sched::Descriptor> {
        match &self.inner.kind {
            Kind::Descriptor(descriptor) => Some(descriptor),
            _ => None,
        }
    }
}

impl fmt::Debug for Waiter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner.kind {
            Kind::Thread { thread, .. } => f
                .debug_struct("Waiter")
                .field("thread", &thread.id())
                .finish(),
            Kind::Task(_) => f.debug_struct("Waiter").field("task", &true).finish(),
            Kind::Descriptor(_) => f.debug_struct("Waiter").field("descriptor", &true).finish(),
        }
    }
}

/// Park the caller, whose waiter `waiter` is, until a [`wake`] or
/// `deadline` (`None`: no deadline). A wake given before the park is not
/// lost: the park returns at once. On a thread this blocks the thread (a
/// futex wait); a task switches to its worker's loop.
pub fn park(waiter: &Waiter, deadline: Option<Instant>) -> Woken {
    match &waiter.inner.kind {
        Kind::Thread { thread, .. } => {
            debug_assert_eq!(
                thread.id(),
                std::thread::current().id(),
                "a thread parks only its own waiter"
            );
            loop {
                if waiter.take_wake() {
                    return Woken::Notified;
                }
                match deadline {
                    None => std::thread::park(),
                    Some(deadline) => {
                        let now = Instant::now();
                        if now >= deadline {
                            return Woken::Deadline;
                        }
                        std::thread::park_timeout(deadline - now);
                    }
                }
            }
        }
        Kind::Task(task) => sched::park(waiter, task, deadline),
        Kind::Descriptor(_) => Woken::Notified,
    }
}

/// Make `waiter` runnable; from any thread. A waiter that is not parked
/// keeps the wake for its next park. A second wake before that park adds
/// nothing and costs one atomic operation.
pub fn wake(waiter: &Waiter) {
    if let Kind::Descriptor(descriptor) = &waiter.inner.kind {
        descriptor.ready(Interest::READ | Interest::WRITE);
        return;
    }
    let before = waiter.inner.state.fetch_or(NOTIFIED, Ordering::SeqCst);
    if before & NOTIFIED != 0 {
        return;
    }
    match &waiter.inner.kind {
        Kind::Thread { thread, reactor } => {
            if before & POLLING != 0
                && let Some(reactor) = reactor.get()
            {
                let _ = reactor.wake();
            }
            thread.unpark();
        }
        Kind::Task(task) => sched::schedule(waiter, task),
        Kind::Descriptor(_) => {}
    }
}

/// [`wake`] for a readiness event: a task waiting for a descriptor learns
/// what it is ready for, and a descriptor waiter wakes the tasks waiting
/// to read it or to write it, as `ready` says.
pub(crate) fn wake_ready(waiter: &Waiter, ready: Interest) {
    match &waiter.inner.kind {
        Kind::Descriptor(descriptor) => descriptor.ready(ready),
        Kind::Task(task) => {
            task.note_ready(ready);
            wake(waiter);
        }
        Kind::Thread { .. } => wake(waiter),
    }
}

/// Pause the caller until `deadline`; wakes meant for other waits do not
/// end it.
pub fn sleep_until(deadline: Instant) {
    let me = Waiter::current();
    while park(&me, Some(deadline)) == Woken::Notified {}
}

/// [`sleep_until`] `duration` from now; a duration past the end of
/// `Instant` sleeps without end.
pub fn sleep(duration: Duration) {
    match Instant::now().checked_add(duration) {
        Some(deadline) => sleep_until(deadline),
        None => {
            let me = Waiter::current();
            loop {
                park(&me, None);
            }
        }
    }
}

/// The waiters of one condition, longest waiting first. It has no lock of
/// its own: it lives in the state the condition is about, under that
/// state's lock.
#[derive(Default)]
pub struct WaitList {
    waiters: VecDeque<Waiter>,
}

impl WaitList {
    pub const fn new() -> WaitList {
        WaitList {
            waiters: VecDeque::new(),
        }
    }

    /// Whether no waiter is queued.
    pub fn is_empty(&self) -> bool {
        self.waiters.is_empty()
    }

    /// Queue `waiter`.
    pub fn push(&mut self, waiter: Waiter) {
        self.waiters.push_back(waiter);
    }

    /// Take `waiter` off the list; whether it was on it.
    pub fn remove(&mut self, waiter: &Waiter) -> bool {
        match self.waiters.iter().position(|queued| queued.same(waiter)) {
            Some(at) => {
                self.waiters.remove(at);
                true
            }
            None => false,
        }
    }

    /// The longest waiting waiter, taken off the list.
    pub fn pop(&mut self) -> Option<Waiter> {
        self.waiters.pop_front()
    }

    /// Every waiter, taken off the list, to wake once the lock is released.
    pub fn take(&mut self) -> WaitList {
        WaitList {
            waiters: std::mem::take(&mut self.waiters),
        }
    }

    /// Wake the longest waiting waiter, if any.
    pub fn wake_one(&mut self) {
        if let Some(waiter) = self.pop() {
            wake(&waiter);
        }
    }

    /// Wake every waiter.
    pub fn wake_all(&mut self) {
        for waiter in self.waiters.drain(..) {
            wake(&waiter);
        }
    }
}

impl fmt::Debug for WaitList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WaitList")
            .field("waiters", &self.waiters.len())
            .finish()
    }
}

/// Wait on the list `list` picks out of the state `guard` holds locked:
/// queue the calling waiter, release the lock, [`park`] until a wake or
/// `deadline`, lock `mutex` again and take the waiter off the list if no
/// waker did. The caller tests its condition again on the returned guard;
/// [`Woken::Deadline`] means the deadline passed and no waker chose this
/// waiter.
pub fn park_in<'a, T>(
    mutex: &'a Mutex<T>,
    mut guard: MutexGuard<'a, T>,
    list: impl Fn(&mut T) -> &mut WaitList,
    deadline: Option<Instant>,
) -> (MutexGuard<'a, T>, Woken) {
    let me = Waiter::current();
    list(&mut guard).push(me.clone());
    drop(guard);
    let woken = park(&me, deadline);
    let mut guard = mutex.lock().unwrap_or_else(PoisonError::into_inner);
    let queued = list(&mut guard);
    if queued.is_empty() || !queued.remove(&me) {
        // A waker took the waiter off: the wake is this waiter's even if
        // the deadline passed first, so the caller looks at the state
        // before it gives up (a `wake_one` must not be lost).
        return (guard, Woken::Notified);
    }
    (guard, woken)
}

/// How a [`wait_descriptor`] ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptorWait {
    /// The descriptor is ready for these of the interests asked for (or
    /// has an error or hang-up, which the next call on it reports).
    Ready(Interest),
    /// A [`wake`] ended the wait.
    Woken,
    /// The deadline passed first.
    Deadline,
}

/// Wait until `descriptor` is ready for one of `interest`, a [`wake`] of
/// the caller's waiter, or `deadline`. Off a worker the wait goes through
/// the thread's own [`PollReactor`]: `poll` on the descriptor and the
/// thread's `eventfd`, which a wake writes. On a worker the descriptor is
/// registered with the worker's reactor for the wait and the task parks.
/// The borrow keeps the descriptor open for the call.
pub fn wait_descriptor(
    descriptor: BorrowedFd<'_>,
    interest: Interest,
    deadline: Option<Instant>,
) -> io::Result<DescriptorWait> {
    let me = Waiter::current();
    let reactor = match &me.inner.kind {
        Kind::Thread { reactor, .. } => reactor,
        Kind::Task(task) => {
            return sched::wait_descriptor(&me, task, descriptor, interest, deadline);
        }
        Kind::Descriptor(_) => return Ok(DescriptorWait::Woken),
    };
    let reactor = match reactor.get() {
        Some(reactor) => reactor,
        None => {
            let made = PollReactor::new()?;
            reactor.get_or_init(|| made)
        }
    };
    let outcome = loop {
        me.inner.state.fetch_or(POLLING, Ordering::SeqCst);
        if me.take_wake() {
            break Ok(DescriptorWait::Woken);
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            break Ok(DescriptorWait::Deadline);
        }
        match reactor.poll_one(descriptor, interest, deadline) {
            Err(error) => break Err(error),
            Ok(ready) if !ready.is_empty() => break Ok(DescriptorWait::Ready(ready)),
            Ok(_) => {}
        }
    };
    me.inner.state.fetch_and(!POLLING, Ordering::SeqCst);
    outcome
}

/// A mutual-exclusion lock that may be held across a [`park`]: a task
/// that waits for it parks instead of blocking its worker's thread, so the
/// task holding it, on the same worker, can run on and release it. A
/// stream's read and write locks are one (a read waits for input holding
/// its read lock, a send for room holding the write lock). Not poisoned
/// by a panic.
///
/// The value sits in a `std::sync::Mutex` that is only ever taken with
/// `try_lock`, so no thread blocks in it; a guard is dropped on the thread
/// that took it, because a task never moves to another worker.
pub struct Lock<T> {
    value: Mutex<T>,
    /// Waiters queued on `waiters`, so an unlock looks at the list only
    /// when someone waits.
    waiting: AtomicUsize,
    waiters: Mutex<WaitList>,
}

/// The holder of a [`Lock`]; dropping it releases the lock.
pub struct LockGuard<'a, T> {
    guard: Option<MutexGuard<'a, T>>,
    lock: &'a Lock<T>,
}

impl<T> Lock<T> {
    pub const fn new(value: T) -> Lock<T> {
        Lock {
            value: Mutex::new(value),
            waiting: AtomicUsize::new(0),
            waiters: Mutex::new(WaitList::new()),
        }
    }

    fn acquire(&self) -> Option<LockGuard<'_, T>> {
        let guard = match self.value.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => return None,
        };
        Some(LockGuard {
            guard: Some(guard),
            lock: self,
        })
    }

    /// The lock, waited for as long as another holder keeps it.
    pub fn lock(&self) -> LockGuard<'_, T> {
        if let Some(guard) = self.acquire() {
            return guard;
        }
        loop {
            let list = self.waiters.lock().unwrap_or_else(PoisonError::into_inner);
            self.waiting.fetch_add(1, Ordering::SeqCst);
            if let Some(guard) = self.acquire() {
                self.waiting.fetch_sub(1, Ordering::SeqCst);
                return guard;
            }
            let (list, _) = park_in(&self.waiters, list, |list| list, None);
            self.waiting.fetch_sub(1, Ordering::SeqCst);
            drop(list);
            if let Some(guard) = self.acquire() {
                return guard;
            }
        }
    }

    /// The lock if no one holds it now.
    pub fn try_lock(&self) -> Option<LockGuard<'_, T>> {
        self.acquire()
    }

    /// The value, through the exclusive borrow that proves no guard is out.
    pub fn get_mut(&mut self) -> &mut T {
        self.value.get_mut().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<T: Default> Default for Lock<T> {
    fn default() -> Lock<T> {
        Lock::new(T::default())
    }
}

impl<T: fmt::Debug> fmt::Debug for Lock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lock")
            .field(
                "held",
                &matches!(self.value.try_lock(), Err(TryLockError::WouldBlock)),
            )
            .finish_non_exhaustive()
    }
}

impl<T> Deref for LockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.guard.as_deref().expect("a lock guard holds its lock")
    }
}

impl<T> DerefMut for LockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.guard
            .as_deref_mut()
            .expect("a lock guard holds its lock")
    }
}

impl<T> Drop for LockGuard<'_, T> {
    fn drop(&mut self) {
        drop(self.guard.take());
        if self.lock.waiting.load(Ordering::SeqCst) > 0 {
            self.lock
                .waiters
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .wake_one();
        }
    }
}
