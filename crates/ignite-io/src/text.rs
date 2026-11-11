//! Byte search for the builtins `Bytes.index_of` and `Text.pieces`
//! (`docs/slice.md` section 13), the one implementation both targets call:
//! generated Rust directly, the LLVM target through `ignite-rt`. The search
//! scans for the needle's first byte and compares the rest of the needle
//! where it finds one: safe code instead of the C library's `memchr` and
//! `memmem`, which std has no safe and stable equivalent of
//! (`story:unsafe-sites-back`). Its worst case is the haystack's length
//! times the needle's, where `memmem` is linear.

/// The offset of the first `needle` in `haystack`; `Some(0)` for an empty
/// needle.
pub fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let Some((&first, rest)) = needle.split_first() else {
        return Some(0);
    };
    // The last offset a match can start at.
    let last = haystack.len().checked_sub(needle.len())?;
    let mut at = 0;
    while at <= last {
        let start = at + haystack[at..=last].iter().position(|&byte| byte == first)?;
        if haystack[start + 1..start + needle.len()] == *rest {
            return Some(start);
        }
        at = start + 1;
    }
    None
}

/// `bytes.index_of(needle, start)`: the offset of the first `needle` in
/// `haystack` at or after `start`, a negative `start` read as 0; `None`
/// when `start` is past the end, also for an empty needle.
pub fn index_of(haystack: impl AsRef<[u8]>, needle: impl AsRef<[u8]>, start: i64) -> Option<i64> {
    let (haystack, needle) = (haystack.as_ref(), needle.as_ref());
    let start = usize::try_from(start.max(0)).unwrap_or(usize::MAX);
    if start > haystack.len() || haystack.len() - start < needle.len() {
        return None;
    }
    find(&haystack[start..], needle).map(|at| (start + at) as i64)
}

/// Calls `piece` with the start and end of every piece of `text` between
/// the separators, in order: "a,,b" at "," gives (0, 1), (2, 2), (3, 4).
/// An empty separator gives the whole text as one piece.
pub fn each_piece(text: &[u8], separator: &[u8], mut piece: impl FnMut(usize, usize)) {
    if separator.is_empty() {
        piece(0, text.len());
        return;
    }
    let mut start = 0;
    while let Some(at) = find(&text[start..], separator) {
        piece(start, start + at);
        start += at + separator.len();
    }
    piece(start, text.len());
}

/// `text.pieces(separator)`: the texts between the separators. A piece of
/// valid UTF-8 cut at a separator of valid UTF-8 is valid UTF-8 (a match
/// starts and ends on a character boundary), so no piece is checked again.
pub fn pieces(text: impl AsRef<str>, separator: impl AsRef<str>) -> Vec<String> {
    let (text, separator) = (text.as_ref(), separator.as_ref());
    let mut out = Vec::new();
    each_piece(text.as_bytes(), separator.as_bytes(), |start, end| {
        out.push(text[start..end].to_string())
    });
    out
}

#[cfg(test)]
mod tests {
    use super::{find, index_of, pieces};

    /// The search the builtins replace: every offset in turn.
    fn naive(haystack: &[u8], needle: &[u8], start: i64) -> Option<i64> {
        let mut at = start.max(0) as usize;
        while at + needle.len() <= haystack.len() {
            if &haystack[at..at + needle.len()] == needle {
                return Some(at as i64);
            }
            at += 1;
        }
        None
    }

    #[test]
    fn find_agrees_with_every_offset_in_turn() {
        let alphabet: [&[u8]; 8] = [
            b"a",
            b"b",
            b"\r\n",
            "é".as_bytes(),
            "€".as_bytes(),
            b"\xff",
            b"ab",
            b",",
        ];
        let mut seed: u64 = 7;
        let mut next = |count: usize| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as usize % count
        };
        for _ in 0..20_000 {
            let haystack: Vec<u8> = (0..next(40))
                .flat_map(|_| alphabet[next(8)].to_vec())
                .collect();
            let needle: Vec<u8> = (0..next(4))
                .flat_map(|_| alphabet[next(8)].to_vec())
                .collect();
            let start = next(haystack.len() + 5) as i64 - 2;
            assert_eq!(
                index_of(&haystack, &needle, start),
                naive(&haystack, &needle, start),
                "{haystack:?} {needle:?} {start}"
            );
            let from = next(haystack.len() + 1);
            let part = &haystack[from..from + next(haystack.len() - from + 1)];
            assert_eq!(
                find(&haystack, part).map(|at| at as i64),
                naive(&haystack, part, 0)
            );
        }
    }

    #[test]
    fn index_of_bounds_the_start() {
        assert_eq!(index_of(b"abc", b"", 3), Some(3));
        assert_eq!(index_of(b"abc", b"", 4), None);
        assert_eq!(index_of(b"abc", b"a", -5), Some(0));
        assert_eq!(index_of(b"abc", b"c", i64::MAX), None);
        assert_eq!(index_of(b"", b"", 0), Some(0));
        assert_eq!(index_of(b"ab", b"abc", 0), None);
    }

    #[test]
    fn pieces_keep_empty_ones() {
        assert_eq!(pieces("a,b,,c", ","), ["a", "b", "", "c"]);
        assert_eq!(pieces("", ","), [""]);
        assert_eq!(pieces("aaa", "aa"), ["", "a"]);
        assert_eq!(pieces("abc", ""), ["abc"]);
        assert_eq!(pieces("x€y€", "€"), ["x", "y", ""]);
        assert_eq!(pieces("a\r\nb\r\n", "\r\n"), ["a", "b", ""]);
    }
}
