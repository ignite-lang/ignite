//! `List` sorting on the LLVM target: the stable merge sort behind
//! `l.sort()` and `l.sort_by(f)` (`docs/slice.md`, "A List is sorted with
//! sort and sort_by"). The generated code gives the keys and the order of
//! two keys; this sorts their positions, and the generated code builds the
//! new list from them. No `unsafe` of its own: the keys are read through
//! [`crate::bytes`] and the positions written through [`crate::text::fresh`].

use crate::{IgniteList, bytes, put};

/// The order of the two keys at the pointers: negative when the first
/// sorts before the second, zero when they are equal, positive when after.
pub type Compare = extern "C" fn(*const u8, *const u8) -> i32;

/// The bytes of one position.
const POSITION: usize = std::mem::size_of::<i64>();

/// `void list_sort(List* out, List* keys, i64 size, ptr compare)`: `keys`
/// is the bytes of the keys, `size` bytes each; `out` gets the positions
/// `0..n` of the `n` keys as native-endian `i64`, in arena memory aligned
/// to 1, ascending by key and, among equal keys, by position. One arena
/// block of `2 * n` positions holds them and the merge's scratch.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_list_sort(
    out: *mut IgniteList,
    keys: *const IgniteList,
    size: i64,
    compare: Compare,
) {
    crate::entry(move || {
        let keys = bytes(keys);
        let size = usize::try_from(size).unwrap_or(0).max(1);
        let len = keys.len() / size;
        let key = |position: usize| keys[position * size..].as_ptr();
        let mut sorted = crate::text::fresh(2 * len * POSITION, |buffer| {
            merge_sort(buffer, len, |x, y| compare(key(x), key(y)) < 0);
        });
        sorted.len = (len * POSITION) as i64;
        sorted.cap = sorted.len;
        put(out, sorted);
    })
}

/// Position `index` of `buffer`.
fn at(buffer: &[u8], index: usize) -> usize {
    let mut word = [0; POSITION];
    word.copy_from_slice(&buffer[index * POSITION..(index + 1) * POSITION]);
    i64::from_ne_bytes(word) as usize
}

/// Set position `index` of `buffer` to `position`.
fn set(buffer: &mut [u8], index: usize, position: usize) {
    buffer[index * POSITION..(index + 1) * POSITION]
        .copy_from_slice(&(position as i64).to_ne_bytes());
}

/// The first `len` positions of `buffer` set to `0..len` and sorted stably
/// by `less`, merging runs of doubling width between them and the next
/// `len` positions, the scratch. Two runs already in order are copied
/// without a merge, so a sorted input costs one pass per width.
fn merge_sort(buffer: &mut [u8], len: usize, less: impl Fn(usize, usize) -> bool) {
    let (order, scratch) = buffer.split_at_mut(len * POSITION);
    for position in 0..len {
        set(order, position, position);
    }
    let mut width = 1;
    let mut in_order = true;
    while width < len {
        let (from, to): (&[u8], &mut [u8]) = if in_order {
            (order, scratch)
        } else {
            (scratch, order)
        };
        let mut start = 0;
        while start < len {
            let middle = (start + width).min(len);
            let end = (start + 2 * width).min(len);
            merge(from, to, (start, middle, end), &less);
            start = end;
        }
        in_order = !in_order;
        width *= 2;
    }
    if !in_order {
        order.copy_from_slice(&scratch[..len * POSITION]);
    }
}

/// The runs `start..middle` and `middle..end` of `from` merged into the
/// same range of `to`; a position of the second run goes first only when
/// its key is less than the first run's, which keeps equal keys in order.
fn merge(
    from: &[u8],
    to: &mut [u8],
    (start, middle, end): (usize, usize, usize),
    less: &impl Fn(usize, usize) -> bool,
) {
    let range = start * POSITION..end * POSITION;
    if middle == start || middle == end || !less(at(from, middle), at(from, middle - 1)) {
        to[range.clone()].copy_from_slice(&from[range]);
        return;
    }
    let (mut i, mut j) = (start, middle);
    for k in start..end {
        if j < end && (i == middle || less(at(from, j), at(from, i))) {
            set(to, k, at(from, j));
            j += 1;
        } else {
            set(to, k, at(from, i));
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{POSITION, at, merge_sort};

    /// Stable and ascending on every length up to 70 with keys that repeat.
    #[test]
    fn merge_sort_is_stable_and_ascending() {
        for len in 0..70usize {
            let keys: Vec<i64> = (0..len).map(|i| ((i * 7919) % 5) as i64).collect();
            let mut buffer = vec![0u8; 2 * len * POSITION];
            merge_sort(&mut buffer, len, |x, y| keys[x] < keys[y]);
            let order: Vec<usize> = (0..len).map(|i| at(&buffer, i)).collect();
            for pair in order.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                assert!(
                    keys[a] < keys[b] || (keys[a] == keys[b] && a < b),
                    "{len}: {order:?}"
                );
            }
            let mut seen = order.clone();
            seen.sort_unstable();
            assert_eq!(seen, (0..len).collect::<Vec<_>>());
        }
    }
}
