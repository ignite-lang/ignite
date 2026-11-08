//! One waiter for every wait of the substrate (`docs/readiness-io.md` 4.2,
//! phase R0).
//!
//! Every place `ignite-io` waits for something another thread does (a value
//! in a channel, bytes in a pipe, a child's end, a free slot in a `Store`,
//! the end of the live tasks, an idle worker's next task, a pause, a
//! listener's next connection) parks a [`Waiter`] with [`park`] and is
//! made runnable by [`wake`]. A [`WaitList`] holds the waiters of one
//! condition; it lives in the state the condition is about, under that
//! state's lock, so a waiter queued before the lock is released cannot
//! miss a wake given after it.
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
//! A waiter today is an OS thread: `park` blocks the thread on its futex
//! (`std::thread::park`), as one thread per task always did. That is one
//! implementation of the seam, not a stage to remove: a later waiter kind
//! (a stackful task on a worker, `story:readiness-scheduler`) is a second
//! variant of [`Kind`], and the IO mode (`threads` or `readiness`,
//! `story:io-mode-setting`) chooses between them. Callers of this module
//! do not know which kind they park.
//!
//! A wait for a descriptor goes through the reactor interface
//! ([`crate::reactor::Reactor`]): off a worker, [`wait_descriptor`]
//! registers the descriptor with the thread's own
//! [`crate::reactor::PollReactor`] (`poll` on the descriptor and the
//! thread's `eventfd`), so a [`wake`] from any thread ends the wait as it
//! ends a [`park`]. That is how the drain wakes `main` in `accept`.

use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::os::fd::RawFd;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread::Thread;
use std::time::{Duration, Instant};

use crate::reactor::{Event, Interest, PollReactor, Reactor};

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

/// The waiter kinds. A task on a worker is the variant R1 adds.
enum Kind {
    /// An OS thread: parked on its futex, or in `poll` on its own reactor
    /// while it waits for a descriptor.
    Thread {
        thread: Thread,
        reactor: OnceLock<PollReactor>,
    },
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
        Waiter {
            inner: Arc::new(Inner {
                state: AtomicU8::new(0),
                kind: Kind::Thread {
                    thread,
                    reactor: OnceLock::new(),
                },
            }),
        }
    }

    /// The waiter of the calling thread (later: of the calling task). Every
    /// call on one thread gives the same waiter.
    pub fn current() -> Waiter {
        CURRENT
            .try_with(Waiter::clone)
            .unwrap_or_else(|_| Waiter::for_thread(std::thread::current()))
    }

    /// The thread a thread waiter parks; `None` for a later waiter kind
    /// that is no thread.
    pub fn thread(&self) -> Option<std::thread::ThreadId> {
        match &self.inner.kind {
            Kind::Thread { thread, .. } => Some(thread.id()),
        }
    }

    /// Whether `self` and `other` are the same waiter.
    pub fn same(&self, other: &Waiter) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// Take a pending wake; whether there was one.
    fn take_wake(&self) -> bool {
        self.inner.state.fetch_and(!NOTIFIED, Ordering::Acquire) & NOTIFIED != 0
    }
}

impl fmt::Debug for Waiter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner.kind {
            Kind::Thread { thread, .. } => f
                .debug_struct("Waiter")
                .field("thread", &thread.id())
                .finish(),
        }
    }
}

/// Park the caller, whose waiter `waiter` is, until a [`wake`] or
/// `deadline` (`None`: no deadline). A wake given before the park is not
/// lost: the park returns at once. On a thread this blocks the thread (a
/// futex wait).
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
    }
}

/// Make `waiter` runnable; from any thread. A waiter that is not parked
/// keeps the wake for its next park. A second wake before that park adds
/// nothing and costs one atomic operation.
pub fn wake(waiter: &Waiter) {
    let before = waiter.inner.state.fetch_or(NOTIFIED, Ordering::Release);
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
/// thread's `eventfd`, which a wake writes. The descriptor stays open for
/// the call.
pub fn wait_descriptor(
    descriptor: RawFd,
    interest: Interest,
    deadline: Option<Instant>,
) -> io::Result<DescriptorWait> {
    let me = Waiter::current();
    let Kind::Thread { reactor, .. } = &me.inner.kind;
    let reactor = match reactor.get() {
        Some(reactor) => reactor,
        None => {
            let made = PollReactor::new()?;
            reactor.get_or_init(|| made)
        }
    };
    let token = reactor.register(descriptor, interest, me.clone())?;
    let mut events: Vec<Event> = Vec::new();
    let outcome = loop {
        me.inner.state.fetch_or(POLLING, Ordering::SeqCst);
        if me.take_wake() {
            break Ok(DescriptorWait::Woken);
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            break Ok(DescriptorWait::Deadline);
        }
        events.clear();
        if let Err(error) = reactor.wait(deadline, &mut events) {
            break Err(error);
        }
        if let Some(event) = events.iter().find(|event| event.token == token) {
            break Ok(DescriptorWait::Ready(event.ready));
        }
    };
    me.inner.state.fetch_and(!POLLING, Ordering::SeqCst);
    let _ = reactor.deregister(token);
    outcome
}
