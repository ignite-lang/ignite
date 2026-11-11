//! TCP over `std::net` sockets. A stream's socket blocks in both IO modes.
//! In `readiness` its reads and sends are `MSG_DONTWAIT` and every wait
//! goes through the stream's `Readiness` (a task parks, a thread polls),
//! except a read of the only task of a worker after a read that emptied
//! the socket, which blocks in `recv` (`sched::block_alone`); in
//! `threads` they block in the kernel.
//!
//! Every handle is `Clone`; a clone shares the socket, which closes when the
//! last clone is dropped. A stream has one lock per direction around the
//! shared socket: a `read` waiting for input holds only the read lock, so
//! another task can write to the same stream meanwhile; two writes do not
//! interleave. `close` takes the write lock.
//!
//! Sockets have `TCP_NODELAY` set: the substrate coalesces writes itself.
//! `write_all` appends to the stream's send buffer and registers the stream
//! with the writing task; [`flush`] sends every buffer the current task
//! wrote to. Every operation that may wait (a read of any stream or of
//! standard input, `accept`, `connect`) flushes first, and so do `close`,
//! the end of a task, the end of `run`, `exit` and the drop of the last
//! handle of a stream; a buffer that reaches [`SEND_BUFFER`] bytes is sent
//! at once. A task therefore sends what it wrote before it waits for
//! anything, and a server that answers a batch of pipelined requests sends
//! the answers with one system call. A failed send is reported by the next
//! `read`, `write_all` or `close` of that stream. The ends of a pipe
//! (`crate::pipe`) share the task's pending list and every flush point.
//!
//! Urgent data (TCP urgent mode, RFC 9293 3.8.5): `write_urgent` sends at
//! once, its last byte as the urgent byte (`MSG_OOB`). Sockets have
//! `SO_OOBINLINE` set, so the urgent byte stays in the stream at its place;
//! a read stops before it (the kernel's rule), and `urgent_pending` holds
//! from its arrival until a read has returned it.

use std::cell::RefCell;
use std::io::ErrorKind;
use std::net::Shutdown;
use std::os::fd::{AsFd, AsRawFd};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, TryLockError, Weak};
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::sys::socket::MsgFlags;

use crate::reactor::Interest;
use crate::sched::Readiness;
use crate::wait::{DescriptorWait, Lock, LockGuard, WaitList, Waiter};
use crate::{Input, IoError, read_input, socket_address};

/// Bytes a send buffer holds before `write_all` sends it at once.
pub const SEND_BUFFER: usize = 64 * 1024;

/// First pause of `accept` after a transient failure.
pub const ACCEPT_BACKOFF_FIRST: Duration = Duration::from_millis(10);
/// Longest pause of `accept` between retries.
pub const ACCEPT_BACKOFF_LAST: Duration = Duration::from_secs(1);

/// An `accept` failure that passes with time: the process or system is out
/// of file descriptors (`EMFILE`, `ENFILE`), buffers (`ENOBUFS`) or memory
/// (`ENOMEM`), the peer aborted the connection before it was accepted
/// (`ECONNABORTED`), or Linux handed the accepting socket a network error
/// that was pending on the new connection (`EPROTO`, `ENETDOWN`,
/// `EHOSTUNREACH`, `ENETUNREACH`, `ENONET`; `accept(2)` says to treat these
/// as `EAGAIN`).
fn transient(error: &std::io::Error) -> bool {
    use nix::errno::Errno;
    if matches!(
        error.kind(),
        ErrorKind::ConnectionAborted | ErrorKind::OutOfMemory
    ) {
        return true;
    }
    let Some(code) = error.raw_os_error() else {
        return false;
    };
    matches!(
        Errno::from_raw(code),
        Errno::ENFILE
            | Errno::EMFILE
            | Errno::ENOBUFS
            | Errno::ENOMEM
            | Errno::ECONNABORTED
            | Errno::EPROTO
            | Errno::ENETDOWN
            | Errno::EHOSTUNREACH
            | Errno::ENETUNREACH
            | Errno::ENONET
    )
}

#[derive(Clone, Debug)]
pub struct TcpListener {
    slot: Arc<Slot>,
}

/// The socket of a listener, `None` once the drain closed it
/// (`crate::drain`), with the address and port it was bound to.
#[derive(Debug)]
pub(crate) struct Slot {
    _counted: crate::census::Counted,
    listener: RwLock<Option<std::net::TcpListener>>,
    address: String,
    port: i64,
    /// Kept through the drain (story `drain-keeps-listeners`): never
    /// registered with `crate::drain`, so never released; a task waiting
    /// in its `accept`, or in a read of a stream it accepted, is idle for
    /// the drain's wait (`crate::task::idle_on_kept`).
    kept: bool,
    /// Waiters in `accept`, which [`Slot::release`] wakes.
    acceptors: Mutex<WaitList>,
    /// The listener's readiness for tasks on workers (`crate::sched`).
    readiness: Readiness,
}

impl Slot {
    fn read(&self) -> RwLockReadGuard<'_, Option<std::net::TcpListener>> {
        self.listener.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// Wait until `listener` (this slot's, non-blocking) has a connection
    /// to accept, after an `accept` begun at `mark` found none, or
    /// [`Slot::release`] wakes the caller. The wait goes through the
    /// reactor interface ([`Readiness::wait`]); on a listener kept through
    /// the drain the waiting task does not hold up the drain's wait for
    /// live tasks.
    fn wait_for_connection(
        &self,
        listener: &std::net::TcpListener,
        mark: u64,
    ) -> Result<(), IoError> {
        let me = Waiter::current();
        lock(&self.acceptors).push(me.clone());
        let wait = || {
            if !self.kept && crate::drain::draining() {
                return Ok(DescriptorWait::Woken);
            }
            self.readiness
                .wait(listener.as_fd(), Interest::READ, mark, None, None)
        };
        let waited = if self.kept {
            crate::task::idle_on_kept(wait)
        } else {
            wait()
        };
        lock(&self.acceptors).remove(&me);
        waited.map(|_| ()).map_err(IoError::from)
    }

    /// Release the listener for the drain: shut its read side down and
    /// wake every waiter in a pending `accept`, which then fails with
    /// "draining", then close it once that `accept` has let go of it. The
    /// `released <address>:<port>` line, or `None` when the listener was
    /// already closed.
    pub(crate) fn release(&self) -> Option<String> {
        {
            let guard = self.read();
            let listener = guard.as_ref()?;
            // SAFETY: the descriptor belongs to `listener`, which the read
            // lock keeps open for the call.
            unsafe { nix::libc::shutdown(listener.as_raw_fd(), nix::libc::SHUT_RD) };
            lock(&self.acceptors).wake_all();
        }
        let closed = self
            .listener
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .take()?;
        self.readiness.deregister();
        drop(closed);
        Some(format!("released {}:{}", self.address, self.port))
    }
}

#[derive(Clone, Debug)]
pub struct TcpStream {
    inner: Arc<Stream>,
}

