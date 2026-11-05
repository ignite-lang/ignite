//! `Clock.monotonic()` (`docs/slice.md` 16.13) over `ignite_io::clock`.

/// `i64 clock_monotonic()`: nanoseconds since an arbitrary start of the
/// process (`ignite_io::clock::monotonic`); it never decreases.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_clock_monotonic() -> i64 {
    crate::entry(ignite_io::clock::monotonic)
}

#[cfg(test)]
mod tests {
    use super::ignite_rt_clock_monotonic;

    #[test]
    fn reads_are_ordered() {
        let first = ignite_rt_clock_monotonic();
        let second = ignite_rt_clock_monotonic();
        assert!(first >= 0 && second >= first, "{first} {second}");
    }
}
