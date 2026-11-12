//! The readiness source of the substrate (`docs/readiness-io.md` 3.4,
//! 4.3): one interface, [`Reactor`], that tells which descriptors are
//! ready and which [`Waiter`]s to make runnable for them.
//!
//! What a reactor offers is what a worker needs from a readiness source:
//! register a descriptor once with its interest and deregister it; wait
//! for readiness until a deadline, returning the waiters to wake; wake a
//! reactor that is waiting, from any thread. The interface names no epoll
//! type: [`EpollReactor`] (epoll plus an `eventfd`) is one implementation,
//! [`PollReactor`] (`poll` plus an `eventfd`, the reactor of a thread that
//! waits for one descriptor, [`crate::wait::wait_descriptor`]) another,
//! and the test double `ScriptedReactor` in this crate's tests a third,
//! with no kernel descriptor at all. A tokio-driven reactor would be a
//! fourth; replacing one changes this crate only, neither backend.
//!
//! The contract, which `crates/ignite-io/tests/reactor.rs` runs against
//! every implementation:
//!
//! - after a registered descriptor becomes ready for one of its interests,
//!   a [`Reactor::wait`] returns an [`Event`] with its token and waiter at
//!   least once (whether it repeats while the descriptor stays ready is the
//!   implementation's: epoll here is edge-triggered, `poll` level-triggered);
//! - a wait with a deadline returns by the deadline when nothing is ready;
//! - [`Reactor::wake`] from any thread ends a wait in progress, or the next
//!   one when none is in progress, with no event for the wake itself;
//! - after [`Reactor::deregister`] a descriptor gives no more events.
//!
//! A descriptor is borrowed ([`BorrowedFd`]) at [`Reactor::register`] and
//! at [`Reactor::deregister`], so it is open at both, and a reactor hands
//! the kernel no descriptor it was not lent: [`EpollReactor`] keeps its
//! number only to compare at the deregistration, [`PollReactor`] keeps a
//! duplicate of its own until then.

use std::fmt;
use std::io;
use std::ops::BitOr;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::poll::{PollFd, PollFlags, PollTimeout};
use nix::sys::epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags};
use nix::sys::eventfd::{EfdFlags, EventFd};

use crate::wait::Waiter;

/// What a descriptor is registered for, and what it is ready for.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Interest(u8);

impl Interest {
    /// Nothing.
    pub const NONE: Interest = Interest(0);
    /// Bytes to read, a connection to accept, or end of stream.
    pub const READ: Interest = Interest(1);
    /// Room to write, or a connection established.
    pub const WRITE: Interest = Interest(2);
    /// Urgent data (`EPOLLPRI`, `POLLPRI`).
    pub const URGENT: Interest = Interest(4);
    /// The peer closed its write half or the connection hung up.
    pub const HANGUP: Interest = Interest(8);
    /// An error is pending on the descriptor; only ever reported, never
    /// asked for.
    pub const ERROR: Interest = Interest(16);

    /// Whether `self` holds every one of `other`.
    pub fn contains(self, other: Interest) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether `self` and `other` share one.
    pub fn intersects(self, other: Interest) -> bool {
        self.0 & other.0 != 0
    }

    /// Whether `self` is [`Interest::NONE`].
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The bits, to keep in an atomic.
    pub(crate) fn bits(self) -> u8 {
        self.0
    }

    /// The interests of `bits` as [`Interest::bits`] gave them.
    pub(crate) fn from_bits(bits: u8) -> Interest {
        Interest(bits & 31)
    }

    fn epoll(self) -> EpollFlags {
        let mut flags = EpollFlags::empty();
        if self.contains(Interest::READ) {
            flags |= EpollFlags::EPOLLIN;
        }
        if self.contains(Interest::WRITE) {
            flags |= EpollFlags::EPOLLOUT;
        }
        if self.contains(Interest::URGENT) {
            flags |= EpollFlags::EPOLLPRI;
        }
        if self.contains(Interest::HANGUP) {
            flags |= EpollFlags::EPOLLRDHUP;
        }
        flags
    }

