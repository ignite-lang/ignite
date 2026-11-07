//! The in-memory stream pair (`docs/slice.md` 18.4, 18.8): what one end
//! writes, the other reads, in the same process.
//!
//! An end coalesces writes exactly as a `TcpStream` does: `write_all`
//! appends to the end's send buffer and registers the end with the writing
//! task, and the task's [`flush`] delivers it (before any read, on
//! `close`, at task end, at the end of `run`, at exit), as do a buffer that
//! reaches [`SEND_BUFFER`] and the drop of the end's last handle. A
//! delivery moves the whole buffer into the peer's receive queue in one
//! step and counts one batch there, which [`PipeStream::received_batches`]
//! reports.
//!
//! The receive queue is bounded by [`PIPE_CAPACITY`] unread bytes. A
//! delivery that does not fit fails as a whole and never blocks; so does a
//! delivery to a peer whose handles are all gone. A failed delivery made by
//! a flush is kept with the writing end and reported by its next `read`,
//! `write_all` or `close`; one made by `write_all` at the threshold or by
//! `close` is that call's error, as for a `TcpStream`.
//!
//! Urgent data mirrors a `TcpStream`'s (`crate::net`): `write_urgent`
//! delivers at once and marks its last byte in the peer's queue; a read
//! stops before the marked byte, and `urgent_pending` holds until a read
//! has returned it.
//!
//! Every handle of an end is a clone that shares the end: its queue, batch
//! counter, idle timeout, closed flag and kept failure. A read holds only
//! the end's read lock and its queue's lock while it waits on the queue's
//! condition variable, so a write to the same end does not wait for it.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::net::{Buffered, lock_until, register};
use crate::{Input, IoError, SEND_BUFFER, flush, read_input};

/// Unread bytes a receive queue holds before a delivery into it fails.
pub const PIPE_CAPACITY: usize = 16 * 1024 * 1024;

/// The kept error of a delivery into a full receive queue.
const QUEUE_FULL: &str = "pipe queue full";
/// The kept error of a delivery to an end whose handles are all gone.
const PEER_GONE: &str = "pipe peer is gone";

/// The in-memory stream pair.
#[derive(Clone, Copy, Debug)]
pub struct Pipe;

impl Pipe {
    /// Two connected ends: what the first writes the second reads, and
    /// the reverse.
    pub fn pair() -> (PipeStream, PipeStream) {
        let into_first = Arc::new(Queue::default());
        let into_second = Arc::new(Queue::default());
        (
            PipeStream::end(Arc::clone(&into_first), Arc::clone(&into_second)),
            PipeStream::end(into_second, into_first),
        )
    }
}

/// One end of a [`Pipe`]; clones share it.
#[derive(Clone, Debug)]
pub struct PipeStream {
    end: Arc<End>,
}

/// The bytes delivered into one end and not yet read.
#[derive(Debug, Default)]
struct Queue {
    state: Mutex<Received>,
    ready: Condvar,
}

#[derive(Debug, Default)]
struct Received {
    bytes: VecDeque<u8>,
    batches: i64,
    /// The writing end closed its write half or is gone: an empty queue
    /// reads as end of stream.
    ended: bool,
    /// The reading end is gone: a delivery into the queue fails.
    gone: bool,
    /// The urgent byte's index in `bytes`, until a read returns it.
    urgent: Option<usize>,
}

#[derive(Debug)]
struct End {
    /// What this end reads.
    incoming: Arc<Queue>,
    /// The peer's `incoming`, which this end's deliveries fill.
    outgoing: Arc<Queue>,
    reading: Mutex<()>,
    writing: Mutex<Out>,
    /// A delivery by a flush failed; the error waits in `Out::failed`.
    failed: AtomicBool,
    /// `close` was called on some handle.
    closed: AtomicBool,
    /// The idle timeout in milliseconds; 0 is none.
    idle: AtomicI64,
}

