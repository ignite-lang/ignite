//! Bounded channels (`docs/slice.md` 30): the C ABI over
//! `ignite_io::{Sender, Receiver}` of heap copies. Each end is a handle of
//! the table in `crate::net`, owned and released like a pipe end, so the
//! last release of all senders or all receivers closes the channel as the
//! drop of the last `ignite-io` handle does on the Rust target. A value
//! sent is copied to the heap with the element type's glue (as a `Store`'s,
//! `crate::store`), and a value received is copied into the receiving
//! task's arena and its heap copy freed: the two tasks share no memory.

use ignite_io::{Channel, Receiver, Sender};

use crate::net::{Handle, lookup_handle, register};
use crate::put;
use crate::store::{FreeHeap, FromHeap, Glue, HeapValue, ToHeap};

/// The sending end in the handle table, with its element type's glue.
#[derive(Clone)]
pub(crate) struct RtSender {
    sender: Sender<HeapValue>,
    glue: Glue,
}

fn sender(handle: i64) -> RtSender {
    match lookup_handle(handle) {
        Ok(Handle::Sender(sender)) => sender,
        _ => ignite_io::fail_with(format!("{handle} is not a channel sender")),
    }
}

fn receiver(handle: i64) -> Receiver<HeapValue> {
    match lookup_handle(handle) {
        Ok(Handle::Receiver(receiver)) => receiver,
        _ => ignite_io::fail_with(format!("{handle} is not a channel receiver")),
    }
}

/// `void channel_bounded(i64 capacity, ptr copy_to_heap, ptr
/// copy_from_heap, ptr free_heap, i64 size, i64 align, i64* sender, i64*
/// receiver)`: a channel of at most `capacity` elements of the glue's type,
/// its two ends registered as handles the calling task owns once. A
/// capacity below 1 fails the task with `ignite-io`'s report.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn ignite_rt_channel_bounded(
    capacity: i64,
    to_heap: Option<ToHeap>,
    from_heap: Option<FromHeap>,
    free: Option<FreeHeap>,
    size: i64,
    align: i64,
    sender_out: *mut i64,
    receiver_out: *mut i64,
) {
    crate::entry(move || {
        let glue = Glue::new(to_heap, from_heap, free, size, align);
        let (sender, receiver) = Channel::bounded(capacity);
        put(
            sender_out,
            register(Handle::Sender(RtSender { sender, glue })),
        );
        put(receiver_out, register(Handle::Receiver(receiver)));
    })
}

/// `i8 channel_send(i64 sender, ptr value)`: 1 when a heap copy of the
/// value at `value` was queued, waiting while the channel is full; 0 when
/// it is closed.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_channel_send(handle: i64, value: *const u8) -> i8 {
    crate::entry(move || {
        let end = sender(handle);
        i8::from(end.sender.send(HeapValue::copy_in(value, end.glue)))
    })
}

/// `i8 channel_try_send(i64 sender, ptr value)`: [`ignite_rt_channel_send`]
/// that returns 0 at once when the channel is full.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_channel_try_send(handle: i64, value: *const u8) -> i8 {
    crate::entry(move || {
        let end = sender(handle);
        i8::from(end.sender.try_send(HeapValue::copy_in(value, end.glue)))
    })
}

/// `void channel_close(i64 end)`: close the channel of either end.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_channel_close(handle: i64) {
    crate::entry(move || match lookup_handle(handle) {
        Ok(Handle::Sender(end)) => end.sender.close(),
        Ok(Handle::Receiver(end)) => end.close(),
        _ => ignite_io::fail_with(format!("{handle} is not a channel end")),
    })
}

/// `i8 channel_receive(i64 receiver, ptr out)`: 1 and the next value in
/// `out`, waiting for one; 0 once the channel is closed and empty.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_channel_receive(handle: i64, out: *mut u8) -> i8 {
    crate::entry(move || deliver(receiver(handle).receive(), out))
}

/// `i8 channel_receive_within(i64 receiver, i64 nanoseconds, ptr out)`:
/// [`ignite_rt_channel_receive`] that returns 0 after `nanoseconds`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_channel_receive_within(
    handle: i64,
    nanoseconds: i64,
    out: *mut u8,
) -> i8 {
    crate::entry(move || deliver(receiver(handle).receive_within(nanoseconds), out))
}

