//! Thread-locals reached through a call that is never inlined
//! (`story:readiness-task-stealing`, `docs/readiness-io.md` risk K1).
//!
//! A task may move to another worker at its first park in a read
//! (`crate::sched`): it parks on one OS thread and resumes on another. The
//! compiler takes the address of a thread-local as a value that cannot
//! change within one function: it computes it once and keeps it across
//! every call, also across the call in which the task moved, and hoists it
//! out of loops (`llvm.threadlocal.address` is `memory(none)`). After the
//! move such a function would read and write the thread-local of the
//! thread the task left, while another task runs there. The substrate's
//! and the LLVM runtime's thread-locals are therefore read only through
//! [`with`] and [`try_with`]: the address is taken inside the call, on the
//! thread that makes it, and no caller holds it. Release builds inline
//! across crates (the Rust target with fat LTO, the LLVM target with
//! ThinLTO against the bitcode runtime), so an accessor that is merely
//! small would end up in the generated function that spans the read.
//!
//! What a task's stack holds across the park that moves it, and why each is
//! right on the thread it resumes on:
//!
//! - the program's frames down to the read (generated code; the LLVM
//!   runtime's entries): the substrate's and the runtime's thread-locals
//!   only through this module; the LLVM runtime's bump state, whose address
//!   generated code keeps for a whole function, is the task's own, in its
//!   arenas (`ignite_rt_arena_state`);
//! - `report::catch` frames: the count of open catches through this module,
//!   and the task's own while it runs (`report::swap_catching`);
//! - the stream's read lock (`wait::Lock`): its guard holds the value, not
//!   a `MutexGuard`;
//! - the runtime's cached stream and read buffer, and the substrate's read
//!   buffer: taken out of their thread-local before the read and put back
//!   after, into the thread of then;
//! - the scheduler's frames (`Readiness::wait_to_read`, `park_leaving`,
//!   `suspend`): their thread-locals through this module; the task's slot
//!   is read again after each switch;
//! - the LLVM runtime's `setjmp` buffers: on the task's stack, and glibc's
//!   pointer guard is the same in every thread of the process.
//!
//! Not covered: thread-locals of the standard library that its inlined
//! code reaches (the owner of stdout's lock, the panic count, the keys of
//! `RandomState`). A function that kept such an address across its first
//! read would use the thread it left; none is known to, and none is shown
//! not to.

use std::thread::{AccessError, LocalKey};

/// `key.with(f)`, in a call of its own (module docs).
#[inline(never)]
pub fn with<T: 'static, R>(key: &'static LocalKey<T>, f: impl FnOnce(&T) -> R) -> R {
    key.with(f)
}

/// `key.try_with(f)`, in a call of its own (module docs).
#[inline(never)]
pub fn try_with<T: 'static, R>(
    key: &'static LocalKey<T>,
    f: impl FnOnce(&T) -> R,
) -> Result<R, AccessError> {
    key.try_with(f)
}
