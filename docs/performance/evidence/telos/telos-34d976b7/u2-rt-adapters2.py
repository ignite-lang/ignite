import sys

path = "/home/timo/projects/telos/crates/telos-rt/src/test.rs"
text = open(path).read()

start = text.index("/// `void assert_failed(")
new_tail = '''/// The text of a `Text*` argument; `None` for null. A Telos `Text` is
/// UTF-8, so the empty fallback is never taken.
fn text<'a>(value: *const TelosList) -> Option<&'a str> {
    if value.is_null() {
        return None;
    }
    // SAFETY: an argument list is valid for the call (crate docs).
    let bytes = unsafe { (*value).as_bytes() };
    match std::str::from_utf8(bytes) {
        Ok(text) => Some(text),
        Err(_) => Some(""),
    }
}

/// The text of a `Text*` argument that is never null.
fn text_or_empty<'a>(value: *const TelosList) -> &'a str {
    match text(value) {
        Some(text) => text,
        None => "",
    }
}

/// `void assert_failed(Text* cond, Text* message, Text* left, Text* right,
/// Text* at)`: [`telos_io::assert_failed`]. `message` is null without a
/// message, `left` and `right` are null unless the condition compares.
///
/// The failure adapters call no generic function: a generic instance they
/// shared with the rest of the runtime would move between its codegen
/// units and change every executable linked against it.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_assert_failed(
    cond: *const TelosList,
    message: *const TelosList,
    left: *const TelosList,
    right: *const TelosList,
    at: *const TelosList,
) -> ! {
    let operands = match (text(left), text(right)) {
        (Some(left), Some(right)) => Some((left, right)),
        _ => None,
    };
    telos_io::assert_failed(
        text_or_empty(cond),
        text(message),
        operands,
        text_or_empty(at),
    )
}

/// `void try_failed(Text* error, Text* at)`: [`telos_io::try_failed`];
/// `error` is null for a `None`.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_try_failed(error: *const TelosList, at: *const TelosList) -> ! {
    telos_io::try_failed(text(error), text_or_empty(at))
}
'''
text = text[:start] + new_tail
text = text.replace("use crate::{TelosList, bytes};", "use crate::TelosList;", 1)
open(path, "w").write(text)
