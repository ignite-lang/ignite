//! Tests (`docs/slice.md` section 12, `docs/llvm.md` section 5): the
//! runner and the failure reports of `assert` and `?` are `telos-io`'s;
//! these functions only convert the LLVM layouts. `telos_rt_test_main`
//! exists with the feature `runner`, which a test build's runtime archive
//! enables (`docs/llvm.md` section 5).

use crate::TelosList;

/// One entry of the table a test main passes to [`telos_rt_test_main`]:
/// the test's full name and its body, `void ()`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TelosTest {
    pub name: TelosList,
    pub run: extern "C" fn(),
}

/// `void test_main(TelosTest* table, i64 count)`: [`telos_io::test_main_with`]
/// over the `count` entries at `table` and the arguments `init` stored.
#[cfg(feature = "runner")]
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_test_main(table: *const TelosTest, count: i64) -> ! {
    let entries: &[TelosTest] = if table.is_null() || count <= 0 {
        &[]
    } else {
        // SAFETY: the generated table holds `count` entries (crate docs).
        unsafe { std::slice::from_raw_parts(table, count as usize) }
    };
    let names: Vec<String> = entries
        .iter()
        // SAFETY: a table name is a constant of the executable.
        .map(|entry| String::from_utf8_lossy(unsafe { entry.name.as_bytes() }).into_owned())
        .collect();
    let runs: Vec<Box<dyn Fn()>> = entries
        .iter()
        .map(|entry| {
            let run = entry.run;
            Box::new(move || run()) as Box<dyn Fn()>
        })
        .collect();
    let tests: Vec<telos_io::Test<'_>> = names
        .iter()
        .zip(&runs)
        .map(|(name, run)| telos_io::Test {
            name,
            run: run.as_ref(),
        })
        .collect();
    telos_io::test_main_with(crate::process::args(), &tests)
}

/// The text of a `Text*` argument, empty for null. A Telos `Text` is
/// UTF-8, so the empty fallback for other bytes is never taken. A `match`,
/// not `unwrap_or_default`, which is generic (see [`telos_rt_assert_failed`]).
#[allow(clippy::manual_unwrap_or_default)]
fn text_or_empty<'a>(value: *const TelosList) -> &'a str {
    if value.is_null() {
        return "";
    }
    // SAFETY: an argument list is valid for the call (crate docs).
    let bytes = unsafe { (*value).as_bytes() };
    match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => "",
    }
}

/// The text of a `Text*` argument; `None` for null.
fn text<'a>(value: *const TelosList) -> Option<&'a str> {
    if value.is_null() {
        None
    } else {
        Some(text_or_empty(value))
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
