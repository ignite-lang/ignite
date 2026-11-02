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
//! A Rust panic inside the runtime is a different path: it unwinds to
//! `ignite_io::spawn`'s `catch_unwind` at the task's thread (`crate::task`),
//! never across the generator's frames; a main-thread panic is reported in
//! the same format by the hook `crate::process` installs.

use std::cell::RefCell;
use std::ffi::{c_int, c_void};

use ignite_io::{FAILURE_EXIT_CODE, assert_report, failure_report, try_report};

use crate::{IgniteList, net};

unsafe extern "C" {
    /// glibc `longjmp`. The matching `setjmp` is a `returns_twice` call in
    /// the generator's IR, so that attribute never sits on a Rust frame.
    fn longjmp(env: *mut c_void, val: c_int) -> !;
}

thread_local! {
    /// Jump buffers of the task-entry wrappers open on this thread, the
    /// innermost last. A task registers one; a nested guard pushes another
    /// for its duration.
    static HANDLERS: RefCell<Vec<*mut c_void>> = const { RefCell::new(Vec::new()) };
}

/// `void guard_push(ptr jb)`: register the jump buffer `jb` of a task-entry
/// wrapper that just called `setjmp`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_guard_push(jb: *mut c_void) {
    let _ = HANDLERS.try_with(|handlers| handlers.borrow_mut().push(jb));
}

/// `void guard_pop()`: drop the innermost jump buffer after its task body
/// returned without failing.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_guard_pop() {
    let _ = HANDLERS.try_with(|handlers| handlers.borrow_mut().pop());
}

/// The text of a `Text*` argument, lossily decoded; empty for null.
fn text(value: *const IgniteList) -> String {
    String::from_utf8_lossy(crate::bytes(value)).into_owned()
}

/// End the current task with `report`: release its handles, then jump back
/// to the innermost task wrapper (`task ended with error: <report>`); with
/// none (the main task or a test), log `error: <report>` and exit
/// [`FAILURE_EXIT_CODE`].
pub(crate) fn fail(report: String) -> ! {
    net::release_task_handles();
    let jb = HANDLERS
        .try_with(|handlers| handlers.borrow_mut().pop())
        .ok()
        .flatten();
    match jb {
        Some(jb) => {
            ignite_io::task_failed(&report);
            ignite_io::flush();
            drop(report);
            // SAFETY: `jb` is a jump buffer a still-live task wrapper set
            // with `setjmp` and registered through `guard_push`; the C
            // frames between it and here carry no destructors (crate docs).
            unsafe { longjmp(jb, 1) }
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
    let op = text(op);
    let detail = format!("{left} {op} {right}");
    fail(failure_report(
        &op,
        &text(file),
        line as usize,
        col as usize,
        &detail,
    ))
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
    fail(failure_report(
        "-",
        &text(file),
        line as usize,
        col as usize,
        &format!("-({value})"),
    ))
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
    fail(failure_report(
        "index",
        &text(file),
        line as usize,
        col as usize,
        &format!("index {index} out of range for length {len}"),
    ))
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
    fail(failure_report(
        "slice",
        &text(file),
        line as usize,
        col as usize,
        &format!("slice {from}..{to} out of range for length {len}"),
    ))
}

/// `void fail_match(Text* file, i64 line, i64 col)`: a `match` with no arm
/// for the value.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_fail_match(file: *const IgniteList, line: i64, col: i64) -> ! {
    fail(failure_report(
        "match",
        &text(file),
        line as usize,
        col as usize,
        "no arm matched",
    ))
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
    let message = optional(message);
    let operands = match (optional(left), optional(right)) {
        (Some(left), Some(right)) => Some((left, right)),
        _ => None,
    };
    let report = assert_report(
        &text(cond),
        message.as_deref(),
        operands
            .as_ref()
            .map(|(left, right)| (left.as_str(), right.as_str())),
        &text(at),
    );
    fail(report)
}

/// `void try_failed(Text* error, Text* at)`: end the task with
/// [`ignite_io::try_report`]; `error` is null for a `None`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_try_failed(error: *const IgniteList, at: *const IgniteList) -> ! {
    let error = optional(error);
    fail(try_report(error.as_deref(), &text(at)))
}

/// The text of a `Text*` argument, or `None` when it is null.
fn optional(value: *const IgniteList) -> Option<String> {
    (!value.is_null()).then(|| text(value))
}
