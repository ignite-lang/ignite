//! Ignite IO substrate (`docs/slice.md` section 7): the one implementation of
//! TCP, standard input and output, terminal mode, process arguments, tasks
//! and the process entry point. Both targets call it: the Rust target's
//! generated code directly, the LLVM target through the C ABI adapter
//! `ignite-rt`.
//!
//! Blocking: `std::net` sockets, one OS thread per running task ([`spawn`], threads reused), no
//! event loop. Each operation keeps the name and signature an async
//! version would have, minus `async`, so making the substrate async
//! changes this crate and the two adapters (the Rust generator's lowering
//! of effectful calls and `spawn`, `ignite-rt`), not the callers.
//!
//! Every handle is `Clone` because the Rust generator clones every place it
//! passes by value.

use std::fmt;
use std::io::Write;

mod idle;
mod net;
mod stdio;
mod store;
mod task;

pub use net::{
    ACCEPT_BACKOFF_FIRST, ACCEPT_BACKOFF_LAST, SEND_BUFFER, TcpListener, TcpStream, flush,
};
pub use stdio::{Stdin, Stdout};
pub use store::Store;
pub use task::{LINGER, spawn, try_spawn};

/// An IO failure, carrying the operating system's message and the exit
/// status [`run`] returns when `main` ends with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IoError {
    pub message: String,
    exit_code: i32,
}

impl IoError {
    /// An error with the given message and exit code 1.
    pub fn new(message: impl Into<String>) -> IoError {
        IoError {
            message: message.into(),
            exit_code: 1,
        }
    }

    /// The same error with exit code `code`.
    pub fn with_exit_code(self, code: i32) -> IoError {
        IoError {
            exit_code: code,
            ..self
        }
    }

    /// The exit status [`run`] returns for this error.
    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for IoError {}

impl From<std::io::Error> for IoError {
    fn from(error: std::io::Error) -> IoError {
        IoError::new(error.to_string())
    }
}

/// One read from a stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    Data(Vec<u8>),
    Eof,
}

/// The running process.
#[derive(Clone, Copy, Debug)]
pub struct Process;

/// Largest buffer a single read allocates.
const MAX_READ: usize = 1024 * 1024;

/// A read buffer of `maximum` bytes, clamped to `1..=1 MiB`.
fn read_buffer(maximum: i64) -> Vec<u8> {
    let size = usize::try_from(maximum.max(1))
        .unwrap_or(MAX_READ)
        .min(MAX_READ);
    vec![0u8; size]
}

/// `Input::Data` of the first `count` bytes, or `Input::Eof` for zero.
fn input(mut buffer: Vec<u8>, count: usize) -> Input {
    if count == 0 {
        return Input::Eof;
    }
    buffer.truncate(count);
    Input::Data(buffer)
}

/// `"{address}:{port}"`, or an error when `port` is outside `0..=65535`.
/// `operation` (`bind`, `connect`) prefixes the message.
fn socket_address(operation: &str, address: &str, port: i64) -> Result<String, IoError> {
    if !(0..=65535).contains(&port) {
        return Err(IoError::new(format!(
            "{operation} {address}:{port}: port {port} is outside 0..=65535"
        )));
    }
    Ok(format!("{address}:{port}"))
}

impl Process {
    /// [`exit`] with `code` truncated to `i32`. Never returns.
    pub fn exit(code: i64) -> Result<(), IoError> {
        exit(code as i32)
    }
}

/// Send the current task's pending writes ([`flush`]), restore the terminal
/// attributes saved by [`Stdin::set_raw`] and end the process at once with
/// status `code`. Other tasks and pending reads are not waited for, and
/// standard output is not locked: `Stdout::write_all` flushes on every call,
/// so there is nothing to send, and a task stalled in one must not hold up
/// the exit.
pub fn exit(code: i32) -> ! {
    flush();
    terminal::restore();
    std::process::exit(code)
}

/// Terminal mode of standard input (`docs/slice.md` section 7).
///
/// Raw mode is non-canonical input without echo, signals (`ISIG`), flow
/// control (`IXON`) or CR translation (`ICRNL`), one byte per read
/// (`VMIN` 1, `VTIME` 0). Output processing (`OPOST`) stays on so `\n`
/// still moves to the start of the next line. The attributes in effect at
/// the first `set_raw(true)` are kept for the life of the process;
/// `set_raw(false)` and [`run`] (when `main` returns or panics) restore
/// them.
pub mod terminal {
    use std::io::IsTerminal;
    use std::sync::{Mutex, PoisonError};

    use nix::sys::termios::{
        InputFlags, LocalFlags, SetArg, SpecialCharacterIndices, Termios, tcgetattr, tcsetattr,
    };

    use super::IoError;

    /// The attributes before the first `set_raw(true)`.
    static SAVED: Mutex<Option<Termios>> = Mutex::new(None);

