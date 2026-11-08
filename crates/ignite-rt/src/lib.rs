//! Runtime of the LLVM target (`docs/llvm.md` sections 3 to 5): the
//! substrate `ignite-io` behind a C ABI, built as `libignite_rt.a` and linked
//! into every executable the LLVM target produces. It converts section-3
//! layouts and handles to and from `ignite-io` values and owns what only the
//! LLVM target needs: the arenas and loop regions, the handle table and the
//! spawn handshake. Every socket, standard input, standard output, terminal
//! and thread operation is a call into `ignite-io`.
//!
//! Frozen skeleton: the `#[repr(C)]` layouts below are the contract with the
//! generator.
//!
//! Every exported function is `extern "C"` with the name and signature of
//! `docs/llvm.md` section 5. Pointer arguments are trusted: the generator is
//! the only caller and passes valid, aligned pointers to section-3 layouts
//! (an aggregate argument is the callee's own copy, an `out`/`err` pointer is
//! writable). That is why the exported functions are safe to call from Rust
//! by signature and dereference their pointers inside; a null `out` or `err`
//! pointer is skipped, a null `Text`/`List` reads as empty. Every one but
//! `init` runs its body through `contain::entry`, so a Rust panic inside it
//! is a failure of the calling task, never an unwind into generated code.
//!
//! Every `Text`, `List` and `IoError` message the runtime returns is
//! allocated in the calling task's arena ([`ignite_rt_alloc`]).
#![allow(clippy::not_unsafe_ptr_arg_deref)]

mod arena;
mod clock;
mod fail;
mod net;
mod process;
mod stdio;
mod store;
mod task;
mod text;

pub use arena::{
    ARENA_DUE, State, arena_reserved, ignite_rt_alloc, ignite_rt_arena_due, ignite_rt_arena_mark,
    ignite_rt_arena_release, ignite_rt_arena_scratch, ignite_rt_arena_scratch_clear,
    ignite_rt_arena_state, ignite_rt_task_end,
};
pub use clock::ignite_rt_clock_monotonic;
pub use fail::{
    ignite_rt_assert_failed, ignite_rt_fail_arith, ignite_rt_fail_index, ignite_rt_fail_match,
    ignite_rt_fail_neg, ignite_rt_fail_slice, ignite_rt_guard_callback, ignite_rt_guard_pop,
    ignite_rt_guard_push, ignite_rt_try_failed,
};
pub use net::{
    ignite_rt_own_mark, ignite_rt_own_release, ignite_rt_tcp_accept, ignite_rt_tcp_bind,
    ignite_rt_tcp_close, ignite_rt_tcp_connect, ignite_rt_tcp_connect_within,
    ignite_rt_tcp_local_port, ignite_rt_tcp_own, ignite_rt_tcp_peer, ignite_rt_tcp_read,
    ignite_rt_tcp_release, ignite_rt_tcp_retain, ignite_rt_tcp_set_idle_timeout,
    ignite_rt_tcp_timed_out, ignite_rt_tcp_urgent_pending, ignite_rt_tcp_write_all,
    ignite_rt_tcp_write_urgent,
};
pub use process::{
    ignite_rt_args_port, ignite_rt_endpoint_of, ignite_rt_exit, ignite_rt_fail, ignite_rt_init,
    ignite_rt_io_error_new, ignite_rt_log, ignite_rt_panic,
};
pub use stdio::{
    ignite_rt_stderr_write_all, ignite_rt_stdin_is_terminal, ignite_rt_stdin_read,
    ignite_rt_stdin_set_raw, ignite_rt_stdout_write_all,
};
pub use store::{
    FreeHeap, FromHeap, RtStore, ToHeap, UpdateThunk, UpsertThunk, ignite_rt_store_get,
    ignite_rt_store_new, ignite_rt_store_next_id, ignite_rt_store_put, ignite_rt_store_remove,
    ignite_rt_store_update, ignite_rt_store_upsert, ignite_rt_store_values,
};
pub use task::{TaskFn, ignite_rt_spawn, ignite_rt_spawn_ready};
pub use text::{
    ignite_rt_int_parse, ignite_rt_int_to_text, ignite_rt_quantity_debug, ignite_rt_text_cmp,
    ignite_rt_text_concat, ignite_rt_text_debug, ignite_rt_text_eq,
    ignite_rt_text_from_bytes_lossy, ignite_rt_text_join, ignite_rt_text_starts_with,
    ignite_rt_text_trim,
};

