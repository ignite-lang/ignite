//! The debug text of a quantity (`docs/slice.md` section 14.1): the
//! literal form in the largest unit that divides the value exactly, the
//! same on both targets. The compiler's `ignite_types::QuantityKind::
//! debug_text` is the authoritative copy of this algorithm; this one runs
//! inside compiled programs, which do not link the compiler.

/// The three quantity types, each an `Int` of base units: nanoseconds,
/// bytes, parts per million.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantity {
    Duration,
    ByteSize,
    Ratio,
}

/// `15 minutes`, `1 second`, `4096 bytes`, `16 KiB`, `99.9%`, `0 ns`:
/// the largest unit that divides `value` exactly, singular for 1 and -1,
/// the smallest unit for 0.
pub fn quantity_debug(kind: Quantity, value: i64) -> String {
    let units: &[(&str, &str, i64)] = match kind {
        Quantity::Duration => &[
            ("day", "days", 86_400_000_000_000),
            ("hour", "hours", 3_600_000_000_000),
            ("minute", "minutes", 60_000_000_000),
            ("second", "seconds", 1_000_000_000),
            ("ms", "ms", 1_000_000),
            ("us", "us", 1_000),
            ("ns", "ns", 1),
        ],
        Quantity::ByteSize => &[
            ("GiB", "GiB", 1 << 30),
            ("MiB", "MiB", 1 << 20),
            ("KiB", "KiB", 1 << 10),
            ("byte", "bytes", 1),
        ],
        Quantity::Ratio => {
            let sign = if value < 0 { "-" } else { "" };
            let abs = value.unsigned_abs();
            let whole = abs / 10_000;
            let frac = abs % 10_000;
            if frac == 0 {
                return format!("{sign}{whole}%");
            }
            let digits = format!("{frac:04}");
            return format!("{sign}{whole}.{}%", digits.trim_end_matches('0'));
        }
    };
    for (one, many, factor) in units {
        if value != 0 && value % factor == 0 {
            let n = value / factor;
            let unit = if n == 1 || n == -1 { one } else { many };
            return format!("{n} {unit}");
        }
    }
    let (_, many, _) = units[units.len() - 1];
    format!("{value} {many}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(
            quantity_debug(Quantity::Duration, 900_000_000_000),
            "15 minutes"
        );
        assert_eq!(
            quantity_debug(Quantity::Duration, 60_000_000_000),
            "1 minute"
        );
        assert_eq!(quantity_debug(Quantity::Duration, 100_000_000), "100 ms");
        assert_eq!(quantity_debug(Quantity::Duration, 1_500_000_000), "1500 ms");
        assert_eq!(
            quantity_debug(Quantity::Duration, 172_800_000_000_000),
            "2 days"
        );
        assert_eq!(quantity_debug(Quantity::Duration, 0), "0 ns");
        assert_eq!(
            quantity_debug(Quantity::Duration, -1_000_000_000),
            "-1 second"
        );
        assert_eq!(quantity_debug(Quantity::Duration, 7), "7 ns");
    }

    #[test]
    fn byte_sizes() {
        assert_eq!(quantity_debug(Quantity::ByteSize, 4096), "4 KiB");
        assert_eq!(quantity_debug(Quantity::ByteSize, 4095), "4095 bytes");
        assert_eq!(quantity_debug(Quantity::ByteSize, 16 << 10), "16 KiB");
        assert_eq!(quantity_debug(Quantity::ByteSize, 1 << 30), "1 GiB");
        assert_eq!(quantity_debug(Quantity::ByteSize, 1), "1 byte");
        assert_eq!(quantity_debug(Quantity::ByteSize, 0), "0 bytes");
    }

    #[test]
    fn ratios() {
        assert_eq!(quantity_debug(Quantity::Ratio, 999_000), "99.9%");
        assert_eq!(quantity_debug(Quantity::Ratio, 990_000), "99%");
        assert_eq!(quantity_debug(Quantity::Ratio, 995_000), "99.5%");
        assert_eq!(quantity_debug(Quantity::Ratio, 1), "0.0001%");
        assert_eq!(quantity_debug(Quantity::Ratio, 0), "0%");
        assert_eq!(quantity_debug(Quantity::Ratio, -5_000), "-0.5%");
    }
}