    fn of_epoll(flags: EpollFlags) -> Interest {
        let mut ready = Interest::NONE;
        if flags.contains(EpollFlags::EPOLLIN) {
            ready = ready | Interest::READ;
        }
        if flags.contains(EpollFlags::EPOLLOUT) {
            ready = ready | Interest::WRITE;
        }
        if flags.contains(EpollFlags::EPOLLPRI) {
            ready = ready | Interest::URGENT;
        }
        if flags.intersects(EpollFlags::EPOLLRDHUP | EpollFlags::EPOLLHUP) {
            ready = ready | Interest::HANGUP;
        }
        if flags.contains(EpollFlags::EPOLLERR) {
            ready = ready | Interest::ERROR;
        }
        ready
    }

    pub(crate) fn poll(self) -> PollFlags {
        let mut flags = PollFlags::empty();
        if self.contains(Interest::READ) {
            flags |= PollFlags::POLLIN;
        }
        if self.contains(Interest::WRITE) {
            flags |= PollFlags::POLLOUT;
        }
        if self.contains(Interest::URGENT) {
            flags |= PollFlags::POLLPRI;
        }
        if self.contains(Interest::HANGUP) {
            flags |= rdhup();
        }
        flags
    }

    fn of_poll(flags: PollFlags) -> Interest {
        let mut ready = Interest::NONE;
        if flags.contains(PollFlags::POLLIN) {
            ready = ready | Interest::READ;
        }
        if flags.contains(PollFlags::POLLOUT) {
            ready = ready | Interest::WRITE;
        }
        if flags.contains(PollFlags::POLLPRI) {
            ready = ready | Interest::URGENT;
        }
        if flags.intersects(rdhup() | PollFlags::POLLHUP) {
            ready = ready | Interest::HANGUP;
        }
        if flags.intersects(PollFlags::POLLERR | PollFlags::POLLNVAL) {
            ready = ready | Interest::ERROR;
        }
        ready
    }
}

impl BitOr for Interest {
    type Output = Interest;
    fn bitor(self, other: Interest) -> Interest {
        Interest(self.0 | other.0)
    }
}

/// `POLLRDHUP`, which `nix` does not name.
fn rdhup() -> PollFlags {
    PollFlags::from_bits_retain(nix::libc::POLLRDHUP)
}

impl fmt::Debug for Interest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = [
            (Interest::READ, "READ"),
            (Interest::WRITE, "WRITE"),
            (Interest::URGENT, "URGENT"),
            (Interest::HANGUP, "HANGUP"),
            (Interest::ERROR, "ERROR"),
        ];
        let held: Vec<&str> = names
            .iter()
            .filter(|(bit, _)| self.contains(*bit))
            .map(|(_, name)| *name)
            .collect();
        if held.is_empty() {
            f.write_str("NONE")
        } else {
            f.write_str(&held.join(" | "))
        }
    }
}

/// A registration, as [`Reactor::register`] returned it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Token(pub u64);

/// A registered descriptor became ready: its token, the waiter registered
/// with it, and what it is ready for.
#[derive(Clone, Debug)]
pub struct Event {
    pub token: Token,
    pub waiter: Waiter,
    pub ready: Interest,
}

/// A readiness source (module docs).
pub trait Reactor: Send + Sync {
    /// Register `descriptor` for `interest`, with the waiter its events
    /// name. The borrow proves the descriptor open for the call; the
    /// caller deregisters it, with the same descriptor, while it is open.
    fn register(
        &self,
        descriptor: BorrowedFd<'_>,
        interest: Interest,
        waiter: Waiter,
    ) -> io::Result<Token>;

