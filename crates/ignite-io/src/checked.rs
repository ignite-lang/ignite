//! The operations of `docs/slice.md` section 14.8 that can fail at run
//! time, each reporting the Ignite position it was given: `Int` and `Byte`
//! arithmetic that overflows or divides by zero, an index out of range, a
//! slice out of range. Generated code of the Rust target calls these in
//! place of the Rust operators, so the report names the Ignite file, line
//! and column, not rustc's panic text. Every fn is `#[inline]`; the failure
//! path is a cold call.

use crate::fail;

#[cold]
fn overflow(op: &str, left: i64, right: i64, file: &str, line: usize, col: usize) -> ! {
    fail(op, file, line, col, &format!("{left} {op} {right}"))
}

/// `left + right` on `Int`.
#[inline]
pub fn add(left: i64, right: i64, file: &str, line: usize, col: usize) -> i64 {
    match left.checked_add(right) {
        Some(value) => value,
        None => overflow("+", left, right, file, line, col),
    }
}

/// `left - right` on `Int`.
#[inline]
pub fn sub(left: i64, right: i64, file: &str, line: usize, col: usize) -> i64 {
    match left.checked_sub(right) {
        Some(value) => value,
        None => overflow("-", left, right, file, line, col),
    }
}

/// `left * right` on `Int`.
#[inline]
pub fn mul(left: i64, right: i64, file: &str, line: usize, col: usize) -> i64 {
    match left.checked_mul(right) {
        Some(value) => value,
        None => overflow("*", left, right, file, line, col),
    }
}

/// `left / right` on `Int`: fails on a zero divisor and on the overflow of
/// the smallest `Int` divided by -1.
#[inline]
pub fn div(left: i64, right: i64, file: &str, line: usize, col: usize) -> i64 {
    match left.checked_div(right) {
        Some(value) => value,
        None => overflow("/", left, right, file, line, col),
    }
}

/// `left % right` on `Int`: fails as [`div`] does.
#[inline]
pub fn rem(left: i64, right: i64, file: &str, line: usize, col: usize) -> i64 {
    match left.checked_rem(right) {
        Some(value) => value,
        None => overflow("%", left, right, file, line, col),
    }
}

/// Prefix `-` on `Int`: fails on the smallest `Int`.
#[inline]
pub fn neg(value: i64, file: &str, line: usize, col: usize) -> i64 {
    match value.checked_neg() {
        Some(negated) => negated,
        None => fail("-", file, line, col, &format!("-({value})")),
    }
}

/// `left + right` on `Byte`.
#[inline]
pub fn add_byte(left: u8, right: u8, file: &str, line: usize, col: usize) -> u8 {
    match left.checked_add(right) {
        Some(value) => value,
        None => overflow("+", left.into(), right.into(), file, line, col),
    }
}

/// `left - right` on `Byte`.
#[inline]
pub fn sub_byte(left: u8, right: u8, file: &str, line: usize, col: usize) -> u8 {
    match left.checked_sub(right) {
        Some(value) => value,
        None => overflow("-", left.into(), right.into(), file, line, col),
    }
}

/// `left * right` on `Byte`.
#[inline]
pub fn mul_byte(left: u8, right: u8, file: &str, line: usize, col: usize) -> u8 {
    match left.checked_mul(right) {
        Some(value) => value,
        None => overflow("*", left.into(), right.into(), file, line, col),
    }
}

/// `left / right` on `Byte`: fails on a zero divisor.
#[inline]
pub fn div_byte(left: u8, right: u8, file: &str, line: usize, col: usize) -> u8 {
    match left.checked_div(right) {
        Some(value) => value,
        None => overflow("/", left.into(), right.into(), file, line, col),
    }
}

/// `left % right` on `Byte`: fails on a zero divisor.
#[inline]
pub fn rem_byte(left: u8, right: u8, file: &str, line: usize, col: usize) -> u8 {
    match left.checked_rem(right) {
        Some(value) => value,
        None => overflow("%", left.into(), right.into(), file, line, col),
    }
}

#[cold]
fn out_of_range(op: &str, detail: String, file: &str, line: usize, col: usize) -> ! {
    fail(op, file, line, col, &detail)
}

/// `list[index]` and `list.at(index)`: the element, or the `index`
/// failure `index <i> out of range for length <n>`.
#[inline]
pub fn at<'a, T>(list: &'a [T], index: i64, file: &str, line: usize, col: usize) -> &'a T {
    match usize::try_from(index).ok().and_then(|i| list.get(i)) {
        Some(item) => item,
        None => out_of_range(
            "index",
            format!("index {index} out of range for length {}", list.len()),
            file,
            line,
            col,
        ),
    }
}

