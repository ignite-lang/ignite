//! TLS client sessions (`docs/slice.md` 38):
//! the C ABI over `ignite_io::TlsStream`. A session is a handle of the one
//! table in `crate::net`, with its generation and reuse rules;
//! `ignite_rt_tcp_own` and `ignite_rt_tcp_release` take it as any handle.
//! A `tls_*` entry given another handle fails with "`<h>` is not a TLS
//! stream". `std.tls.TlsOptions` crosses as its record layout
//! ([`IgniteTlsOptions`]). The session, its locks and its errors are
//! `ignite-io`'s.

use ignite_io::{IoError, TlsOptions, TlsStream};

use crate::net::{Handle, lookup_handle, read_into_arena, register, tcp_stream};
use crate::text::texts;
use crate::{IgniteInput, IgniteIoError, IgniteList, bytes, get, put, status};

/// `std.tls.TlsOptions`: its fields in declaration order (`docs/llvm.md`
/// section 3); `handshake_timeout` in nanoseconds.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IgniteTlsOptions {
    pub alpn: IgniteList,
    pub authorities: IgniteList,
    pub verify: i8,
    pub handshake_timeout: i64,
}

fn text(list: &IgniteList) -> String {
    String::from_utf8_lossy(bytes(list)).into_owned()
}

impl IgniteTlsOptions {
    /// The `ignite-io` options of the record at `value`.
    fn read(value: *const IgniteTlsOptions) -> Result<TlsOptions, IoError> {
        let record = get(value).ok_or_else(|| IoError::new("no TLS options"))?;
        Ok(TlsOptions {
            alpn: texts(&record.alpn).iter().map(text).collect(),
            authorities: text(&record.authorities),
            verify: record.verify != 0,
            handshake_timeout: record.handshake_timeout,
        })
    }
}

/// The TLS session `handle` names; any other handle is "`<h>` is not a
/// TLS stream".
fn tls(handle: i64) -> Result<TlsStream, IoError> {
    match lookup_handle(handle)? {
        Handle::Tls(session) => Ok(session),
        _ => Err(IoError::new(format!("{handle} is not a TLS stream"))),
    }
}

/// `i32 tls_client(i64 stream, Text* server_name, TlsOptions* options,
/// i64* out, IoError* err)`: the client handshake on the TCP stream
/// `stream`, a new handle for the session in `out`. The stream's handle
/// stays its owner's; the session holds a clone of the stream.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tls_client(
    stream: i64,
    server_name: *const IgniteList,
    options: *const IgniteTlsOptions,
    out: *mut i64,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let opened = tcp_stream(stream).and_then(|stream| {
            let name = String::from_utf8_lossy(bytes(server_name)).into_owned();
            TlsStream::client(stream, name, IgniteTlsOptions::read(options)?)
        });
        status(opened.map(|s| register(Handle::Tls(s))), out, err)
    })
}

/// `void tls_alpn(Text* out, i64 session)`: the protocol the server chose
/// by ALPN, empty for none or a handle that is no TLS stream.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tls_alpn(out: *mut IgniteList, handle: i64) {
    crate::entry(move || {
        let alpn = tls(handle).map(|s| s.alpn()).unwrap_or_default();
        put(out, IgniteList::from_bytes(alpn.as_bytes()));
    })
}

/// `i32 tls_read(i64 session, i64 maximum, Input* out, IoError* err)`: one
/// read of up to `maximum` bytes of plaintext (clamped to `1..=1 MiB`)
/// into the task's arena; zero bytes is `Eof`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tls_read(
    handle: i64,
    maximum: i64,
    out: *mut IgniteInput,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let read = tls(handle)
            .and_then(|session| read_into_arena(maximum, |buffer| session.read_into(buffer)));
        status(read, out, err)
    })
}

/// `i32 tls_write_all(i64 session, List* bytes, IoError* err)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tls_write_all(
    handle: i64,
    data: *const IgniteList,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let written = tls(handle).and_then(|session| session.write_from(bytes(data)));
        status(written, std::ptr::null_mut(), err)
    })
}

/// `i32 tls_close(i64 session, IoError* err)`: close_notify, then the
/// write half; a second close is not an error.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tls_close(handle: i64, err: *mut IgniteIoError) -> i32 {
    crate::entry(move || {
        let closed = tls(handle).and_then(|session| session.close());
        status(closed, std::ptr::null_mut(), err)
    })
}

/// `i32 tls_set_idle_timeout(i64 session, i64 ms, IoError* err)`: the idle
/// timeout of the stream underneath.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tls_set_idle_timeout(
    handle: i64,
    ms: i64,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let set = tls(handle).and_then(|session| session.set_idle_timeout(ms));
        status(set, std::ptr::null_mut(), err)
    })
}

/// `i32 tls_write_urgent(i64 session, List* bytes, IoError* err)`: refused,
/// TLS carries no urgent data.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tls_write_urgent(
    handle: i64,
    data: *const IgniteList,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let written = tls(handle).and_then(|session| session.write_urgent(bytes(data).to_vec()));
        status(written, std::ptr::null_mut(), err)
    })
}

/// `i32 tls_urgent_pending(i64 session, i8* out, IoError* err)`: always 0.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tls_urgent_pending(
    handle: i64,
    out: *mut i8,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let pending = tls(handle).and_then(|session| session.urgent_pending());
        status(pending.map(i8::from), out, err)
    })
}