    /// Stop the events of `token`, which `descriptor` was registered
    /// with. Another descriptor than the registered one is refused
    /// (`InvalidInput`), after the token's events have stopped.
    fn deregister(&self, token: Token, descriptor: BorrowedFd<'_>) -> io::Result<()>;

    /// Wait until a registered descriptor is ready, a [`Reactor::wake`], or
    /// `deadline` (`None`: no deadline), and append the events to
    /// `events`. A wake, a deadline or an interrupted wait appends none.
    fn wait(&self, deadline: Option<Instant>, events: &mut Vec<Event>) -> io::Result<()>;

    /// End a [`Reactor::wait`] in progress on another thread, or the next
    /// one; from any thread.
    fn wake(&self) -> io::Result<()>;
}

/// The milliseconds until `deadline`, rounded up so a wait never ends
/// before it; `PollTimeout::NONE` without one.
fn timeout(deadline: Option<Instant>) -> PollTimeout {
    let Some(deadline) = deadline else {
        return PollTimeout::NONE;
    };
    timeout_left(deadline.saturating_duration_since(crate::sched::read_clock()))
}

/// The milliseconds of `left`, rounded up as [`timeout`] does.
pub(crate) fn timeout_left(left: Duration) -> PollTimeout {
    let millis = left.as_micros().div_ceil(1000);
    PollTimeout::try_from(millis.min(i32::MAX as u128)).unwrap_or(PollTimeout::NONE)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn eventfd() -> io::Result<EventFd> {
    Ok(EventFd::from_flags(
        EfdFlags::EFD_CLOEXEC | EfdFlags::EFD_NONBLOCK,
    )?)
}

/// How one `poll` of a descriptor beside a wake ended ([`poll_beside`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Polled {
    /// The descriptor may be ready for the interest asked for.
    pub(crate) ready: bool,
    /// The wake descriptor is readable.
    pub(crate) woken: bool,
    /// The kernel ended the `poll` for its timeout: the whole timeout
    /// passed and neither was ready. Never true for an interrupted `poll`.
    pub(crate) timed_out: bool,
}

/// One `poll` of `descriptor` for `interest` and of `wake` for input, on
/// the calling thread, until either may be ready or `timeout` passes. The
/// timeout is the caller's (`crate::wait::Limit::poll_timeout`): the
/// `poll` reads no clock, and the kernel says whether the timeout ended it.
/// An interrupted `poll` reports nothing.
pub(crate) fn poll_beside(
    descriptor: BorrowedFd<'_>,
    interest: Interest,
    wake: BorrowedFd<'_>,
    timeout: PollTimeout,
) -> io::Result<Polled> {
    let mut fds = [
        PollFd::new(descriptor, interest.poll()),
        PollFd::new(wake, PollFlags::POLLIN),
    ];
    let timed_out = match nix::poll::poll(&mut fds, timeout) {
        Ok(ready) => ready == 0,
        Err(Errno::EINTR) => false,
        Err(error) => return Err(error.into()),
    };
    let ready = |fd: &PollFd<'_>| fd.revents().is_some_and(|ready| !ready.is_empty());
    Ok(Polled {
        ready: ready(&fds[0]),
        woken: ready(&fds[1]),
        timed_out,
    })
}

/// Reset `eventfd`'s counter; a counter already zero is fine.
fn drain(eventfd: &EventFd) {
    let _ = eventfd.read();
}

/// One descriptor registered with an [`EpollReactor`]: its number, to
/// compare at [`Reactor::deregister`] only, never handed to the kernel.
struct Entry {
    descriptor: RawFd,
    interest: Interest,
    waiter: Waiter,
}

/// The registrations of an [`EpollReactor`] by token: one slot per
/// registration, reused after it ends. A token holds the slot's index plus
/// one in its low half (so never [`WAKE`]) and the slot's generation, which
/// each registration in it raises, in its high half: the token of an
/// earlier registration of a reused slot finds nothing (an event the kernel
/// reported for it, a late deregistration). A `HashMap` here was 6 KB of
/// code with its hasher and the random seed it reads (story
/// `binary-size-back`).
struct Entries {
    slots: Vec<Slot>,
    /// The index plus one of the first free slot, 0 for none; each free
    /// slot names the next in [`Slot::next`]. A list in the slots, not a
    /// vector of its own: a worker's reactor is new with the worker, and
    /// with a second vector a connection that a new worker served took
    /// about 145 instructions more, in `malloc`.
    free: u32,
}

/// A slot of [`Entries`].
struct Slot {
    generation: u32,
    /// While the slot is free: the index plus one of the next free slot.
    next: u32,
    entry: Option<Entry>,
}

impl Entries {
    const fn new() -> Entries {
        Entries {
            slots: Vec::new(),
            free: 0,
        }
    }