/// The write side of an end: bytes written and not yet delivered.
#[derive(Debug, Default)]
struct Out {
    buffer: Vec<u8>,
    failed: Option<IoError>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Queue {
    /// Move all of `buffer` into the queue as one batch, or nothing.
    /// `buffer` is empty afterwards either way.
    fn deliver(&self, buffer: &mut Vec<u8>) -> Result<(), IoError> {
        self.deliver_marked(buffer, false)
    }

    /// [`Queue::deliver`]; with `urgent`, the last byte of `buffer` becomes
    /// the queue's urgent byte, replacing an earlier one as a later urgent
    /// pointer does in TCP.
    fn deliver_marked(&self, buffer: &mut Vec<u8>, urgent: bool) -> Result<(), IoError> {
        if buffer.is_empty() {
            return Ok(());
        }
        let mut state = lock(&self.state);
        let delivered = if state.gone {
            Err(IoError::new(PEER_GONE))
        } else if state.bytes.len() + buffer.len() > PIPE_CAPACITY {
            Err(IoError::new(QUEUE_FULL))
        } else {
            if urgent {
                state.urgent = Some(state.bytes.len() + buffer.len() - 1);
            }
            state.bytes.extend(buffer.iter());
            state.batches += 1;
            self.ready.notify_all();
            Ok(())
        };
        drop(state);
        buffer.clear();
        if buffer.capacity() > SEND_BUFFER {
            *buffer = Vec::new();
        }
        delivered
    }

    /// The writing end will deliver nothing more.
    fn end(&self) {
        lock(&self.state).ended = true;
        self.ready.notify_all();
    }
}

impl End {
    fn out(&self) -> MutexGuard<'_, Out> {
        lock(&self.writing)
    }

    /// Deliver the buffer; the caller holds the write lock.
    fn send(&self, out: &mut Out) -> Result<(), IoError> {
        self.outgoing.deliver(&mut out.buffer)
    }

    /// The failure of an earlier delivery, taken; the caller holds the
    /// write lock.
    fn take_failure(&self, out: &mut Out) -> Result<(), IoError> {
        match out.failed.take() {
            Some(error) => {
                self.failed.store(false, Ordering::Release);
                Err(error)
            }
            None => Ok(()),
        }
    }

    /// The failure of an earlier delivery, if any, without waiting for the
    /// write lock when there is none.
    fn check_failure(&self) -> Result<(), IoError> {
        if !self.failed.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut out = self.out();
        self.take_failure(&mut out)
    }
}

impl Buffered for End {
    fn flush_kept(&self) {
        let mut out = self.out();
        if let Err(error) = self.send(&mut out) {
            out.failed = Some(error);
            self.failed.store(true, Ordering::Release);
        }
    }

    /// A delivery never waits for room (a full queue is an error), so
    /// only the write lock could hold up the exit.
    fn flush_at_exit(&self, deadline: Instant) {
        if let Some(mut out) = lock_until(&self.writing, deadline) {
            let _ = self.send(&mut out);
        }
    }
}

/// The last handle of an end is gone: deliver what was written, close the
/// write half, and fail every later delivery into this end's queue.
impl Drop for End {
    fn drop(&mut self) {
        let mut out = std::mem::take(
            self.writing
                .get_mut()
                .unwrap_or_else(PoisonError::into_inner),
        );
        let _ = self.send(&mut out);
        self.outgoing.end();
        let mut incoming = lock(&self.incoming.state);
        incoming.gone = true;
        incoming.bytes = VecDeque::new();
        incoming.urgent = None;
    }
}

impl PipeStream {
    fn end(incoming: Arc<Queue>, outgoing: Arc<Queue>) -> PipeStream {
        PipeStream {
            end: Arc::new(End {
                incoming,
                outgoing,
                reading: Mutex::new(()),
                writing: Mutex::new(Out::default()),
                failed: AtomicBool::new(false),
                closed: AtomicBool::new(false),
                idle: AtomicI64::new(0),
            }),
        }
    }

    /// One read of up to `maximum` bytes, clamped to `1..=1 MiB`. Zero
    /// bytes is `Input::Eof`. Flushes the current task's writes first.
    pub fn read(&self, maximum: i64) -> Result<Input, IoError> {
        read_input(maximum, |buffer| self.read_into(buffer))
    }