use ignite_io::{Input, IoError};

/// `Text` and `List<T>`: `{ ptr, len, cap }`, lengths in elements.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IgniteList {
    pub ptr: *mut u8,
    pub len: i64,
    pub cap: i64,
}

/// `IoError`: `{ Text message, i32 exit_code }`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IgniteIoError {
    pub message: IgniteList,
    pub exit_code: i32,
}

/// `Input`: tag 0 `Data(Bytes)`, tag 1 `Eof`; payload at offset 8.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IgniteInput {
    pub tag: i32,
    pub data: IgniteList,
}

/// `std.net.Endpoint`: `{ Text address, i64 port }`, the record's fields in
/// declaration order (`docs/llvm.md` section 3); the `out` of
/// [`ignite_rt_endpoint_of`].
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IgniteEndpoint {
    pub address: IgniteList,
    pub port: i64,
}

impl IgniteList {
    /// A copy of `bytes` in the current task's arena, `cap` equal to `len`.
    pub fn from_bytes(bytes: &[u8]) -> IgniteList {
        let ptr = arena::alloc(bytes.len(), 1);
        // SAFETY: `ptr` is a fresh arena block of `bytes.len()` bytes.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
        IgniteList {
            ptr,
            len: bytes.len() as i64,
            cap: bytes.len() as i64,
        }
    }

    /// The `len` bytes at `ptr`; empty when `ptr` is null or `len` ≤ 0.
    ///
    /// # Safety
    ///
    /// `ptr` must point to at least `len` readable bytes that outlive `'a`
    /// (for runtime-made values: until the owning task's `task_end`).
    pub unsafe fn as_bytes<'a>(&self) -> &'a [u8] {
        if self.ptr.is_null() || self.len <= 0 {
            return &[];
        }
        // SAFETY: guaranteed by the caller.
        unsafe { std::slice::from_raw_parts(self.ptr, self.len as usize) }
    }
}

impl IgniteInput {
    /// `Eof`.
    fn eof() -> IgniteInput {
        IgniteInput {
            tag: 1,
            data: IgniteList {
                ptr: std::ptr::null_mut(),
                len: 0,
                cap: 0,
            },
        }
    }

    /// `input` with its bytes copied into the current task's arena.
    fn from_input(input: &Input) -> IgniteInput {
        match input {
            Input::Data(bytes) => IgniteInput {
                tag: 0,
                data: IgniteList::from_bytes(bytes),
            },
            Input::Eof => IgniteInput::eof(),
        }
    }
}

impl IgniteIoError {
    fn from_error(error: &IoError) -> IgniteIoError {
        IgniteIoError {
            message: IgniteList::from_bytes(error.message.as_bytes()),
            exit_code: error.exit_code(),
        }
    }
}

/// Write `value` to `out` unless `out` is null.
fn put<T>(out: *mut T, value: T) {
    if !out.is_null() {
        // SAFETY: a non-null `out` is writable (crate docs).
        unsafe { out.write(value) };
    }
}

/// The aggregate at `value`, or `None` when the pointer is null.
fn get<T: Copy>(value: *const T) -> Option<T> {
    if value.is_null() {
        None
    } else {
        // SAFETY: a non-null argument points to a valid layout (crate docs).
        Some(unsafe { value.read() })
    }
}

/// The bytes of the `Text` or `Bytes` argument at `value`.
fn bytes<'a>(value: *const IgniteList) -> &'a [u8] {
    match get(value) {
        // SAFETY: an argument list is valid for the call (crate docs).
        Some(list) => unsafe { list.as_bytes() },
        None => &[],
    }
}