    /// Hold `entry`; its token.
    fn insert(&mut self, entry: Entry) -> u64 {
        let index = match self.free.checked_sub(1) {
            Some(index) => {
                let index = index as usize;
                self.free = self.slots[index].next;
                index
            }
            None => {
                self.slots.push(Slot {
                    generation: 0,
                    next: 0,
                    entry: None,
                });
                self.slots.len() - 1
            }
        };
        let slot = &mut self.slots[index];
        slot.generation = slot.generation.wrapping_add(1);
        slot.entry = Some(entry);
        (u64::from(slot.generation) << 32) | (index as u64 + 1)
    }

    /// The index of the slot `token` names, while it holds that
    /// registration.
    fn index(&self, token: u64) -> Option<usize> {
        let index = (token & 0xffff_ffff).checked_sub(1)? as usize;
        let slot = self.slots.get(index)?;
        (u64::from(slot.generation) == token >> 32 && slot.entry.is_some()).then_some(index)
    }

    fn get(&self, token: u64) -> Option<&Entry> {
        self.slots[self.index(token)?].entry.as_ref()
    }

    fn get_mut(&mut self, token: u64) -> Option<&mut Entry> {
        let index = self.index(token)?;
        self.slots[index].entry.as_mut()
    }

    fn remove(&mut self, token: u64) -> Option<Entry> {
        let index = self.index(token)?;
        let slot = &mut self.slots[index];
        slot.next = self.free;
        self.free = index as u32 + 1;
        slot.entry.take()
    }
}

/// One descriptor registered with a [`PollReactor`]: a duplicate the
/// reactor owns, so it is open at every `poll` until it is deregistered,
/// and the number of the one it was lent, to compare at
/// [`Reactor::deregister`].
struct Watched {
    registered: RawFd,
    descriptor: Arc<OwnedFd>,
    interest: Interest,
    waiter: Waiter,
}

/// The refusal of a [`Reactor::deregister`] with another descriptor than
/// the registered one.
fn another_descriptor() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "deregistered with another descriptor than the registered one",
    )
}

/// The token of [`EpollReactor`]'s own `eventfd`.
const WAKE: u64 = 0;

/// Events [`EpollReactor::wait`] takes from the kernel per call.
pub(crate) const BATCH: usize = 64;

/// epoll plus an `eventfd` for wakes (`docs/readiness-io.md` 4.3). Every
/// descriptor is registered edge-triggered for its interest, so a
/// descriptor that stays ready gives one event per transition.
pub struct EpollReactor {
    epoll: Epoll,
    wakes: EventFd,
    /// A wake wrote to `wakes` since it was last read. `wait` never reads
    /// it back, so its counter stays up, and a `poll` of it
    /// ([`EpollReactor::poll_one`]) would end at once for a wake already
    /// taken ([`EpollReactor::forget_wakes`]).
    unread: AtomicBool,
    entries: Mutex<Entries>,
}

impl EpollReactor {
    pub fn new() -> io::Result<EpollReactor> {
        let epoll = Epoll::new(EpollCreateFlags::EPOLL_CLOEXEC)?;
        let wakes = eventfd()?;
        // Edge-triggered: every write to an eventfd raises a new edge, also
        // while its counter is not zero, so `wait` never reads it back (one
        // system call less per wake). The counter cannot fill: at one per
        // wake it reaches its limit of 2^64 - 2 in no lifetime.
        epoll.add(
            &wakes,
            EpollEvent::new(EpollFlags::EPOLLIN | EpollFlags::EPOLLET, WAKE),
        )?;
        Ok(EpollReactor {
            epoll,
            wakes,
            unread: AtomicBool::new(false),
            entries: Mutex::new(Entries::new()),
        })
    }
}

