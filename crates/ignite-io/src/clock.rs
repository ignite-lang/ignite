//! `Clock.monotonic()` (`docs/slice.md` 16.13): the time since an arbitrary
//! start of the process, in nanoseconds of `Duration`; it never decreases.
//! `Clock.sleep(d)` and `Clock.now()` (21.20): a pause of the calling task
//! and the wall clock.

use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The instant of the first read; every read is measured from it.
static START: OnceLock<Instant> = OnceLock::new();

/// The monotonic time since an arbitrary start of the process, as the base
/// units of `Duration` (nanoseconds). The first read is 0; later reads never
/// decrease and saturate at `i64::MAX`.
pub fn monotonic() -> i64 {
    let start = START.get_or_init(Instant::now);
    i64::try_from(start.elapsed().as_nanos()).unwrap_or(i64::MAX)
}

/// `Clock.sleep(d)`: send what the calling task wrote ([`crate::flush`],
/// a sleep is an operation that waits), then block the task for
/// `nanoseconds`; zero or less returns after the flush.
pub fn sleep(nanoseconds: i64) {
    crate::flush();
    if let Ok(nanoseconds) = u64::try_from(nanoseconds)
        && nanoseconds > 0
    {
        std::thread::sleep(Duration::from_nanos(nanoseconds));
    }
}

/// `Clock.now()`: the wall clock as a `Duration` since
/// 1970-01-01T00:00:00Z in nanoseconds, the instant of `std.time`;
/// negative before it, saturating at the bounds of `i64`.
pub fn now() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_nanos()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_nanos())
            .map(|n| -n)
            .unwrap_or(i64::MIN),
    }
}

#[cfg(test)]
mod tests {
    use super::{monotonic, now, sleep};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    /// 21.20: a sleep waits at least its duration; zero, a negative one
    /// and `i64::MIN` return at once.
    #[test]
    fn sleep_waits_its_duration_and_none_below_zero() {
        let first = monotonic();
        sleep(20_000_000);
        assert!(monotonic() - first >= 20_000_000);
        let again = std::time::Instant::now();
        sleep(0);
        sleep(-5_000_000_000);
        sleep(i64::MIN);
        assert!(again.elapsed() < Duration::from_secs(1));
    }

    /// 21.20: the wall clock in nanoseconds since 1970-01-01T00:00:00Z.
    #[test]
    fn now_is_the_wall_clock_since_the_unix_epoch() {
        let read = || {
            i64::try_from(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
            )
            .unwrap()
        };
        let before = read();
        let at = now();
        let after = read();
        assert!(before <= at && at <= after, "{before} {at} {after}");
    }

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
