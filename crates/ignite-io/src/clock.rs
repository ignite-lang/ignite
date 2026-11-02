//! `Clock.monotonic()` (`docs/slice.md` 16.13): the time since an arbitrary
//! start of the process, in nanoseconds of `Duration`; it never decreases.

use std::sync::OnceLock;
use std::time::Instant;

/// The instant of the first read; every read is measured from it.
static START: OnceLock<Instant> = OnceLock::new();

/// The monotonic time since an arbitrary start of the process, as the base
/// units of `Duration` (nanoseconds). The first read is 0; later reads never
/// decrease and saturate at `i64::MAX`.
pub fn monotonic() -> i64 {
    let start = START.get_or_init(Instant::now);
    i64::try_from(start.elapsed().as_nanos()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::monotonic;

    #[test]
    fn two_reads_are_ordered() {
        let first = monotonic();
        let second = monotonic();
        assert!(first >= 0);
        assert!(second >= first, "{second} < {first}");
    }

    #[test]
    fn time_passes() {
        let first = monotonic();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = monotonic();
        assert!(second - first >= 2_000_000, "{first} {second}");
    }
}