impl EpollReactor {
    /// Wait on the calling thread, past the epoll instance, until
    /// `descriptor` may be ready for `interest`, [`Reactor::wake`] is
    /// called or `timeout` passes; whether the descriptor is ready, and
    /// whether the timeout ended the wait ([`poll_beside`]). The wait of a
    /// worker's only task (`crate::sched`): one `poll`, as a blocked
    /// thread's, instead of a switch to the worker's loop and its
    /// `epoll_wait`. The descriptor may also be registered here; its edge
    /// then waits in the epoll instance for the next `wait`.
    pub(crate) fn poll_one(
        &self,
        descriptor: BorrowedFd<'_>,
        interest: Interest,
        timeout: PollTimeout,
    ) -> io::Result<Polled> {
        let polled = poll_beside(descriptor, interest, self.wakes.as_fd(), timeout)?;
        if polled.woken {
            // Relaxed (here and in `forget_wakes`, `wake`): a hint that
            // saves a `read`; a wake is never lost to it, because a worker
            // looks for what was handed over before it sleeps.
            self.unread.store(false, Ordering::Relaxed);
            drain(&self.wakes);
        }
        Ok(polled)
    }

    /// Read back the wakes written since the eventfd was last read, before
    /// a [`EpollReactor::poll_one`]: only by the worker, while it does not
    /// sleep, so they are wakes it has taken. Without, the `poll` after a
    /// worker's wake ends at once, and its task registers and parks for
    /// nothing.
    pub(crate) fn forget_wakes(&self) {
        if self.unread.swap(false, Ordering::Relaxed) {
            drain(&self.wakes);
        }
    }

    /// Drop the entry of `token` without a system call, for a descriptor
    /// that closes right after: the close takes it out of the epoll
    /// instance. An event the kernel reports for it until then names a
    /// token without an entry, which [`Reactor::wait`] skips: a token is not
    /// reused ([`Entries`]).
    pub(crate) fn forget(&self, token: Token) {
        lock(&self.entries).remove(token.0);
    }

    /// Register `descriptor`, the one `token` was registered for, for
    /// `interest` instead. What it is ready for at the change is reported
    /// as an event (`EPOLL_CTL_MOD` looks at the descriptor). A token that
    /// is gone is not an error.
    pub(crate) fn rearm(
        &self,
        token: Token,
        descriptor: BorrowedFd<'_>,
        interest: Interest,
    ) -> io::Result<()> {
        self.modify(token, descriptor, interest, EpollFlags::EPOLLET)
    }

    fn modify(
        &self,
        token: Token,
        descriptor: BorrowedFd<'_>,
        interest: Interest,
        mode: EpollFlags,
    ) -> io::Result<()> {
        let mut entries = lock(&self.entries);
        let Some(entry) = entries.get_mut(token.0) else {
            return Ok(());
        };
        let flags = interest.epoll() | mode;
        self.epoll
            .modify(descriptor, &mut EpollEvent::new(flags, token.0))?;
        entry.interest = interest;
        Ok(())
    }

