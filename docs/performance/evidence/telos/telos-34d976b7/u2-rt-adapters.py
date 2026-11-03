import sys

path = "/home/timo/projects/telos/crates/telos-rt/src/test.rs"
text = open(path).read()

old_text_fn = '''/// The text of a `Text*` argument, lossily decoded; `None` for null.
fn text(value: *const TelosList) -> Option<String> {
    get(value).map(|_| String::from_utf8_lossy(bytes(value)).into_owned())
}

'''
start = text.index("/// `void assert_failed(")
old_tail = text[start:]
new_tail = '''/// `void assert_failed(Text* cond, Text* message, Text* left, Text* right,
/// Text* at)`: [`telos_io::assert_failed`]. `message` is null without a
/// message, `left` and `right` are null unless the condition compares.
/// The adapters of a failure avoid generic helpers, so a runtime that has
/// them shares no instance with the rest and links as it did before tests.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_assert_failed(
    cond: *const TelosList,
    message: *const TelosList,
    left: *const TelosList,
    right: *const TelosList,
    at: *const TelosList,
) -> ! {
    let cond = String::from_utf8_lossy(bytes(cond));
    let message_text = String::from_utf8_lossy(bytes(message));
    let left_text = String::from_utf8_lossy(bytes(left));
    let right_text = String::from_utf8_lossy(bytes(right));
    let at = String::from_utf8_lossy(bytes(at));
    let message = if message.is_null() {
        None
    } else {
        Some(&*message_text)
    };
    let operands = if left.is_null() || right.is_null() {
        None
    } else {
        Some((&*left_text, &*right_text))
    };
    telos_io::assert_failed(&cond, message, operands, &at)
}

/// `void try_failed(Text* error, Text* at)`: [`telos_io::try_failed`];
/// `error` is null for a `None`.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_try_failed(error: *const TelosList, at: *const TelosList) -> ! {
    let error_text = String::from_utf8_lossy(bytes(error));
    let at = String::from_utf8_lossy(bytes(at));
    let error = if error.is_null() {
        None
    } else {
        Some(&*error_text)
    };
    telos_io::try_failed(error, &at)
}
'''
if text.count(old_text_fn) != 1:
    sys.exit("text fn not found")
text = text.replace(old_text_fn, "", 1)
text = text.replace(old_tail, new_tail, 1)
text = text.replace("use crate::{TelosList, bytes, get};", "use crate::{TelosList, bytes};", 1)
open(path, "w").write(text)
