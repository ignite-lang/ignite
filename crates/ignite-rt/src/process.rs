//! Process entry and exit: `init`, arguments, `exit`, `fail`, `panic`, `log`
//! and `IoError.new`. Every way out of the process restores the terminal
//! attributes saved by `stdin_set_raw`: `exit`, `fail` and `panic` directly,
//! a return from C `main` (or any `exit(3)`) through an `atexit` handler, a
//! Rust panic inside the runtime through the panic hook. A process killed by
//! a signal cannot restore them.

use std::ffi::{CStr, c_char, c_int};
use std::sync::{Once, PoisonError, RwLock};

use ignite_io::IoError;
use nix::sys::signal::{SigHandler, Signal, signal};

use crate::{IgniteIoError, IgniteList, bytes, get, put, status};

/// Exit status of a panic (`docs/llvm.md` section 4).
const PANIC_EXIT_CODE: i32 = 101;

/// The arguments stored by `init`; `None` before it (then the process
/// arguments as std sees them are used).
static ARGS: RwLock<Option<Vec<String>>> = RwLock::new(None);

static HOOKS: Once = Once::new();

unsafe extern "C" {
    fn atexit(callback: extern "C" fn()) -> c_int;
}

extern "C" fn restore_at_exit() {
    ignite_io::terminal::restore();
}

/// The arguments `argv[0..argc]`, lossily decoded; null entries are skipped.
fn decode_args(argc: i32, argv: *const *const c_char) -> Vec<String> {
    if argv.is_null() {
        return Vec::new();
    }
    (0..usize::try_from(argc).unwrap_or(0))
        .filter_map(|index| {
            // SAFETY: `argv` holds `argc` entries (C `main` contract).
            let arg = unsafe { *argv.add(index) };
            // SAFETY: a non-null entry is a NUL-terminated string.
            (!arg.is_null()).then(|| {
                unsafe { CStr::from_ptr(arg) }
                    .to_string_lossy()
                    .into_owned()
            })
        })
        .collect()
}

fn store_args(args: Vec<String>) {
    *ARGS.write().unwrap_or_else(PoisonError::into_inner) = Some(args);
}

pub(crate) fn args() -> Vec<String> {
    ARGS.read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .unwrap_or_else(|| {
            std::env::args_os()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect()
        })
}

/// `message` (lossily decoded) and a newline to stderr, through `ignite-io`.
fn stderr_line(message: &[u8]) {
    ignite_io::log(&String::from_utf8_lossy(message));
}

/// `panic: <message>` to stderr, terminal restored, exit status 101.
pub(crate) fn die(message: &str) -> ! {
    ignite_io::log(&format!("panic: {message}"));
    ignite_io::exit(PANIC_EXIT_CODE)
}

/// `void init(i32 argc, ptr argv)`: the first call in every `main`. Stores
/// the arguments, ignores `SIGPIPE` (a write to a closed socket or pipe is
/// an error, as in a Rust program), and installs, once per process, a panic
/// hook (exit status 101, terminal restored) and an `atexit` terminal
/// restore.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_init(argc: i32, argv: *const *const c_char) {
    store_args(decode_args(argc, argv));
    HOOKS.call_once(|| {
        // SAFETY: ignoring a signal installs no handler code.
        let _ = unsafe { signal(Signal::SIGPIPE, SigHandler::SigIgn) };
        std::panic::set_hook(Box::new(|info| {
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|message| (*message).to_owned())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| String::from("Box<dyn Any>"));
            let report = match info.location() {
                Some(location) => format!(
                    "error: panic at {}:{}:{}: {message}",
                    location.file(),
                    location.line(),
                    location.column()
                ),
                None => format!("error: panic: {message}"),
            };
            ignite_io::log(&report);
            ignite_io::exit(PANIC_EXIT_CODE)
        }));
        // SAFETY: `restore_at_exit` is a plain function that never unwinds.
        unsafe { atexit(restore_at_exit) };
    });
}

