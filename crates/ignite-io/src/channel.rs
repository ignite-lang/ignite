//! Bounded channels between the tasks of one process (`docs/slice.md` 30):
//! `Channel.bounded(capacity)` gives a [`Sender`] and a [`Receiver`] over
//! one queue of at most `capacity` values, guarded by a mutex with one
//! wait list per direction. No descriptor is opened: a waiter is parked
//! through [`crate::wait`].
//!
//! Every handle of an end is a clone that shares the end. `close` on any
//! handle of either end closes the channel, and so does the drop of the last
//! handle of all senders or of all receivers. A closed channel takes no more
//! values and wakes every waiter: a waiting `send` returns false, a waiting
//! `receive` returns the values queued before the close, then `None`.
//!
//! An operation that is about to wait first sends what the calling task
//! wrote ([`crate::flush`]), as every operation that may wait does (section
//! 7); one that does not wait sends nothing.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::wait::{WaitList, Woken, park_in, wake};

/// The constructor of bounded channels.
#[derive(Clone, Copy, Debug)]
pub struct Channel;

impl Channel {
    /// A channel that holds at most `capacity` values; a capacity below 1
    /// fails the calling task.
    pub fn bounded<T>(capacity: i64) -> (Sender<T>, Receiver<T>) {
        let Ok(capacity) = usize::try_from(capacity) else {
            refuse(capacity)
        };
        if capacity == 0 {
            refuse(0)
        }
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                queue: VecDeque::new(),
                capacity,
                closed: false,
                not_empty: WaitList::new(),
                not_full: WaitList::new(),
            }),
        });
        (
            Sender {
                end: Arc::new(End(Arc::clone(&shared))),
            },
            Receiver {
                end: Arc::new(End(shared)),
            },
        )
    }
}

#[cold]
fn refuse(capacity: i64) -> ! {
    crate::fail_with(format!(
        "Channel.bounded: the capacity must be at least 1, found {capacity}"
    ))
}

/// The sending end of a channel; clones share it.
pub struct Sender<T> {
    end: Arc<End<T>>,
}

/// The receiving end of a channel; clones share it.
pub struct Receiver<T> {
    end: Arc<End<T>>,
}

/// One end: its drop, when the last handle of the end goes, closes the
/// channel.
struct End<T>(Arc<Shared<T>>);

struct Shared<T> {
    state: Mutex<State<T>>,
}

struct State<T> {
    queue: VecDeque<T>,
    capacity: usize,
    closed: bool,
    /// Receivers waiting for a value to be queued or the channel to close.
    not_empty: WaitList,
    /// Senders waiting for a value to be taken or the channel to close.
    not_full: WaitList,
}

impl<T> Drop for End<T> {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl<T> Shared<T> {
    fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The state, locked; when `waits` holds of it, the calling task's
    /// writes are sent first, with the lock released.
    fn lock_to_wait(&self, waits: impl Fn(&State<T>) -> bool) -> MutexGuard<'_, State<T>> {
        let state = self.lock();
        if !waits(&state) {
            return state;
        }
        drop(state);
        crate::flush();
        self.lock()
    }

    /// Queue `value` in the locked `state`, which has room, and wake the
    /// longest waiting receiver once the lock is released.
    fn queue(&self, mut state: MutexGuard<'_, State<T>>, value: T) {
        state.queue.push_back(value);
        let receiver = state.not_empty.pop();
        drop(state);
        if let Some(receiver) = receiver {
            wake(&receiver);
        }
    }

    fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        let receivers = state.not_empty.take();
        let senders = state.not_full.take();
        drop(state);
        for mut list in [receivers, senders] {
            list.wake_all();
        }
    }

    /// The next value, waiting for one until `deadline` (forever when
    /// `None`); `None` once the channel is closed and empty, or at the
    /// deadline.
    fn receive(&self, deadline: Option<Instant>) -> Option<T> {
        let empty = |state: &State<T>| state.queue.is_empty() && !state.closed;
        let mut state = self.lock_to_wait(empty);
        loop {
            if let Some(value) = state.queue.pop_front() {
                let sender = state.not_full.pop();
                drop(state);
                if let Some(sender) = sender {
                    wake(&sender);
                }
                return Some(value);
            }
            if state.closed {
                return None;
            }
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                return None;
            }
            let woken;
            (state, woken) = park_in(&self.state, state, |s| &mut s.not_empty, deadline);
            if woken == Woken::Deadline && state.queue.is_empty() {
                return None;
            }
        }
    }
}

impl<T> Sender<T> {
    /// Queue `value`, waiting while the channel is full; false, and
    /// `value` dropped, when the channel is or becomes closed.
    pub fn send(&self, value: T) -> bool {
        let shared = &self.end.0;
        let full = |state: &State<T>| state.queue.len() >= state.capacity && !state.closed;
        let mut state = shared.lock_to_wait(full);
        loop {
            if state.closed {
                return false;
            }
            if state.queue.len() < state.capacity {
                shared.queue(state, value);
                return true;
            }
            state = park_in(&shared.state, state, |s| &mut s.not_full, None).0;
        }
    }

    /// Queue `value` when there is room; false, and `value` dropped, when
    /// the channel is full or closed. Never waits.
    pub fn try_send(&self, value: T) -> bool {
        let shared = &self.end.0;
        let state = shared.lock();
        if state.closed || state.queue.len() >= state.capacity {
            return false;
        }
        shared.queue(state, value);
        true
    }

    /// Close the channel.
    pub fn close(&self) {
        self.end.0.close();
    }
}

impl<T> Receiver<T> {
    /// The next value, waiting for one; `None` once the channel is closed
    /// and empty.
    pub fn receive(&self) -> Option<T> {
        self.end.0.receive(None)
    }

    /// [`Receiver::receive`] that gives up after `nanoseconds`; zero or
    /// less takes a queued value without waiting.
    pub fn receive_within(&self, nanoseconds: i64) -> Option<T> {
        let wait = Duration::from_nanos(u64::try_from(nanoseconds).unwrap_or(0));
        let deadline = Instant::now().checked_add(wait);
        match deadline {
            Some(deadline) => self.end.0.receive(Some(deadline)),
            None => self.end.0.receive(None),
        }
    }

    /// Whether the channel is closed; values queued before the close may
    /// still be received.
    pub fn closed(&self) -> bool {
        self.end.0.lock().closed
    }

    /// Close the channel.
    pub fn close(&self) {
        self.end.0.close();
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> {
        Sender {
            end: Arc::clone(&self.end),
        }
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Receiver<T> {
        Receiver {
            end: Arc::clone(&self.end),
        }
    }
}

impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Sender")
    }
}

impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Receiver")
    }
}

#[cfg(test)]
mod tests {
    use super::Channel;
    use std::time::{Duration, Instant};

    #[test]
    fn values_arrive_in_order_and_try_send_refuses_when_full() {
        let (tx, rx) = Channel::bounded::<i64>(2);
        assert!(tx.try_send(1));
        assert!(tx.try_send(2));
        assert!(!tx.try_send(3));
        assert_eq!(rx.receive(), Some(1));
        assert!(tx.send(3));
        assert_eq!(rx.receive(), Some(2));
        assert_eq!(rx.receive(), Some(3));
    }

    #[test]
    fn receive_within_gives_up_at_the_deadline() {
        let (tx, rx) = Channel::bounded::<i64>(1);
        let started = Instant::now();
        assert_eq!(rx.receive_within(50_000_000), None);
        assert!(started.elapsed() >= Duration::from_millis(50));
        assert!(!rx.closed());
        assert!(tx.send(7));
        assert_eq!(rx.receive_within(0), Some(7));
        assert_eq!(rx.receive_within(-1), None);
        assert_eq!(rx.receive_within(i64::MIN), None);
    }