    /// The epoll instance, to register it with another reactor: that one
    /// reports an event of its own while this one has events to take
    /// (`crate::sched`, the shared reactor of a task's first read).
    pub(crate) fn as_fd(&self) -> BorrowedFd<'_> {
        self.epoll.0.as_fd()
    }

    /// [`Reactor::register`] for one event: the kernel reports the first
    /// and then nothing until [`EpollReactor::rearm_once`]
    /// (`EPOLLONESHOT`), so a registration that is done with needs no
    /// `EPOLL_CTL_DEL`; the close of the descriptor takes it out.
    pub(crate) fn register_once(
        &self,
        descriptor: BorrowedFd<'_>,
        interest: Interest,
        waiter: Waiter,
    ) -> io::Result<Token> {
        self.add(
            descriptor,
            interest,
            waiter,
            EpollFlags::EPOLLET | EpollFlags::EPOLLONESHOT,
        )
    }

    /// [`EpollReactor::rearm`] of a registration made with
    /// [`EpollReactor::register_once`], for one more event.
    pub(crate) fn rearm_once(
        &self,
        token: Token,
        descriptor: BorrowedFd<'_>,
        interest: Interest,
    ) -> io::Result<()> {
        self.modify(
            token,
            descriptor,
            interest,
            EpollFlags::EPOLLET | EpollFlags::EPOLLONESHOT,
        )
    }

    /// [`Reactor::wait`] that does not wait: the events ready now; how
    /// many the kernel returned, wakes included, so a caller that got
    /// [`BATCH`] knows to look again.
    pub(crate) fn poll_now(&self, events: &mut Vec<Event>) -> io::Result<usize> {
        self.take(PollTimeout::ZERO, events)
    }

    fn add(
        &self,
        descriptor: BorrowedFd<'_>,
        interest: Interest,
        waiter: Waiter,
        mode: EpollFlags,
    ) -> io::Result<Token> {
        let mut entries = lock(&self.entries);
        let token = entries.insert(Entry {
            descriptor: descriptor.as_raw_fd(),
            interest,
            waiter,
        });
        let flags = interest.epoll() | mode;
        if let Err(error) = self.epoll.add(descriptor, EpollEvent::new(flags, token)) {
            entries.remove(token);
            return Err(error.into());
        }
        Ok(Token(token))
    }

    fn take(&self, timeout: PollTimeout, events: &mut Vec<Event>) -> io::Result<usize> {
        let mut ready = [EpollEvent::empty(); BATCH];
        let count = match self.epoll.wait(&mut ready, timeout) {
            Ok(count) => count,
            Err(Errno::EINTR) => return Ok(0),
            Err(error) => return Err(error.into()),
        };
        let entries = lock(&self.entries);
        for event in &ready[..count] {
            if event.data() == WAKE {
                continue;
            }
            if let Some(entry) = entries.get(event.data()) {
                events.push(Event {
                    token: Token(event.data()),
                    waiter: entry.waiter.clone(),
                    ready: Interest::of_epoll(event.events()),
                });
            }
        }
        Ok(count)
    }
}

impl fmt::Debug for EpollReactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EpollReactor")
            .field("epoll", &self.epoll.0.as_raw_fd())
            .finish()
    }
}

impl Reactor for EpollReactor {
    fn register(
        &self,
        descriptor: BorrowedFd<'_>,
        interest: Interest,
        waiter: Waiter,
    ) -> io::Result<Token> {
        self.add(descriptor, interest, waiter, EpollFlags::EPOLLET)
    }

    fn deregister(&self, token: Token, descriptor: BorrowedFd<'_>) -> io::Result<()> {
        let Some(entry) = lock(&self.entries).remove(token.0) else {
            return Ok(());
        };
        if entry.descriptor != descriptor.as_raw_fd() {
            return Err(another_descriptor());
        }
        match self.epoll.delete(descriptor) {
            Ok(()) | Err(Errno::EBADF | Errno::ENOENT) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn wait(&self, deadline: Option<Instant>, events: &mut Vec<Event>) -> io::Result<()> {
        self.take(timeout(deadline), events).map(|_| ())
    }

    fn wake(&self) -> io::Result<()> {
        self.unread.store(true, Ordering::Relaxed);
        self.wakes.write(1)?;
        Ok(())
    }
}

/// `poll` over the registered descriptors plus an `eventfd` for wakes,
/// level-triggered. A thread off a worker that waits for one descriptor
/// ([`crate::wait::wait_descriptor`]) waits with
/// `PollReactor::poll_one`, which registers nothing; a registration
/// keeps a duplicate of the descriptor (`Watched`) until it is
/// deregistered.
pub struct PollReactor {
    wakes: EventFd,
    entries: Mutex<Vec<(u64, Watched)>>,
    next: AtomicU64,
}

impl PollReactor {
    pub fn new() -> io::Result<PollReactor> {
        Ok(PollReactor {
            wakes: eventfd()?,
            entries: Mutex::new(Vec::new()),
            next: AtomicU64::new(1),
        })
    }

    /// Wait until `descriptor`, borrowed for the call and not registered,
    /// is ready for one of `interest`, [`Reactor::wake`] is called or
    /// `timeout` passes; what it is ready for, [`Interest::NONE`] after a
    /// wake, a timeout or an interrupted wait, and whether the timeout
    /// ended the wait (the kernel says so: no clock is read for it). One
    /// `poll` of the descriptor and the eventfd, and a `read` of the
    /// eventfd after a wake: the calls of a registration and a
    /// [`Reactor::wait`], without the registration.
    pub(crate) fn poll_one(
        &self,
        descriptor: BorrowedFd<'_>,
        interest: Interest,
        timeout: PollTimeout,
    ) -> io::Result<(Interest, bool)> {
        let mut fds = [
            PollFd::new(self.wakes.as_fd(), PollFlags::POLLIN),
            PollFd::new(descriptor, interest.poll()),
        ];
        let timed_out = match nix::poll::poll(&mut fds, timeout) {
            Ok(ready) => ready == 0,
            Err(Errno::EINTR) => return Ok((Interest::NONE, false)),
            Err(error) => return Err(error.into()),
        };
        if fds[0].revents().is_some_and(|ready| !ready.is_empty()) {
            drain(&self.wakes);
        }
        Ok((
            Interest::of_poll(fds[1].revents().unwrap_or(PollFlags::empty())),
            timed_out,
        ))
    }
}

impl fmt::Debug for PollReactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PollReactor")
            .field("eventfd", &self.wakes.as_raw_fd())
            .finish()
    }
}