/// `i32 args_port(i64* out_port, i32* present, IoError* err)`: `--port N`
/// or `--port=N` from the arguments `init` stored. Absent: `present` 0.
/// A missing value, a non-integer or a port outside `0..=65535` is an error
/// with exit code 2.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_args_port(
    out_port: *mut i64,
    present: *mut i32,
    err: *mut IgniteIoError,
) -> i32 {
    let result = ignite_io::args::port_from_args(args().into_iter());
    if let Ok(port) = result {
        put(present, i32::from(port.is_some()));
    }
    status(result.map(|port| port.unwrap_or(0)), out_port, err)
}

/// `void exit(i32 code)`: `ignite_io::exit`: restore the terminal, flush
/// standard output, exit.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_exit(code: i32) -> ! {
    ignite_io::exit(code)
}

/// `void fail(IoError* error)`: `error: <message>` to stderr, restore the
/// terminal, exit with the error's exit code.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_fail(error: *const IgniteIoError) -> ! {
    let (message, code) = match get(error) {
        // SAFETY: the argument is valid for the call (crate docs).
        Some(error) => (unsafe { error.message.as_bytes() }, error.exit_code),
        None => (&b""[..], 1),
    };
    let mut line = b"error: ".to_vec();
    line.extend_from_slice(message);
    stderr_line(&line);
    ignite_io::exit(code)
}

/// `void panic(Text* message)`: `panic: <message>` to stderr, terminal
/// restored, exit status 101.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_panic(message: *const IgniteList) -> ! {
    die(&String::from_utf8_lossy(bytes(message)))
}

/// `void log(Text* line)`: the line and a newline to stderr.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_log(line: *const IgniteList) {
    stderr_line(bytes(line));
}

/// `void io_error_new(IoError* out, Text* message)`: exit code 1. The
/// message is taken over as passed (the caller's fresh copy).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_io_error_new(out: *mut IgniteIoError, message: *const IgniteList) {
    let message = get(message).unwrap_or_else(|| IgniteList::from_bytes(&[]));
    put(
        out,
        IgniteIoError {
            message,
            exit_code: IoError::new("").exit_code(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(args: &[&str]) -> Result<Option<i64>, (String, i32)> {
        store_args(
            std::iter::once("prog")
                .chain(args.iter().copied())
                .map(String::from)
                .collect(),
        );
        let (mut out, mut present) = (-7i64, -7i32);
        let mut err = IgniteIoError {
            message: IgniteList::from_bytes(b"untouched"),
            exit_code: -1,
        };
        let code = ignite_rt_args_port(&mut out, &mut present, &mut err);
        if code == 0 {
            Ok((present == 1).then_some(out))
        } else {
            assert_eq!(code, 1);
            // SAFETY: written by the runtime in this task's arena.
            let message = String::from_utf8(unsafe { err.message.as_bytes() }.to_vec()).unwrap();
            Err((message, err.exit_code))
        }
    }

    #[test]
    fn args_port() {
        assert_eq!(port(&[]), Ok(None));
        assert_eq!(port(&["--verbose"]), Ok(None));
        assert_eq!(port(&["--port", "0"]), Ok(Some(0)));
        assert_eq!(port(&["--port", "2323"]), Ok(Some(2323)));
        assert_eq!(port(&["--port=65535"]), Ok(Some(65535)));
        for bad in [
            &["--port", "abc"][..],
            &["--port=abc"],
            &["--port", "65536"],
            &["--port", "-1"],
            &["--port"],
        ] {
            let (message, code) = port(bad).unwrap_err();
            assert_eq!(code, 2, "{bad:?}");
            assert!(message.contains("--port"), "{message}");
        }
    }

    #[test]
    fn decode_args_skips_nulls() {
        let first = c"terminald";
        let second = c"--port";
        let argv = [first.as_ptr(), std::ptr::null(), second.as_ptr()];
        assert_eq!(decode_args(3, argv.as_ptr()), ["terminald", "--port"]);
        assert!(decode_args(2, std::ptr::null()).is_empty());
    }

    #[test]
    fn io_error_new_has_exit_code_one() {
        let mut out = IgniteIoError {
            message: IgniteList::from_bytes(b""),
            exit_code: 0,
        };
        let message = IgniteList::from_bytes(b"boom");
        ignite_rt_io_error_new(&mut out, &message);
        assert_eq!(out.exit_code, 1);
        // SAFETY: arena memory of this task.
        assert_eq!(unsafe { out.message.as_bytes() }, b"boom");
    }
}
