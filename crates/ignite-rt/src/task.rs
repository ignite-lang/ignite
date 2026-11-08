//! Tasks (`docs/llvm.md` section 4): one `ignite_io::spawn` task per
//! `spawn`, each with its own arena. The spawning task builds the new
//! task's environment in transfer memory ([`ignite_rt_spawn_begin`]),
//! which `spawn` hands to the new task with the environment pointer; it
//! returns at once. The task's arena starts with that memory
//! ([`crate::arena::adopt`]), so the task reads its arguments where they
//! were copied, and its end frees them with the rest of its arena.
//!
//! Until story spawn-without-handshake the environment lay in the parent's
//! memory and `spawn` blocked on a channel until the task had copied it:
//! the parent's wait and the task's wake-up were 1.9 of the 5.2 futex
//! calls per connection of the terminal example server under connect-8
//! (plain Rust 2.8; 2026-11-08).
//!
//! In the IO mode `readiness` several tasks share a worker thread, so the
//! runtime's per-task thread-locals (the arena with its bump state and
//! transfer arena, the failure handlers, the owned handles and cached
//! stream, the transfer memory not adopted) are exchanged whenever a
//! different task resumes: [`Locals`], registered with
//! `ignite_io::set_task_locals`.

use std::cell::RefCell;
use std::ffi::c_void;

use crate::arena::{TaskArena, Transfer, adopt, task_end, transfer_begin, transfer_end};
use crate::fail::TaskHandlers;
use crate::net::TaskHandles;

/// A task's entry point: `void fn(ptr env)`.
pub type TaskFn = extern "C" fn(*mut c_void);

thread_local! {
    /// The transfer memory of the running task's environment when its
    /// arena could not adopt it, until the task ends or calls
    /// `spawn_ready`.
    static READY: RefCell<Option<Transfer>> = const { RefCell::new(None) };
}

/// The environment pointer, moved to the new task with the transfer memory
/// it points into.
struct Environment {
    env: *mut c_void,
    transfer: Option<Transfer>,
}

// SAFETY: the environment lies in the transfer memory moved with it (or,
// for a caller that began no transfer, in memory that caller keeps alive
// and unchanged until the task's `spawn_ready`); the spawning task gave
// that arena up at `transfer_end` and touches neither after `spawn`.
unsafe impl Send for Environment {}

impl Environment {
    fn into_parts(self) -> (*mut c_void, Option<Transfer>) {
        (self.env, self.transfer)
    }
}

/// The runtime's per-task thread-locals of a task that is not running
/// (module docs).
struct Locals {
    arena: TaskArena,
    handlers: TaskHandlers,
    handles: TaskHandles,
    ready: Option<Transfer>,
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

/// `void spawn_begin()`: later allocations of this task go to a new
/// transfer arena, until the next `spawn` hands it to the task it starts.
/// Generated code builds the environment there: the environment block and
/// a deep copy of every argument.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_spawn_begin() {
    crate::entry(transfer_begin)
}

/// `i8 spawn(ptr fn, ptr env)`: run `fn(env)` as a new task with a new
/// arena and return 1 without waiting for it. The transfer memory begun by
/// `spawn_begin` becomes the bottom of the task's arena; this task
/// allocates in its own arenas again. The task's arena is freed when `fn`
/// returns, also if `fn` did not call `task_end`. A null `fn` is fatal (exit 101). When no task can
/// start, `fn` does not run, the transfer memory is freed and the result is
/// 0 at once (`ignite_io::try_spawn` logged why); the caller still owns the
/// stream handles `env` names, and the generated code releases them.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_spawn(function: Option<TaskFn>, env: *mut c_void) -> i8 {
    crate::entry(move || {
        let transfer = transfer_end();
        let Some(function) = function else {
            crate::process::die("spawn of a null function");
        };
        ignite_io::set_task_locals(new_locals);
        let environment = Environment { env, transfer };
        let result = ignite_io::try_spawn(move || {
            let (env, transfer) = environment.into_parts();
            let kept = transfer.and_then(adopt);
            READY.with(|slot| *slot.borrow_mut() = kept);
            function(env);
            spawn_ready();
            task_end();
            crate::net::task_ended();
        });
        ignite_io::flush();
        i8::from(result.is_ok())
    })
}

/// `void spawn_ready()`: the task no longer reads its environment. When
/// its transfer memory could not become part of its arena (the task's
/// arena was not empty), free it now instead of at the task's end;
/// otherwise nothing. Generated code does not call it. Later calls, and
/// calls on a task that was not spawned, do nothing.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_spawn_ready() {
    crate::entry(spawn_ready)
}

/// The body of [`ignite_rt_spawn_ready`], which [`ignite_rt_spawn`]'s
/// task calls from Rust after the task's function returned (as
/// `arena::task_end`).
fn spawn_ready() {
    let transfer = READY
        .try_with(|slot| slot.borrow_mut().take())
        .ok()
        .flatten();
    drop(transfer);
}
