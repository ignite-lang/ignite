//! Failure containment for the LLVM target (`docs/llvm.md` section 4,
//! `docs/slice.md` section 14.8). A failed `assert` or `?`, an arithmetic
//! overflow, a division by zero, an index or slice out of range or a
//! non-exhaustive `match` in generated code ends its task without unwinding
//! through the target's frames, which carry no landing pads.
//!
//! The task-entry wrapper the generator emits calls `setjmp` (`returns_twice`,
//! which Rust cannot express) and registers the jump buffer with
//! [`ignite_rt_guard_push`]. A failure builds its report in `ignite-io`'s
//! format, logs it as `task ended with error: <report>`, releases the
//! task's open handles and `longjmp`s back to that wrapper, which frees the
//! task's arena. With no wrapper registered (the main task and a test) the
//! report is logged as `error: <report>` and the process exits
//! [`ignite_io::FAILURE_EXIT_CODE`].
//!
//! A Rust panic inside the runtime ends its task on the same path: the
//! entry it happened in catches it and calls [`fail`] with the panic's
//! report (`crate::contain`), so it never unwinds across the generator's
//! frames.
//!
//! A runtime entry that calls generated code while it holds a Rust resource
//! (the store's lock around `update`'s `f`) must not be jumped over: the
//! generated thunk registers a callback guard ([`ignite_rt_guard_callback`])
//! before it calls the Ignite fn. A failure whose innermost guard is a
//! callback guard keeps its report and jumps back to the thunk, which
//! returns null; the runtime entry releases what it holds and calls
//! [`resume`], which ends the task with the kept report as above.

use std::cell::RefCell;
use std::ffi::{c_int, c_void};

use ignite_io::{FAILURE_EXIT_CODE, assert_report, failure_report, try_report};

use crate::{IgniteList, net};

unsafe extern "C" {
    /// glibc `longjmp`. The matching `setjmp` is a `returns_twice` call in
    /// the generator's IR, so that attribute never sits on a Rust frame.
    fn longjmp(env: *mut c_void, val: c_int) -> !;
}

/// A registered jump buffer and whether it is a callback guard.
#[derive(Clone, Copy)]
struct Handler {
    jb: *mut c_void,
    callback: bool,
}

impl Handler {
    /// Jump back to the `setjmp` of this handler's jump buffer: the task
    /// wrapper's, or the callback guard's.
    fn jump(self) -> ! {
        // SAFETY: a `Handler` is made only by `guard_push` and
        // `guard_callback` from the jump buffer of a still-live generated
        // frame that called `setjmp`, and is popped before its jump, so
        // `jb` is live; the frames between it and here are generated code
        // and this runtime's failure entries, which hold nothing that needs
        // a destructor beyond what the jump of a task wrapper already skips
        // (crate docs).
        unsafe { longjmp(self.jb, 1) }
    }
}

thread_local! {
    /// Jump buffers of the task-entry wrappers and callback guards open on
    /// this thread, the innermost last. A task registers one; a nested
    /// guard pushes another for its duration.
    static HANDLERS: RefCell<Vec<Handler>> = const { RefCell::new(Vec::new()) };
    /// The report of a failure stopped at a callback guard, until the
    /// runtime entry that called the callback [`resume`]s it.
    static PENDING: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// A task's jump buffers and kept report while another task runs on its
/// worker (`ignite_io::TaskLocals`, IO mode `readiness`).
#[derive(Default)]
pub(crate) struct TaskHandlers {
    handlers: Vec<Handler>,
    pending: Option<String>,
}

impl TaskHandlers {
    /// Exchange the saved handlers with this thread's.
    pub(crate) fn swap(&mut self) {
        HANDLERS.with(|handlers| std::mem::swap(&mut *handlers.borrow_mut(), &mut self.handlers));
        PENDING.with(|pending| std::mem::swap(&mut *pending.borrow_mut(), &mut self.pending));
    }
}

fn push(handler: Handler) {
    let _ = ignite_io::fresh::try_with(&HANDLERS, |handlers| handlers.borrow_mut().push(handler));
}

/// `void guard_push(ptr jb)`: register the jump buffer `jb` of a task-entry
/// wrapper that just called `setjmp`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_guard_push(jb: *mut c_void) {
    crate::entry(move || {
        push(Handler {
            jb,
            callback: false,
        });
    })
}

/// `void guard_callback(ptr jb)`: register the jump buffer `jb` of a
/// generated callback (`store_update.<T>`) that just called `setjmp`; a
/// failure inside the callback jumps back to it instead of ending the task
/// (crate docs).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_guard_callback(jb: *mut c_void) {
    crate::entry(move || {
        push(Handler { jb, callback: true });
    })
}

/// `void guard_pop()`: drop the innermost jump buffer after its task body
/// or callback returned without failing.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_guard_pop() {
    crate::entry(move || {
        let _ = ignite_io::fresh::try_with(&HANDLERS, |handlers| handlers.borrow_mut().pop());
    })
}

/// End the current task with the report of the failure a callback guard
/// stopped, once the runtime entry that called the callback has released
/// what it held.
pub(crate) fn resume() -> ! {
    let report = ignite_io::fresh::try_with(&PENDING, |pending| pending.borrow_mut().take())
        .ok()
        .flatten()
        .unwrap_or_else(|| String::from("a callback failed without a report"));
    fail(report)
}

