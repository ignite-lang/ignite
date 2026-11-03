//! Pipes (`docs/slice.md` 18.9): the C ABI over `ignite_io::{Pipe,
//! PipeStream}`. An end is a handle of the one table in `crate::net`, with
//! its generation and reuse rules; `ignite_rt_tcp_own` and
//! `ignite_rt_tcp_release` take it as any handle. A `pipe_*` entry given a
//! TCP handle fails with "`<h>` is not a pipe", and a TCP entry given a
//! pipe with "`<h>` is not a TCP stream". Buffers, deliveries and batches
//! are `ignite-io`'s.

use ignite_io::Pipe;

use crate::net::{Handle, pipe, read_into_arena, register};
use crate::{IgniteInput, IgniteIoError, IgniteList, bytes, put, status};

/// `void pipe_pair(i64* first, i64* second)`: two connected ends.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_pipe_pair(first: *mut i64, second: *mut i64) {
    let (one, other) = Pipe::pair();
    put(first, register(Handle::Pipe(one)));
    put(second, register(Handle::Pipe(other)));
}

/// `i32 pipe_read(i64 pipe, i64 maximum, Input* out, IoError* err)`: one
/// read of up to `maximum` bytes (clamped to `1..=1 MiB`), straight into
/// the task's arena as `tcp_read` does; zero bytes is `Eof`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_pipe_read(
    handle: i64,
    maximum: i64,
    out: *mut IgniteInput,
    err: *mut IgniteIoError,
) -> i32 {
    let read =
        pipe(handle).and_then(|pipe| read_into_arena(maximum, |buffer| pipe.read_into(buffer)));
    status(read, out, err)
}

/// `i32 pipe_write_all(i64 pipe, List* bytes, IoError* err)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_pipe_write_all(
    handle: i64,
    data: *const IgniteList,
    err: *mut IgniteIoError,
) -> i32 {
    let written = pipe(handle).and_then(|pipe| pipe.write_from(bytes(data)));
    status(written, std::ptr::null_mut(), err)
}

/// `i32 pipe_close(i64 pipe, IoError* err)`: deliver what was written,
/// then close the write half; a second close is not an error.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_pipe_close(handle: i64, err: *mut IgniteIoError) -> i32 {
    let closed = pipe(handle).and_then(|pipe| pipe.close());
    status(closed, std::ptr::null_mut(), err)
}

/// `i32 pipe_set_idle_timeout(i64 pipe, i64 ms, IoError* err)`: a later
/// read that waits longer than `ms` milliseconds ends with `Eof`; 0 or
/// less removes the timeout.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_pipe_set_idle_timeout(
    handle: i64,
    ms: i64,
    err: *mut IgniteIoError,
) -> i32 {
    let set = pipe(handle).and_then(|pipe| pipe.set_idle_timeout(ms));
    status(set, std::ptr::null_mut(), err)
}

/// `i64 pipe_received_batches(i64 pipe)`: the deliveries into the end
/// since `pipe_pair`; 0 when the handle is not a pipe, as
/// `tcp_local_port` gives 0 for a handle that is not a listener.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_pipe_received_batches(handle: i64) -> i64 {
    pipe(handle).map_or(0, |pipe| pipe.received_batches())
}
