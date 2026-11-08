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
//! A registered descriptor must stay open until it is deregistered.

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::ops::BitOr;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

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

    fn poll(self) -> PollFlags {
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
    /// name. The descriptor stays open until [`Reactor::deregister`].
    fn register(&self, descriptor: RawFd, interest: Interest, waiter: Waiter) -> io::Result<Token>;

    /// Stop the events of `token`. A descriptor closed in the meantime is
    /// not an error.
    fn deregister(&self, token: Token) -> io::Result<()>;

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
    let left = deadline.saturating_duration_since(Instant::now());
    let millis = left.as_micros().div_ceil(1000);
    PollTimeout::try_from(millis.min(i32::MAX as u128)).unwrap_or(PollTimeout::NONE)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn eventfd() -> io::Result<EventFd> {
    Ok(EventFd::from_flags(
        EfdFlags::EFD_CLOEXEC | EfdFlags::EFD_NONBLOCK,
    )?)
}

/// Reset `eventfd`'s counter; a counter already zero is fine.
fn drain(eventfd: &EventFd) {
    let _ = eventfd.read();
}

/// One registered descriptor.
struct Entry {
    descriptor: RawFd,
    interest: Interest,
    waiter: Waiter,
}

/// The token of [`EpollReactor`]'s own `eventfd`.
const WAKE: u64 = 0;

/// Events [`EpollReactor::wait`] takes from the kernel per call.
const BATCH: usize = 64;

/// epoll plus an `eventfd` for wakes (`docs/readiness-io.md` 4.3). Every
/// descriptor is registered edge-triggered for its interest, so a
/// descriptor that stays ready gives one event per transition.
pub struct EpollReactor {
    epoll: Epoll,
    wakes: EventFd,
    entries: Mutex<HashMap<u64, Entry>>,
    next: AtomicU64,
}

impl EpollReactor {
    pub fn new() -> io::Result<EpollReactor> {
        let epoll = Epoll::new(EpollCreateFlags::EPOLL_CLOEXEC)?;
        let wakes = eventfd()?;
        epoll.add(&wakes, EpollEvent::new(EpollFlags::EPOLLIN, WAKE))?;
        Ok(EpollReactor {
            epoll,
            wakes,
            entries: Mutex::new(HashMap::new()),
            next: AtomicU64::new(WAKE + 1),
        })
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
    fn register(&self, descriptor: RawFd, interest: Interest, waiter: Waiter) -> io::Result<Token> {
        let token = self.next.fetch_add(1, Ordering::Relaxed);
        let mut entries = lock(&self.entries);
        // SAFETY: the caller keeps `descriptor` open until it deregisters
        // it (the trait's contract); the borrow lasts for this call only.
        let borrowed = unsafe { BorrowedFd::borrow_raw(descriptor) };
        let flags = interest.epoll() | EpollFlags::EPOLLET;
        self.epoll.add(borrowed, EpollEvent::new(flags, token))?;
        entries.insert(
            token,
            Entry {
                descriptor,
                interest,
                waiter,
            },
        );
        Ok(Token(token))
    }

    fn deregister(&self, token: Token) -> io::Result<()> {
        let Some(entry) = lock(&self.entries).remove(&token.0) else {
            return Ok(());
        };
        // SAFETY: as in `register`; a descriptor closed meanwhile is
        // reported as EBADF or ENOENT and ignored.
        let borrowed = unsafe { BorrowedFd::borrow_raw(entry.descriptor) };
        match self.epoll.delete(borrowed) {
            Ok(()) | Err(Errno::EBADF | Errno::ENOENT) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn wait(&self, deadline: Option<Instant>, events: &mut Vec<Event>) -> io::Result<()> {
        let mut ready = [EpollEvent::empty(); BATCH];
        let count = match self.epoll.wait(&mut ready, timeout(deadline)) {
            Ok(count) => count,
            Err(Errno::EINTR) => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let entries = lock(&self.entries);
        for event in &ready[..count] {
            if event.data() == WAKE {
                drain(&self.wakes);
                continue;
            }
            if let Some(entry) = entries.get(&event.data()) {
                events.push(Event {
                    token: Token(event.data()),
                    waiter: entry.waiter.clone(),
                    ready: Interest::of_epoll(event.events()),
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

/// `poll` over the registered descriptors plus an `eventfd` for wakes: the
/// reactor of a thread off a worker that waits for a descriptor
/// ([`crate::wait::wait_descriptor`]). Registration costs no system call,
/// so a thread registers for each wait. Level-triggered.
pub struct PollReactor {
    wakes: EventFd,
    entries: Mutex<Vec<(u64, Entry)>>,
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
}

impl fmt::Debug for PollReactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PollReactor")
            .field("eventfd", &self.wakes.as_raw_fd())
            .finish()
    }
}

impl Reactor for PollReactor {
    fn register(&self, descriptor: RawFd, interest: Interest, waiter: Waiter) -> io::Result<Token> {
        let token = self.next.fetch_add(1, Ordering::Relaxed);
        lock(&self.entries).push((
            token,
            Entry {
                descriptor,
                interest,
                waiter,
            },
        ));
        Ok(Token(token))
    }

    fn deregister(&self, token: Token) -> io::Result<()> {
        lock(&self.entries).retain(|(known, _)| *known != token.0);
        Ok(())
    }

    fn wait(&self, deadline: Option<Instant>, events: &mut Vec<Event>) -> io::Result<()> {
        let watched: Vec<(u64, RawFd, Interest, Waiter)> = lock(&self.entries)
            .iter()
            .map(|(token, entry)| {
                (
                    *token,
                    entry.descriptor,
                    entry.interest,
                    entry.waiter.clone(),
                )
            })
            .collect();
        let mut fds = Vec::with_capacity(watched.len() + 1);
        fds.push(PollFd::new(self.wakes.as_fd(), PollFlags::POLLIN));
        for (_, descriptor, interest, _) in &watched {
            // SAFETY: registered descriptors stay open until deregistered
            // (the trait's contract); the borrow lasts for the `poll`.
            let borrowed = unsafe { BorrowedFd::borrow_raw(*descriptor) };
            fds.push(PollFd::new(borrowed, interest.poll()));
        }
        match nix::poll::poll(&mut fds, timeout(deadline)) {
            Ok(_) => {}
            Err(Errno::EINTR) => return Ok(()),
            Err(error) => return Err(error.into()),
        }
        if fds[0].revents().is_some_and(|ready| !ready.is_empty()) {
            drain(&self.wakes);
        }
        for (fd, (token, _, _, waiter)) in fds[1..].iter().zip(watched) {
            let ready = Interest::of_poll(fd.revents().unwrap_or(PollFlags::empty()));
            if !ready.is_empty() {
                events.push(Event {
                    token: Token(token),
                    waiter,
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
