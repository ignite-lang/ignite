//! `Clock.monotonic()` (`docs/slice.md` 16.13) over `ignite_io::clock`.

/// `i64 clock_monotonic()`: nanoseconds since an arbitrary start of the
/// process (`ignite_io::clock::monotonic`); it never decreases.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_clock_monotonic() -> i64 {
    crate::entry(ignite_io::clock::monotonic)
}

/// `void clock_sleep(i64 nanoseconds)`: `Clock.sleep(d)` (21.20): flush the
/// task's writes, then block it; zero or less does not block.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_clock_sleep(nanoseconds: i64) {
    crate::entry(move || ignite_io::clock::sleep(nanoseconds))
}

/// `i64 clock_now()`: `Clock.now()` (21.20): the wall clock in nanoseconds
/// since 1970-01-01T00:00:00Z.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_clock_now() -> i64 {
    crate::entry(ignite_io::clock::now)
}

#[cfg(test)]
mod tests {
    use super::{ignite_rt_clock_monotonic, ignite_rt_clock_now, ignite_rt_clock_sleep};

    #[test]
    fn reads_are_ordered() {
        let first = ignite_rt_clock_monotonic();
        let second = ignite_rt_clock_monotonic();
        assert!(first >= 0 && second >= first, "{first} {second}");
    }

    /// 21.20: a sleep waits at least its duration; a negative one not.
    #[test]
    fn sleep_waits() {
        let first = ignite_rt_clock_monotonic();
        ignite_rt_clock_sleep(10_000_000);
        ignite_rt_clock_sleep(-1);
        assert!(ignite_rt_clock_monotonic() - first >= 10_000_000);
    }

    /// 21.20: the wall clock in nanoseconds since the Unix epoch.
    #[test]
    fn now_is_after_2026() {
        assert!(ignite_rt_clock_now() > 1_767_225_600 * 1_000_000_000);
    }
}