/// The text of a `Text*` argument, lossily decoded; empty for null.
fn text(value: *const IgniteList) -> String {
    String::from_utf8_lossy(crate::bytes(value)).into_owned()
}

/// End the current task with `report`: release its handles, then jump back
/// to the innermost task wrapper (`task ended with error: <report>`); with
/// none (the main task or a test), log `error: <report>` and exit
/// [`FAILURE_EXIT_CODE`]. When the innermost guard is a callback guard,
/// keep `report` for [`resume`] and jump back to the callback instead.
pub(crate) fn fail(report: String) -> ! {
    let handler = ignite_io::fresh::try_with(&HANDLERS, |handlers| handlers.borrow_mut().pop())
        .ok()
        .flatten();
    if let Some(guard @ Handler { callback: true, .. }) = handler {
        let _ =
            ignite_io::fresh::try_with(&PENDING, |pending| *pending.borrow_mut() = Some(report));
        guard.jump()
    }
    net::release_task_handles();
    match handler {
        Some(wrapper) => {
            ignite_io::task_failed(&report);
            ignite_io::flush();
            drop(report);
            wrapper.jump()
        }
        None => {
            ignite_io::log(&format!("error: {report}"));
            ignite_io::exit(FAILURE_EXIT_CODE)
        }
    }
}

/// `void fail_arith(Text* op, Text* file, i64 line, i64 col, i64 left,
/// i64 right)`: a binary `+`, `-`, `*`, `/` or `%` that overflowed or
/// divided by zero. The detail is `<left> <op> <right>` in base units.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_fail_arith(
    op: *const IgniteList,
    file: *const IgniteList,
    line: i64,
    col: i64,
    left: i64,
    right: i64,
) -> ! {
    fail(crate::entry(move || {
        let op = text(op);
        let detail = format!("{left} {op} {right}");
        failure_report(&op, &text(file), line as usize, col as usize, &detail)
    }))
}

/// `void fail_neg(Text* file, i64 line, i64 col, i64 value)`: a unary `-`
/// that overflowed (`-i64::MIN`). The detail is `-(<value>)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_fail_neg(
    file: *const IgniteList,
    line: i64,
    col: i64,
    value: i64,
) -> ! {
    fail(crate::entry(move || {
        failure_report(
            "-",
            &text(file),
            line as usize,
            col as usize,
            &format!("-({value})"),
        )
    }))
}

/// `void fail_index(Text* file, i64 line, i64 col, i64 index, i64 len)`: an
/// index out of range.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_fail_index(
    file: *const IgniteList,
    line: i64,
    col: i64,
    index: i64,
    len: i64,
) -> ! {
    fail(crate::entry(move || {
        failure_report(
            "index",
            &text(file),
            line as usize,
            col as usize,
            &format!("index {index} out of range for length {len}"),
        )
    }))
}

/// `void fail_slice(Text* file, i64 line, i64 col, i64 from, i64 to,
/// i64 len)`: a slice out of range.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_fail_slice(
    file: *const IgniteList,
    line: i64,
    col: i64,
    from: i64,
    to: i64,
    len: i64,
) -> ! {
    fail(crate::entry(move || {
        failure_report(
            "slice",
            &text(file),
            line as usize,
            col as usize,
            &format!("slice {from}..{to} out of range for length {len}"),
        )
    }))
}

/// `void fail_match(Text* file, i64 line, i64 col)`: a `match` with no arm
/// for the value.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_fail_match(file: *const IgniteList, line: i64, col: i64) -> ! {
    fail(crate::entry(move || {
        failure_report(
            "match",
            &text(file),
            line as usize,
            col as usize,
            "no arm matched",
        )
    }))
}

/// `void assert_failed(Text* cond, Text* message, Text* left, Text* right,
/// Text* at)`: end the task with [`ignite_io::assert_report`]. `message` is
/// null without a message, `left` and `right` are null unless the condition
/// compares.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_assert_failed(
    cond: *const IgniteList,
    message: *const IgniteList,
    left: *const IgniteList,
    right: *const IgniteList,
    at: *const IgniteList,
) -> ! {
    fail(crate::entry(move || {
        let message = optional(message);
        let operands = match (optional(left), optional(right)) {
            (Some(left), Some(right)) => Some((left, right)),
            _ => None,
        };
        assert_report(
            &text(cond),
            message.as_deref(),
            operands
                .as_ref()
                .map(|(left, right)| (left.as_str(), right.as_str())),
            &text(at),
        )
    }))
}

/// `void try_failed(Text* error, Text* at)`: end the task with
/// [`ignite_io::try_report`]; `error` is null for a `None`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_try_failed(error: *const IgniteList, at: *const IgniteList) -> ! {
    fail(crate::entry(move || {
        try_report(optional(error).as_deref(), &text(at))
    }))
}

/// The text of a `Text*` argument, or `None` when it is null.
fn optional(value: *const IgniteList) -> Option<String> {
    (!value.is_null()).then(|| text(value))
}
