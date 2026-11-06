//! Standard input and output. Standard input is read with one blocking
//! `read(0)` per call, unbuffered, so a raw-mode terminal yields each byte
//! as it is typed. Standard output and standard error are written and flushed
//! on every call.

use std::io::Write;
use std::os::fd::AsFd;

use nix::errno::Errno;

use crate::{Input, IoError, read_input, terminal};

/// Standard input of the process, as a byte stream.
#[derive(Clone, Copy, Debug)]
pub struct Stdin;

/// Standard output of the process, as a byte stream.
#[derive(Clone, Copy, Debug)]
pub struct Stdout;

impl Stdin {
    /// One read of standard input, up to `maximum` bytes (clamped to
    /// `1..=1 MiB`). Zero bytes is `Input::Eof`. [`crate::run`] does not
    /// wait for a read that has no input yet.
    pub fn read(maximum: i64) -> Result<Input, IoError> {
        crate::flush();
        let stdin = std::io::stdin();
        read_input(maximum, |buffer| {
            loop {
                match nix::unistd::read(stdin.as_fd(), &mut *buffer) {
                    Ok(count) => return Ok(count),
                    Err(Errno::EINTR) => continue,
                    Err(errno) => return Err(std::io::Error::from(errno).into()),
                }
            }
        })
    }

    /// Whether standard input is a terminal.
    pub fn is_terminal() -> Result<bool, IoError> {
        Ok(terminal::is_terminal())
    }

    /// `true`: save the terminal attributes (once) and switch standard
    /// input to raw mode; `false`: restore the saved attributes. A no-op on
    /// a standard input that is not a terminal. Raw mode is non-canonical
    /// input without echo, `ISIG`, `IXON` or `ICRNL`, one byte per read;
    /// `OPOST` stays on.
    pub fn set_raw(on: bool) -> Result<(), IoError> {
        terminal::set_raw(on)
    }
}

impl Stdout {
    /// Write all of `data` to standard output and flush it. An interrupted
    /// write is retried by `write_all` and `flush` themselves.
    pub fn write_all(data: Vec<u8>) -> Result<(), IoError> {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(&data)?;
        stdout.flush()?;
        Ok(())
    }
}

/// Standard error of the process, as a byte stream (`docs/slice.md` 25.3).
#[derive(Clone, Copy, Debug)]
pub struct Stderr;

impl Stderr {
    /// Write all of `data` to standard error and flush it, as
    /// [`Stdout::write_all`] does for standard output. `log` writes to the
    /// same stream.
    pub fn write_all(data: Vec<u8>) -> Result<(), IoError> {
        let mut stderr = std::io::stderr().lock();
        stderr.write_all(&data)?;
        stderr.flush()?;
        Ok(())
    }
}
