//! `Text` helpers. Every result is a fresh `Text` in the calling task's
//! arena; arguments are only read.

use crate::{IgniteList, bytes, get, put};

/// The `Text` elements of the `List<Text>` at `list`.
fn texts<'a>(list: *const IgniteList) -> &'a [IgniteList] {
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
    let data = bytes(data);
    if data.is_ascii() {
        put(out, IgniteList::from_bytes(data));
        return;
    }
    let text = String::from_utf8_lossy(data);
    put(out, IgniteList::from_bytes(text.as_bytes()));
}

/// `void text_trim(Text* out, Text* text)`: leading and trailing Unicode
/// whitespace removed, as Rust `str::trim`. A text that is not UTF-8 (which
/// no runtime or generated operation produces) loses ASCII whitespace only.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_trim(out: *mut IgniteList, text: *const IgniteList) {
    let text = bytes(text);
    let trimmed = match std::str::from_utf8(text) {
        Ok(text) => text.trim().as_bytes(),
        Err(_) => text.trim_ascii(),
    };
    put(out, IgniteList::from_bytes(trimmed));
}

/// `void text_concat(Text* out, Text* a, Text* b)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_concat(
    out: *mut IgniteList,
    a: *const IgniteList,
    b: *const IgniteList,
) {
    put(out, IgniteList::from_bytes(&[bytes(a), bytes(b)].concat()));
}

/// `i8 text_eq(Text* a, Text* b)`: 1 when equal byte for byte, else 0.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_eq(a: *const IgniteList, b: *const IgniteList) -> i8 {
    i8::from(bytes(a) == bytes(b))
}

/// `void text_debug(Text* out, Text* text)`: the debug text of a `Text`
/// (`docs/slice.md` section 13.6), quoted, with `"`, `\`, `\n`, `\r`, `\t`
/// and `\0` escaped, other control characters and DEL as `\u{hex}`, and
/// everything else verbatim. Invalid UTF-8 is decoded lossily.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_debug(out: *mut IgniteList, text: *const IgniteList) {
    put(out, IgniteList::from_bytes(debug(bytes(text)).as_bytes()));
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
    i8::from(bytes(text).starts_with(bytes(prefix)))
}

/// `void text_join(Text* out, List* texts, Text* separator)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_join(
    out: *mut IgniteList,
    list: *const IgniteList,
    separator: *const IgniteList,
) {
    let parts: Vec<&[u8]> = texts(list).iter().map(|text| bytes(text)).collect();
    put(out, IgniteList::from_bytes(&parts.join(bytes(separator))));
}

/// `i8 int_parse(Text* text, i64* out)`: `Int.parse`, 1 and the value in
/// `out` for decimal digits with an optional leading `-`; 0 for anything
/// else (a `+`, spaces, no digits, overflow): [`ignite_io::parse_int`].
/// `text` may be the bytes of `Bytes`: the lowering of
/// `Int.parse(bytes.to_text())` passes them without a `Text`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_int_parse(text: *const IgniteList, out: *mut i64) -> i8 {
    match ignite_io::parse_int(bytes(text)) {
        Some(value) => {
            put(out, value);
            1
        }
        None => 0,
    }
}

/// `void int_to_text(Text* out, i64 value)`: decimal, `-` for negatives.
/// Formats into a stack buffer of 20 bytes, the length of `i64::MIN`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_int_to_text(out: *mut IgniteList, value: i64) {
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
}

/// `i32 text_cmp(Text* a, Text* b)`: -1, 0 or 1 for the bytewise order of
/// the two texts, the key order a `Map<Text, V>` is kept in.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_text_cmp(a: *const IgniteList, b: *const IgniteList) -> i32 {
    match bytes(a).cmp(bytes(b)) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// `void quantity_debug(Text* out, i32 kind, i64 value)`: the debug text of
/// a quantity of `value` base units (`docs/slice.md` section 14.1), kind 0
/// `Duration`, 1 `ByteSize`, 2 `Ratio`. An unknown kind reads as `Duration`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_quantity_debug(out: *mut IgniteList, kind: i32, value: i64) {
    let kind = match kind {
        1 => ignite_io::Quantity::ByteSize,
        2 => ignite_io::Quantity::Ratio,
        _ => ignite_io::Quantity::Duration,
    };
    put(
        out,
        IgniteList::from_bytes(ignite_io::quantity_debug(kind, value).as_bytes()),
    );
}

#[cfg(test)]
mod tests {
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
