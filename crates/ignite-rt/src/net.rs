//! TCP: the handle table and the C ABI over `ignite_io::{TcpListener,
//! TcpStream}`; the table also holds the pipe ends of `crate::pipe`. A
//! handle names a slot of a process-wide table and stays valid until its
//! last owner releases it: `register` gives it one owner, `tcp_retain` adds
//! one (a task spawned with it), `tcp_release` drops one (`docs/llvm.md`
//! section 4). A table entry is a clone of the `ignite-io`
//! handle; the socket closes when the last operation that was already using
//! it at release time ends. Locking, reads, writes and shutdown are
//! `ignite-io`'s.
//!
//! The table takes no process-wide lock on the IO path. It is a list of
//! segments that never move: segment `k` holds `FIRST << k` slots and is
//! allocated when the first handle in it is issued. A slot has its own
//! lock, held only to clone or take the entry, so operations on different
//! handles never wait for each other.
//!
//! Released slots are reused, so the table's size follows the number of
//! open handles, not the number ever issued. A handle is the slot's index
//! plus one in its low 32 bits and the slot's generation in the high bits;
//! a release bumps the generation, so a stale handle never reaches a newer
//! socket and no handle value is issued twice. A slot whose generation is
//! used up is not reused again. Free slots wait on a list behind one lock,
//! taken only by `register` and `tcp_release`.
//!
//! The stream operations take no slot lock at all: each thread reuses a
//! clone of the stream it used last while the slot's `live` value still
//! names its handle ([`with_stream`]). The lock and the reference count
//! per read and write were 4.75 % of the terminal server's user time
//! (2026-11-08).

use std::cell::RefCell;
use std::sync::atomic::{AtomicI64, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use ignite_io::{
    ChildProcess, CommandLine, Directory, Entropy, Environment, IoError, Launcher, PipeStream,
    Setting, TcpListener, TcpStream,
};

use ignite_io::fresh;

use crate::{IgniteInput, IgniteIoError, IgniteList, arena, bytes, status};

thread_local! {
    /// Handles the current task owns and must close if it fails before its
    /// normal release (`docs/slice.md` section 14.8): a session task owns
    /// the stream the accept loop handed it. One thread per task, so the
    /// list starts empty for each task and is dropped when the task ends.
    static OWNED: RefCell<Vec<i64>> = const { RefCell::new(Vec::new()) };
}

/// A released registration in `OWNED` below live ones ([`release`]); no
/// handle decodes from it, so releasing it does nothing.
const TOMBSTONE: i64 = 0;

/// `void tcp_own(i64 handle)`: mark `handle`, any handle (a listener, a TCP
/// stream or a pipe end), as the current task's, so a failure
/// ([`crate::fail`]) releases it before the task ends.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_own(handle: i64) {
    crate::entry(move || {
        let _ = fresh::try_with(&OWNED, |owned| owned.borrow_mut().push(handle));
    })
}

/// Release every handle the current task still owns, on a failure. The
/// normal release path drops each handle from the list as it closes it, so
/// this closes only those a failure skipped.
pub(crate) fn release_task_handles() {
    let owned = fresh::try_with(&OWNED, |owned| std::mem::take(&mut *owned.borrow_mut()))
        .unwrap_or_default();
    for handle in owned {
        release_owner(handle, None);
    }
}

/// `i64 own_mark()`: the number of handles the current task owns, for
/// [`ignite_rt_own_release`]. A block of an LLVM program takes it before
/// its first owned stream binding (`docs/llvm.md` section 4).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_own_mark() -> i64 {
    crate::entry(move || fresh::try_with(&OWNED, |owned| owned.borrow().len() as i64).unwrap_or(0))
}

/// `void own_release(i64 mark)`: release every handle the current task
/// registered with `tcp_own` since `own_mark` returned `mark` and still
/// owns, newest first: the stream bindings of a block that is left.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_own_release(mark: i64) {
    crate::entry(move || {
        let mark = usize::try_from(mark).unwrap_or(0);
        let above = fresh::try_with(&OWNED, |owned| {
            let mut owned = owned.borrow_mut();
            let at = mark.min(owned.len());
            owned.split_off(at)
        })
        .unwrap_or_default();
        for handle in above.into_iter().rev() {
            release_owner(handle, None);
        }
    })
}

/// `void tcp_retain(i64 handle)`: add an owner to `handle` (a task it is
/// spawned with, `docs/llvm.md` section 4), so it stays open until that
/// owner releases it too. Nothing for a released or unknown handle.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_retain(handle: i64) {
    crate::entry(move || {
        let Some((index, generation)) = decode(handle) else {
            return;
        };
        let Some(slot) = slot_at(index, false) else {
            return;
        };
        let mut entry = lock(&slot.entry);
        // Saturating: an owner a program never releases (a store `get`
        // whose copy no block owns) keeps the handle open instead of
        // wrapping the count to an early close.
        if entry.generation == generation && entry.handle.is_some() {
            entry.owners = entry.owners.saturating_add(1);
        }
    })
}

/// `void handle_release(i64 handle)`: drop one owner of `handle` that no
/// block or task registered: the owner a heap copy in a store or a channel
/// holds (`docs/slice.md` 30.3), dropped when the copy is freed. Unlike
/// [`ignite_rt_tcp_release`] it leaves the current task's registrations
/// alone, so a block that owns the same handle still releases its own.
/// Nothing for a released or unknown handle.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_handle_release(handle: i64) {
    crate::entry(move || release_owner(handle, None))
}

