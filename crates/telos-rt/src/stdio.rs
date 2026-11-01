//! Standard input and output: the C ABI over `telos_io::{Stdin, Stdout}`.
//! Reads, writes and terminal mode are `telos-io`'s: the same termios
//! rules, the same saved attributes and the same restore.

use telos_io::{Stdin, Stdout};

use crate::{TelosInput, TelosIoError, TelosList, bytes, status};

/// `i32 stdin_read(i64 maximum, Input* out, IoError* err)`: one blocking
/// read of up to `maximum` bytes (clamped to `1..=1 MiB`); zero bytes is
/// `Eof`.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_stdin_read(
    maximum: i64,
    out: *mut TelosInput,
    err: *mut TelosIoError,
) -> i32 {
    let read = Stdin::read(maximum);
    status(read.map(|input| TelosInput::from_input(&input)), out, err)
}

/// `i32 stdin_is_terminal(i8* out, IoError* err)`: 1 when standard input is
/// a terminal, else 0. Never fails.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_stdin_is_terminal(out: *mut i8, err: *mut TelosIoError) -> i32 {
    status(Stdin::is_terminal().map(i8::from), out, err)
}

/// `i32 stdin_set_raw(i8 on, IoError* err)`: `docs/slice.md` section 7
/// terminal mode; a no-op on a standard input that is not a terminal.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_stdin_set_raw(on: i8, err: *mut TelosIoError) -> i32 {
    status(Stdin::set_raw(on != 0), std::ptr::null_mut(), err)
}

/// `i32 stdout_write_all(List* bytes, IoError* err)`: write and flush.
#[unsafe(no_mangle)]
pub extern "C" fn telos_rt_stdout_write_all(data: *const TelosList, err: *mut TelosIoError) -> i32 {
    let written = Stdout::write_all(bytes(data).to_vec());
    status(written, std::ptr::null_mut(), err)
}
