//! The open-files limit (`docs/slice.md` 28): [`raise_open_files_limit`]
//! runs once at the process entry of both targets, before `K.main`;
//! [`open_files_limit`] is `Process.open_files_limit()`.

use std::sync::Once;

use nix::sys::resource::{RLIM_INFINITY, Resource, getrlimit, setrlimit};

/// The soft limit under which a failed raise is reported: below it a
/// server runs out of descriptors under ordinary load.
pub const OPEN_FILES_REPORT_BELOW: u64 = 4096;

static RAISE: Once = Once::new();

/// Raise the open-files soft limit to the hard limit, once per process.
/// When the hard limit is unlimited, the kernel's ceiling
/// `/proc/sys/fs/nr_open` is the target, as `setrlimit` refuses an
/// unlimited `RLIMIT_NOFILE`. A failure is ignored; it is reported once on
/// stderr only when the soft limit stays under
/// [`OPEN_FILES_REPORT_BELOW`].
pub fn raise_open_files_limit() {
    RAISE.call_once(|| {
        if let Some(line) = raise() {
            crate::log(&line);
        }
    });
}

/// The raise itself: the line to report, if any.
fn raise() -> Option<String> {
    let (soft, hard) = match getrlimit(Resource::RLIMIT_NOFILE) {
        Ok(limits) => limits,
        Err(error) => return Some(format!("warning: open-files limit unknown: {error}")),
    };
    let target = if hard == RLIM_INFINITY {
        nr_open().unwrap_or(soft)
    } else {
        hard
    };
    if target <= soft {
        return None;
    }
    match setrlimit(Resource::RLIMIT_NOFILE, target, hard) {
        Ok(()) => None,
        Err(error) => report(soft, &error.to_string()),
    }
}

/// The report of a failed raise from `soft`: a line when `soft` is under
/// [`OPEN_FILES_REPORT_BELOW`], else nothing.
fn report(soft: u64, error: &str) -> Option<String> {
    (soft < OPEN_FILES_REPORT_BELOW)
        .then(|| format!("warning: open-files limit stays at {soft}: {error}"))
}

/// The kernel's per-process ceiling on descriptors.
fn nr_open() -> Option<u64> {
    std::fs::read_to_string("/proc/sys/fs/nr_open")
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// `Process.open_files_limit()`: the current open-files soft limit; an
/// unlimited one, or one past `Int`, is `i64::MAX`, and an unreadable one
/// 0.
pub fn open_files_limit() -> i64 {
    match getrlimit(Resource::RLIMIT_NOFILE) {
        Ok((soft, _)) if soft == RLIM_INFINITY => i64::MAX,
        Ok((soft, _)) => i64::try_from(soft).unwrap_or(i64::MAX),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{open_files_limit, raise_open_files_limit, report};

    /// 27: a failed raise is reported only from a soft limit under 4,096.
    #[test]
    fn a_failure_is_reported_only_under_4096() {
        assert_eq!(
            report(256, "EPERM").as_deref(),
            Some("warning: open-files limit stays at 256: EPERM")
        );
        assert!(report(4095, "EPERM").is_some());
        assert_eq!(report(4096, "EPERM"), None);
        assert_eq!(report(65536, "EPERM"), None);
    }

    /// 27: after the raise the soft limit equals the hard limit (or the
    /// kernel's ceiling when the hard limit is unlimited).
    #[test]
    fn the_raise_reaches_the_hard_limit() {
        raise_open_files_limit();
        raise_open_files_limit();
        let (soft, hard) =
            nix::sys::resource::getrlimit(nix::sys::resource::Resource::RLIMIT_NOFILE).unwrap();
        if hard != nix::sys::resource::RLIM_INFINITY {
            assert_eq!(soft, hard);
        }
        assert_eq!(open_files_limit(), i64::try_from(soft).unwrap_or(i64::MAX));
    }
}