#[derive(Clone)]
pub(crate) enum Handle {
    Listener(TcpListener),
    Stream(TcpStream),
    /// An end of an in-memory pipe (`crate::pipe`).
    Pipe(PipeStream),
    /// A TLS client session (`crate::tls`), its type erased: the session's
    /// drop and clone are reached through the vtable only
    /// `ignite_rt_tls_client` makes, so a program that opens no session
    /// links no TLS engine (`docs/slice.md` 38.2).
    Tls(crate::tls::Session),
    /// A root directory (`crate::fs`, `docs/slice.md` 21.4).
    Directory(Directory),
    /// The authority to start children (`crate::child`, 21.5).
    Launcher(Launcher),
    /// A started child (`crate::child`, 21.5).
    Child(ChildProcess),
    /// A source of random bytes (`crate::entropy`, 21.7).
    Entropy(Entropy),
    /// A launch argument (`crate::process`, 21.6).
    Setting(Setting),
    /// The whole command line (`crate::process`, 21.19).
    CommandLine(CommandLine),
    /// The environment variables (`crate::process`, 21.20).
    Environment(Environment),
    /// The sending end of a channel (`crate::channel`, 30).
    Sender(crate::channel::RtSender),
    /// The receiving end of a channel (`crate::channel`, 30).
    Receiver(ignite_io::Receiver<crate::store::HeapValue>),
}

/// The capability `handle` names, picked by `pick`; another handle is
/// "`<h>` is not a <what>" (`docs/slice.md` 21.8).
pub(crate) fn capability<T>(
    handle: i64,
    what: &str,
    pick: impl FnOnce(Handle) -> Option<T>,
) -> Result<T, IoError> {
    pick(lookup(handle)?).ok_or_else(|| IoError::new(format!("{handle} is not a {what}")))
}

/// One table entry: the generation of the handle it holds or will hold
/// next, and the handle; `None` before it is filled and once released.
/// `waited` holds, per generation, the process id of each waited child
/// this slot held: 16 bytes per waited child, kept until the process
/// exits, because an LLVM program may ask a waited handle's id at any
/// time (`docs/slice.md` 21.5).
#[derive(Default)]
struct Entry {
    generation: u32,
    handle: Option<Handle>,
    waited: Vec<(u32, i64)>,
    /// Owners of the handle (`tcp_retain` adds one); `tcp_release` drops
    /// one and empties the slot when it drops the last.
    owners: u32,
}

/// One slot: its entry behind a lock, and the value of the handle it holds
/// (0 when empty), which [`with_stream`] reads without the lock.
#[derive(Default)]
struct Slot {
    live: AtomicI64,
    entry: Mutex<Entry>,
}

/// Slots in segment 0.
const FIRST: usize = 64;
/// Segments; together they hold more slots than a handle's 32 index bits.
const SEGMENTS: usize = 27;
/// Generations a slot goes through before it is retired; handles stay
/// positive.
const GENERATIONS: u32 = 1 << 31;

#[allow(clippy::declare_interior_mutable_const)]
const NO_SEGMENT: AtomicPtr<Slot> = AtomicPtr::new(std::ptr::null_mut());
static TABLE: [AtomicPtr<Slot>; SEGMENTS] = [NO_SEGMENT; SEGMENTS];
/// Slots handed out so far; the next new slot has this index.
static ISSUED: AtomicUsize = AtomicUsize::new(0);
/// Released slots that can be reused, most recently released last.
static FREE: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// Segment and offset of slot `index`: segment `k` starts at
/// `FIRST * (2^k - 1)`.
fn locate(index: usize) -> (usize, usize) {
    let scaled = index / FIRST + 1;
    let segment = (usize::BITS - 1 - scaled.leading_zeros()) as usize;
    (segment, index - FIRST * ((1 << segment) - 1))
}

fn segment_len(segment: usize) -> usize {
    FIRST << segment
}

/// The slot at `index`, allocating its segment when `create`.
fn slot_at(index: usize, create: bool) -> Option<&'static Slot> {
    let (segment, offset) = locate(index);
    let cell = TABLE.get(segment)?;
    let mut base = cell.load(Ordering::Acquire);
    if base.is_null() {
        if !create {
            return None;
        }
        let fresh: Box<[Slot]> = (0..segment_len(segment)).map(|_| Slot::default()).collect();
        let fresh = Box::into_raw(fresh) as *mut Slot;
        match cell.compare_exchange(
            std::ptr::null_mut(),
            fresh,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => base = fresh,
            Err(winner) => {
                // SAFETY: `fresh` came from `Box::into_raw` above and was not
                // published.
                drop(unsafe {
                    Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                        fresh,
                        segment_len(segment),
                    ))
                });
                base = winner;
            }
        }
    }
    // SAFETY: a published segment has `segment_len(segment)` slots, is never
    // freed or moved, and `offset` is below that length.
    Some(unsafe { &*base.add(offset) })
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The handle value of slot `index` in `generation`.
fn encode(index: usize, generation: u32) -> i64 {
    (i64::from(generation) << 32) | (index as i64 + 1)
}