/// Status 0 and `value` in `out` on success; status 1 and the error in
/// `err` on failure.
fn status<T>(result: Result<T, IoError>, out: *mut T, err: *mut IgniteIoError) -> i32 {
    match result {
        Ok(value) => {
            put(out, value);
            0
        }
        Err(error) => {
            put(err, IgniteIoError::from_error(&error));
            1
        }
    }
}

// Tests (`docs/slice.md` section 12) come last, so the line numbers of
// everything above, and with them the debug info of a program without
// tests, stay as they were before tests existed.
mod test;
pub use test::IgniteTest;
#[cfg(feature = "runner")]
pub use test::ignite_rt_test_main;

// Slice 14 (`docs/slice.md` 18.9): the pipe entries, after the tests for
// the same reason.
mod pipe;
pub use pipe::{
    ignite_rt_pipe_close, ignite_rt_pipe_pair, ignite_rt_pipe_read,
    ignite_rt_pipe_received_batches, ignite_rt_pipe_set_idle_timeout,
    ignite_rt_pipe_urgent_pending, ignite_rt_pipe_write_all, ignite_rt_pipe_write_urgent,
};

// Slice 17 (`docs/slice.md` 21.8): the capability entries, after the tests
// for the same reason. Surface: U1; `fs` U2, `child` U3, `entropy` U5.
mod child;
mod entropy;
mod fs;
pub use child::{
    IgniteChildExit, IgniteInvocation, ignite_rt_child_id, ignite_rt_child_kill,
    ignite_rt_child_wait, ignite_rt_launcher_execute, ignite_rt_launcher_new,
    ignite_rt_launcher_start,
};
pub use entropy::{ignite_rt_entropy_new, ignite_rt_entropy_open, ignite_rt_entropy_random_bytes};
pub use fs::{
    ignite_rt_directory_absolute_path, ignite_rt_directory_create_directory,
    ignite_rt_directory_launch, ignite_rt_directory_read_directory, ignite_rt_directory_read_file,
    ignite_rt_directory_remove, ignite_rt_directory_remove_tree, ignite_rt_directory_temporary,
    ignite_rt_directory_write_file,
};
pub use process::{
    ignite_rt_args_flag, ignite_rt_setting_launch, ignite_rt_setting_of, ignite_rt_setting_value,
};

impl IgniteList {
    /// A `List<Text>` of copies of `texts` in the current task's arena.
    pub fn from_texts(texts: &[String]) -> IgniteList {
        let size = std::mem::size_of::<IgniteList>();
        let ptr = arena::alloc(texts.len() * size, std::mem::align_of::<IgniteList>());
        for (index, text) in texts.iter().enumerate() {
            let element = IgniteList::from_bytes(text.as_bytes());
            // SAFETY: `ptr` is a fresh arena block of `texts.len()` aligned
            // elements; `index` is in range.
            unsafe { ptr.cast::<IgniteList>().add(index).write(element) };
        }
        IgniteList {
            ptr,
            len: texts.len() as i64,
            cap: texts.len() as i64,
        }
    }
}

// Panic containment at the C ABI (`crate::contain`), after the tests for
// the same reason.
mod contain;
pub use contain::UNGUARDED_ENTRIES;
use contain::{entry, entry_never};

/// The log line on which [`ignite_rt_log`] panics in an archive built with
/// the feature `panic-probe`: the one way a test drives a Rust panic inside
/// a runtime entry from generated code (`docs/llvm.md` section 4). Only
/// `ignite-llvm`'s tests enable the feature (a dev-dependency); the archive
/// `ignite build` links, built by `crates/ignite/build.rs` and by
/// `cargo build -p ignite-rt [--features runner]`, has no probe.
#[cfg(feature = "panic-probe")]
pub const PANIC_PROBE: &str = "ignite-rt panic probe";

#[cfg(feature = "panic-probe")]
fn panic_probe(line: &[u8]) {
    if line == PANIC_PROBE.as_bytes() {
        panic!("{PANIC_PROBE}");
    }
}
