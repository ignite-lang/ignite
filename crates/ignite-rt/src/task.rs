//! Tasks (`docs/llvm.md` section 4): one `ignite_io::spawn` task per
//! `spawn`, each with its own arena. `spawn` hands the task a pointer into
//! the parent's memory and waits until the task has copied what it needs
//! and called [`ignite_rt_spawn_ready`].
//!
//! The wait parks the parent's waiter (`ignite_io::wait`), so a parent
//! that is itself a task on a worker (IO mode `readiness`) lets the
//! worker run the child meanwhile. In that mode several tasks share a
//! worker thread, so the runtime's per-task thread-locals (the arena and
//! its bump state, the failure handlers, the owned handles and cached
//! stream, the pending `spawn_ready`) are exchanged at every switch:
//! [`Locals`], registered with `ignite_io::set_task_locals`.

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ignite_io::wait::{Waiter, park, wake};

use crate::arena::{TaskArena, task_end};
use crate::fail::TaskHandlers;
use crate::net::TaskHandles;

/// A task's entry point: `void fn(ptr env)`.
pub type TaskFn = extern "C" fn(*mut c_void);

/// The parent's side of the handshake: set, and the parent woken, once
/// the child called `spawn_ready` or ended or was dropped without it.
struct Ready {
    done: AtomicBool,
    parent: Waiter,
}

/// The child's side of the handshake; dropping it lets the parent go on.
struct Started(Arc<Ready>);

impl Drop for Started {
    fn drop(&mut self) {
        self.0.done.store(true, Ordering::Release);
        wake(&self.0.parent);
    }
}

thread_local! {
    /// The handshake of a spawned task that has not called `spawn_ready`
    /// yet.
    static READY: RefCell<Option<Started>> = const { RefCell::new(None) };
}

/// The environment pointer, moved to the new task. The parent keeps the
/// memory alive until `spawn_ready`, and does not touch it meanwhile.
struct Environment(*mut c_void);

// SAFETY: see `Environment`; the parent waits while the child reads.
unsafe impl Send for Environment {}

impl Environment {
    fn into_inner(self) -> *mut c_void {
        self.0
    }
}

/// The runtime's per-task thread-locals of a task that is not running
/// (module docs).
struct Locals {
    arena: TaskArena,
    handlers: TaskHandlers,
    handles: TaskHandles,
    ready: Option<Started>,
}

impl ignite_io::TaskLocals for Locals {
    fn swap(&mut self) {
        self.arena.swap();
        self.handlers.swap();
        self.handles.swap();
        READY.with(|ready| std::mem::swap(&mut *ready.borrow_mut(), &mut self.ready));
    }
}

fn new_locals() -> Box<dyn ignite_io::TaskLocals> {
    Box::new(Locals {
        arena: TaskArena::new(),
        handlers: TaskHandlers::default(),
        handles: TaskHandles::default(),
        ready: None,
    })
}

/// `i8 spawn(ptr fn, ptr env)`: run `fn(env)` as a new task with a new
/// arena; return 1 once that task called `spawn_ready` (or ended without
/// calling it). The task's arena is freed when `fn` returns, also if `fn`
/// did not call `task_end`. A null `fn` is fatal (exit 101). When no task
/// can start, `fn` does not run and the result is 0 at once
/// (`ignite_io::try_spawn` logged why); the caller still owns what `env`
/// names, and the generated code releases the stream handles among them.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_spawn(function: Option<TaskFn>, env: *mut c_void) -> i8 {
    crate::entry(move || {
        let Some(function) = function else {
            crate::process::die("spawn of a null function");
        };
        ignite_io::set_task_locals(new_locals);
        let ready = Arc::new(Ready {
            done: AtomicBool::new(false),
            parent: Waiter::current(),
        });
        let started = Started(Arc::clone(&ready));
        let env = Environment(env);
        let result = ignite_io::try_spawn(move || {
            READY.with(|slot| *slot.borrow_mut() = Some(started));
            function(env.into_inner());
            spawn_ready();
            task_end();
            crate::net::task_ended();
        });
        ignite_io::flush();
        if result.is_err() {
            return 0;
        }
        while !ready.done.load(Ordering::Acquire) {
            park(&ready.parent, None);
        }
        1
    })
}

/// `void spawn_ready()`: let the parent of this task continue. Later calls,
/// and calls on a task that was not spawned, do nothing.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_spawn_ready() {
    crate::entry(spawn_ready)
}

/// The body of [`ignite_rt_spawn_ready`], which [`ignite_rt_spawn`]'s
/// task calls from Rust after the task's function returned (as
/// `arena::task_end`).
fn spawn_ready() {
    let started = READY
        .try_with(|slot| slot.borrow_mut().take())
        .ok()
        .flatten();
    drop(started);
}