/// Slot index and generation of a handle value, if it can name a slot.
fn decode(handle: i64) -> Option<(usize, u32)> {
    let low = handle & 0xffff_ffff;
    if handle < 1 || low == 0 {
        return None;
    }
    let index = usize::try_from(low - 1).ok()?;
    let generation = u32::try_from(handle >> 32).ok()?;
    (index < ISSUED.load(Ordering::Acquire)).then_some((index, generation))
}

pub(crate) fn register(handle: Handle) -> i64 {
    let reused = lock(&FREE).pop();
    let index = reused.unwrap_or_else(|| {
        ignite_io::census::add(ignite_io::census::Kind::HandleSlots, 1);
        ISSUED.fetch_add(1, Ordering::AcqRel)
    });
    if index >= u32::MAX as usize {
        crate::process::die("handle table full");
    }
    let slot = slot_at(index, true).unwrap_or_else(|| crate::process::die("handle table full"));
    let mut entry = lock(&slot.entry);
    entry.handle = Some(handle);
    entry.owners = 1;
    let value = encode(index, entry.generation);
    slot.live.store(value, Ordering::Release);
    ignite_io::census::add(ignite_io::census::Kind::Handles, 1);
    value
}

/// Whether `handle` is the handle its slot holds now. A handle value is
/// issued once, so a match means the same registration is still open.
fn is_live(handle: i64) -> bool {
    decode(handle)
        .and_then(|(index, _)| slot_at(index, false))
        .is_some_and(|slot| slot.live.load(Ordering::Acquire) == handle)
}

thread_local! {
    /// The stream this thread used last, by handle: a clone taken from the
    /// table once and reused while its handle stays live, so a read or
    /// write takes no slot lock and moves no reference count. Dropped when
    /// this thread releases that handle ([`release_owner`]) and when its
    /// task ends ([`task_ended`]); a task uses only the streams it owns,
    /// and releases them on its own thread, so the clone never keeps a
    /// socket open past its last owner's release.
    static CACHED: RefCell<Option<(i64, TcpStream)>> = const { RefCell::new(None) };
}

/// `f` applied to the stream `handle` names: this thread's cached clone
/// when it is that handle's and the handle is live, else a clone from the
/// table that replaces the cached one.
///
/// The clone is taken out of the cache while `f` runs and put back after:
/// `f` may wait (a read, a send), and a task waiting on a worker must not
/// hold a borrow of a thread-local another task of the worker uses
/// (`ignite_io::TaskLocals`).
fn with_stream<R>(
    handle: i64,
    f: impl FnOnce(&TcpStream) -> Result<R, IoError>,
) -> Result<R, IoError> {
    // Both looks through `fresh`: `f` may move the task to another worker
    // (a first read), whose thread-locals hold the task's from then on.
    let cached = fresh::try_with(&CACHED, |cell| {
        cell.try_borrow_mut()
            .ok()
            .and_then(|mut cached| cached.take())
    })
    .ok()
    .flatten();
    let found = match cached {
        Some((held, found)) if held == handle && is_live(handle) => found,
        _ => stream(handle)?,
    };
    let result = f(&found);
    let _ = fresh::try_with(&CACHED, |cell| {
        if let Ok(mut cached) = cell.try_borrow_mut() {
            *cached = Some((handle, found));
        }
    });
    result
}

/// A task's owned handles and cached stream while another task runs on
/// its worker (`ignite_io::TaskLocals`, IO mode `readiness`).
#[derive(Default)]
pub(crate) struct TaskHandles {
    owned: Vec<i64>,
    cached: Option<(i64, TcpStream)>,
}

impl TaskHandles {
    /// Exchange the saved handles with this thread's.
    pub(crate) fn swap(&mut self) {
        OWNED.with(|owned| std::mem::swap(&mut *owned.borrow_mut(), &mut self.owned));
        CACHED.with(|cached| std::mem::swap(&mut *cached.borrow_mut(), &mut self.cached));
    }
}

/// Drop this thread's cached clone of `handle`'s stream, if it holds one.
fn forget(handle: i64) {
    let _ = fresh::try_with(&CACHED, |cell| {
        if let Ok(mut cached) = cell.try_borrow_mut()
            && matches!(&*cached, Some((held, _)) if *held == handle)
        {
            *cached = None;
        }
    });
}

/// The end of the current task on this thread: the cached stream and the
/// kept read buffer go, so an idle worker holds neither a socket nor
/// 64 KiB of buffer.
pub(crate) fn task_ended() {
    let _ = fresh::try_with(&CACHED, |cell| {
        if let Ok(mut cached) = cell.try_borrow_mut() {
            *cached = None;
        }
    });
    let _ = fresh::try_with(&READ_BUFFER, |cell| {
        if let Ok(mut buffer) = cell.try_borrow_mut() {
            *buffer = Vec::new();
        }
    });
}

/// [`lookup`], for the channel entries (`crate::channel`).
pub(crate) fn lookup_handle(handle: i64) -> Result<Handle, IoError> {
    lookup(handle)
}