/// [`Stream::emptied`] when the last read did not empty the socket.
const NOT_EMPTIED: u64 = u64::MAX;

/// [`Stream::emptied`] before the first read of the stream.
const UNREAD: u64 = u64::MAX - 1;

/// [`Stream::emptied`] before the first read of a stream that sent first
/// (a server's greeting): that read waits for readiness before its first
/// `recv`, which would find nothing yet, as a read after one that emptied
/// the socket does, against the mark 0 (the count of a descriptor no
/// event was counted for; a count that moved since ends the wait at
/// once). Unlike that read it never blocks its worker: a session that has
/// not been sent anything yet keeps no worker to itself while a burst of
/// connections is placed.
const SPOKE_FIRST: u64 = u64::MAX - 2;

#[derive(Debug)]
struct Stream {
    /// Before `socket`: dropped, so forgotten by the workers' reactors,
    /// as the socket closes.
    readiness: Readiness,
    socket: std::net::TcpStream,
    _counted: crate::census::Counted,
    reading: Lock<()>,
    writing: Lock<Out>,
    /// The socket blocks (IO mode `threads`): a read and a send wait in
    /// the kernel, and the idle timeout is `SO_RCVTIMEO` (`crate::idle`).
    /// Otherwise it is non-blocking and every wait goes through
    /// `readiness`.
    blocking: bool,
    /// The readiness mark taken before the last read when that read
    /// emptied the socket, else [`NOT_EMPTIED`]: the next read waits for
    /// readiness against it before its first `recv`, since a `recv` right
    /// away would find nothing (a request-reply peer has not answered
    /// yet). A read that filled its buffer leaves it unset, so the next
    /// read tries at once; so does a short read once the readiness
    /// reported an end of stream, an error or urgent data
    /// (`Readiness::short_read_may_leave_input`). [`UNREAD`] and
    /// [`SPOKE_FIRST`] before the first read. Only with `blocking` false.
    emptied: AtomicU64,
    /// The idle timeout in milliseconds; 0 is none (`crate::idle`).
    idle: AtomicI64,
    /// The idle timeout the socket has as `SO_RCVTIMEO`, which a `recv`
    /// that blocks needs and sets first ([`Stream::apply_idle`]).
    receive_timeout: AtomicI64,
    /// A send of the buffer failed; the error waits in `Out::failed`.
    failed: AtomicBool,
    /// `close` was called on some handle: a later `write_all` is the
    /// writer's error, not a failed send kept for the next reader.
    closed: AtomicBool,
    /// The last read ended because the idle timeout passed
    /// ([`TcpStream::timed_out`]).
    timed_out: AtomicBool,
    /// The other end's address and port, read once when the stream was
    /// made ([`TcpStream::peer`]).
    peer: (String, i64),
    /// Accepted by a listener kept through the drain: a task waiting in a
    /// read of it is idle for the drain's wait.
    kept: bool,
}

/// The write side of a stream: bytes written and not yet sent.
#[derive(Debug, Default)]
struct Out {
    buffer: Vec<u8>,
    failed: Option<IoError>,
}

/// A stream with a send buffer that the writing task's [`flush`] sends: a
/// `TcpStream` or an end of a pipe (`crate::pipe`).
pub(crate) trait Buffered: Send + Sync {
    /// Send the buffer, keeping a failure for the stream's next operation.
    fn flush_kept(&self);
    /// [`flush_at_exit`] of this stream: waits for its write lock and for
    /// room until `deadline` at most.
    fn flush_at_exit(&self, deadline: Instant);
}

/// Streams a task wrote to since it last flushed: what its next flush
/// sends (`docs/slice.md` section 7, "the writing task's next flush").
/// Sent when the thread ends, so a thread that is not a task (one
/// `std::thread` started) does not lose what it wrote. On a worker each
/// task has its own, in the thread-local while the task runs
/// ([`swap_pending`]): a task's flush sends its own writes only and never
/// waits for the write lock of a stream another task of the worker holds
/// while it is parked for room. The references are weak: the list must
/// not keep a stream alive, because dropping the last handle sends the
/// buffer and closes the socket at once (`Stream`'s `Drop`), not at the
/// next flush.
#[derive(Default)]
pub(crate) struct Pending(Vec<Weak<dyn Buffered>>);

impl Drop for Pending {
    fn drop(&mut self) {
        for stream in self.0.drain(..).filter_map(|weak| weak.upgrade()) {
            stream.flush_kept();
        }
    }
}

thread_local! {
    /// Streams the current task wrote to since it last flushed.
    static PENDING: RefCell<Pending> = const { RefCell::new(Pending(Vec::new())) };
}

/// Exchange the running thread's pending list with `list`: a worker puts a
/// task's list in place when it resumes the task and takes it back when
/// the task parks or ends (`crate::sched`).
pub(crate) fn swap_pending(list: &mut Pending) {
    let _ = PENDING.try_with(|pending| std::mem::swap(&mut *pending.borrow_mut(), list));
}

/// Register `stream` with the current task's pending list, once. `false`
/// when the thread's list is already gone (the thread is ending): the
/// caller sends at once.
pub(crate) fn register<T: Buffered + 'static>(stream: &Arc<T>) -> bool {
    PENDING
        .try_with(|pending| {
            let pending = &mut pending.borrow_mut().0;
            if !pending
                .iter()
                .any(|known| std::ptr::addr_eq(Weak::as_ptr(known), Arc::as_ptr(stream)))
            {
                let weak: Weak<T> = Arc::downgrade(stream);
                pending.push(weak);
            }
        })
        .is_ok()
}

/// How long [`flush_at_exit`] waits in all, across every stream, for write
/// locks other tasks hold and for room in socket buffers.
pub(crate) const EXIT_FLUSH: Duration = Duration::from_millis(100);

