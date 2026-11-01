//! Runtime of the LLVM target (`docs/llvm.md` sections 3 to 5): the
//! substrate `telos-io` behind a C ABI, built as `libtelos_rt.a` and linked
//! into every executable the LLVM target produces. It converts section-3
//! layouts and handles to and from `telos-io` values and owns what only the
//! LLVM target needs: the arenas and loop regions, the handle table and the
//! spawn handshake. Every socket, standard input, standard output, terminal
//! and thread operation is a call into `telos-io`.
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
//! pointer is skipped, a null `Text`/`List` reads as empty.
//!
//! Every `Text`, `List` and `IoError` message the runtime returns is
//! allocated in the calling task's arena ([`telos_rt_alloc`]).
#![allow(clippy::not_unsafe_ptr_arg_deref)]

mod arena;
mod net;
mod process;
mod stdio;
mod store;
mod task;
mod text;

pub use arena::{
    State, arena_reserved, telos_rt_alloc, telos_rt_arena_due, telos_rt_arena_mark,
    telos_rt_arena_release, telos_rt_arena_scratch, telos_rt_arena_scratch_clear,
    telos_rt_arena_state, telos_rt_task_end,
};
pub use net::{
    telos_rt_tcp_accept, telos_rt_tcp_bind, telos_rt_tcp_close, telos_rt_tcp_connect,
    telos_rt_tcp_local_port, telos_rt_tcp_read, telos_rt_tcp_release,
    telos_rt_tcp_set_idle_timeout, telos_rt_tcp_write_all,
};
pub use process::{
    telos_rt_args_port, telos_rt_exit, telos_rt_fail, telos_rt_init, telos_rt_io_error_new,
    telos_rt_log, telos_rt_panic,
};
pub use stdio::{
    telos_rt_stdin_is_terminal, telos_rt_stdin_read, telos_rt_stdin_set_raw,
    telos_rt_stdout_write_all,
};
pub use store::{
    FreeHeap, FromHeap, RtStore, ToHeap, UpdateThunk, telos_rt_store_get, telos_rt_store_new,
    telos_rt_store_next_id, telos_rt_store_put, telos_rt_store_remove, telos_rt_store_update,
    telos_rt_store_values,
};
pub use task::{TaskFn, telos_rt_spawn, telos_rt_spawn_copy, telos_rt_spawn_ready};
pub use text::{
    telos_rt_int_parse, telos_rt_int_to_text, telos_rt_text_concat, telos_rt_text_eq,
    telos_rt_text_from_bytes_lossy, telos_rt_text_join, telos_rt_text_starts_with,
    telos_rt_text_trim,
};

use telos_io::{Input, IoError};

/// `Text` and `List<T>`: `{ ptr, len, cap }`, lengths in elements.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TelosList {
    pub ptr: *mut u8,
    pub len: i64,
    pub cap: i64,
}

/// `IoError`: `{ Text message, i32 exit_code }`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TelosIoError {
    pub message: TelosList,
    pub exit_code: i32,
}

/// `Input`: tag 0 `Data(Bytes)`, tag 1 `Eof`; payload at offset 8.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TelosInput {
    pub tag: i32,
    pub data: TelosList,
}

impl TelosList {
    /// A copy of `bytes` in the current task's arena, `cap` equal to `len`.
    pub fn from_bytes(bytes: &[u8]) -> TelosList {
        let ptr = arena::alloc(bytes.len(), 1);
        // SAFETY: `ptr` is a fresh arena block of `bytes.len()` bytes.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
        TelosList {
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

impl TelosInput {
    /// `Eof`.
    fn eof() -> TelosInput {
        TelosInput {
            tag: 1,
            data: TelosList {
                ptr: std::ptr::null_mut(),
                len: 0,
                cap: 0,
            },
        }
    }

    /// `input` with its bytes copied into the current task's arena.
    fn from_input(input: &Input) -> TelosInput {
        match input {
            Input::Data(bytes) => TelosInput {
                tag: 0,
                data: TelosList::from_bytes(bytes),
            },
            Input::Eof => TelosInput::eof(),
        }
    }
}

impl TelosIoError {
    fn from_error(error: &IoError) -> TelosIoError {
        TelosIoError {
            message: TelosList::from_bytes(error.message.as_bytes()),
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
fn bytes<'a>(value: *const TelosList) -> &'a [u8] {
    match get(value) {
        // SAFETY: an argument list is valid for the call (crate docs).
        Some(list) => unsafe { list.as_bytes() },
        None => &[],
    }
}

/// Status 0 and `value` in `out` on success; status 1 and the error in
/// `err` on failure.
fn status<T>(result: Result<T, IoError>, out: *mut T, err: *mut TelosIoError) -> i32 {
    match result {
        Ok(value) => {
            put(out, value);
            0
        }
        Err(error) => {
            put(err, TelosIoError::from_error(&error));
            1
        }
    }
}

// Tests (`docs/slice.md` section 12) come last, so the line numbers of
// everything above, and with them the debug info of a program without
// tests, stay as they were before tests existed.
mod test;
#[cfg(feature = "runner")]
pub use test::telos_rt_test_main;
pub use test::{TelosTest, telos_rt_assert_failed, telos_rt_try_failed};