/// The handle's entry: `Err` with the reason when it was released or never
/// issued.
fn lookup(handle: i64) -> Result<Handle, IoError> {
    let (index, generation) =
        decode(handle).ok_or_else(|| IoError::new(format!("{handle} is not a handle")))?;
    let slot =
        slot_at(index, false).ok_or_else(|| IoError::new(format!("{handle} is not a handle")))?;
    let entry = lock(&slot.entry);
    if entry.generation == generation
        && let Some(found) = &entry.handle
    {
        return Ok(found.clone());
    }
    if generation > entry.generation {
        return Err(IoError::new(format!("{handle} is not a handle")));
    }
    Err(IoError::new(format!("handle {handle} was released")))
}

fn listener(handle: i64) -> Result<TcpListener, IoError> {
    match lookup(handle)? {
        Handle::Listener(listener) => Ok(listener),
        _ => Err(IoError::new(format!("{handle} is not a TCP listener"))),
    }
}

/// The TCP stream `handle` names; any other handle is "`<h>` is not a TCP
/// stream".
pub(crate) fn tcp_stream(handle: i64) -> Result<TcpStream, IoError> {
    stream(handle)
}

fn stream(handle: i64) -> Result<TcpStream, IoError> {
    match lookup(handle)? {
        Handle::Stream(stream) => Ok(stream),
        _ => Err(IoError::new(format!("{handle} is not a TCP stream"))),
    }
}

/// The pipe end `handle` names; any other handle is "`<h>` is not a pipe".
pub(crate) fn pipe(handle: i64) -> Result<PipeStream, IoError> {
    match lookup(handle)? {
        Handle::Pipe(pipe) => Ok(pipe),
        _ => Err(IoError::new(format!("{handle} is not a pipe"))),
    }
}

fn text(value: *const IgniteList) -> String {
    String::from_utf8_lossy(bytes(value)).into_owned()
}

/// `i32 tcp_bind(Text* address, i64 port, i64* out, IoError* err)`. Port 0
/// picks a free port.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_bind(
    address: *const IgniteList,
    port: i64,
    out: *mut i64,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let bound = TcpListener::bind(text(address), port);
        status(bound.map(|l| register(Handle::Listener(l))), out, err)
    })
}

/// `i32 tcp_bind_kept(Text* address, i64 port, i64 kept, i64* out,
/// IoError* err)`: `tcp_bind` of a listener resource that says
/// `keep_through_drain`; kept through the drain when `kept` is not 0
/// (`docs/slice.md` 16.13, story `drain-keeps-listeners`).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_bind_kept(
    address: *const IgniteList,
    port: i64,
    kept: i64,
    out: *mut i64,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let bound = TcpListener::bind_with(text(address), port, kept != 0);
        status(bound.map(|l| register(Handle::Listener(l))), out, err)
    })
}

/// `i32 tcp_accept(i64 listener, i64* out, IoError* err)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_accept(handle: i64, out: *mut i64, err: *mut IgniteIoError) -> i32 {
    crate::entry(move || {
        let accepted = listener(handle).and_then(|listener| listener.accept());
        status(accepted.map(|s| register(Handle::Stream(s))), out, err)
    })
}

/// `i64 tcp_local_port(i64 listener)`: the bound port; 0 when the handle is
/// not a listener or the operating system cannot report it.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_local_port(handle: i64) -> i64 {
    crate::entry(move || listener(handle).map_or(0, |listener| listener.local_port()))
}

/// `i32 tcp_connect(Text* address, i64 port, i64* out, IoError* err)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_connect(
    address: *const IgniteList,
    port: i64,
    out: *mut i64,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let connected = TcpStream::connect(text(address), port);
        status(connected.map(|s| register(Handle::Stream(s))), out, err)
    })
}

/// `i32 tcp_connect_within(Text* address, i64 port, i64 timeout_ns, i64*
/// out, IoError* err)`: `tcp_connect` that gives up after `timeout_ns`
/// nanoseconds (`ignite_io::TcpStream::connect_within`, `docs/slice.md`
/// 26.3).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_connect_within(
    address: *const IgniteList,
    port: i64,
    timeout_ns: i64,
    out: *mut i64,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let connected = TcpStream::connect_within(text(address), port, timeout_ns);
        status(connected.map(|s| register(Handle::Stream(s))), out, err)
    })
}

/// `void tcp_peer(Endpoint* out, i64 stream)`: the other end of the
/// stream, its address allocated in the task's arena
/// (`ignite_io::TcpStream::peer`, `docs/slice.md` 26.1); an empty address
/// and port 0 when the handle is not a stream, as `tcp_local_port` gives 0
/// for one that is not a listener.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_peer(out: *mut crate::IgniteEndpoint, handle: i64) {
    crate::entry(move || {
        let (address, port) = stream(handle).map(|s| s.peer()).unwrap_or_default();
        crate::put(
            out,
            crate::IgniteEndpoint {
                address: IgniteList::from_bytes(address.as_bytes()),
                port,
            },
        );
    })
}

/// `void tcp_local(Endpoint* out, i64 stream)`: this end of the stream,
/// its address allocated in the task's arena (`ignite_io::TcpStream::local`);
/// an empty address and port 0 when the
/// handle is not a stream, as `tcp_peer` gives.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_local(out: *mut crate::IgniteEndpoint, handle: i64) {
    crate::entry(move || {
        let (address, port) = stream(handle).map(|s| s.local()).unwrap_or_default();
        crate::put(
            out,
            crate::IgniteEndpoint {
                address: IgniteList::from_bytes(address.as_bytes()),
                port,
            },
        );
    })
}