impl Reactor for PollReactor {
    fn register(
        &self,
        descriptor: BorrowedFd<'_>,
        interest: Interest,
        waiter: Waiter,
    ) -> io::Result<Token> {
        let token = self.next.fetch_add(1, Ordering::Relaxed);
        let duplicate = Arc::new(descriptor.try_clone_to_owned()?);
        lock(&self.entries).push((
            token,
            Watched {
                registered: descriptor.as_raw_fd(),
                descriptor: duplicate,
                interest,
                waiter,
            },
        ));
        Ok(Token(token))
    }

    fn deregister(&self, token: Token, descriptor: BorrowedFd<'_>) -> io::Result<()> {
        let mut entries = lock(&self.entries);
        let Some(at) = entries.iter().position(|(known, _)| *known == token.0) else {
            return Ok(());
        };
        let (_, entry) = entries.remove(at);
        if entry.registered != descriptor.as_raw_fd() {
            return Err(another_descriptor());
        }
        Ok(())
    }

    fn wait(&self, deadline: Option<Instant>, events: &mut Vec<Event>) -> io::Result<()> {
        let watched: Vec<(u64, Arc<OwnedFd>, Interest, Waiter)> = lock(&self.entries)
            .iter()
            .map(|(token, entry)| {
                (
                    *token,
                    Arc::clone(&entry.descriptor),
                    entry.interest,
                    entry.waiter.clone(),
                )
            })
            .collect();
        let mut fds = Vec::with_capacity(watched.len() + 1);
        fds.push(PollFd::new(self.wakes.as_fd(), PollFlags::POLLIN));
        for (_, descriptor, interest, _) in &watched {
            fds.push(PollFd::new(descriptor.as_fd(), interest.poll()));
        }
        match nix::poll::poll(&mut fds, timeout(deadline)) {
            Ok(_) => {}
            Err(Errno::EINTR) => return Ok(()),
            Err(error) => return Err(error.into()),
        }
        if fds[0].revents().is_some_and(|ready| !ready.is_empty()) {
            drain(&self.wakes);
        }
        for (fd, (token, _, _, waiter)) in fds[1..].iter().zip(&watched) {
            let ready = Interest::of_poll(fd.revents().unwrap_or(PollFlags::empty()));
            if !ready.is_empty() {
                events.push(Event {
                    token: Token(*token),
                    waiter: waiter.clone(),
                    ready,
                });
            }
        }
        Ok(())
    }

    fn wake(&self) -> io::Result<()> {
        self.wakes.write(1)?;
        Ok(())
    }
}