    fn error(operation: &str, errno: nix::Error) -> IoError {
        IoError::new(format!("terminal {operation}: {errno}"))
    }

    pub fn is_terminal() -> bool {
        std::io::stdin().is_terminal()
    }

    pub fn set_raw(on: bool) -> Result<(), IoError> {
        let stdin = std::io::stdin();
        if !stdin.is_terminal() {
            return Ok(());
        }
        let mut saved = SAVED.lock().unwrap_or_else(PoisonError::into_inner);
        if !on {
            if let Some(original) = saved.as_ref() {
                tcsetattr(&stdin, SetArg::TCSANOW, original).map_err(|e| error("restore", e))?;
            }
            return Ok(());
        }
        let original = match saved.as_ref() {
            Some(original) => original.clone(),
            None => {
                let current = tcgetattr(&stdin).map_err(|e| error("read attributes", e))?;
                *saved = Some(current.clone());
                current
            }
        };
        let mut raw = original;
        raw.local_flags
            .remove(LocalFlags::ICANON | LocalFlags::ECHO | LocalFlags::ISIG);
        raw.input_flags.remove(InputFlags::IXON | InputFlags::ICRNL);
        raw.control_chars[SpecialCharacterIndices::VMIN as usize] = 1;
        raw.control_chars[SpecialCharacterIndices::VTIME as usize] = 0;
        tcsetattr(&stdin, SetArg::TCSANOW, &raw).map_err(|e| error("set raw mode", e))
    }

    /// Restore the saved attributes, if any; errors are ignored.
    pub fn restore() {
        let saved = SAVED.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(original) = saved.as_ref() {
            let _ = tcsetattr(std::io::stdin(), SetArg::TCSANOW, original);
        }
    }

    /// Restores the saved attributes when dropped, also during a panic.
    pub struct Restore;

    impl Drop for Restore {
        fn drop(&mut self) {
            restore();
        }
    }
}

/// Run `main` on the calling thread. Returns 0 on `Ok`; on `Err` writes
/// `error: <message>` to stderr and returns [`IoError::exit_code`]; when
/// `main` fails (a failed `assert`, an overflow, an index out of range, a
/// Rust panic: `docs/slice.md` section 14.8) writes `error: <report>`
/// and returns [`FAILURE_EXIT_CODE`]. Tasks started with [`spawn`] keep
/// running after `run` returns, including one blocked in a [`Stdin::read`]
/// that has no input yet; a generated `main` passes the status to
/// `std::process::exit`, which ends them. Terminal attributes saved by
/// [`Stdin::set_raw`] are restored when `main` returns and when it fails.
pub fn run<F>(main: F) -> i32
where
    F: FnOnce() -> Result<(), IoError>,
{
    let restore = terminal::Restore;
    let result = report::catch(main);
    flush();
    drop(restore);
    match result {
        Ok(Ok(())) => 0,
        Ok(Err(e)) => {
            log(&format!("error: {e}"));
            e.exit_code()
        }
        Err(report) => {
            log(&format!("error: {report}"));
            FAILURE_EXIT_CODE
        }
    }
}

/// Write `line` and a newline to stderr in one call; errors are ignored.
pub fn log(line: &str) {
    let mut bytes = Vec::with_capacity(line.len() + 1);
    bytes.extend_from_slice(line.as_bytes());
    bytes.push(b'\n');
    let _ = std::io::stderr().lock().write_all(&bytes);
}

pub mod args {
    use super::IoError;

    /// Exit status of a malformed or out-of-range `--port`.
    const PORT_EXIT_CODE: i32 = 2;

    /// `--port N` from the process arguments, if present and valid.
    pub fn port_override() -> Option<i64> {
        from_args(std::env::args())
    }

    /// `--port N` from the process arguments: `Ok(None)` when absent, an
    /// error with exit code 2 when malformed or outside `0..=65535`.
    pub fn port() -> Result<Option<i64>, IoError> {
        port_from_args(std::env::args())
    }

    /// The first `--port N` or `--port=N` in `args`, parsed as an integer
    /// in `0..=65535`. Any other form, a missing value or an out-of-range
    /// value is `None`.
    pub fn from_args(args: impl Iterator<Item = String>) -> Option<i64> {
        port_from_args(args).ok().flatten()
    }

    /// The first `--port N` or `--port=N` in `args`: `Ok(None)` when there
    /// is none, an error with exit code 2 when its value is missing, not an
    /// integer or outside `0..=65535`.
    pub fn port_from_args(args: impl Iterator<Item = String>) -> Result<Option<i64>, IoError> {
        let mut args = args;
        while let Some(arg) = args.next() {
            if arg == "--port" {
                return match args.next() {
                    Some(value) => parse_port(&value).map(Some),
                    None => {
                        Err(IoError::new("--port needs a value").with_exit_code(PORT_EXIT_CODE))
                    }
                };
            }
            if let Some(value) = arg.strip_prefix("--port=") {
                return parse_port(value).map(Some);
            }
        }
        Ok(None)
    }