/// Exit's flush ([`crate::exit`]): send what the current task wrote,
/// waiting for other tasks' write locks and for room in socket buffers
/// only until [`EXIT_FLUSH`] after the call. A stream whose lock is still
/// held then (a task stalled in a send to a peer that does not read) is
/// skipped; a socket that still has no room keeps what was not sent. The
/// rest is lost with the process.
pub(crate) fn flush_at_exit() {
    let deadline = Instant::now() + EXIT_FLUSH;
    let pending = PENDING
        .try_with(|pending| std::mem::take(&mut pending.borrow_mut().0))
        .unwrap_or_default();
    for stream in pending.into_iter().filter_map(|weak| weak.upgrade()) {
        stream.flush_at_exit(deadline);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The lock `held`, waited for until `deadline` at most.
pub(crate) fn lock_until<T>(held: &Lock<T>, deadline: Instant) -> Option<LockGuard<'_, T>> {
    loop {
        if let Some(guard) = held.try_lock() {
            return Some(guard);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_micros(100));
    }
}

/// Send everything the current task wrote and has not sent yet, to
/// sockets and to pipes. A failed send is kept with its stream and
/// reported by the stream's next `read`, `write_all` or `close`.
pub fn flush() {
    let pending = PENDING
        .try_with(|pending| std::mem::take(&mut pending.borrow_mut().0))
        .unwrap_or_default();
    for stream in pending.into_iter().filter_map(|weak| weak.upgrade()) {
        stream.flush_kept();
    }
}

impl Stream {
    fn out(&self) -> LockGuard<'_, Out> {
        self.writing.lock()
    }

    /// Make the socket's `SO_RCVTIMEO` the idle timeout, if it is not yet:
    /// before a `recv` that blocks. A timeout changed while such a `recv`
    /// waits holds from the next one, as the option itself does.
    fn apply_idle(&self) -> std::io::Result<()> {
        let ms = self.idle.load(Ordering::Acquire);
        // Relaxed: a cache of the option's value; two readers that both
        // find it stale set the same or a newer value.
        if self.receive_timeout.load(Ordering::Relaxed) == ms {
            return Ok(());
        }
        let timeout = u64::try_from(ms)
            .ok()
            .filter(|ms| *ms > 0)
            .map(Duration::from_millis);
        self.socket.set_read_timeout(timeout)?;
        self.receive_timeout.store(ms, Ordering::Relaxed);
        Ok(())
    }

    /// Send all of `bytes`, `send` sending a part of what is left at a
    /// time, waiting for room whenever a non-blocking socket has none:
    /// a task parks, a thread polls. The caller holds the write lock.
    fn send_all(
        &self,
        bytes: &[u8],
        mut send: impl FnMut(&[u8]) -> std::io::Result<usize>,
    ) -> Result<(), IoError> {
        let mut at = 0;
        while at < bytes.len() {
            let mark = self.readiness.mark(Interest::WRITE);
            match send(&bytes[at..]) {
                Ok(0) => return Err(std::io::Error::from(ErrorKind::WriteZero).into()),
                Ok(count) => at += count,
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    self.readiness
                        .wait(self.socket.as_fd(), Interest::WRITE, mark, None, None)?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        if !self.blocking && !bytes.is_empty() {
            // Relaxed: a hint for the first read ([`SPOKE_FIRST`]).
            let _ = self.emptied.compare_exchange(
                UNREAD,
                SPOKE_FIRST,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
        }
        Ok(())
    }

    /// Wait until the socket has room to send or `deadline` passes;
    /// whether it may have room (an interrupted wait counts as room, so
    /// the caller tries the send again).
    fn writable_before(&self, deadline: Instant) -> bool {
        use nix::poll::{PollFd, PollFlags, poll};
        use std::os::fd::AsFd;
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        let millis = u16::try_from(left.as_millis().max(1)).unwrap_or(u16::MAX);
        let mut fds = [PollFd::new(self.socket.as_fd(), PollFlags::POLLOUT)];
        match poll(&mut fds, millis) {
            Ok(ready) => ready > 0,
            Err(nix::errno::Errno::EINTR) => true,
            Err(_) => false,
        }
    }

    /// The flags of a `send`: the socket blocks in both IO modes, so in
    /// `readiness` every send is `MSG_DONTWAIT` and waits for room through
    /// the reactor; none raises `SIGPIPE`.
    fn send_flags(&self) -> MsgFlags {
        if self.blocking {
            MsgFlags::MSG_NOSIGNAL
        } else {
            MsgFlags::MSG_NOSIGNAL | MsgFlags::MSG_DONTWAIT
        }
    }

    /// Send the buffer; the caller holds the write lock.
    fn send(&self, out: &mut Out) -> Result<(), IoError> {
        if out.buffer.is_empty() {
            return Ok(());
        }
        let flags = self.send_flags();
        let descriptor = self.socket.as_raw_fd();
        let sent = self.send_all(&out.buffer, |rest| {
            nix::sys::socket::send(descriptor, rest, flags).map_err(std::io::Error::from)
        });
        out.buffer.clear();
        if out.buffer.capacity() > SEND_BUFFER {
            out.buffer = Vec::new();
        }
        sent
    }

    /// The failure of an earlier send, taken; the caller holds the write
    /// lock.
    fn take_failure(&self, out: &mut Out) -> Result<(), IoError> {
        match out.failed.take() {
            Some(error) => {
                self.failed.store(false, Ordering::Release);
                Err(error)
            }
            None => Ok(()),
        }
    }

    /// The failure of an earlier send, if any, without waiting for the
    /// write lock when there is none.
    fn check_failure(&self) -> Result<(), IoError> {
        if !self.failed.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut out = self.out();
        self.take_failure(&mut out)
    }
}

impl Buffered for Stream {
    fn flush_kept(&self) {
        let mut out = self.out();
        if let Err(error) = self.send(&mut out) {
            out.failed = Some(error);
            self.failed.store(true, Ordering::Release);
        }
    }

    fn flush_at_exit(&self, deadline: Instant) {
        let Some(mut out) = lock_until(&self.writing, deadline) else {
            return;
        };
        let mut at = 0;
        while at < out.buffer.len() {
            let rest = &out.buffer[at..];
            match nix::sys::socket::send(
                self.socket.as_raw_fd(),
                rest,
                MsgFlags::MSG_DONTWAIT | MsgFlags::MSG_NOSIGNAL,
            ) {
                Ok(count) => at += count,
                Err(Errno::EINTR) => {}
                Err(Errno::EAGAIN) if self.writable_before(deadline) => {}
                Err(_) => break,
            }
        }
        out.buffer.clear();
    }
}

/// The last handle is gone: send what was written, then the socket closes
/// with the struct (the peer reads the bytes, then end of stream), without
/// waiting for the writing task's next flush.
impl Drop for Stream {
    fn drop(&mut self) {
        let mut out = std::mem::take(self.writing.get_mut());
        let _ = self.send(&mut out);
    }
}

/// One `accept4` on `listener`: the connection, close-on-exec, and the
/// peer the call reported (one system call instead of `accept` and a
/// `getpeername`). The socket blocks in both IO modes: in `readiness` its
/// reads and sends are `MSG_DONTWAIT` unless a read may block its worker
/// (`sched::block_alone`).
fn accept_socket(
    listener: &std::net::TcpListener,
) -> std::io::Result<(std::net::TcpStream, Option<std::net::SocketAddr>)> {
    use nix::libc;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
    use std::os::fd::{FromRawFd, OwnedFd};
    let flags = libc::SOCK_CLOEXEC;
    // SAFETY: `storage` is zeroed (a valid `sockaddr_storage`) and `length`
    // holds its size, so `accept4` writes within it; a descriptor it
    // returns is new and owned by nothing else, so `OwnedFd` takes it; the
    // casts read `storage` as the address type its `ss_family` names, both
    // smaller than `sockaddr_storage` and fully written by `accept4` then.
    unsafe {
        let mut storage: libc::sockaddr_storage = std::mem::zeroed();
        let mut length = std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
        let descriptor = libc::accept4(
            listener.as_raw_fd(),
            (&raw mut storage).cast(),
            &raw mut length,
            flags,
        );
        if descriptor < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let socket = std::net::TcpStream::from(OwnedFd::from_raw_fd(descriptor));
        let peer = match i32::from(storage.ss_family) {
            libc::AF_INET => {
                let v4 = &*(&raw const storage).cast::<libc::sockaddr_in>();
                Some(SocketAddr::from((
                    Ipv4Addr::from(u32::from_be(v4.sin_addr.s_addr)),
                    u16::from_be(v4.sin_port),
                )))
            }
            libc::AF_INET6 => {
                let v6 = &*(&raw const storage).cast::<libc::sockaddr_in6>();
                Some(SocketAddr::from((
                    Ipv6Addr::from(v6.sin6_addr.s6_addr),
                    u16::from_be(v6.sin6_port),
                )))
            }
            _ => None,
        };
        Ok((socket, peer))
    }
}

impl TcpListener {
    /// Bind `address:port`. Port 0 picks a free port.
    pub fn bind(address: String, port: i64) -> Result<TcpListener, IoError> {
        Self::bind_with(address, port, false)
    }

    /// [`TcpListener::bind`] of a listener kept through the drain
    /// (`docs/slice.md` 16.13, story `drain-keeps-listeners`): SIGTERM does
    /// not release it, and it accepts until the process exits. The drain's
    /// wait for live tasks does not wait for a task blocked in its
    /// `accept` or in a read of a stream it accepted.
    pub fn bind_kept(address: String, port: i64) -> Result<TcpListener, IoError> {
        Self::bind_with(address, port, true)
    }

    /// [`TcpListener::bind`], kept through the drain when `kept`.
    pub fn bind_with(address: String, port: i64, kept: bool) -> Result<TcpListener, IoError> {
        let target = socket_address("bind", &address, port)?;
        let listener = std::net::TcpListener::bind(&target)
            .map_err(|e| IoError::new(format!("bind {target}: {e}")))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| IoError::new(format!("bind {target}: {e}")))?;
        // A socket Linux accepts has its listener's `TCP_NODELAY` and
        // `SO_OOBINLINE`, so `accept` sets neither: two system calls less
        // per connection, on the one thread every connection passes.
        nix::sys::socket::setsockopt(&listener, nix::sys::socket::sockopt::TcpNoDelay, &true)
            .and_then(|()| {
                nix::sys::socket::setsockopt(&listener, nix::sys::socket::sockopt::OobInline, &true)
            })
            .map_err(|e| IoError::new(format!("bind {target}: {e}")))?;
        let port = listener
            .local_addr()
            .map(|bound| i64::from(bound.port()))
            .unwrap_or(0);
        let slot = Arc::new(Slot {
            _counted: crate::census::Counted::new(crate::census::Kind::Listeners),
            listener: RwLock::new(Some(listener)),
            address,
            port,
            kept,
            acceptors: Mutex::new(WaitList::new()),
            readiness: Readiness::new(),
        });
        if !kept {
            crate::drain::register(&slot);
        }
        Ok(TcpListener { slot })
    }

    /// The next connection. Flushes the current task's writes first. An
    /// interrupted wait is retried at once. A failure that passes with
    /// time ([`transient`]: out of descriptors, buffers or memory, or a
    /// connection aborted before it was accepted) is retried after a pause
    /// of [`ACCEPT_BACKOFF_FIRST`], doubling up to [`ACCEPT_BACKOFF_LAST`];
    /// the first failure of a run is logged once. Any other failure is
    /// returned. While the process drains (`crate::drain`), and after the
    /// drain closed the listener, the error is "draining"; a listener kept
    /// through the drain ([`TcpListener::bind_kept`]) accepts on.
    pub fn accept(&self) -> Result<TcpStream, IoError> {
        flush();
        // Only `Slot::release` takes the write lock: a read lock that is
        // not free at once means the drain is closing the listener. A
        // blocking `read` here could stop a worker whose other task holds
        // the read lock, parked in `accept`, while the drain waits for it.
        let guard = match self.slot.listener.try_read() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => return Err(IoError::new(crate::drain::DRAINING)),
        };
        let Some(listener) = guard.as_ref() else {
            return Err(IoError::new(crate::drain::DRAINING));
        };
        let kept = self.slot.kept;
        // Off a worker (the main thread's accept loop of a compiled server,
        // a task of the IO mode `threads`) the wait is a `poll`, which sees
        // a connection already waiting: it comes first, so no `accept4`
        // fails before it (`poll` and `accept4` per connection, not a
        // failed `accept4` as well whenever the next has not arrived yet).
        if crate::sched::running().is_none() {
            if !kept && crate::drain::draining() {
                return Err(IoError::new(crate::drain::DRAINING));
            }
            self.slot.wait_for_connection(listener, 0)?;
        }
        let mut pause = ACCEPT_BACKOFF_FIRST;
        let mut failing = false;
        loop {
            let mark = self.slot.readiness.mark(Interest::READ);
            match accept_socket(listener) {
                Ok((socket, peer)) => return Ok(TcpStream::over(socket, peer, kept)),
                // The two codes every accept loop meets compared as numbers,
                // not through `ErrorKind`'s decoding of every code.
                Err(e) if e.raw_os_error() == Some(nix::libc::EINTR) => continue,
                Err(e) if e.raw_os_error() == Some(nix::libc::EAGAIN) => {
                    if !kept && crate::drain::draining() {
                        return Err(IoError::new(crate::drain::DRAINING));
                    }
                    self.slot.wait_for_connection(listener, mark)?;
                }
                Err(e) if transient(&e) => {
                    if !failing {
                        crate::log(&format!("accept: {e}; retrying"));
                        failing = true;
                    }
                    crate::wait::sleep(pause);
                    pause = (pause * 2).min(ACCEPT_BACKOFF_LAST);
                }
                Err(_) if !kept && crate::drain::draining() => {
                    return Err(IoError::new(crate::drain::DRAINING));
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// The port bound; 0 only if the operating system could not report it.
    pub fn local_port(&self) -> i64 {
        self.slot.port
    }
}

impl TcpStream {
    /// A stream over a socket this process connected: `TCP_NODELAY` and
    /// `SO_OOBINLINE` are set here (an accepted socket has its listener's,
    /// [`TcpListener::bind_with`]).
    fn from_socket(socket: std::net::TcpStream) -> TcpStream {
        let peer = socket.peer_addr().ok();
        let _ = socket.set_nodelay(true);
        let _ = nix::sys::socket::setsockopt(&socket, nix::sys::socket::sockopt::OobInline, &true);
        Self::over(socket, peer, false)
    }

    /// A stream over `socket`, a blocking one in both IO modes (`Stream`)
    /// that has `TCP_NODELAY` and `SO_OOBINLINE` already, with the peer the
    /// operating system reported; `kept` when a listener kept through the
    /// drain accepted it.
    fn over(
        socket: std::net::TcpStream,
        peer: Option<std::net::SocketAddr>,
        kept: bool,
    ) -> TcpStream {
        let blocking = !crate::sched::readiness();
        let peer = peer
            .map(|peer| (peer.ip().to_canonical().to_string(), i64::from(peer.port())))
            .unwrap_or_default();
        TcpStream {
            inner: Arc::new(Stream {
                readiness: Readiness::new(),
                _counted: crate::census::Counted::new(crate::census::Kind::Streams),
                socket,
                reading: Lock::new(()),
                writing: Lock::new(Out::default()),
                blocking,
                emptied: AtomicU64::new(UNREAD),
                idle: AtomicI64::new(0),
                receive_timeout: AtomicI64::new(0),
                failed: AtomicBool::new(false),
                closed: AtomicBool::new(false),
                timed_out: AtomicBool::new(false),
                peer,
                kept,
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn socket(&self) -> &std::net::TcpStream {
        &self.inner.socket
    }

    /// Connect to `address:port`. Flushes the current task's writes first.
    pub fn connect(address: String, port: i64) -> Result<TcpStream, IoError> {
        flush();
        let target = socket_address("connect", &address, port)?;
        let socket = std::net::TcpStream::connect(&target)
            .map_err(|e| IoError::new(format!("connect {target}: {e}")))?;
        Ok(TcpStream::from_socket(socket))
    }

    /// One read of up to `maximum` bytes. Zero bytes is `Input::Eof`.
    /// `maximum` is clamped to `1..=1 MiB`. Flushes the current task's
    /// writes first, then holds only the read lock.
    pub fn read(&self, maximum: i64) -> Result<Input, IoError> {
        let read = self.read_with(|receive| read_input(maximum, receive))?;
        Ok(read.unwrap_or(Input::Eof))
    }

    /// One read into `buffer`, as [`TcpStream::read`]: the number of bytes
    /// read, 0 at end of stream and when the idle timeout passed, which
    /// [`TcpStream::timed_out`] tells apart.
    pub fn read_into(&self, buffer: &mut [u8]) -> Result<usize, IoError> {
        let read = self.read_with(|receive| receive(buffer))?;
        Ok(read.unwrap_or(0))
    }

    /// One read, as [`TcpStream::read`], made by `attempt`: it is called
    /// with the socket's `recv`, and again after each wait
    /// when that found nothing to read (`ErrorKind::WouldBlock`, which
    /// `attempt` returns as it got it). `None` when the idle timeout
    /// passed (on a blocking socket: when `recv` timed out). Between
    /// attempts the caller holds no buffer, so a task
    /// that waits keeps no read buffer of its worker's (the LLVM
    /// runtime's reads into its arena or a buffer per thread).
    pub fn read_with<T>(
        &self,
        mut attempt: impl FnMut(
            &mut dyn FnMut(&mut [u8]) -> std::io::Result<usize>,
        ) -> std::io::Result<T>,
    ) -> Result<Option<T>, IoError> {
        flush();
        if let Err(error) = self.inner.check_failure() {
            self.inner.timed_out.store(false, Ordering::Release);
            return Err(error);
        }
        let _reading = self.inner.reading.lock();
        // Set under the read lock from this read's own outcome, so the flag
        // is that of the read that ended last, also with readers queued on
        // the lock (`docs/slice.md` 26.2).
        let ended = |timed_out: bool| self.inner.timed_out.store(timed_out, Ordering::Release);
        let descriptor = self.inner.socket.as_raw_fd();
        // Whether the last `recv` of this read left the socket empty: it
        // read less than it asked for (but not the end of the stream), or
        // found nothing.
        let emptied = std::cell::Cell::new(false);
        // The next `recv` may block the thread (`sched::block_alone`, or
        // the IO mode `threads`); otherwise it is `MSG_DONTWAIT`. The
        // socket itself blocks in both modes.
        let block_next = std::cell::Cell::new(self.inner.blocking);
        let mut receive = |buffer: &mut [u8]| {
            let flags = if block_next.replace(self.inner.blocking) {
                MsgFlags::empty()
            } else {
                MsgFlags::MSG_DONTWAIT
            };
            let received = nix::sys::socket::recv(descriptor, buffer, flags);
            emptied.set(match &received {
                Ok(n) => *n > 0 && *n < buffer.len(),
                Err(errno) => *errno == Errno::EAGAIN,
            });
            received.map_err(std::io::Error::from)
        };
        let mut deadline: Option<Option<Instant>> = None;
        // The read before this one emptied the socket: wait for readiness
        // first, against the mark taken before that read, instead of a
        // `recv` that would find nothing (`Stream::emptied`); so does the
        // first read of a stream that sent first. `in_rhythm`: the stream
        // answers requests, its last read returned one and emptied the
        // socket. Only such a read may block its worker (below): a worker
        // blocked in `recv` takes no spawned task, and tasks do not move,
        // so a session that has not been sent anything yet (a burst of
        // connections opened before the first request) must not keep its
        // worker to itself while the rest of the burst is placed.
        let (mut wait_first, in_rhythm) = if self.inner.blocking {
            (None, false)
        } else {
            match self.inner.emptied.swap(NOT_EMPTIED, Ordering::Relaxed) {
                NOT_EMPTIED | UNREAD => (None, false),
                SPOKE_FIRST => (Some(0), false),
                mark => (Some(mark), true),
            }
        };
        loop {
            // A wait that this read would make now: either this task may
            // block its worker in the `recv` itself (it is the worker's only
            // task, the stream is in a request-reply rhythm, and the read's
            // idle deadline has not begun, so the socket's `SO_RCVTIMEO`,
            // set first, is that deadline), or it waits for readiness.
            let mut blocking = None;
            if let Some(mark) = wait_first.take() {
                blocking = (in_rhythm && deadline.is_none())
                    .then(crate::sched::block_alone)
                    .flatten();
                if blocking.is_some() && self.inner.apply_idle().is_err() {
                    blocking = None;
                }
                if blocking.is_none()
                    && let Some(ended_early) = self.wait_readable(mark, &mut deadline)
                {
                    ended(ended_early.is_ok());
                    return ended_early.map(|()| None);
                }
            }
            block_next.set(self.inner.blocking || blocking.is_some());
            let mark = self.inner.readiness.mark(Interest::READ);
            let attempted = if (self.inner.blocking || blocking.is_some()) && self.inner.kept {
                crate::task::idle_on_kept(|| attempt(&mut receive))
            } else {
                attempt(&mut receive)
            };
            let blocked = blocking.take().is_some();
            match attempted {
                Ok(value) => {
                    // A short read after an end of stream, an error or
                    // urgent data was reported may have left them behind,
                    // with no edge to come for them: the next read tries
                    // at once.
                    if !self.inner.blocking
                        && emptied.get()
                        && !self.inner.readiness.short_read_may_leave_input()
                    {
                        self.inner.emptied.store(mark, Ordering::Relaxed);
                    }
                    ended(false);
                    return Ok(Some(value));
                }
                Err(e) if e.raw_os_error() == Some(nix::libc::EINTR) => {}
                Err(e)
                    if (self.inner.blocking || blocked)
                        && matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    ended(true);
                    return Ok(None);
                }
                Err(e) if e.raw_os_error() == Some(nix::libc::EAGAIN) => {
                    wait_first = Some(mark);
                }
                Err(e) => {
                    ended(false);
                    return Err(e.into());
                }
            }
        }
    }

    /// The wait of [`TcpStream::read_with`] for the socket to become
    /// readable after `mark`. `None`: attempt the read. `Some(Ok(()))`:
    /// the idle timeout passed. `Some(Err(_))`: the wait failed. The first
    /// wait of a read computes its deadline, reading the clock once.
    fn wait_readable(
        &self,
        mark: u64,
        deadline: &mut Option<Option<Instant>>,
    ) -> Option<Result<(), IoError>> {
        let mut now = None;
        let until = *deadline.get_or_insert_with(|| {
            let (until, at) = self.idle_deadline();
            now = at;
            until
        });
        let socket = &self.inner.socket;
        let wait = || {
            self.inner
                .readiness
                .wait(socket.as_fd(), Interest::READ, mark, until, now)
        };
        let waited = if self.inner.kept {
            crate::task::idle_on_kept(wait)
        } else {
            wait()
        };
        match waited {
            Ok(DescriptorWait::Deadline) => Some(Ok(())),
            Ok(_) => None,
            Err(e) => Some(Err(e.into())),
        }
    }

    /// When a read that begins to wait now ends for the idle timeout, and
    /// the instant read for it; neither without an idle timeout.
    fn idle_deadline(&self) -> (Option<Instant>, Option<Instant>) {
        let ms = self.inner.idle.load(Ordering::Acquire);
        let Some(ms) = u64::try_from(ms).ok().filter(|ms| *ms > 0) else {
            return (None, None);
        };
        let now = Instant::now();
        (now.checked_add(Duration::from_millis(ms)), Some(now))
    }

    /// The idle timeout in milliseconds, 0 for none, and the socket, whose
    /// `SO_RCVTIMEO` carries the timeout for a read that blocks in `recv`
    /// (`crate::idle`). Read by a TLS session, which restores the timeout
    /// after its handshake, and by tests.
    #[cfg(any(test, feature = "tls"))]
    pub(crate) fn idle_ms(&self) -> (&AtomicI64, &std::net::TcpStream) {
        (&self.inner.idle, &self.inner.socket)
    }

    /// Set the idle timeout to `ms` milliseconds, 0 for none. A socket
    /// whose reads block in `recv` (IO mode `threads`) gets it as
    /// `SO_RCVTIMEO` now; in the mode `readiness` only a read that blocks
    /// its worker needs the option and sets it then ([`Stream::apply_idle`]),
    /// so a stream whose reads all wait for readiness never makes the call.
    pub(crate) fn store_idle(&self, ms: i64) -> std::io::Result<()> {
        self.inner.idle.store(ms, Ordering::Release);
        if self.inner.blocking {
            self.inner.apply_idle()
        } else {
            Ok(())
        }
    }

    /// Whether the last read of the stream, on any handle, returned end of
    /// stream because the idle timeout passed rather than because the peer
    /// closed (`docs/slice.md` 26.2). False before the first read; every
    /// read sets it anew. Never waits.
    pub fn timed_out(&self) -> bool {
        self.inner.timed_out.load(Ordering::Acquire)
    }

    /// The other end's address and port (`docs/slice.md` 26.1): the
    /// address as text the way its family writes it (`127.0.0.1`, `::1`;
    /// an IPv4 peer of an IPv6 socket as IPv4), read when the stream was
    /// accepted or connected and kept after `close`. `("", 0)` when the
    /// operating system could not report it then.
    pub fn peer(&self) -> (String, i64) {
        self.inner.peer.clone()
    }

    /// This end's address and port: the
    /// address the connection arrived at or left from, written as
    /// [`TcpStream::peer`] writes its address, asked of the operating
    /// system at each call (`getsockname`), so a stream that never asks
    /// pays nothing. `("", 0)` when the operating system cannot report it.
    pub fn local(&self) -> (String, i64) {
        self.inner
            .socket
            .local_addr()
            .map(|local| {
                (
                    local.ip().to_canonical().to_string(),
                    i64::from(local.port()),
                )
            })
            .unwrap_or_default()
    }

    /// [`TcpStream::connect`] that gives up after `timeout_ns`
    /// nanoseconds (`docs/slice.md` 26.3). Each address the name resolves
    /// to is tried in turn with the time left; resolving the name is not
    /// bounded. A timeout that is not positive is refused.
    pub fn connect_within(
        address: String,
        port: i64,
        timeout_ns: i64,
    ) -> Result<TcpStream, IoError> {
        use std::net::ToSocketAddrs;
        use std::time::Instant;
        flush();
        let target = socket_address("connect", &address, port)?;
        let fail = |e: &dyn std::fmt::Display| IoError::new(format!("connect {target}: {e}"));
        let timeout = u64::try_from(timeout_ns)
            .ok()
            .filter(|ns| *ns > 0)
            .map(Duration::from_nanos)
            .ok_or_else(|| fail(&"the timeout must be positive"))?;
        let deadline = Instant::now() + timeout;
        let mut last: Option<std::io::Error> = None;
        for candidate in target.to_socket_addrs().map_err(|e| fail(&e))? {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                last = Some(ErrorKind::TimedOut.into());
                break;
            }
            match std::net::TcpStream::connect_timeout(&candidate, left) {
                Ok(socket) => return Ok(TcpStream::from_socket(socket)),
                Err(e) => last = Some(e),
            }
        }
        Err(match last {
            Some(e) => fail(&e),
            None => fail(&"no address"),
        })
    }

    /// Write all of `data`: append it to the send buffer (module docs).
    /// Holds only the write lock. After `close` on any handle of the
    /// stream the write is refused with `write to a closed stream`; the
    /// error is the writer's and is not kept for another task's `read`.
    pub fn write_all(&self, data: Vec<u8>) -> Result<(), IoError> {
        self.append(|buffer| {
            if buffer.is_empty() && buffer.capacity() < data.len() {
                *buffer = data;
            } else {
                buffer.extend_from_slice(&data);
            }
        })
    }

    /// [`TcpStream::write_all`] of a borrowed slice.
    pub fn write_from(&self, data: &[u8]) -> Result<(), IoError> {
        self.append(|buffer| buffer.extend_from_slice(data))
    }

    fn append(&self, add: impl FnOnce(&mut Vec<u8>)) -> Result<(), IoError> {
        let stream = &self.inner;
        let mut out = stream.out();
        if stream.closed.load(Ordering::Acquire) {
            return Err(IoError::new("write to a closed stream"));
        }
        stream.take_failure(&mut out)?;
        add(&mut out.buffer);
        if out.buffer.len() >= SEND_BUFFER {
            return stream.send(&mut out);
        }
        drop(out);
        if !register(stream) {
            let mut out = stream.out();
            return stream.send(&mut out);
        }
        Ok(())
    }

    /// Send what was written, then shut down the write half; a later
    /// `write_all` on any handle is the writer's error. Reads can continue
    /// until the peer closes. A socket that is already disconnected is not
    /// an error.
    pub fn close(&self) -> Result<(), IoError> {
        let stream = &self.inner;
        let mut out = stream.out();
        stream.closed.store(true, Ordering::Release);
        stream.take_failure(&mut out)?;
        stream.send(&mut out)?;
        match stream.socket.shutdown(Shutdown::Write) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotConnected => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Write `data` as urgent data (module docs): what was written before
    /// and `data` are sent at once, in one `send` with `MSG_OOB`, the last
    /// byte of `data` as the urgent byte. Empty `data` sends what was
    /// written and marks nothing. Refused after `close` as `write_all` is.
    pub fn write_urgent(&self, data: Vec<u8>) -> Result<(), IoError> {
        self.write_urgent_from(&data)
    }

    /// [`TcpStream::write_urgent`] of a borrowed slice.
    pub fn write_urgent_from(&self, data: &[u8]) -> Result<(), IoError> {
        let stream = &self.inner;
        let mut out = stream.out();
        if stream.closed.load(Ordering::Acquire) {
            return Err(IoError::new("write to a closed stream"));
        }
        stream.take_failure(&mut out)?;
        if data.is_empty() {
            return stream.send(&mut out);
        }
        out.buffer.extend_from_slice(data);
        // One `send` with `MSG_OOB`, so the urgent pointer is after its last
        // byte: one call, one segment for a short message, the urgent byte
        // arriving with the bytes before it. `send_all` sends what a short
        // send left the same way, which moves the pointer to the same byte.
        let flags = stream.send_flags() | MsgFlags::MSG_OOB;
        let descriptor = stream.socket.as_raw_fd();
        let sent = stream.send_all(&out.buffer, |rest| {
            nix::sys::socket::send(descriptor, rest, flags).map_err(std::io::Error::from)
        });
        out.buffer.clear();
        if out.buffer.capacity() > SEND_BUFFER {
            out.buffer = Vec::new();
        }
        sent
    }

    /// Whether urgent data is ahead of the reader: an urgent byte has
    /// arrived and no read has returned it yet. A read stops before the
    /// urgent byte, so the bytes a read returns while this holds came
    /// before it. Never waits.
    pub fn urgent_pending(&self) -> Result<bool, IoError> {
        use nix::poll::{PollFd, PollFlags, poll};
        use std::os::fd::AsFd;
        let mut fds = [PollFd::new(self.inner.socket.as_fd(), PollFlags::POLLPRI)];
        poll(&mut fds, 0u16).map_err(|e| IoError::new(format!("urgent: {e}")))?;
        Ok(fds[0]
            .revents()
            .is_some_and(|events| events.contains(PollFlags::POLLPRI)))
    }
}

/// `Endpoint.of(ref)` (`docs/slice.md` 16.9, 16.13): the address and port
/// of the `--ref <path>=<address>:<port>` process argument whose path is
/// `reference`'s text; otherwise the IoError "no binding for <path>". The
/// driver `ignite run` passes one `--ref` per reference the program
/// resolves; outside it every reference is unbound
/// ([`crate::args::endpoint_from_args`]).
pub fn endpoint_of(reference: &str) -> Result<(String, i64), IoError> {
    crate::args::endpoint_from_os_args(std::env::args_os(), reference)
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;

    use super::transient;

    #[test]
    fn descriptor_and_memory_shortages_are_transient() {
        for code in [23, 24] {
            assert!(
                transient(&std::io::Error::from_raw_os_error(code)),
                "{code}"
            );
        }
        assert!(transient(&std::io::ErrorKind::ConnectionAborted.into()));
        assert!(!transient(&std::io::ErrorKind::InvalidInput.into()));
        assert!(!transient(&std::io::Error::from_raw_os_error(9)));
    }

    #[test]
    fn pending_network_errors_of_accept_are_transient() {
        use nix::errno::Errno;
        for errno in [
            Errno::ENOBUFS,
            Errno::ENOMEM,
            Errno::ECONNABORTED,
            Errno::EPROTO,
            Errno::ENETDOWN,
            Errno::EHOSTUNREACH,
            Errno::ENETUNREACH,
            Errno::ENONET,
        ] {
            assert!(
                transient(&std::io::Error::from_raw_os_error(errno as i32)),
                "{errno}"
            );
        }
        for errno in [Errno::EBADF, Errno::EINVAL, Errno::ENOTSOCK, Errno::EPERM] {
            assert!(
                !transient(&std::io::Error::from_raw_os_error(errno as i32)),
                "{errno}"
            );
        }
    }

    /// An accepted socket comes from one `accept4`: blocking in both IO
    /// modes (in `readiness` its reads and sends say `MSG_DONTWAIT`),
    /// close-on-exec, and with the peer the call reported, the connecting
    /// socket's own address.
    #[test]
    fn an_accepted_socket_blocks_is_close_on_exec_and_has_its_peer() {
        use nix::fcntl::{FcntlArg, FdFlag, OFlag, fcntl};
        let listener = super::TcpListener::bind("127.0.0.1".to_string(), 0).unwrap();
        let client =
            std::net::TcpStream::connect(("127.0.0.1", listener.local_port() as u16)).unwrap();
        let stream = listener.accept().unwrap();
        let socket = stream.socket();
        let status = OFlag::from_bits_truncate(fcntl(socket, FcntlArg::F_GETFL).unwrap());
        assert!(!status.contains(OFlag::O_NONBLOCK));
        let descriptor = FdFlag::from_bits_truncate(fcntl(socket, FcntlArg::F_GETFD).unwrap());
        assert!(descriptor.contains(FdFlag::FD_CLOEXEC));
        let local = client.local_addr().unwrap();
        assert_eq!(
            stream.peer(),
            (local.ip().to_string(), i64::from(local.port()))
        );
        // Both inherited from the listener (`TcpListener::bind_with`).
        assert!(
            nix::sys::socket::getsockopt(socket, nix::sys::socket::sockopt::OobInline).unwrap()
        );
        assert!(
            nix::sys::socket::getsockopt(socket, nix::sys::socket::sockopt::TcpNoDelay).unwrap()
        );
    }

    /// `TcpStream::local`: an accepted
    /// stream's local end is the listener's address and port, a connected
    /// one's the address and port the client socket left from, in both
    /// families, and it stays known after `close`.
    #[test]
    fn a_stream_knows_its_local_end_in_both_families() {
        for address in ["127.0.0.1", "::1"] {
            let listener = super::TcpListener::bind(address.to_string(), 0).unwrap();
            let port = listener.local_port();
            let client = super::TcpStream::connect(address.to_string(), port).unwrap();
            let accepted = listener.accept().unwrap();
            assert_eq!(accepted.local(), (address.to_string(), port));
            assert_eq!(client.local(), accepted.peer());
            accepted.close().unwrap();
            assert_eq!(accepted.local(), (address.to_string(), port));
        }
    }

    /// `Slot::release` (the drain, 16.13): a pending `accept` is woken
    /// and fails, the port is free again at once (the next generation
    /// binds it), the line names the bound address and port, and a second
    /// release has nothing to say. The drain mode itself is not set here:
    /// it is process-wide, and this test binary accepts elsewhere.
    #[test]
    fn a_released_listener_wakes_its_accept_and_frees_the_port() {
        let listener = super::TcpListener::bind("127.0.0.1".to_string(), 0).unwrap();
        let port = listener.local_port();
        assert!(port > 0);
        let accepting = listener.clone();
        let (started, go) = std::sync::mpsc::channel();
        let pending = std::thread::spawn(move || {
            started.send(()).unwrap();
            accepting.accept().map(|_| ())
        });
        go.recv().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let line = listener.slot.release();
        assert_eq!(line, Some(format!("released 127.0.0.1:{port}")));
        assert!(
            pending.join().unwrap().is_err(),
            "the pending accept failed"
        );
        assert!(listener.accept().is_err(), "a later accept fails too");
        assert_eq!(listener.local_port(), port, "the port stays known");
        let again = super::TcpListener::bind("127.0.0.1".to_string(), port)
            .unwrap_or_else(|e| panic!("the port is free again: {e}"));
        assert_eq!(again.local_port(), port);
        assert_eq!(listener.slot.release(), None);
        assert!(again.slot.release().is_some());
    }

    /// A connected pair: the stream under test and its peer, which never
    /// reads unless the test does.
    fn pair() -> (super::TcpStream, std::net::TcpStream) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (peer, _) = listener.accept().unwrap();
        (super::TcpStream::from_socket(client), peer)
    }

    /// `write_all(b"bye")`, then `between`, then `flush_at_exit`, on a
    /// thread of its own (the pending list is the thread's); whether the
    /// flush returned within 5 s.
    fn exit_flush_returns(
        stream: &super::TcpStream,
        between: impl FnOnce() + Send + 'static,
    ) -> bool {
        let stream = stream.clone();
        let (done, returned) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            stream.write_all(b"bye".to_vec()).unwrap();
            between();
            super::flush_at_exit();
            let _ = done.send(());
        });
        returned
            .recv_timeout(std::time::Duration::from_secs(5))
            .is_ok()
    }

    /// Story `drain-exit`: the exit's flush sends what the task wrote.
    #[test]
    fn the_exit_flush_sends_the_pending_writes() {
        let (stream, mut peer) = pair();
        assert!(exit_flush_returns(&stream, || {}));
        let mut received = [0u8; 3];
        std::io::Read::read_exact(&mut peer, &mut received).unwrap();
        assert_eq!(&received, b"bye");
    }

    /// Story `drain-exit`: the exit's flush waits at most its deadline for
    /// a write lock another task holds (a task stalled in its send), then
    /// skips the stream. The holder takes the lock only after `write_all`,
    /// inside `between`, so the write itself never waits for it.
    #[test]
    fn the_exit_flush_skips_a_stream_another_task_is_writing() {
        let (stream, _peer) = pair();
        let held = stream.clone();
        let (go, take) = std::sync::mpsc::channel::<()>();
        let (locked, is_locked) = std::sync::mpsc::channel();
        let (release, released) = std::sync::mpsc::channel::<()>();
        let holder = std::thread::spawn(move || {
            take.recv().unwrap();
            let _out = held.inner.out();
            locked.send(()).unwrap();
            let _ = released.recv();
        });
        let flushed = exit_flush_returns(&stream, move || {
            go.send(()).unwrap();
            is_locked.recv().unwrap();
        });
        assert!(flushed, "the exit's flush waited");
        release.send(()).unwrap();
        holder.join().unwrap();
    }

    /// Story `drain-exit`: the exit's flush does not wait for a peer that
    /// does not read: with the socket's buffers full it sends what fits
    /// and returns.
    #[test]
    fn the_exit_flush_does_not_wait_for_a_peer_that_does_not_read() {
        let (stream, _peer) = pair();
        fill(&stream);
        assert!(
            exit_flush_returns(&stream, || {}),
            "the exit's flush waited"
        );
    }

    /// Fill the socket's buffers with zeros until a send would block.
    fn fill(stream: &super::TcpStream) {
        let chunk = [0u8; 65536];
        loop {
            // SAFETY: the descriptor belongs to `stream`, which outlives
            // the call, and `chunk` is readable for its length.
            let sent = unsafe {
                nix::libc::send(
                    stream.socket().as_raw_fd(),
                    chunk.as_ptr().cast(),
                    chunk.len(),
                    nix::libc::MSG_DONTWAIT | nix::libc::MSG_NOSIGNAL,
                )
            };
            if sent < 0 {
                break;
            }
        }
    }

    /// Story `drain-exit`, ADV2-2: a writer that holds the lock briefly
    /// (one append, well inside the deadline) is waited for, and the
    /// task's write is sent.
    #[test]
    fn the_exit_flush_waits_for_a_lock_held_briefly() {
        let (stream, mut peer) = pair();
        let held = stream.clone();
        let (go, take) = std::sync::mpsc::channel::<()>();
        let (locked, is_locked) = std::sync::mpsc::channel();
        let holder = std::thread::spawn(move || {
            take.recv().unwrap();
            let out = held.inner.out();
            locked.send(()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
            drop(out);
        });
        let flushed = exit_flush_returns(&stream, move || {
            go.send(()).unwrap();
            is_locked.recv().unwrap();
        });
        assert!(flushed, "the exit's flush did not return");
        holder.join().unwrap();
        let mut received = [0u8; 3];
        peer.set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        std::io::Read::read_exact(&mut peer, &mut received)
            .unwrap_or_else(|e| panic!("the peer did not receive `bye`: {e}"));
        assert_eq!(&received, b"bye");
    }

    /// Story `drain-exit`, ADV2-2: a peer that reads, only later than the
    /// first send, gets everything when it reads within the deadline.
    #[test]
    fn the_exit_flush_sends_everything_to_a_peer_that_reads_late() {
        let (stream, mut peer) = pair();
        fill(&stream);
        let reader = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            peer.set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut received = Vec::new();
            let mut buffer = [0u8; 65536];
            while let Ok(count @ 1..) = std::io::Read::read(&mut peer, &mut buffer) {
                received.extend_from_slice(&buffer[..count]);
                if received.ends_with(b"bye") {
                    break;
                }
            }
            received.ends_with(b"bye")
        });
        assert!(
            exit_flush_returns(&stream, || {}),
            "the exit's flush waited"
        );
        assert!(reader.join().unwrap(), "the peer did not receive `bye`");
    }
}
