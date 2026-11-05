//! Tests (`docs/slice.md` section 12, `docs/llvm.md` section 5): the
//! runner over a table of test bodies. `ignite_rt_test_main` exists with the
//! feature `runner`, which a test build's runtime archive enables
//! (`docs/llvm.md` section 5). A failing test ends its process through
//! [`crate::fail`] (no task wrapper is registered, so the report is logged
//! and the process exits [`ignite_io::FAILURE_EXIT_CODE`]).

use crate::IgniteList;

/// One entry of the table a test main passes to [`ignite_rt_test_main`]:
/// the test's full name and its body, `void ()`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IgniteTest {
    pub name: IgniteList,
    pub run: extern "C" fn(),
}

/// `void test_main(IgniteTest* table, i64 count)`: [`ignite_io::test_main_with`]
/// over the `count` entries at `table` and the arguments `init` stored.
#[cfg(feature = "runner")]
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_test_main(table: *const IgniteTest, count: i64) -> ! {
    crate::entry_never(move || test_main(table, count))
}

/// The body of [`ignite_rt_test_main`].
#[cfg(feature = "runner")]
fn test_main(table: *const IgniteTest, count: i64) -> ! {
    let entries: &[IgniteTest] = if table.is_null() || count <= 0 {
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
    let tests: Vec<ignite_io::Test<'_>> = names
        .iter()
        .zip(&runs)
        .map(|(name, run)| ignite_io::Test {
            name,
            run: run.as_ref(),
        })
        .collect();
    ignite_io::test_main_with(crate::process::args(), &tests)
}
