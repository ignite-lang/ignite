//! Failure reports (`docs/slice.md` sections 12.5 and 14.8). A failed
//! `assert`, a failed `?` in a test, an arithmetic overflow, an index out
//! of range, a `match` without a matching arm: each builds one report,
//! `<operation> failed at <file>:<line>:<column>: <detail>`, and
//! ends the task that ran it by unwinding with a [`Failure`] payload. The
//! substrate catches it at the task's thread ([`crate::spawn`] logs
//! `task ended with error: <report>` and the thread takes its next task)
//! or in [`crate::run`] (the report is logged as `error: <report>` and the process exits
//! [`FAILURE_EXIT_CODE`]). A Rust panic from any other source is reported
//! the same way, as `panic at <file>:<line>:<column>: <message>`.
//!
//! Both targets format through the `*_report` fns; the LLVM target does
//! not unwind (`docs/llvm.md` section 4) and takes the text only.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::panic::AssertUnwindSafe;
use std::sync::Once;

/// Exit status of a failing main task and of a failing test.
pub const FAILURE_EXIT_CODE: i32 = 101;

/// The payload a Ignite failure unwinds with: its report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub report: String,
}

/// `<op> failed at <file>:<line>:<col>: <detail>`.
pub fn failure_report(op: &str, file: &str, line: usize, col: usize, detail: &str) -> String {
    format!("{op} failed at {file}:{line}:{col}: {detail}")
}

/// End the current task with the report of [`failure_report`].
#[cold]
pub fn fail(op: &str, file: &str, line: usize, col: usize, detail: &str) -> ! {
    fail_with(failure_report(op, file, line, col, detail))
}

/// End the current task with `report`: unwind with a [`Failure`] payload,
/// which [`crate::spawn`] and [`crate::run`] catch.
#[cold]
pub fn fail_with(report: String) -> ! {
    install_hook();
    std::panic::panic_any(Failure { report })
}

/// Log `task ended with error: <report>`, the line of a task that failed.
pub fn task_failed(report: &str) {
    crate::log(&format!("task ended with error: {report}"));
}

/// The report of a failed `assert`: `assert failed at <at>: <cond>`,
/// then `  message: <message>`, `  left: <left>` and `  right: <right>` on
/// lines of their own when given.
#[inline]
pub fn assert_report(
    cond: &str,
    message: Option<&str>,
    operands: Option<(&str, &str)>,
    at: &str,
) -> String {
    let mut report = format!("assert failed at {at}: {cond}");
    if let Some(message) = message {
        report.push_str("\n  message: ");
        report.push_str(message);
    }
    if let Some((left, right)) = operands {
        report.push_str("\n  left: ");
        report.push_str(left);
        report.push_str("\n  right: ");
        report.push_str(right);
    }
    report
}

/// The report of a failed `?` in a test: `? failed at <at>:
/// Err(<error>)`, or `None` for a `None`.
#[inline]
pub fn try_report(error: Option<&str>, at: &str) -> String {
    match error {
        Some(error) => format!("? failed at {at}: Err({error})"),
        None => format!("? failed at {at}: None"),
    }
}

/// A failed `assert`: end the task with [`assert_report`].
#[inline]
pub fn assert_failed(
    cond: &str,
    message: Option<&str>,
    operands: Option<(&str, &str)>,
    at: &str,
) -> ! {
    fail_with(assert_report(cond, message, operands, at))
}

/// A failed `?` in a test: end the task with [`try_report`].
#[inline]
pub fn try_failed(error: Option<&str>, at: &str) -> ! {
    fail_with(try_report(error, at))
}

thread_local! {
    /// The report of the last Rust panic on this thread, written by the
    /// panic hook and taken by the catcher.
    static LAST_PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
    /// How many [`catch`] calls are open on this thread.
    static CATCHING: Cell<usize> = const { Cell::new(0) };
}

static HOOK: Once = Once::new();