/// `i8 tcp_timed_out(i64 stream)`: 1 when the last read of the stream
/// ended because the idle timeout passed, else 0, also for a handle that
/// is not a stream (`ignite_io::TcpStream::timed_out`, `docs/slice.md`
/// 26.2).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_timed_out(handle: i64) -> i8 {
    crate::entry(move || i8::from(with_stream(handle, |s| Ok(s.timed_out())).unwrap_or(false)))
}

/// Largest buffer a single read allocates, as in `ignite-io`.
const MAX_READ: usize = 1024 * 1024;

/// `i32 tcp_read(i64 stream, i64 maximum, Input* out, IoError* err)`: one
/// read of up to `maximum` bytes (clamped to `1..=1 MiB`); zero bytes is
/// `Eof`. The bytes go to the task's arena ([`read_into_arena`]).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_read(
    handle: i64,
    maximum: i64,
    out: *mut IgniteInput,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let read = with_stream(handle, |stream| {
            let read = stream.read_with(|receive| read_into_arena(maximum, receive))?;
            Ok(read.unwrap_or_else(IgniteInput::eof))
        });
        status(read, out, err)
    })
}

/// Read buffers up to this size stay with their thread, until its task
/// ends, for the next read that does not go in place (the std servers
/// read 16 to 64 KiB at a time).
const KEPT_READ_BUFFER: usize = 64 * 1024;

thread_local! {
    /// The buffer a read that does not go in place goes to before its
    /// bytes are copied into the arena.
    static READ_BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// One read by `read_into` of up to `maximum` bytes (clamped to `1..=1
/// MiB`). Zero bytes is `Eof`. When `maximum` fits in what is left of the
/// current chunk the bytes go straight into it: a block of `maximum`
/// bytes whose unused tail goes back. Otherwise the read goes into this
/// thread's buffer (kept up to 64 KiB until the task ends) and the bytes
/// read are copied into the arena: a request over [`arena::LARGE`] would
/// be a block of its own (a zeroed `malloc` block that keeps all `maximum`
/// bytes and leaves no chunk for the inline allocator), and a smaller one
/// would start a new chunk for headroom it mostly gives back (a server
/// reading 16 KiB at a time held two touched chunks per connection).
pub(crate) fn read_into_arena<E>(
    maximum: i64,
    read_into: impl FnOnce(&mut [u8]) -> Result<usize, E>,
) -> Result<IgniteInput, E> {
    let size = usize::try_from(maximum.max(1))
        .unwrap_or(MAX_READ)
        .min(MAX_READ);
    if size > arena::LARGE || size > arena::room() {
        return read_through_buffer(size, read_into);
    }
    let block = arena::alloc(size, 1);
    // SAFETY: `block` is a fresh arena block of `size` bytes, nothing
    // else refers to it, and arena memory is always initialized (its
    // blocks are allocated zeroed).
    let buffer = unsafe { std::slice::from_raw_parts_mut(block, size) };
    let count = read_into(buffer);
    let used = count.as_ref().map_or(0, |count| *count);
    arena::trim(block, size, used);
    let count = count?;
    Ok(if count == 0 {
        IgniteInput::eof()
    } else {
        IgniteInput {
            tag: 0,
            data: IgniteList {
                ptr: block,
                len: count as i64,
                cap: count as i64,
            },
        }
    })
}

/// [`read_into_arena`] of `size` bytes that does not go in place: into the
/// thread's read buffer, or a buffer of its own for a size over 64 KiB,
/// then `count` bytes copied into the arena.
///
/// The thread's buffer is taken out of the thread-local while `read_into`
/// runs and put back after, not borrowed across it: the read may wait, and
/// a task that waits holds no borrow of a thread-local, as another task of
/// the worker uses it meanwhile, and the waiting task may move to another
/// worker at its first read and come back on that worker's thread. A
/// nested read meanwhile finds the thread's buffer empty and grows it.
fn read_through_buffer<E>(
    size: usize,
    read_into: impl FnOnce(&mut [u8]) -> Result<usize, E>,
) -> Result<IgniteInput, E> {
    let kept = size <= KEPT_READ_BUFFER;
    let mut buffer = if kept {
        fresh::try_with(&READ_BUFFER, |cell| {
            cell.try_borrow_mut()
                .map(|mut buffer| std::mem::take(&mut *buffer))
                .unwrap_or_default()
        })
        .unwrap_or_default()
    } else {
        Vec::new()
    };
    if buffer.len() < size {
        buffer.resize(size, 0);
    }
    let result = read_into(&mut buffer[..size]).map(|count| {
        let count = count.min(size);
        if count == 0 {
            IgniteInput::eof()
        } else {
            IgniteInput {
                tag: 0,
                data: IgniteList::from_bytes(&buffer[..count]),
            }
        }
    });
    if kept {
        // Through `fresh`, as the take above: the thread of now.
        let _ = fresh::try_with(&READ_BUFFER, |cell| {
            if let Ok(mut kept) = cell.try_borrow_mut()
                && kept.capacity() < buffer.capacity()
            {
                *kept = buffer;
            }
        });
    }
    result
}

/// `i32 tcp_write_all(i64 stream, List* bytes, IoError* err)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_write_all(
    handle: i64,
    data: *const IgniteList,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let written = with_stream(handle, |stream| stream.write_from(bytes(data)));
        status(written, std::ptr::null_mut(), err)
    })
}

