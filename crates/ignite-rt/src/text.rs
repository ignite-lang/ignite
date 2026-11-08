//! `Text` helpers. Every result is a fresh `Text` in the calling task's
//! arena; arguments are only read.

use crate::{IgniteList, bytes, get, put};

/// The `Text` elements of the `List<Text>` at `list`.
pub(crate) fn texts<'a>(list: *const IgniteList) -> &'a [IgniteList] {
    match get(list) {
        Some(list) if !list.ptr.is_null() && list.len > 0 => {
            // SAFETY: a `List<Text>` argument holds `len` aligned `Text`s.
            unsafe { std::slice::from_raw_parts(list.ptr.cast::<IgniteList>(), list.len as usize) }
        }
        _ => &[],
    }
}

/// `void text_from_bytes_lossy(Text* out, List* bytes)`: invalid UTF-8
/// sequences become U+FFFD. ASCII is valid UTF-8 and is copied as it is,
/// without the decoder.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_from_bytes_lossy(out: *mut IgniteList, data: *const IgniteList) {
    crate::entry(move || {
        let data = bytes(data);
        if data.is_ascii() {
            put(out, IgniteList::from_bytes(data));
            return;
        }
        let text = String::from_utf8_lossy(data);
        put(out, IgniteList::from_bytes(text.as_bytes()));
    })
}

/// `void text_trim(Text* out, Text* text)`: leading and trailing Unicode
/// whitespace removed, as Rust `str::trim`. A text that is not UTF-8 (which
/// no runtime or generated operation produces) loses ASCII whitespace only.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_trim(out: *mut IgniteList, text: *const IgniteList) {
    crate::entry(move || put(out, IgniteList::from_bytes(trim(bytes(text)))))
}

/// The bytes `str::trim` keeps. The ASCII whitespace of Unicode's
/// `White_Space` (tab, LF, VT, FF, CR, space) is dropped from both ends
/// first; when what is left is empty or starts and ends with an ASCII
/// byte, that is `str::trim`'s answer for valid UTF-8 (an ASCII byte that
/// is no whitespace stops it), so the UTF-8 pass over the whole text runs
/// only when an end is a non-ASCII byte.
fn trim(text: &[u8]) -> &[u8] {
    let space = |b: &u8| matches!(b, b'\t' | b'\n' | 0x0B | 0x0C | b'\r' | b' ');
    let start = text.iter().position(|b| !space(b)).unwrap_or(text.len());
    let end = text
        .iter()
        .rposition(|b| !space(b))
        .map_or(start, |i| i + 1);
    let inner = &text[start..end];
    match (inner.first(), inner.last()) {
        (Some(first), Some(last)) if !first.is_ascii() || !last.is_ascii() => {
            match std::str::from_utf8(text) {
                Ok(text) => text.trim().as_bytes(),
                Err(_) => text.trim_ascii(),
            }
        }
        _ => inner,
    }
}

/// A fresh `Text` of `len` bytes in the task's arena, written by `fill`
/// (which must write all of them).
fn fresh(len: usize, fill: impl FnOnce(&mut [u8])) -> IgniteList {
    let ptr = crate::arena::alloc(len, 1);
    // SAFETY: `ptr` is a fresh arena block of `len` bytes (a dangling,
    // non-null pointer when `len` is 0); arena memory is initialized.
    fill(unsafe { std::slice::from_raw_parts_mut(ptr, len) });
    IgniteList {
        ptr,
        len: len as i64,
        cap: len as i64,
    }
}

/// `void text_concat(Text* out, Text* a, Text* b)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_concat(
    out: *mut IgniteList,
    a: *const IgniteList,
    b: *const IgniteList,
) {
    crate::entry(move || {
        put(out, IgniteList::from_bytes(&[bytes(a), bytes(b)].concat()));
    })
}

/// `i8 text_eq(Text* a, Text* b)`: 1 when equal byte for byte, else 0.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_eq(a: *const IgniteList, b: *const IgniteList) -> i8 {
    crate::entry(move || i8::from(bytes(a) == bytes(b)))
}

/// `void text_debug(Text* out, Text* text)`: the debug text of a `Text`
/// (`docs/slice.md` section 13.6), quoted, with `"`, `\`, `\n`, `\r`, `\t`
/// and `\0` escaped, other control characters and DEL as `\u{hex}`, and
/// everything else verbatim. Invalid UTF-8 is decoded lossily.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_debug(out: *mut IgniteList, text: *const IgniteList) {
    crate::entry(move || {
        let text = bytes(text);
        if text
            .iter()
            .all(|&b| (0x20..0x7f).contains(&b) && b != b'"' && b != b'\\')
        {
            put(
                out,
                fresh(text.len() + 2, |buffer| {
                    let last = buffer.len() - 1;
                    buffer[0] = b'"';
                    buffer[1..last].copy_from_slice(text);
                    buffer[last] = b'"';
                }),
            );
            return;
        }
        put(out, IgniteList::from_bytes(debug(text).as_bytes()));
    })
}

fn debug(text: &[u8]) -> String {
    let mut out = String::from('"');
    for c in String::from_utf8_lossy(text).chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `i8 text_starts_with(Text* text, Text* prefix)`: 1 or 0.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_starts_with(
    text: *const IgniteList,
    prefix: *const IgniteList,
) -> i8 {
    crate::entry(move || i8::from(bytes(text).starts_with(bytes(prefix))))
}

/// `i64 bytes_index_of(List* bytes, List* needle, i64 start)`:
/// [`ignite_io::text::index_of`], -1 for `None`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_bytes_index_of(
    data: *const IgniteList,
    needle: *const IgniteList,
    start: i64,
) -> i64 {
    crate::entry(move || ignite_io::text::index_of(bytes(data), bytes(needle), start).unwrap_or(-1))
}

