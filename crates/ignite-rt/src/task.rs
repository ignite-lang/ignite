//! Tasks (`docs/llvm.md` section 4): one `ignite_io::spawn` task per
//! `spawn`, each with its own arena. `spawn` hands the thread a pointer into
//! the parent's memory and blocks until the thread has copied what it needs
//! and called [`ignite_rt_spawn_ready`].

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::mpsc::{SyncSender, sync_channel};

use crate::arena::ignite_rt_task_end;

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

/// `i8 spawn(ptr fn, ptr env)`: run `fn(env)` as a new task with a new
/// arena; return 1 once that thread called `spawn_ready` (or ended without
/// calling it). The thread's arena is freed when `fn` returns, also if `fn`
/// did not call `task_end`. A null `fn` is fatal (exit 101). When no thread
/// can start, `fn` does not run and the result is 0 at once
/// (`ignite_io::try_spawn` logged why); the caller still owns what `env`
/// names, and the generated code releases the stream handles among them.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_spawn(function: Option<TaskFn>, env: *mut c_void) -> i8 {
    let Some(function) = function else {
        crate::process::die("spawn of a null function");
    };
    let (ready, started) = sync_channel::<()>(1);
    let env = Environment(env);
    let result = ignite_io::try_spawn(move || {
        READY.with(|slot| *slot.borrow_mut() = Some(ready));
        function(env.into_inner());
        ignite_rt_spawn_ready();
        ignite_rt_task_end();
    });
    ignite_io::flush();
    if result.is_err() {
        return 0;
    }
    let _ = started.recv();
    1
}

/// `void spawn_ready()`: let the parent of this task continue. Later calls,
/// and calls on a task that was not spawned, do nothing.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_spawn_ready() {
    let ready = READY
        .try_with(|slot| slot.borrow_mut().take())
        .ok()
        .flatten();
    if let Some(ready) = ready {
        let _ = ready.send(());
    }
}
