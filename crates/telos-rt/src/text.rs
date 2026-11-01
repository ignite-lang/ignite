//! `Text` helpers. Every result is a fresh `Text` in the calling task's
//! arena; arguments are only read.

use crate::{TelosList, bytes, get, put};

/// The `Text` elements of the `List<Text>` at `list`.
fn texts<'a>(list: *const TelosList) -> &'a [TelosList] {
    match get(list) {
        Some(list) if !list.ptr.is_null() && list.len > 0 => {
            // SAFETY: a `List<Text>` argument holds `len` aligned `Text`s.
            unsafe { std::slice::from_raw_parts(list.ptr.cast::<TelosList>(), list.len as usize) }
        }
        _ => &[],
    }
}

/// `void text_from_bytes_lossy(Text* out, List* bytes)`: invalid UTF-8
/// sequences become U+FFFD.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_text_from_bytes_lossy(out: *mut TelosList, data: *const TelosList) {
    let text = String::from_utf8_lossy(bytes(data));
    put(out, TelosList::from_bytes(text.as_bytes()));
}

/// `void text_trim(Text* out, Text* text)`: leading and trailing Unicode
/// whitespace removed, as Rust `str::trim`. A text that is not UTF-8 (which
/// no runtime or generated operation produces) loses ASCII whitespace only.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_text_trim(out: *mut TelosList, text: *const TelosList) {
    let text = bytes(text);
    let trimmed = match std::str::from_utf8(text) {
        Ok(text) => text.trim().as_bytes(),
        Err(_) => text.trim_ascii(),
    };
    put(out, TelosList::from_bytes(trimmed));
}

/// `void text_concat(Text* out, Text* a, Text* b)`.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_text_concat(
    out: *mut TelosList,
    a: *const TelosList,
    b: *const TelosList,
) {
    put(out, TelosList::from_bytes(&[bytes(a), bytes(b)].concat()));
}

/// `i8 text_eq(Text* a, Text* b)`: 1 when equal byte for byte, else 0.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_text_eq(a: *const TelosList, b: *const TelosList) -> i8 {
    i8::from(bytes(a) == bytes(b))
}

/// `i8 text_starts_with(Text* text, Text* prefix)`: 1 or 0.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_text_starts_with(
    text: *const TelosList,
    prefix: *const TelosList,
) -> i8 {
    i8::from(bytes(text).starts_with(bytes(prefix)))
}

/// `void text_join(Text* out, List* texts, Text* separator)`.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_text_join(
    out: *mut TelosList,
    list: *const TelosList,
    separator: *const TelosList,
) {
    let parts: Vec<&[u8]> = texts(list).iter().map(|text| bytes(text)).collect();
    put(out, TelosList::from_bytes(&parts.join(bytes(separator))));
}

/// `i8 int_parse(Text* text, i64* out)`: `Int.parse`, 1 and the value in
/// `out` for decimal digits with an optional leading `-`; 0 for anything
/// else (a `+`, spaces, no digits, overflow).
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_int_parse(text: *const TelosList, out: *mut i64) -> i8 {
    let text = bytes(text);
    if text.first() == Some(&b'+') {
        return 0;
    }
    match std::str::from_utf8(text)
        .ok()
        .and_then(|t| t.parse::<i64>().ok())
    {
        Some(value) => {
            put(out, value);
            1
        }
        None => 0,
    }
}

/// `void int_to_text(Text* out, i64 value)`: decimal, `-` for negatives.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_int_to_text(out: *mut TelosList, value: i64) {
    put(out, TelosList::from_bytes(value.to_string().as_bytes()));
}
