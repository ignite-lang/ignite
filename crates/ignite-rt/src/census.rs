//! `Runtime.census()` and `Runtime.settle(..)` (`docs/slice.md` 32) over
//! `ignite_io::census`, and the counting allocator of the LLVM target.
//!
//! The runtime's own Rust allocates through [`ignite_io::census::Counting`],
//! so `heap_bytes` holds the substrate's objects, the handle table and the
//! large arena blocks. Released large blocks an arena keeps for reuse are a
//! pool and are left out; so are the chunks, which come from `mmap`.
//! `arena_bytes` is the blocks of every task's arenas but the caller's.

#[global_allocator]
static ALLOCATOR: ignite_io::census::Counting = ignite_io::census::Counting;

/// The census as the LLVM target reads it.
pub(crate) fn read() -> ignite_io::census::Census {
    let mut census = ignite_io::census::read();
    census.heap_bytes -= crate::arena::pooled_large_bytes();
    census.arena_bytes -= crate::arena::own_bytes();
    census
}

/// `void runtime_census(Census* out)`: `Runtime.census()`, the record
/// `std.runtime.Census` (sixteen `i64`s in declaration order).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_runtime_census(out: *mut ignite_io::census::Census) {
    crate::entry(move || crate::put(out, read()))
}

/// `i8 runtime_settle(i64 tasks, i64 within)`: `Runtime.settle(tasks,
/// within)`, 1 once at most `tasks` tasks are live, 0 at the deadline.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_runtime_settle(tasks: i64, within: i64) -> i8 {
    crate::entry(move || i8::from(ignite_io::census::settle(tasks, within)))
}