/// Install the process's panic hook once. Under a [`catch`] it prints
/// nothing (the catcher logs) and records `panic at <file>:<line>:
/// <col>: <message>` for a panic that is not a [`Failure`]. Outside one
/// (a thread the substrate did not start) a [`Failure`] logs its report
/// and any other panic goes to the hook that was installed before.
///
/// The LLVM runtime calls it at start, after its own hook; [`catch`]
/// still checks, inline, for the Rust target and for tests. The check is
/// one load: `catch` guards every runtime entry, and a call per entry was
/// 3 % of a server's user time (2026-11-08).
#[inline]
pub fn install_hook() {
    if !HOOK.is_completed() {
        install_hook_once();
    }
}

#[cold]
#[inline(never)]
fn install_hook_once() {
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let catching = CATCHING.try_with(|depth| depth.get() > 0).unwrap_or(false);
            if let Some(failure) = info.payload().downcast_ref::<Failure>() {
                if !catching {
                    crate::log(&format!("error: {}", failure.report));
                }
                return;
            }
            if !catching {
                previous(info);
                return;
            }
            let message = panic_message(info.payload());
            let report = match info.location() {
                Some(location) => format!(
                    "panic at {}:{}:{}: {message}",
                    location.file(),
                    location.line(),
                    location.column()
                ),
                None => format!("panic: {message}"),
            };
            let _ = LAST_PANIC.try_with(|last| *last.borrow_mut() = Some(report));
        }));
    });
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| String::from("Box<dyn Any>"))
}

/// The report of a caught panic: a [`Failure`]'s own, else the one the hook
/// recorded for this thread.
pub fn report_of(payload: &(dyn Any + Send)) -> String {
    if let Some(failure) = payload.downcast_ref::<Failure>() {
        return failure.report.clone();
    }
    LAST_PANIC
        .try_with(|last| last.borrow_mut().take())
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("panic: {}", panic_message(payload)))
}

/// Run `f` on this thread, catching a failure or panic as its report.
pub fn catch<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    install_hook();
    let _ = CATCHING.try_with(|depth| depth.set(depth.get() + 1));
    let result = std::panic::catch_unwind(AssertUnwindSafe(f));
    let _ = CATCHING.try_with(|depth| depth.set(depth.get().saturating_sub(1)));
    result.map_err(|payload| report_of(&*payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports() {
        assert_eq!(
            assert_report("a == b", Some("why"), Some(("1", "2")), "f.ignite:3:5"),
            "assert failed at f.ignite:3:5: a == b\n  message: why\n  left: 1\n  right: 2"
        );
        assert_eq!(
            assert_report("ok", None, None, "f.ignite:1:1"),
            "assert failed at f.ignite:1:1: ok"
        );
        assert_eq!(
            try_report(Some("boom"), "f.ignite:2:9"),
            "? failed at f.ignite:2:9: Err(boom)"
        );
        assert_eq!(
            try_report(None, "f.ignite:2:9"),
            "? failed at f.ignite:2:9: None"
        );
        assert_eq!(
            failure_report(
                "+",
                "examples/todo/todo.ignite",
                41,
                17,
                "9223372036854775807 + 1"
            ),
            "+ failed at examples/todo/todo.ignite:41:17: 9223372036854775807 + 1"
        );
    }

    #[test]
    fn a_failure_is_caught_with_its_report() {
        let caught = catch(|| {
            fail(
                "index",
                "f.ignite",
                7,
                3,
                "index 5 out of range for length 2",
            )
        });
        assert_eq!(
            caught.unwrap_err(),
            "index failed at f.ignite:7:3: index 5 out of range for length 2"
        );
        let caught = catch(|| assert_failed("x", None, None, "f.ignite:1:1"));
        assert_eq!(caught.unwrap_err(), "assert failed at f.ignite:1:1: x");
        assert_eq!(catch(|| 5), Ok(5));
    }

    #[test]
    fn a_rust_panic_is_reported_with_its_location() {
        let caught = catch(|| -> () { panic!("boom {}", 1) });
        let report = caught.unwrap_err();
        assert!(report.starts_with("panic at "), "{report}");
        assert!(report.contains("report.rs:"), "{report}");
        assert!(report.ends_with(": boom 1"), "{report}");
    }
}