    fn parse_port(value: &str) -> Result<i64, IoError> {
        value
            .parse::<i64>()
            .ok()
            .filter(|port| (0..=65535).contains(port))
            .ok_or_else(|| {
                IoError::new(format!("--port {value}: expected an integer in 0..=65535"))
                    .with_exit_code(PORT_EXIT_CODE)
            })
    }

    #[cfg(test)]
    mod tests {
        use super::{from_args, port_from_args};

        fn checked(args: &[&str]) -> Result<Option<i64>, i32> {
            port_from_args(
                std::iter::once("prog")
                    .chain(args.iter().copied())
                    .map(String::from),
            )
            .map_err(|e| e.exit_code())
        }

        #[test]
        fn checked_port() {
            assert_eq!(checked(&[]), Ok(None));
            assert_eq!(checked(&["--port", "0"]), Ok(Some(0)));
            assert_eq!(checked(&["--port=65535"]), Ok(Some(65535)));
            assert_eq!(checked(&["--port", "abc"]), Err(2));
            assert_eq!(checked(&["--port=abc"]), Err(2));
            assert_eq!(checked(&["--port", "65536"]), Err(2));
            assert_eq!(checked(&["--port", "-1"]), Err(2));
            assert_eq!(checked(&["--port"]), Err(2));
        }

        fn parse(args: &[&str]) -> Option<i64> {
            from_args(
                std::iter::once("prog")
                    .chain(args.iter().copied())
                    .map(String::from),
            )
        }

        #[test]
        fn separate_value() {
            assert_eq!(parse(&["--port", "0"]), Some(0));
            assert_eq!(parse(&["--port", "2323"]), Some(2323));
        }

        #[test]
        fn joined_value() {
            assert_eq!(parse(&["--port=2323"]), Some(2323));
        }

        #[test]
        fn out_of_range_is_none() {
            assert_eq!(parse(&["--port", "70000"]), None);
            assert_eq!(parse(&["--port", "-1"]), None);
        }

        #[test]
        fn missing_value_is_none() {
            assert_eq!(parse(&["--port"]), None);
        }

        #[test]
        fn absent_is_none() {
            assert_eq!(parse(&[]), None);
            assert_eq!(parse(&["--verbose"]), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_display_is_message() {
        let error = IoError::new("boom");
        assert_eq!(error.to_string(), "boom");
    }

    #[test]
    fn io_error_from_std() {
        let std_error = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "peer reset");
        let error = IoError::from(std_error);
        assert_eq!(error.message, "peer reset");
        assert_eq!(error.to_string(), "peer reset");
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn io_error_exit_codes() {
        let error = IoError::new("boom");
        assert_eq!(error.exit_code(), 1);
        let error = error.with_exit_code(3);
        assert_eq!(error.exit_code(), 3);
        assert_eq!(error.message, "boom");
    }

    #[test]
    fn run_returns_the_exit_code_of_the_error() {
        assert_eq!(run(|| Ok(())), 0);
        assert_eq!(run(|| Err(IoError::new("plain"))), 1);
        assert_eq!(run(|| Err(IoError::new("no peer").with_exit_code(3))), 3);
        assert_eq!(run(|| Err(IoError::new("--port abc").with_exit_code(2))), 2);
    }

    #[test]
    fn run_returns_101_on_a_failure_or_panic() {
        assert_eq!(
            run(|| fail("+", "f.ignite", 1, 2, "9223372036854775807 + 1")),
            FAILURE_EXIT_CODE
        );
        assert_eq!(run(|| panic!("boom")), FAILURE_EXIT_CODE);
        assert_eq!(run(|| Ok(())), 0);
    }

    #[test]
    fn port_outside_range_is_error() {
        let error = socket_address("bind", "127.0.0.1", 70000).unwrap_err();
        assert!(error.message.contains("70000"), "{error}");
        assert!(socket_address("bind", "127.0.0.1", -1).is_err());
        assert_eq!(
            socket_address("bind", "127.0.0.1", 0).unwrap(),
            "127.0.0.1:0"
        );
    }
}

// Tests (`docs/slice.md` section 12) come last, so the line numbers of
// everything above, and with them the debug info of a program without
// tests, stay as they were before tests existed.
mod report;
#[cfg(feature = "runner")]
mod test;
pub use report::{
    FAILURE_EXIT_CODE, Failure, assert_failed, assert_report, catch, fail, fail_with,
    failure_report, report_of, task_failed, try_failed, try_report,
};

// Slice 10 (`docs/slice.md` sections 14.1 and 14.8): the checked operations
// and the quantity debug text, after the tests for the same reason.
pub mod checked;
mod quantity;
pub use quantity::{Quantity, quantity_debug};
#[cfg(feature = "runner")]
pub use test::{
    Arguments, DEFAULT_TIMEOUT, Test, USAGE_EXIT_CODE, Verdict, test_main, test_main_with,
};