    #[test]
    fn a_send_from_another_thread_wakes_a_waiting_receiver() {
        let (tx, rx) = Channel::bounded::<&str>(1);
        let started = Instant::now();
        let ringer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            tx.send("ring")
        });
        assert_eq!(rx.receive_within(5_000_000_000), Some("ring"));
        assert!(started.elapsed() >= Duration::from_millis(50));
        assert!(started.elapsed() < Duration::from_secs(4));
        assert!(ringer.join().unwrap());
    }

    #[test]
    fn send_waits_while_full() {
        let (tx, rx) = Channel::bounded::<i64>(1);
        assert!(tx.send(1));
        let started = Instant::now();
        let taker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            (rx.receive(), rx.receive())
        });
        assert!(tx.send(2));
        assert!(started.elapsed() >= Duration::from_millis(50));
        assert_eq!(taker.join().unwrap(), (Some(1), Some(2)));
    }

    #[test]
    fn close_ends_every_waiter() {
        let (tx, rx) = Channel::bounded::<i64>(1);
        let waiters: Vec<_> = (0..3)
            .map(|_| {
                let rx = rx.clone();
                std::thread::spawn(move || rx.receive())
            })
            .collect();
        std::thread::sleep(Duration::from_millis(50));
        tx.close();
        for waiter in waiters {
            assert_eq!(waiter.join().unwrap(), None);
        }
        assert!(rx.closed());
        assert!(!tx.send(1));
        assert!(!tx.try_send(1));

        let (full, sink) = Channel::bounded::<i64>(1);
        assert!(full.send(1));
        let sender = std::thread::spawn(move || full.send(2));
        std::thread::sleep(Duration::from_millis(50));
        sink.close();
        assert!(!sender.join().unwrap());
    }

    #[test]
    fn a_closed_channel_gives_what_was_queued() {
        let (tx, rx) = Channel::bounded::<i64>(4);
        assert!(tx.send(1));
        assert!(tx.send(2));
        tx.close();
        assert!(rx.closed());
        assert_eq!(rx.receive(), Some(1));
        assert_eq!(rx.receive_within(1_000_000_000), Some(2));
        assert_eq!(rx.receive(), None);
    }

    #[test]
    fn the_last_handle_of_an_end_closes_the_channel() {
        let (tx, rx) = Channel::bounded::<i64>(2);
        let copy = tx.clone();
        drop(tx);
        assert!(!rx.closed());
        let producer = std::thread::spawn(move || {
            for i in 0..5 {
                copy.send(i);
            }
        });
        let mut got = Vec::new();
        while let Some(value) = rx.receive_within(5_000_000_000) {
            got.push(value);
        }
        producer.join().unwrap();
        assert_eq!(got, [0, 1, 2, 3, 4]);
        assert!(rx.closed());

        let (tx, rx) = Channel::bounded::<i64>(1);
        drop(rx);
        assert!(!tx.send(1));
    }

    #[test]
    fn a_capacity_below_one_fails_the_task() {
        for capacity in [0, -1, i64::MIN] {
            let report = crate::catch(move || {
                let _ = Channel::bounded::<i64>(capacity);
            })
            .expect_err("refused");
            assert_eq!(
                report,
                format!("Channel.bounded: the capacity must be at least 1, found {capacity}")
            );
        }
    }

    #[test]
    fn many_waiters_are_woken_by_one_sender() {
        let (tx, rx) = Channel::bounded::<i64>(1000);
        let (done, dones) = Channel::bounded::<i64>(1000);
        let waiters: Vec<_> = (0..200)
            .map(|_| {
                let rx = rx.clone();
                let done = done.clone();
                std::thread::spawn(move || done.send(rx.receive().unwrap_or(0)))
            })
            .collect();
        for i in 1..=200 {
            assert!(tx.send(i));
        }
        let mut sum = 0;
        for _ in 0..200 {
            sum += dones.receive_within(5_000_000_000).expect("woken");
        }
        for waiter in waiters {
            assert!(waiter.join().unwrap());
        }
        assert_eq!(sum, 200 * 201 / 2);
    }
}