/// `void text_pieces(List* out, Text* text, Text* separator)`:
/// [`ignite_io::text::each_piece`], every piece a fresh `Text` copied
/// without a UTF-8 check (a valid text cut at a valid separator is valid).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_pieces(
    out: *mut IgniteList,
    text: *const IgniteList,
    separator: *const IgniteList,
) {
    crate::entry(move || {
        let (text, separator) = (bytes(text), bytes(separator));
        let mut count = 0usize;
        ignite_io::text::each_piece(text, separator, |_, _| count += 1);
        let size = std::mem::size_of::<IgniteList>();
        let ptr = crate::arena::alloc(count * size, std::mem::align_of::<IgniteList>())
            .cast::<IgniteList>();
        let mut index = 0;
        ignite_io::text::each_piece(text, separator, |start, end| {
            let piece = IgniteList::from_bytes(&text[start..end]);
            // SAFETY: `ptr` is a fresh arena block of `count` aligned
            // elements and the second pass yields the same `count` pieces.
            unsafe { ptr.add(index).write(piece) };
            index += 1;
        });
        put(
            out,
            IgniteList {
                ptr: ptr.cast(),
                len: count as i64,
                cap: count as i64,
            },
        );
    })
}

/// `void text_join(Text* out, List* texts, Text* separator)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_join(
    out: *mut IgniteList,
    list: *const IgniteList,
    separator: *const IgniteList,
) {
    crate::entry(move || {
        let parts = texts(list);
        let separator = bytes(separator);
        let len = parts.iter().map(|text| bytes(text).len()).sum::<usize>()
            + separator.len() * parts.len().saturating_sub(1);
        put(
            out,
            fresh(len, |buffer| {
                let mut at = 0;
                for (i, part) in parts.iter().enumerate() {
                    if i > 0 {
                        buffer[at..at + separator.len()].copy_from_slice(separator);
                        at += separator.len();
                    }
                    let part = bytes(part);
                    buffer[at..at + part.len()].copy_from_slice(part);
                    at += part.len();
                }
            }),
        );
    })
}

/// `i8 int_parse(Text* text, i64* out)`: `Int.parse`, 1 and the value in
/// `out` for decimal digits with an optional leading `-`; 0 for anything
/// else (a `+`, spaces, no digits, overflow): [`ignite_io::parse_int`].
/// `text` may be the bytes of `Bytes`: the lowering of
/// `Int.parse(bytes.to_text())` passes them without a `Text`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_int_parse(text: *const IgniteList, out: *mut i64) -> i8 {
    crate::entry(move || match ignite_io::parse_int(bytes(text)) {
        Some(value) => {
            put(out, value);
            1
        }
        None => 0,
    })
}

/// `void int_to_text(Text* out, i64 value)`: decimal, `-` for negatives.
/// Formats into a stack buffer of 20 bytes, the length of `i64::MIN`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_int_to_text(out: *mut IgniteList, value: i64) {
    crate::entry(move || {
        let mut buffer = [0u8; 20];
        let mut at = buffer.len();
        let mut rest = value.unsigned_abs();
        loop {
            at -= 1;
            buffer[at] = b'0' + (rest % 10) as u8;
            rest /= 10;
            if rest == 0 {
                break;
            }
        }
        if value < 0 {
            at -= 1;
            buffer[at] = b'-';
        }
        put(out, IgniteList::from_bytes(&buffer[at..]));
    })
}

/// `i32 text_cmp(Text* a, Text* b)`: -1, 0 or 1 for the bytewise order of
/// the two texts, the key order a `Map<Text, V>` is kept in.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_cmp(a: *const IgniteList, b: *const IgniteList) -> i32 {
    crate::entry(move || match bytes(a).cmp(bytes(b)) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    })
}

/// `void quantity_debug(Text* out, i32 kind, i64 value)`: the debug text of
/// a quantity of `value` base units (`docs/slice.md` section 14.1), kind 0
/// `Duration`, 1 `ByteSize`, 2 `Ratio`. An unknown kind reads as `Duration`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_quantity_debug(out: *mut IgniteList, kind: i32, value: i64) {
    crate::entry(move || {
        let kind = match kind {
            1 => ignite_io::Quantity::ByteSize,
            2 => ignite_io::Quantity::Ratio,
            _ => ignite_io::Quantity::Duration,
        };
        put(
            out,
            IgniteList::from_bytes(ignite_io::quantity_debug(kind, value).as_bytes()),
        );
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn trim_keeps_what_str_trim_keeps() {
        for text in [
            "",
            " ",
            "a",
            " a b ",
            "\t\r\n",
            "\x0b a \x0b",
            "\x0c\x0bx",
            "\x1fx\x1f",
            "\u{a0}x\u{a0}",
            " \u{3000}é ",
            "x\u{2003}",
            "\u{85}y",
            " é",
            "é ",
            "\u{2028} \u{2029}",
        ] {
            assert_eq!(
                super::trim(text.as_bytes()),
                text.trim().as_bytes(),
                "{text:?}"
            );
        }
        assert_eq!(super::trim(b" \xff a \xff "), b"\xff a \xff");
        assert_eq!(super::trim(b" a\xff b "), b"a\xff b");
    }

    #[test]
    fn debug_escapes_as_section_13_6() {
        assert_eq!(
            super::debug(b"a\"b\\c\nd\re\tf\0g"),
            r#""a\"b\\c\nd\re\tf\0g""#
        );
        assert_eq!(super::debug(b"\x1b\x7f\x01"), r#""\u{1b}\u{7f}\u{1}""#);
        assert_eq!(super::debug("ä€".as_bytes()), "\"ä€\"");
        assert_eq!(super::debug(b""), "\"\"");
    }
}