/// `i32 tcp_set_idle_timeout(i64 stream, i64 ms, IoError* err)`: a later
/// read that waits longer than `ms` milliseconds ends with `Eof`; 0 or less
/// removes the timeout (`ignite_io::TcpStream::set_idle_timeout`).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_set_idle_timeout(
    handle: i64,
    ms: i64,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let set = with_stream(handle, |stream| stream.set_idle_timeout(ms));
        status(set, std::ptr::null_mut(), err)
    })
}

/// `i32 tcp_write_urgent(i64 stream, List* bytes, IoError* err)`: what was
/// written and `bytes` sent at once, the last byte as urgent data
/// (`ignite_io::TcpStream::write_urgent`).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_write_urgent(
    handle: i64,
    data: *const IgniteList,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let written = with_stream(handle, |stream| stream.write_urgent_from(bytes(data)));
        status(written, std::ptr::null_mut(), err)
    })
}

/// `i32 tcp_urgent_pending(i64 stream, i8* out, IoError* err)`: 1 when an
/// urgent byte is ahead of the reader, else 0
/// (`ignite_io::TcpStream::urgent_pending`).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_urgent_pending(
    handle: i64,
    out: *mut i8,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let pending = with_stream(handle, |stream| stream.urgent_pending());
        status(pending.map(i8::from), out, err)
    })
}

/// `i32 tcp_close(i64 stream, IoError* err)`: shut down the write half. A
/// socket that is already disconnected is not an error.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_close(handle: i64, err: *mut IgniteIoError) -> i32 {
    crate::entry(move || {
        let closed = with_stream(handle, |stream| stream.close());
        status(closed, std::ptr::null_mut(), err)
    })
}

/// `void tcp_release(i64 handle)`: drop one owner of any handle (a
/// listener, a TCP stream or a pipe end) and the current task's
/// registration of it, if any. The last owner's release drops the handle
/// from the table: later use of it is an error (exit code 1), and a
/// release after it, or of a handle that was never issued, does nothing.
/// Releasing the last handle of a pipe end delivers what it wrote and
/// closes it.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_tcp_release(handle: i64) {
    crate::entry(move || {
        release(handle, None);
    })
}

/// Release the handle of a waited child and keep its process id in the
/// slot, by the handle's generation, so `child.id()` answers it for that
/// handle value also after the slot was reused (`docs/slice.md` 21.5).
pub(crate) fn release_waited_child(handle: i64, id: i64) {
    release(handle, Some(id));
}

/// The process id kept for `handle` by [`release_waited_child`].
pub(crate) fn waited_child_id(handle: i64) -> Option<i64> {
    let (index, generation) = decode(handle)?;
    let entry = lock(&slot_at(index, false)?.entry);
    entry
        .waited
        .iter()
        .find(|(g, _)| *g == generation)
        .map(|(_, id)| *id)
}

/// Drop the current task's newest registration of `handle`, then one owner.
/// The registration becomes the tombstone `0` (no handle, [`decode`])
/// instead of being removed, so the marks `own_mark` returned earlier keep
/// pointing at the same registrations: a block nested in the one that owns
/// a stream binding may take its mark before a `spawn` hands that binding
/// over (`docs/slice.md` 20.3.1). A tombstone goes with the `own_release`
/// of the block that registered the handle, or with the task. Not even a
/// tombstone at the end is dropped: an open block's mark may lie above it.
fn release(handle: i64, waited_child: Option<i64>) {
    let _ = fresh::try_with(&OWNED, |owned| {
        let mut owned = owned.borrow_mut();
        if let Some(at) = owned.iter().rposition(|owned| *owned == handle) {
            owned[at] = TOMBSTONE;
        }
    });
    release_owner(handle, waited_child);
}

