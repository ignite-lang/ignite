//! The driver's clock (`docs/slice.md` 16.4): every event carries `at`, a
//! `Duration` since the journal's epoch in nanoseconds, and reducers read
//! no clock. [`Real`] offsets the monotonic clock by the segment's start;
//! [`Manual`] moves only on `advance`, firing every due timer in `(at,
//! set order)` order before `now` reaches the target. The settle rule
//! between instants is [`super::control::Adapters::settle`] (O29).

use std::time::Instant;

use ignite_syntax::ast::QUANTITY_UNITS;

/// The clock of `--clock real`: nanoseconds since the segment started.
#[derive(Clone, Copy, Debug)]
pub struct Real {
    start: Instant,
}

impl Real {
    /// A clock whose zero is now.
    pub fn start() -> Real {
        Real {
            start: Instant::now(),
        }
    }

    /// Nanoseconds since the start; never decreases.
    pub fn now(&self) -> i64 {
        i64::try_from(self.start.elapsed().as_nanos()).unwrap_or(i64::MAX)
    }
}

/// The manual clock of `--clock manual`: `now` in base units of `Duration`
/// since the epoch. Timers are `(at, id)` pairs in the order they were set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manual {
    pub now: i64,
}

impl Manual {
    /// `advance <quantity>` in one step: the timers due at or before
    /// `target`, in `(at, set order)` order; `now` becomes `target`
    /// afterwards (it never moves back). The engine that fires them one
    /// by one, settling between instants, uses [`Manual::next_due`] and
    /// [`Manual::advance_to`] instead.
    pub fn advance(&mut self, target: i64, timers: &[(i64, i64)]) -> Vec<(i64, i64)> {
        let mut due: Vec<(i64, i64)> = timers
            .iter()
            .copied()
            .filter(|(at, _)| *at <= target)
            .collect();
        due.sort_by_key(|(at, _)| *at);
        self.advance_to(target);
        due
    }

    /// The earliest timer due at or before `target` (the first set among
    /// equals); `now` moves to its instant (never back). `None` leaves
    /// `now` where it is.
    pub fn next_due(&mut self, target: i64, timers: &[(i64, i64)]) -> Option<(i64, i64)> {
        let (at, id) = timers
            .iter()
            .copied()
            .filter(|(at, _)| *at <= target)
            .min_by_key(|(at, _)| *at)?;
        self.advance_to(at);
        Some((at, id))
    }

    /// `now` becomes `target` unless it is already later.
    pub fn advance_to(&mut self, target: i64) {
        if target > self.now {
            self.now = target;
        }
    }
}

/// A `Duration` literal (`docs/slice.md` 14.1) as nanoseconds: an integer,
/// optional whitespace and a unit (`ns`, `us`, `ms`, `second(s)`,
/// `minute(s)`, `hour(s)`, `day(s)`, and `s` for `--settle 1 s`). `None`
/// when malformed or out of range.
pub fn parse_duration(text: &str) -> Option<i64> {
    let text = text.trim();
    let digits = text
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_digit())
        .count();
    if digits == 0 {
        return None;
    }
    let (number, unit) = text.split_at(digits);
    let unit = unit.trim_start();
    let factor = match unit {
        "s" => 1_000_000_000,
        _ => QUANTITY_UNITS
            .iter()
            .find(|(name, kind, _)| *name == unit && *kind == "Duration")
            .map(|(_, _, factor)| *factor)?,
    };
    number.parse::<i64>().ok()?.checked_mul(factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_parse() {
        assert_eq!(parse_duration("100 ms"), Some(100_000_000));
        assert_eq!(parse_duration("2 seconds"), Some(2_000_000_000));
        assert_eq!(parse_duration("1 second"), Some(1_000_000_000));
        assert_eq!(parse_duration("1 s"), Some(1_000_000_000));
        assert_eq!(parse_duration("1s"), Some(1_000_000_000));
        assert_eq!(parse_duration(" 30 days "), Some(2_592_000_000_000_000));
        assert_eq!(parse_duration("0 ns"), Some(0));
        assert_eq!(parse_duration("ms"), None);
        assert_eq!(parse_duration("1"), None);
        assert_eq!(parse_duration("1 bytes"), None);
        assert_eq!(parse_duration("-1 ms"), None);
        assert_eq!(parse_duration("99999999999999 days"), None);
    }

    #[test]
    fn advance_fires_due_timers_in_order() {
        let mut clock = Manual::default();
        let timers = [(300, 1), (100, 2), (200, 3), (100, 4), (400, 5)];
        assert_eq!(
            clock.advance(300, &timers),
            vec![(100, 2), (100, 4), (200, 3), (300, 1)]
        );
        assert_eq!(clock.now, 300);
        assert_eq!(clock.advance(350, &[(400, 5)]), vec![]);
        assert_eq!(clock.now, 350);
        assert_eq!(clock.advance(100, &[]), vec![]);
        assert_eq!(clock.now, 350, "time never moves back");
    }

    #[test]
    fn next_due_steps_one_instant_at_a_time() {
        let mut clock = Manual { now: 50 };
        let timers = [(300, 1), (100, 2), (100, 4)];
        assert_eq!(clock.next_due(300, &timers), Some((100, 2)));
        assert_eq!(clock.now, 100);
        assert_eq!(clock.next_due(300, &timers[..1]), Some((300, 1)));
        assert_eq!(clock.now, 300);
        assert_eq!(clock.next_due(350, &[(400, 5)]), None);
        assert_eq!(clock.now, 300);
        clock.advance_to(350);
        assert_eq!(clock.now, 350);
        assert_eq!(clock.next_due(350, &[(20, 7)]), Some((20, 7)));
        assert_eq!(clock.now, 350, "a timer set in the past fires now");
    }

    #[test]
    fn the_real_clock_is_monotonic() {
        let clock = Real::start();
        let first = clock.now();
        let second = clock.now();
        assert!(second >= first);
    }
}
