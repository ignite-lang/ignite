//! The reports of a failed `assert` and a failed `?` in a test
//! (`docs/slice.md` section 12.5). Both end the process with exit status
//! 101, in a test and outside one alike. Every function is `#[inline]`, so
//! it is compiled only into a crate that calls it: a program without an
//! `assert` links the same code as before tests existed.

/// Exit status of a failed assert, a failed `?` and a panic.
pub const FAILURE_EXIT_CODE: i32 = 101;

/// The report of a failed `assert`, without a trailing newline.
#[inline]
pub fn assert_report(
    cond: &str,
    message: Option<&str>,
    operands: Option<(&str, &str)>,
    at: &str,
) -> String {
    let mut report = format!("assertion failed: {cond}");
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
    report.push_str("\n  at ");
    report.push_str(at);
    report
}

/// The report of a failed `?` in a test: `error` is the shown error of an
/// `Err`, `None` for a `None`.
#[inline]
pub fn try_report(error: Option<&str>, at: &str) -> String {
    match error {
        Some(error) => format!("`?` failed on Err: {error}\n  at {at}"),
        None => format!("`?` failed on None\n  at {at}"),
    }
}

/// A failed `assert`: the report to stderr, then [`crate::exit`] with 101
/// (pending writes sent, terminal restored, stdout flushed).
#[inline]
pub fn assert_failed(
    cond: &str,
    message: Option<&str>,
    operands: Option<(&str, &str)>,
    at: &str,
) -> ! {
    crate::log(&assert_report(cond, message, operands, at));
    crate::exit(FAILURE_EXIT_CODE)
}

/// A failed `?` in a test: the report to stderr, exit 101.
#[inline]
pub fn try_failed(error: Option<&str>, at: &str) -> ! {
    crate::log(&try_report(error, at));
    crate::exit(FAILURE_EXIT_CODE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports() {
        assert_eq!(
            assert_report("a == b", Some("why"), Some(("1", "2")), "f.telos:3:5"),
            "assertion failed: a == b\n  message: why\n  left: 1\n  right: 2\n  at f.telos:3:5"
        );
        assert_eq!(
            assert_report("ok", None, None, "f.telos:1:1"),
            "assertion failed: ok\n  at f.telos:1:1"
        );
        assert_eq!(
            try_report(Some("boom"), "f.telos:2:9"),
            "`?` failed on Err: boom\n  at f.telos:2:9"
        );
        assert_eq!(
            try_report(None, "f.telos:2:9"),
            "`?` failed on None\n  at f.telos:2:9"
        );
    }
}
