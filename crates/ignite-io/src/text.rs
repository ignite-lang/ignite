//! Byte search for the builtins `Bytes.index_of` and `Text.pieces`
//! (`docs/slice.md` section 13), the one implementation both targets call:
//! generated Rust directly, the LLVM target through `ignite-rt`. The search
//! looks for the needle's first byte eight bytes at a time ([`find_byte`])
//! and compares the rest of the needle where it finds one: safe code
//! instead of the C library's `memchr` and `memmem`, which std has no safe
//! and stable equivalent of (`story:unsafe-sites-back`), and as cheap for
//! the short texts a server parses. Its worst case is the haystack's
//! length times the needle's, where `memmem` is linear.

/// The offset of the first `needle` in `haystack`; `Some(0)` for an empty
/// needle. The search for one byte, the most frequent, goes to
/// [`find_byte`] alone, which keeps no registers of its caller's.
pub fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    match needle {
        [] => Some(0),
        [byte] => find_byte(haystack, *byte),
        _ => find_long(haystack, needle),
    }
}

/// [`find`] for a needle of two bytes or more.
fn find_long(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let Some((&first, rest)) = needle.split_first() else {
        return Some(0);
    };
    // The offsets a match can start at.
    let starts = haystack.len().checked_sub(needle.len())? + 1;
    let mut at = 0;
    while let Some(found) = find_byte(&haystack[at..starts], first) {
        let start = at + found;
        // Compared here, byte by byte: the rest of a needle is short, and
        // a call of `memcmp` would cost more than it saves.
        let after = &haystack[start + 1..start + needle.len()];
        if after.iter().zip(rest).all(|(have, want)| have == want) {
            return Some(start);
        }
        at = start + 1;
    }
    None
}

/// Eight copies of 1 and of 0x80, one per byte of a word.
const ONES: u64 = u64::from_le_bytes([0x01; 8]);
const HIGHS: u64 = u64::from_le_bytes([0x80; 8]);

/// The offset of the first `byte` in `haystack`, read eight bytes at a
/// time ([`first_in`]); the bytes after the last whole word are read as
/// the haystack's last eight bytes, which overlap bytes already read and
/// found without `byte`. A haystack shorter than a word is read byte by
/// byte.
fn find_byte(haystack: &[u8], byte: u8) -> Option<usize> {
    let length = haystack.len();
    if length < 8 {
        return haystack.iter().position(|&each| each == byte);
    }
    let copies = ONES * u64::from(byte);
    let mut at = 0;
    while at + 8 <= length {
        if let Some(found) = first_in(word_at(haystack, at), copies) {
            return Some(at + found);
        }
        at += 8;
    }
    if at < length {
        let last = length - 8;
        return first_in(word_at(haystack, last), copies).map(|found| last + found);
    }
    None
}

/// The eight bytes of `haystack` at `at` as a little-endian word.
fn word_at(haystack: &[u8], at: usize) -> u64 {
    let mut eight = [0; 8];
    eight.copy_from_slice(&haystack[at..at + 8]);
    u64::from_le_bytes(eight)
}

/// The offset of the first byte of `word` equal to the byte `copies` holds
/// eight of: `word ^ copies` has a zero byte there, and
/// `(w - ONES) & !w & HIGHS` sets the high bit of the first zero byte of
/// `w` (a borrow can only set bits above it), so the lowest set bit of the
/// little-endian word names the first match.
fn first_in(word: u64, copies: u64) -> Option<usize> {
    let differs = word ^ copies;
    let zero = differs.wrapping_sub(ONES) & !differs & HIGHS;
    (zero != 0).then(|| (zero.trailing_zeros() / 8) as usize)
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

    /// The cases a word-at-a-time search can get wrong: an empty needle, a
    /// needle longer than the haystack, a match at the first and at the
    /// last byte, a match across a word boundary (and a first byte that
    /// matches in one word while the rest does not), a byte of 0x80 and
    /// above next to its neighbours (the borrow of the zero-byte test), and
    /// no match; then haystacks of every length up to three words with the
    /// needle at every offset, and generated ones with a fixed seed, each
    /// against every offset in turn.
    #[test]
    fn find_holds_at_the_edges_of_a_word() {
        let cases: [(&[u8], &[u8], Option<usize>); 14] = [
            (b"abc", b"", Some(0)),
            (b"", b"", Some(0)),
            (b"ab", b"abc", None),
            (b"", b"a", None),
            (b"xabcdefghijklmnop", b"x", Some(0)),
            (b"abcdefghijklmnopx", b"x", Some(16)),
            (b"abcdefghijklmnopx", b"opx", Some(14)),
            (b"abcdefgxyzabcdef", b"gxy", Some(6)),
            (b"abcdefgx_abcdefgxy", b"xy", Some(16)),
            (
                b"\x01\x00\x81\x80\xff\x7f\x80\x00\xff",
                b"\x80\x00",
                Some(6),
            ),
            (b"\x00\x00\x00\x00\x00\x00\x00\x01\x00", b"\x01", Some(7)),
            (b"\xff\xff\xff\xff\xff\xff\xff\xff\xfe", b"\xfe", Some(8)),
            (b"\x80\x81\x80\x81\x80\x81\x80\x81\x80\x81", b"\x01", None),
            (b"abcdefghijklmnopqrstuvwxyz", b"zz", None),
        ];
        for (haystack, needle, expected) in cases {
            assert_eq!(find(haystack, needle), expected, "{haystack:?} {needle:?}");
            assert_eq!(
                find(haystack, needle).map(|at| at as i64),
                naive(haystack, needle, 0)
            );
        }
        for length in 0..=24 {
            for needle in [&b"\x80"[..], b"\x7f\x80", b"\xff\x00\x01"] {
                for at in 0..=length {
                    let mut haystack = vec![0x81u8; length];
                    let end = (at + needle.len()).min(length);
                    haystack[at..end].copy_from_slice(&needle[..end - at]);
                    assert_eq!(
                        find(&haystack, needle).map(|found| found as i64),
                        naive(&haystack, needle, 0),
                        "{haystack:?} {needle:?}"
                    );
                }
            }
        }
        let mut seed: u64 = 11;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as u8
        };
        for _ in 0..20_000 {
            let length = usize::from(next() % 48);
            let haystack: Vec<u8> = (0..length).map(|_| next() % 4 + 0x7e).collect();
            let needle: Vec<u8> = (0..usize::from(next() % 4))
                .map(|_| next() % 4 + 0x7e)
                .collect();
            assert_eq!(
                find(&haystack, &needle).map(|at| at as i64),
                naive(&haystack, &needle, 0),
                "{haystack:?} {needle:?}"
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
