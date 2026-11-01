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
//! `read`, `write_all` or `close` of that stream.

use std::cell::RefCell;
use std::io::{ErrorKind, Read, Write};
use std::net::Shutdown;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use crate::{Input, IoError, input, read_buffer, socket_address};

/// Bytes a send buffer holds before `write_all` sends it at once.
pub const SEND_BUFFER: usize = 64 * 1024;

/// First pause of `accept` after a transient failure.
pub const ACCEPT_BACKOFF_FIRST: Duration = Duration::from_millis(10);
/// Longest pause of `accept` between retries.
pub const ACCEPT_BACKOFF_LAST: Duration = Duration::from_secs(1);

/// An `accept` failure that passes with time: the process or system is out
/// of file descriptors (`EMFILE`, `ENFILE`), buffers (`ENOBUFS`) or memory,
/// or the peer aborted the connection before it was accepted.
fn transient(error: &std::io::Error) -> bool {
    const ENFILE: i32 = 23;
    const EMFILE: i32 = 24;
    #[cfg(target_os = "linux")]
    const ENOBUFS: i32 = 105;
    #[cfg(not(target_os = "linux"))]
    const ENOBUFS: i32 = 55;
    matches!(
        error.kind(),
        ErrorKind::ConnectionAborted | ErrorKind::OutOfMemory
    ) || matches!(error.raw_os_error(), Some(ENFILE | EMFILE | ENOBUFS))
}

#[derive(Clone, Debug)]
pub struct TcpListener {
    inner: Arc<std::net::TcpListener>,
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
}

/// The write side of a stream: bytes written and not yet sent.
#[derive(Debug, Default)]
struct Out {
    buffer: Vec<u8>,
    failed: Option<IoError>,
}

/// Streams a thread wrote to since it last flushed; sent when the thread
/// ends, so a thread that is not a task (one `std::thread` started) does not
/// lose what it wrote.
#[derive(Default)]
struct Pending(Vec<Arc<Stream>>);

impl Drop for Pending {
    fn drop(&mut self) {
        for stream in self.0.drain(..) {
            stream.flush_kept();
        }
    }
}

thread_local! {
    /// Streams the current task wrote to since it last flushed.
    static PENDING: RefCell<Pending> = const { RefCell::new(Pending(Vec::new())) };
}

/// Send everything the current task wrote and has not sent yet. A failed
/// send is kept with its stream and reported by the stream's next `read`,
/// `write_all` or `close`.
pub fn flush() {
    let pending = PENDING
        .try_with(|pending| std::mem::take(&mut pending.borrow_mut().0))
        .unwrap_or_default();
    for stream in pending {
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

    /// Send the buffer, keeping a failure for the stream's next operation.
    fn flush_kept(&self) {
        let mut out = self.out();
        if let Err(error) = self.send(&mut out) {
            out.failed = Some(error);
            self.failed.store(true, Ordering::Release);
        }
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
        Ok(TcpListener {
            inner: Arc::new(listener),
        })
    }

    /// The next connection. Flushes the current task's writes first. An
    /// interrupted wait is retried at once. A failure that passes with
    /// time ([`transient`]: out of descriptors, buffers or memory, or a
    /// connection aborted before it was accepted) is retried after a pause
    /// of [`ACCEPT_BACKOFF_FIRST`], doubling up to [`ACCEPT_BACKOFF_LAST`];
    /// the first failure of a run is logged once. Any other failure is
    /// returned.
    pub fn accept(&self) -> Result<TcpStream, IoError> {
        flush();
        let mut pause = ACCEPT_BACKOFF_FIRST;
        let mut failing = false;
        loop {
            match self.inner.accept() {
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
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// The bound port; 0 only if the operating system cannot report it.
    pub fn local_port(&self) -> i64 {
        self.inner
            .local_addr()
            .map(|address| i64::from(address.port()))
            .unwrap_or(0)
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
    /// Holds only the write lock.
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
        stream.take_failure(&mut out)?;
        add(&mut out.buffer);
        if out.buffer.len() >= SEND_BUFFER {
            return stream.send(&mut out);
        }
        drop(out);
        let registered = PENDING.try_with(|pending| {
            let pending = &mut pending.borrow_mut().0;
            if !pending.iter().any(|known| Arc::ptr_eq(known, stream)) {
                pending.push(Arc::clone(stream));
            }
        });
        if registered.is_err() {
            let mut out = stream.out();
            return stream.send(&mut out);
        }
        Ok(())
    }

    /// Send what was written, then shut down the write half. Reads can
    /// continue until the peer closes. A socket that is already
    /// disconnected is not an error.
    pub fn close(&self) -> Result<(), IoError> {
        let stream = &self.inner;
        let mut out = stream.out();
        stream.take_failure(&mut out)?;
        stream.send(&mut out)?;
        match stream.socket.shutdown(Shutdown::Write) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotConnected => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
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
}
