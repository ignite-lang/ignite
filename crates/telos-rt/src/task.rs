//! Tasks (`docs/llvm.md` section 4): one `telos_io::spawn` task per
//! `spawn`, each with its own arena. `spawn` hands the thread a pointer into
//! the parent's memory and blocks until the thread has copied what it needs
//! and called [`telos_rt_spawn_ready`].

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::mpsc::{SyncSender, sync_channel};

use crate::arena::telos_rt_task_end;

/// A task's entry point: `void fn(ptr env)`.
pub type TaskFn = extern "C" fn(*mut c_void);

thread_local! {
    /// The parent's wake-up of a spawned task that has not called
    /// `spawn_ready` yet.
    static READY: RefCell<Option<SyncSender<()>>> = const { RefCell::new(None) };
}

/// The environment pointer, moved to the new task. The parent keeps the
/// memory alive until `spawn_ready`, and does not touch it meanwhile.
struct Environment(*mut c_void);

// SAFETY: see `Environment`; the parent is blocked while the child reads.
unsafe impl Send for Environment {}

impl Environment {
    fn into_inner(self) -> *mut c_void {
        self.0
    }
}

/// `void spawn(ptr fn, ptr env)`: run `fn(env)` as a new task with a new
/// arena; return once that thread called `spawn_ready` (or ended without
/// calling it). The thread's arena is freed when `fn` returns, also if `fn`
/// did not call `task_end`. A null `fn` is fatal (exit 101). When no thread
/// can start, `fn` does not run and `spawn` returns at once
/// (`telos_io::spawn` logged why).
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_spawn(function: Option<TaskFn>, env: *mut c_void) {
    let Some(function) = function else {
        crate::process::die("spawn of a null function");
    };
    let (ready, started) = sync_channel::<()>(1);
    let env = Environment(env);
    telos_io::spawn(move || {
        READY.with(|slot| *slot.borrow_mut() = Some(ready));
        function(env.into_inner());
        telos_rt_spawn_ready();
        telos_rt_task_end();
    });
    telos_io::flush();
    let _ = started.recv();
}

/// `i8 spawn_copy(ptr fn, ptr env, i64 size)`: run `fn(copy)` on another
/// thread with a new arena, where `copy` is a copy of the `size` bytes at
/// `env` that lives until `fn` returns; return 1 at once. When no thread
/// can start, `fn` does not run and the result is 0 (`telos_io::try_spawn`
/// logged why); the caller still owns what `env` names. The bytes are
/// copied shallowly, so every pointer in them must stay valid while the
/// task may read it: the generated service `main` passes the stream handle
/// and the `application` value, which it evaluated before its accept loop
/// and never frees. The thread's arena is freed when `fn` returns.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_spawn_copy(
    function: Option<TaskFn>,
    env: *const c_void,
    size: i64,
) -> i8 {
    let Some(function) = function else {
        crate::process::die("spawn of a null function");
    };
    let size = usize::try_from(size).unwrap_or(0);
    if size <= INLINE_ENV * 8 {
        // An environment this small travels inside the task's closure, so
        // starting the task allocates once.
        let mut copy = [0u64; INLINE_ENV];
        // SAFETY: the caller passes `size` readable bytes at `env`; `copy`
        // holds at least `size` bytes and does not overlap them.
        unsafe { std::ptr::copy_nonoverlapping(env.cast::<u8>(), copy.as_mut_ptr().cast(), size) };
        return started(telos_io::try_spawn(move || {
            let mut copy = copy;
            function(copy.as_mut_ptr().cast());
            telos_rt_task_end();
        }));
    }
    let mut copy = vec![0u64; size.div_ceil(8)].into_boxed_slice();
    // SAFETY: as above.
    unsafe { std::ptr::copy_nonoverlapping(env.cast::<u8>(), copy.as_mut_ptr().cast(), size) };
    started(telos_io::try_spawn(move || {
        let mut copy = copy;
        function(copy.as_mut_ptr().cast());
        telos_rt_task_end();
    }))
}

/// 1 when the task started, 0 when not.
fn started(result: Result<(), telos_io::IoError>) -> i8 {
    i8::from(result.is_ok())
}

/// Words of a `spawn_copy` environment carried inline.
const INLINE_ENV: usize = 8;

/// `void spawn_ready()`: let the parent of this task continue. Later calls,
/// and calls on a task that was not spawned, do nothing.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_spawn_ready() {
    let ready = READY
        .try_with(|slot| slot.borrow_mut().take())
        .ok()
        .flatten();
    if let Some(ready) = ready {
        let _ = ready.send(());
    }
}