/// `i8 channel_closed(i64 receiver)`: 1 when the channel is closed.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_channel_closed(handle: i64) -> i8 {
    crate::entry(move || i8::from(receiver(handle).closed()))
}

/// 1 and the value copied into `out` (its heap copy freed after), or 0.
fn deliver(value: Option<HeapValue>, out: *mut u8) -> i8 {
    match value {
        Some(value) => {
            value.copy_out(out);
            1
        }
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI64, Ordering};

    use super::*;
    use crate::ignite_rt_tcp_release;

    static LIVE: AtomicI64 = AtomicI64::new(0);

    extern "C" fn to_heap(value: *const u8) -> *mut u8 {
        LIVE.fetch_add(1, Ordering::SeqCst);
        // SAFETY: the tests send `i64`s.
        Box::into_raw(Box::new(unsafe { value.cast::<i64>().read() })).cast()
    }

    extern "C" fn from_heap(dst: *mut u8, heap: *const u8) {
        // SAFETY: as above.
        unsafe { dst.cast::<i64>().write(heap.cast::<i64>().read()) };
    }

    extern "C" fn free(heap: *mut u8) {
        LIVE.fetch_sub(1, Ordering::SeqCst);
        // SAFETY: made by `to_heap`.
        drop(unsafe { Box::from_raw(heap.cast::<i64>()) });
    }

    fn bounded(capacity: i64) -> (i64, i64) {
        let (mut sender, mut receiver) = (0, 0);
        ignite_rt_channel_bounded(
            capacity,
            Some(to_heap),
            Some(from_heap),
            Some(free),
            8,
            8,
            &raw mut sender,
            &raw mut receiver,
        );
        (sender, receiver)
    }

    fn send(sender: i64, value: i64) -> i8 {
        ignite_rt_channel_send(sender, (&raw const value).cast())
    }

    fn receive_within(receiver: i64, nanoseconds: i64) -> Option<i64> {
        let mut out: i64 = 0;
        let got = ignite_rt_channel_receive_within(receiver, nanoseconds, (&raw mut out).cast());
        (got == 1).then_some(out)
    }

    /// Values are copied in and out, every heap copy is freed, and the
    /// last release of the sender closes the channel: one test, because
    /// `LIVE` counts every channel of the test binary.
    #[test]
    fn values_are_copied_and_freed_and_the_last_release_closes() {
        let (sender, receiver) = bounded(2);
        assert_eq!(send(sender, 7), 1);
        let eight: i64 = 8;
        assert_eq!(
            ignite_rt_channel_try_send(sender, (&raw const eight).cast()),
            1
        );
        assert_eq!(
            ignite_rt_channel_try_send(sender, (&raw const eight).cast()),
            0
        );
        assert_eq!(LIVE.load(Ordering::SeqCst), 2);
        let mut out: i64 = 0;
        assert_eq!(
            ignite_rt_channel_receive(receiver, (&raw mut out).cast()),
            1
        );
        assert_eq!(out, 7);
        assert_eq!(receive_within(receiver, 0), Some(8));
        assert_eq!(LIVE.load(Ordering::SeqCst), 0);
        assert_eq!(receive_within(receiver, 10_000_000), None);
        assert_eq!(ignite_rt_channel_closed(receiver), 0);

        assert_eq!(send(sender, 9), 1);
        ignite_rt_tcp_release(sender);
        assert_eq!(ignite_rt_channel_closed(receiver), 1);
        assert_eq!(receive_within(receiver, 1_000_000_000), Some(9));
        assert_eq!(receive_within(receiver, 1_000_000_000), None);
        assert_eq!(LIVE.load(Ordering::SeqCst), 0);

        let (sender, receiver) = bounded(4);
        assert_eq!(send(sender, 1), 1);
        assert_eq!(send(sender, 2), 1);
        ignite_rt_channel_close(receiver);
        assert_eq!(send(sender, 3), 0);
        ignite_rt_tcp_release(receiver);
        ignite_rt_tcp_release(sender);
        assert_eq!(LIVE.load(Ordering::SeqCst), 0);
    }
}