/// Drop one owner of `handle`; the last empties its slot. This thread's
/// cached clone of it goes first, so the last release closes the socket.
fn release_owner(handle: i64, waited_child: Option<i64>) {
    forget(handle);
    let Some((index, generation)) = decode(handle) else {
        return;
    };
    let Some(slot) = slot_at(index, false) else {
        return;
    };
    let mut entry = lock(&slot.entry);
    if entry.generation != generation || entry.handle.is_none() {
        return;
    }
    if entry.owners > 1 {
        entry.owners -= 1;
        return;
    }
    if let Some(id) = waited_child {
        entry.waited.push((generation, id));
    }
    slot.live.store(0, Ordering::Release);
    let released = entry.handle.take();
    entry.generation += 1;
    ignite_io::census::release(ignite_io::census::Kind::Handles);
    let reusable = entry.generation < GENERATIONS;
    drop(entry);
    drop(released);
    if reusable {
        lock(&FREE).push(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_tile_the_indices() {
        assert_eq!(locate(0), (0, 0));
        assert_eq!(locate(FIRST - 1), (0, FIRST - 1));
        assert_eq!(locate(FIRST), (1, 0));
        assert_eq!(locate(3 * FIRST - 1), (1, 2 * FIRST - 1));
        assert_eq!(locate(3 * FIRST), (2, 0));
        for index in [0, 1, 63, 64, 191, 192, 1000, 100_000, 10_000_000] {
            let (segment, offset) = locate(index);
            assert!(offset < segment_len(segment), "{index}");
        }
        let (last, offset) = locate(u32::MAX as usize - 1);
        assert!(last < SEGMENTS && offset < segment_len(last));
    }

    #[test]
    fn handles_encode_slot_and_generation() {
        assert_eq!(encode(0, 0), 1);
        assert_eq!(encode(41, 3), (3 << 32) | 42);
        assert!(encode(u32::MAX as usize - 2, GENERATIONS - 1) > 0);
        assert_eq!(decode(0), None);
        assert_eq!(decode(-5), None);
        assert_eq!(decode(1 << 32), None);
    }

    #[test]
    fn a_handle_stays_until_its_last_owner_releases_it() {
        let listener = TcpListener::bind("127.0.0.1".to_string(), 0).unwrap();
        let handle = register(Handle::Listener(listener));
        ignite_rt_tcp_retain(handle);
        ignite_rt_tcp_retain(handle);
        ignite_rt_tcp_release(handle);
        ignite_rt_tcp_release(handle);
        assert!(lookup(handle).is_ok(), "released before its last owner");
        ignite_rt_tcp_release(handle);
        assert!(lookup(handle).is_err());
        ignite_rt_tcp_retain(handle);
        ignite_rt_tcp_release(handle);
    }

    #[test]
    fn own_release_releases_the_handles_owned_since_the_mark() {
        let listener = TcpListener::bind("127.0.0.1".to_string(), 0).unwrap();
        let kept = register(Handle::Listener(listener.clone()));
        ignite_rt_tcp_own(kept);
        let mark = ignite_rt_own_mark();
        let shared = register(Handle::Listener(listener.clone()));
        ignite_rt_tcp_retain(shared);
        ignite_rt_tcp_own(shared);
        let single = register(Handle::Listener(listener));
        ignite_rt_tcp_own(single);
        assert_eq!(ignite_rt_own_mark(), mark + 2);
        ignite_rt_own_release(mark);
        assert_eq!(ignite_rt_own_mark(), mark);
        assert!(lookup(single).is_err());
        assert!(lookup(shared).is_ok(), "the retained owner remains");
        assert!(lookup(kept).is_ok());
        ignite_rt_tcp_release(shared);
        ignite_rt_tcp_release(kept);
        assert!(lookup(shared).is_err() && lookup(kept).is_err());
        // `kept`'s registration stays as a tombstone, so marks keep their place.
        assert_eq!(ignite_rt_own_mark(), mark);
        ignite_rt_own_release(mark - 1);
        assert_eq!(ignite_rt_own_mark(), mark - 1);
    }

    /// A spawn hands over a binding registered below a nested block's mark
    /// (`docs/slice.md` 20.3.1): the release leaves a tombstone, so the
    /// nested block's `own_release` still releases what it registered.
    #[test]
    fn a_release_below_a_mark_leaves_the_mark_valid() {
        let listener = TcpListener::bind("127.0.0.1".to_string(), 0).unwrap();
        let outer_mark = ignite_rt_own_mark();
        let outer = register(Handle::Listener(listener.clone()));
        ignite_rt_tcp_own(outer);
        let inner_mark = ignite_rt_own_mark();
        let first = register(Handle::Listener(listener.clone()));
        ignite_rt_tcp_own(first);
        let second = register(Handle::Listener(listener));
        ignite_rt_tcp_own(second);
        ignite_rt_tcp_release(outer);
        assert!(lookup(outer).is_err());
        ignite_rt_own_release(inner_mark);
        assert!(
            lookup(first).is_err() && lookup(second).is_err(),
            "the inner block released both of its registrations"
        );
        assert_eq!(ignite_rt_own_mark(), inner_mark);
        ignite_rt_own_release(outer_mark);
        assert_eq!(ignite_rt_own_mark(), outer_mark);
    }

    /// A connected pair: the server end registered as a stream handle and
    /// the client end as a plain socket.
    fn stream_pair() -> (i64, std::net::TcpStream) {
        let listener = TcpListener::bind("127.0.0.1".to_string(), 0).unwrap();
        let client =
            std::net::TcpStream::connect(("127.0.0.1", listener.local_port() as u16)).unwrap();
        let server = listener.accept().unwrap();
        (register(Handle::Stream(server)), client)
    }

    /// One `tcp_read` of up to 64 bytes: the bytes, empty on `Eof`, or the
    /// error's message.
    fn read(handle: i64) -> Result<Vec<u8>, String> {
        let mut input = IgniteInput::eof();
        let mut error = IgniteIoError {
            message: IgniteList::from_bytes(b""),
            exit_code: 0,
        };
        if ignite_rt_tcp_read(handle, 64, &mut input, &mut error) != 0 {
            return Err(String::from_utf8_lossy(bytes(&error.message)).into_owned());
        }
        Ok(if input.tag == 0 {
            bytes(&input.data).to_vec()
        } else {
            Vec::new()
        })
    }

    /// A read whose maximum does not fit in what is left of the current
    /// chunk does not start a new chunk for it: its bytes go through the
    /// thread's buffer into the current one. One that fits reads in place.
    #[test]
    fn a_read_takes_no_new_chunk_for_its_headroom() {
        let reserved = crate::arena::arena_reserved;
        crate::arena::ignite_rt_task_end();
        let first = crate::arena::ignite_rt_alloc(8, 8);
        let held = reserved();
        crate::arena::ignite_rt_alloc(60_000, 1);
        let read = read_into_arena(16_384, |buffer| {
            buffer[..5].copy_from_slice(b"hello");
            Ok::<usize, IoError>(5)
        })
        .unwrap();
        assert_eq!(bytes(&read.data), b"hello");
        assert_eq!(reserved(), held, "a chunk was taken for the read");
        crate::arena::ignite_rt_task_end();
        let first_again = crate::arena::ignite_rt_alloc(8, 8);
        let in_place = read_into_arena(16_384, |buffer| {
            buffer[..2].copy_from_slice(b"hi");
            Ok::<usize, IoError>(2)
        })
        .unwrap();
        assert_eq!(in_place.data.ptr as usize, first_again as usize + 8);
        let _ = first;
        crate::arena::ignite_rt_task_end();
    }

    /// A stream operation does not take its slot's lock: a read on a
    /// handle this thread used before goes ahead while another thread
    /// holds the slot.
    #[test]
    fn a_stream_operation_takes_no_slot_lock() {
        use std::io::Write;
        use std::sync::mpsc::channel;
        let (handle, mut client) = stream_pair();
        let (warm, warmed) = channel();
        let (go, went) = channel::<()>();
        let (done, finished) = channel();
        let reader = std::thread::spawn(move || {
            let first = read(handle);
            warm.send(first).unwrap();
            went.recv().unwrap();
            done.send(read(handle)).unwrap();
        });
        client.write_all(b"one").unwrap();
        assert_eq!(warmed.recv().unwrap(), Ok(b"one".to_vec()));
        let (index, _) = decode(handle).unwrap();
        let held = lock(&slot_at(index, false).unwrap().entry);
        go.send(()).unwrap();
        client.write_all(b"two").unwrap();
        let second = finished.recv_timeout(std::time::Duration::from_secs(5));
        drop(held);
        reader.join().unwrap();
        assert_eq!(
            second,
            Ok(Ok(b"two".to_vec())),
            "the read waited for the slot"
        );
        ignite_rt_tcp_release(handle);
    }

    /// Releasing the last owner closes the socket also after the handle
    /// was used, here by two owners on two threads, the other of which
    /// stays alive; a later use is the released error on both threads.
    #[test]
    fn a_used_stream_closes_at_its_last_release() {
        use std::io::{Read, Write};
        use std::sync::mpsc::channel;
        let (handle, mut client) = stream_pair();
        ignite_rt_tcp_retain(handle);
        client.write_all(b"ping").unwrap();
        assert_eq!(read(handle), Ok(b"ping".to_vec()));
        let (used, uses) = channel();
        let (check, checks) = channel::<()>();
        let other = std::thread::spawn(move || {
            let got = read(handle);
            // Released before the main thread hears of the read: its
            // release is then the last one, and its next read cannot block
            // on an open socket.
            ignite_rt_tcp_release(handle);
            used.send(got).unwrap();
            checks.recv().unwrap();
            read(handle)
        });
        client.write_all(b"pong").unwrap();
        assert_eq!(uses.recv().unwrap(), Ok(b"pong".to_vec()));
        ignite_rt_tcp_release(handle);
        let stale = read(handle).expect_err("a released stream is an error");
        assert!(stale.contains("was released"), "{stale}");
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut rest = Vec::new();
        assert_eq!(
            client.read_to_end(&mut rest).unwrap(),
            0,
            "the socket closed"
        );
        check.send(()).unwrap();
        let stale = other
            .join()
            .unwrap()
            .expect_err("released on the other thread");
        assert!(stale.contains("was released"), "{stale}");
    }

    #[test]
    fn a_released_slot_is_reused_under_a_new_handle() {
        let listener = TcpListener::bind("127.0.0.1".to_string(), 0).unwrap();
        let first = register(Handle::Listener(listener.clone()));
        ignite_rt_tcp_release(first);
        let second = register(Handle::Listener(listener));
        assert_ne!(first, second);
        assert!(lookup(second).is_ok());
        let stale = lookup(first).err().expect("a stale handle is an error");
        assert!(stale.message.contains("was released"), "{}", stale.message);
        ignite_rt_tcp_release(first);
        assert!(
            lookup(second).is_ok(),
            "a stale release left the new handle"
        );
        ignite_rt_tcp_release(second);
    }

    /// The thread's read buffer is not borrowed while a read through it
    /// runs: the read may park, another task of the worker reads meanwhile,
    /// and the parked task may move to another worker at its first read
    /// (`ignite_io::fresh`). It is taken out and put back, and kept.
    #[test]
    fn a_read_through_the_buffer_holds_no_borrow_of_it() {
        let input = read_through_buffer(16, |buffer: &mut [u8]| -> Result<usize, ()> {
            let free = READ_BUFFER.with(|cell| cell.try_borrow_mut().is_ok());
            assert!(free, "the thread's read buffer is borrowed across the read");
            buffer[..2].copy_from_slice(b"hi");
            Ok(2)
        })
        .unwrap();
        assert_eq!((input.tag, input.data.len), (0, 2));
        assert!(READ_BUFFER.with(|cell| cell.borrow().capacity()) >= 16);
    }
}