    /// One read into `buffer`, as [`PipeStream::read`]: the number of bytes
    /// read, 0 at end of stream. Waits while the queue is empty and the
    /// peer's write half is open, up to the idle timeout. One read may
    /// take bytes of several deliveries.
    pub fn read_into(&self, buffer: &mut [u8]) -> Result<usize, IoError> {
        flush();
        let end = &self.end;
        end.check_failure()?;
        let _reading = lock(&end.reading);
        let deadline = idle(end.idle.load(Ordering::Acquire)).map(|wait| Instant::now() + wait);
        let queue = &end.incoming;
        let mut state = lock(&queue.state);
        loop {
            if !state.bytes.is_empty() {
                let mut count = buffer.len().min(state.bytes.len());
                if let Some(mark) = state.urgent.filter(|mark| *mark > 0) {
                    count = count.min(mark);
                }
                state.urgent = state.urgent.and_then(|mark| mark.checked_sub(count));
                let (front, back) = state.bytes.as_slices();
                let split = front.len().min(count);
                buffer[..split].copy_from_slice(&front[..split]);
                buffer[split..count].copy_from_slice(&back[..count - split]);
                state.bytes.drain(..count);
                if state.bytes.is_empty() && state.bytes.capacity() > SEND_BUFFER {
                    state.bytes = VecDeque::new();
                }
                return Ok(count);
            }
            if state.ended || buffer.is_empty() {
                return Ok(0);
            }
            state = match deadline {
                None => queue
                    .ready
                    .wait(state)
                    .unwrap_or_else(PoisonError::into_inner),
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        return Ok(0);
                    }
                    queue
                        .ready
                        .wait_timeout(state, deadline - now)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
            };
        }
    }

    /// Write all of `data`: append it to the send buffer (module docs).
    /// After `close` on any handle of the end the write is refused with
    /// `write to a closed stream`; that error is the writer's and is not
    /// kept.
    pub fn write_all(&self, data: Vec<u8>) -> Result<(), IoError> {
        self.append(|buffer| {
            if buffer.is_empty() && buffer.capacity() < data.len() {
                *buffer = data;
            } else {
                buffer.extend_from_slice(&data);
            }
        })
    }

    /// [`PipeStream::write_all`] of a borrowed slice.
    pub fn write_from(&self, data: &[u8]) -> Result<(), IoError> {
        self.append(|buffer| buffer.extend_from_slice(data))
    }

    fn append(&self, add: impl FnOnce(&mut Vec<u8>)) -> Result<(), IoError> {
        let end = &self.end;
        let mut out = end.out();
        if end.closed.load(Ordering::Acquire) {
            return Err(IoError::new("write to a closed stream"));
        }
        end.take_failure(&mut out)?;
        add(&mut out.buffer);
        if out.buffer.len() >= SEND_BUFFER {
            return end.send(&mut out);
        }
        drop(out);
        if !register(end) {
            let mut out = end.out();
            return end.send(&mut out);
        }
        Ok(())
    }

    /// Deliver what was written, then close the write half: the peer
    /// reads the queue, then end of stream. A later `write_all` on any
    /// handle is the writer's error; reads continue. A second `close` is
    /// not an error. The write half closes also when the kept failure or
    /// the delivery fails, and the call returns that error.
    pub fn close(&self) -> Result<(), IoError> {
        let end = &self.end;
        let mut out = end.out();
        end.closed.store(true, Ordering::Release);
        let closed = end.take_failure(&mut out).and_then(|()| end.send(&mut out));
        if closed.is_err() {
            out.buffer = Vec::new();
        }
        end.outgoing.end();
        closed
    }

    /// Write `data` as urgent data, as `TcpStream::write_urgent`: what was
    /// written before and `data` are delivered at once as one batch, the
    /// last byte of `data` as the peer's urgent byte. The peer's reads stop
    /// before it, and its [`PipeStream::urgent_pending`] holds until a read
    /// returns it. Empty `data` delivers what was written and marks
    /// nothing. Refused after `close` as `write_all` is.
    pub fn write_urgent(&self, data: Vec<u8>) -> Result<(), IoError> {
        self.write_urgent_from(&data)
    }

    /// [`PipeStream::write_urgent`] of a borrowed slice.
    pub fn write_urgent_from(&self, data: &[u8]) -> Result<(), IoError> {
        let end = &self.end;
        let mut out = end.out();
        if end.closed.load(Ordering::Acquire) {
            return Err(IoError::new("write to a closed stream"));
        }
        end.take_failure(&mut out)?;
        out.buffer.extend_from_slice(data);
        end.outgoing
            .deliver_marked(&mut out.buffer, !data.is_empty())
    }

    /// Whether urgent data is ahead of this end's reader: the peer's last
    /// `write_urgent` was delivered and no read has returned its urgent
    /// byte yet. Never waits.
    pub fn urgent_pending(&self) -> Result<bool, IoError> {
        Ok(lock(&self.end.incoming.state).urgent.is_some())
    }

    /// Set the idle timeout of this end to `ms` milliseconds: a read that
    /// waits longer reads as end of stream. 0 or less removes it. Clones
    /// share it.
    pub fn set_idle_timeout(&self, ms: i64) -> Result<(), IoError> {
        self.end.idle.store(ms.max(0), Ordering::Release);
        Ok(())
    }

    /// The deliveries into this end's queue since [`Pipe::pair`]; reads do
    /// not change it.
    pub fn received_batches(&self) -> i64 {
        lock(&self.end.incoming.state).batches
    }
}

/// The wait an idle timeout of `ms` milliseconds allows; `None` for none.
fn idle(ms: i64) -> Option<Duration> {
    u64::try_from(ms)
        .ok()
        .filter(|ms| *ms > 0)
        .map(Duration::from_millis)
}