/// `list.slice(from, to)`: a copy of the elements `from..to`, or the
/// `slice` failure `slice <from>..<to> out of range for length <n>`
/// (also when `from > to`).
#[inline]
pub fn slice<T: Clone>(
    list: &[T],
    from: i64,
    to: i64,
    file: &str,
    line: usize,
    col: usize,
) -> Vec<T> {
    let range = usize::try_from(from)
        .ok()
        .zip(usize::try_from(to).ok())
        .filter(|(start, end)| start <= end && *end <= list.len());
    match range {
        Some((start, end)) => list[start..end].to_vec(),
        None => out_of_range(
            "slice",
            format!("slice {from}..{to} out of range for length {}", list.len()),
            file,
            line,
            col,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::catch;

    fn failure<T>(f: impl FnOnce() -> T) -> String {
        catch(f).err().expect("a failure")
    }

    #[test]
    fn arithmetic_in_range() {
        assert_eq!(add(2, 3, "f", 1, 1), 5);
        assert_eq!(sub(2, 3, "f", 1, 1), -1);
        assert_eq!(mul(-2, 3, "f", 1, 1), -6);
        assert_eq!(div(7, -2, "f", 1, 1), -3);
        assert_eq!(rem(7, -2, "f", 1, 1), 1);
        assert_eq!(neg(5, "f", 1, 1), -5);
        assert_eq!(add_byte(200, 55, "f", 1, 1), 255);
        assert_eq!(sub_byte(1, 1, "f", 1, 1), 0);
        assert_eq!(mul_byte(15, 17, "f", 1, 1), 255);
        assert_eq!(div_byte(7, 2, "f", 1, 1), 3);
        assert_eq!(rem_byte(7, 2, "f", 1, 1), 1);
    }

    #[test]
    fn overflow_and_zero_division_report_the_operands() {
        assert_eq!(
            failure(|| add(i64::MAX, 1, "t.ignite", 41, 17)),
            "+ failed at t.ignite:41:17: 9223372036854775807 + 1"
        );
        assert_eq!(
            failure(|| sub(i64::MIN, 1, "t.ignite", 1, 2)),
            "- failed at t.ignite:1:2: -9223372036854775808 - 1"
        );
        assert_eq!(
            failure(|| mul(i64::MAX, 2, "t.ignite", 1, 2)),
            "* failed at t.ignite:1:2: 9223372036854775807 * 2"
        );
        assert_eq!(
            failure(|| div(5, 0, "t.ignite", 1, 2)),
            "/ failed at t.ignite:1:2: 5 / 0"
        );
        assert_eq!(
            failure(|| div(i64::MIN, -1, "t.ignite", 1, 2)),
            "/ failed at t.ignite:1:2: -9223372036854775808 / -1"
        );
        assert_eq!(
            failure(|| rem(5, 0, "t.ignite", 1, 2)),
            "% failed at t.ignite:1:2: 5 % 0"
        );
        assert_eq!(
            failure(|| rem(i64::MIN, -1, "t.ignite", 1, 2)),
            "% failed at t.ignite:1:2: -9223372036854775808 % -1"
        );
        assert_eq!(
            failure(|| neg(i64::MIN, "t.ignite", 1, 2)),
            "- failed at t.ignite:1:2: -(-9223372036854775808)"
        );
        assert_eq!(
            failure(|| add_byte(255, 1, "t.ignite", 1, 2)),
            "+ failed at t.ignite:1:2: 255 + 1"
        );
        assert_eq!(
            failure(|| sub_byte(0, 1, "t.ignite", 1, 2)),
            "- failed at t.ignite:1:2: 0 - 1"
        );
        assert_eq!(
            failure(|| mul_byte(16, 16, "t.ignite", 1, 2)),
            "* failed at t.ignite:1:2: 16 * 16"
        );
        assert_eq!(
            failure(|| div_byte(1, 0, "t.ignite", 1, 2)),
            "/ failed at t.ignite:1:2: 1 / 0"
        );
        assert_eq!(
            failure(|| rem_byte(1, 0, "t.ignite", 1, 2)),
            "% failed at t.ignite:1:2: 1 % 0"
        );
    }

    #[test]
    fn index_and_slice() {
        let list = vec![10, 20];
        assert_eq!(*at(&list, 1, "f", 1, 1), 20);
        assert_eq!(
            failure(|| *at(&list, 5, "t.ignite", 3, 4)),
            "index failed at t.ignite:3:4: index 5 out of range for length 2"
        );
        assert_eq!(
            failure(|| *at(&list, -1, "t.ignite", 3, 4)),
            "index failed at t.ignite:3:4: index -1 out of range for length 2"
        );
        assert_eq!(slice(&list, 0, 2, "f", 1, 1), [10, 20]);
        assert_eq!(slice(&list, 1, 1, "f", 1, 1), []);
        assert_eq!(
            failure(|| slice(&list, 1, 3, "t.ignite", 3, 4)),
            "slice failed at t.ignite:3:4: slice 1..3 out of range for length 2"
        );
        assert_eq!(
            failure(|| slice(&list, 2, 1, "t.ignite", 3, 4)),
            "slice failed at t.ignite:3:4: slice 2..1 out of range for length 2"
        );
        assert_eq!(
            failure(|| slice(&list, -1, 1, "t.ignite", 3, 4)),
            "slice failed at t.ignite:3:4: slice -1..1 out of range for length 2"
        );
    }
}
