//! TCP over blocking `std::net` sockets.
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

use std::cell::RefCell;
use std::io::{ErrorKind, Read, Write};
use std::net::Shutdown;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, Weak};
use std::time::Duration;

use crate::{Input, IoError, input, read_buffer, socket_address};

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
    listener: RwLock<Option<std::net::TcpListener>>,
    address: String,
    port: i64,
}

impl Slot {
    fn read(&self) -> RwLockReadGuard<'_, Option<std::net::TcpListener>> {
        self.listener.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// Release the listener for the drain: shut its read side down, which
    /// wakes a pending `accept` with an error, then close it once that
    /// `accept` has let go of it. The `released <address>:<port>` line, or
    /// `None` when the listener was already closed.
    pub(crate) fn release(&self) -> Option<String> {
        {
            let guard = self.read();
            let listener = guard.as_ref()?;
            // SAFETY: the descriptor belongs to `listener`, which the read
            // lock keeps open for the call.
            unsafe { nix::libc::shutdown(listener.as_raw_fd(), nix::libc::SHUT_RD) };
        }
        let closed = self
            .listener
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .take()?;
        drop(closed);
        Some(format!("released {}:{}", self.address, self.port))
    }
}

#[derive(Clone, Debug)]
pub struct TcpStream {
    inner: Arc<Stream>,
}

#[derive(Debug)]
struct Stream {
    socket: std::net::TcpStream,
    reading: Mutex<()>,
    writing: Mutex<Out>,
    /// A send of the buffer failed; the error waits in `Out::failed`.
    failed: AtomicBool,
    /// `close` was called on some handle: a later `write_all` is the
    /// writer's error, not a failed send kept for the next reader.
    closed: AtomicBool,
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
}

/// Streams a thread wrote to since it last flushed; sent when the thread
/// ends, so a thread that is not a task (one `std::thread` started) does not
/// lose what it wrote. The references are weak: the list must not keep a
/// stream alive, because dropping the last handle sends the buffer and
/// closes the socket at once (`Stream`'s `Drop`), not at the next flush.
#[derive(Default)]
struct Pending(Vec<Weak<dyn Buffered>>);

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
    fn out(&self) -> MutexGuard<'_, Out> {
        self.writing.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Send the buffer; the caller holds the write lock.
    fn send(&self, out: &mut Out) -> Result<(), IoError> {
        if out.buffer.is_empty() {
            return Ok(());
        }
        let sent = (&self.socket).write_all(&out.buffer);
        out.buffer.clear();
        if out.buffer.capacity() > SEND_BUFFER {
            out.buffer = Vec::new();
        }
        sent.map_err(IoError::from)
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
}

/// The last handle is gone: send what was written, then the socket closes
/// with the struct (the peer reads the bytes, then end of stream), without
/// waiting for the writing task's next flush.
impl Drop for Stream {
    fn drop(&mut self) {
        let mut out = std::mem::take(
            self.writing
                .get_mut()
                .unwrap_or_else(PoisonError::into_inner),
        );
        let _ = self.send(&mut out);
    }
}

impl TcpListener {
    /// Bind `address:port`. Port 0 picks a free port.
    pub fn bind(address: String, port: i64) -> Result<TcpListener, IoError> {
        let target = socket_address("bind", &address, port)?;
        let listener = std::net::TcpListener::bind(&target)
            .map_err(|e| IoError::new(format!("bind {target}: {e}")))?;
        let port = listener
            .local_addr()
            .map(|bound| i64::from(bound.port()))
            .unwrap_or(0);
        let slot = Arc::new(Slot {
            listener: RwLock::new(Some(listener)),
            address,
            port,
        });
        crate::drain::register(&slot);
        Ok(TcpListener { slot })
    }

    /// The next connection. Flushes the current task's writes first. An
    /// interrupted wait is retried at once. A failure that passes with
    /// time ([`transient`]: out of descriptors, buffers or memory, or a
    /// connection aborted before it was accepted) is retried after a pause
    /// of [`ACCEPT_BACKOFF_FIRST`], doubling up to [`ACCEPT_BACKOFF_LAST`];
    /// the first failure of a run is logged once. Any other failure is
    /// returned. While the process drains (`crate::drain`), and after the
    /// drain closed the listener, the error is "draining".
    pub fn accept(&self) -> Result<TcpStream, IoError> {
        flush();
        let guard = self.slot.read();
        let Some(listener) = guard.as_ref() else {
            return Err(IoError::new(crate::drain::DRAINING));
        };
        let mut pause = ACCEPT_BACKOFF_FIRST;
        let mut failing = false;
        loop {
            match listener.accept() {
                Ok((socket, _peer)) => return Ok(TcpStream::from_socket(socket)),
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) if transient(&e) => {
                    if !failing {
                        crate::log(&format!("accept: {e}; retrying"));
                        failing = true;
                    }
                    std::thread::sleep(pause);
                    pause = (pause * 2).min(ACCEPT_BACKOFF_LAST);
                }
                Err(_) if crate::drain::draining() => {
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
    fn from_socket(socket: std::net::TcpStream) -> TcpStream {
        let _ = socket.set_nodelay(true);
        TcpStream {
            inner: Arc::new(Stream {
                socket,
                reading: Mutex::new(()),
                writing: Mutex::new(Out::default()),
                failed: AtomicBool::new(false),
                closed: AtomicBool::new(false),
            }),
        }
    }

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
        let mut buffer = read_buffer(maximum);
        let count = self.read_into(&mut buffer)?;
        Ok(input(buffer, count))
    }

    /// One read into `buffer`, as [`TcpStream::read`]: the number of bytes
    /// read, 0 at end of stream.
    pub fn read_into(&self, buffer: &mut [u8]) -> Result<usize, IoError> {
        flush();
        self.inner.check_failure()?;
        let _reading = self
            .inner
            .reading
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        loop {
            match (&self.inner.socket).read(buffer) {
                Ok(count) => return Ok(count),
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    return Ok(0);
                }
                Err(e) => return Err(e.into()),
            }
        }
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
}

/// `Endpoint.of(ref)` (`docs/slice.md` 16.9, 16.13): the address and port
/// of the `--ref <path>=<address>:<port>` process argument whose path is
/// `reference`'s text; otherwise the IoError "no binding for <path>". The
/// driver `ignite run` passes one `--ref` per reference the program
/// resolves; outside it every reference is unbound
/// ([`crate::args::endpoint_from_args`]).
pub fn endpoint_of(reference: &str) -> Result<(String, i64), IoError> {
    crate::args::endpoint_from_args(std::env::args(), reference)
}

#[cfg(test)]
mod tests {
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
}
