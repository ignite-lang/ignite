//! Process entry and exit: `init`, arguments, `exit`, `fail`, `panic`, `log`
//! and `IoError.new`. Every way out of the process restores the terminal
//! attributes saved by `stdin_set_raw`: `exit`, `fail` and `panic` directly,
//! a return from C `main` (or any `exit(3)`) through an `atexit` handler, a
//! Rust panic inside the runtime through the panic hook. A process killed by
//! a signal cannot restore them.

use std::ffi::{CStr, OsStr, OsString, c_char, c_int};
use std::os::unix::ffi::OsStrExt;
use std::sync::{Once, PoisonError, RwLock};

use ignite_io::IoError;
use nix::sys::signal::{SigHandler, Signal, signal};

use crate::{IgniteEndpoint, IgniteIoError, IgniteList, bytes, get, put, status};

/// Exit status of a panic (`docs/llvm.md` section 4).
const PANIC_EXIT_CODE: i32 = 101;

/// The arguments stored by `init`, as the OS gave them; `None` before it
/// (then the process arguments as std sees them are used).
static ARGS: RwLock<Option<Vec<OsString>>> = RwLock::new(None);

static HOOKS: Once = Once::new();

unsafe extern "C" {
    fn atexit(callback: extern "C" fn()) -> c_int;
}

extern "C" fn restore_at_exit() {
    ignite_io::terminal::restore();
}

/// The arguments `argv[0..argc]`, their bytes as they are (`ignite_io::args`
/// decides what a byte string that is not UTF-8 means, 21.6, as on the Rust
/// target); null entries are skipped.
fn decode_args(argc: i32, argv: *const *const c_char) -> Vec<OsString> {
    if argv.is_null() {
        return Vec::new();
    }
    (0..usize::try_from(argc).unwrap_or(0))
        .filter_map(|index| {
            // SAFETY: `argv` holds `argc` entries (C `main` contract).
            let arg = unsafe { *argv.add(index) };
            // SAFETY: a non-null entry is a NUL-terminated string.
            (!arg.is_null()).then(|| {
                OsStr::from_bytes(unsafe { CStr::from_ptr(arg) }.to_bytes()).to_os_string()
            })
        })
        .collect()
}

fn store_args(args: Vec<OsString>) {
    *ARGS.write().unwrap_or_else(PoisonError::into_inner) = Some(args);
}

/// The arguments `init` stored, as the OS gave them: what `--port`,
/// `--ref` and the flags are read from.
fn os_args() -> Vec<OsString> {
    ARGS.read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .unwrap_or_else(|| std::env::args_os().collect())
}

/// The arguments of a test executable for its runner (`ignite_io::test`),
/// lossily decoded: the runner's filter and options, no program's flags.
pub(crate) fn args() -> Vec<String> {
    os_args()
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
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
    ignite_io::drain();
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
    let result = ignite_io::args::port_from_os_args(os_args().into_iter());
    if let Ok(port) = result {
        put(present, i32::from(port.is_some()));
    }
    status(result.map(|port| port.unwrap_or(0)), out_port, err)
}

/// The value of `flag` in the arguments `init` stored
/// (`ignite_io::args::flag_from_args`); `None` for an empty flag.
pub(crate) fn flag_value(flag: &str) -> Result<Option<String>, IoError> {
    if flag.is_empty() {
        return Ok(None);
    }
    ignite_io::args::flag_from_os_args(os_args().into_iter(), flag)
}

/// `i32 args_flag(Text* flag, Text* out, i32* present, IoError* err)`: the
/// value of `--flag V` or `--flag=V` from the arguments `init` stored
/// (`docs/slice.md` 21.6). Absent: `present` 0. A flag without a value is
/// an error with exit code 2.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_args_flag(
    flag: *const IgniteList,
    out: *mut IgniteList,
    present: *mut i32,
    err: *mut IgniteIoError,
) -> i32 {
    let flag = String::from_utf8_lossy(bytes(flag)).into_owned();
    let result = flag_value(&flag);
    if let Ok(value) = &result {
        put(present, i32::from(value.is_some()));
    }
    status(
        result.map(|value| IgniteList::from_bytes(value.unwrap_or_default().as_bytes())),
        out,
        err,
    )
}

/// `i32 setting_launch(Text* flag, Text* default, i8 has_default, i64* out,
/// IoError* err)`: the resource `Setting` at launch (21.6): the flag's
/// value, else `default` when `has_default`, else the error `<flag>:
/// required argument missing` with exit code 2.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_setting_launch(
    flag: *const IgniteList,
    default: *const IgniteList,
    has_default: i8,
    out: *mut i64,
    err: *mut IgniteIoError,
) -> i32 {
    let flag = String::from_utf8_lossy(bytes(flag)).into_owned();
    let default = (has_default != 0).then(|| String::from_utf8_lossy(bytes(default)).into_owned());
    let made = flag_value(&flag).and_then(|given| match given.or(default) {
        Some(value) => Ok(ignite_io::Setting::of(&flag, value)),
        None => Err(IoError::new(format!("{flag}: required argument missing")).with_exit_code(2)),
    });
    status(
        made.map(|setting| crate::net::register(crate::net::Handle::Setting(setting))),
        out,
        err,
    )
}

/// `i64 setting_of(Text* flag, Text* value)`: `Setting.of(flag, value)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_setting_of(flag: *const IgniteList, value: *const IgniteList) -> i64 {
    let setting = ignite_io::Setting::of(
        String::from_utf8_lossy(bytes(flag)),
        String::from_utf8_lossy(bytes(value)),
    );
    crate::net::register(crate::net::Handle::Setting(setting))
}

/// `void setting_value(Text* out, i64 s)`: `setting.value()`; empty for a
/// handle that is no setting.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_setting_value(out: *mut IgniteList, handle: i64) {
    let value = crate::net::capability(handle, "setting", |found| match found {
        crate::net::Handle::Setting(setting) => Some(setting.value()),
        _ => None,
    })
    .unwrap_or_default();
    put(out, IgniteList::from_bytes(value.as_bytes()));
}

/// `i32 command_line_launch(i64* out, IoError* err)`: the resource
/// `CommandLine` at launch (21.19): the arguments `init` stored after the
/// executable's name; one that is not UTF-8 is an error with exit code 2.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_command_line_launch(out: *mut i64, err: *mut IgniteIoError) -> i32 {
    let made = ignite_io::CommandLine::from_os_args(os_args().into_iter().skip(1));
    status(
        made.map(|command_line| {
            crate::net::register(crate::net::Handle::CommandLine(command_line))
        }),
        out,
        err,
    )
}

/// `i64 command_line_of(List<Text>* arguments)`: `CommandLine.of(arguments)`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_command_line_of(arguments: *const IgniteList) -> i64 {
    let command_line = ignite_io::CommandLine::of(
        crate::text::texts(arguments)
            .iter()
            .map(|argument| String::from_utf8_lossy(bytes(argument)).into_owned())
            .collect(),
    );
    crate::net::register(crate::net::Handle::CommandLine(command_line))
}

/// `void command_line_arguments(List<Text>* out, i64 c)`:
/// `command_line.arguments()`; empty for a handle that is no command line.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_command_line_arguments(out: *mut IgniteList, handle: i64) {
    let arguments = crate::net::capability(handle, "command line", |found| match found {
        crate::net::Handle::CommandLine(command_line) => Some(command_line.arguments()),
        _ => None,
    })
    .unwrap_or_default();
    put(out, IgniteList::from_texts(&arguments));
}

/// `i32 endpoint_of(Text* reference, Endpoint* out, IoError* err)`: the
/// `--ref <path>=<address>:<port>` argument `init` stored whose path is
/// `reference` (`docs/slice.md` 16.9, 16.13), as an `Endpoint` whose
/// address is allocated in the task's arena. No such binding is the error
/// `no binding for <path>` (exit code 1); a malformed `--ref` is an error
/// with exit code 2 (`ignite_io::args::endpoint_from_args`).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_endpoint_of(
    reference: *const IgniteList,
    out: *mut IgniteEndpoint,
    err: *mut IgniteIoError,
) -> i32 {
    let reference = String::from_utf8_lossy(bytes(reference)).into_owned();
    let result = ignite_io::args::endpoint_from_os_args(os_args().into_iter(), &reference);
    status(
        result.map(|(address, port)| IgniteEndpoint {
            address: IgniteList::from_bytes(address.as_bytes()),
            port,
        }),
        out,
        err,
    )
}

/// `void exit(i32 code)`: `ignite_io::exit`: restore the terminal, flush
/// standard output, exit. Called by the generated main when `K.main`
/// returns `Ok`; while the process drains (`docs/slice.md` 16.13) it
/// waits for the live tasks first and exits 0, as `ignite_io::run` does.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_exit(code: i32) -> ! {
    if ignite_io::draining() {
        drained();
    }
    ignite_io::exit(code)
}

/// `void fail(IoError* error)`: `error: <message>` to stderr, restore the
/// terminal, exit with the error's exit code. Called by the generated main
/// when `K.main` returns `Err`; while the process drains, an error other
/// than `draining` (the refused `accept`) is printed, then the live tasks
/// are waited for and the status is 0 (16.13).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_fail(error: *const IgniteIoError) -> ! {
    let (message, code) = match get(error) {
        // SAFETY: the argument is valid for the call (crate docs).
        Some(error) => (unsafe { error.message.as_bytes() }, error.exit_code),
        None => (&b""[..], 1),
    };
    if ignite_io::draining() {
        if message != ignite_io::DRAINING.as_bytes() {
            let mut line = b"error: ".to_vec();
            line.extend_from_slice(message);
            stderr_line(&line);
        }
        drained();
    }
    let mut line = b"error: ".to_vec();
    line.extend_from_slice(message);
    stderr_line(&line);
    ignite_io::exit(code)
}

/// The end of a drained process: wait for the live tasks, exit 0.
fn drained() -> ! {
    ignite_io::wait_for_tasks();
    ignite_io::exit(0)
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

    // Both ABI tests initialize the same process-wide argument slot.
    static ARGUMENTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn port(args: &[&str]) -> Result<Option<i64>, (String, i32)> {
        let _guard = ARGUMENTS.lock().unwrap();
        store_args(
            std::iter::once("prog")
                .chain(args.iter().copied())
                .map(OsString::from)
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

    fn endpoint(args: &[&str], reference: &str) -> Result<(String, i64), (String, i32)> {
        let _guard = ARGUMENTS.lock().unwrap();
        store_args(
            std::iter::once("prog")
                .chain(args.iter().copied())
                .map(OsString::from)
                .collect(),
        );
        let mut out = IgniteEndpoint {
            address: IgniteList::from_bytes(b"untouched"),
            port: -7,
        };
        let mut err = IgniteIoError {
            message: IgniteList::from_bytes(b"untouched"),
            exit_code: -1,
        };
        let text = IgniteList::from_bytes(reference.as_bytes());
        let code = ignite_rt_endpoint_of(&text, &mut out, &mut err);
        // SAFETY: both lists were written by the runtime in this task's arena.
        let read = |list: &IgniteList| String::from_utf8(unsafe { list.as_bytes() }.to_vec());
        if code == 0 {
            Ok((read(&out.address).unwrap(), out.port))
        } else {
            assert_eq!(code, 1);
            Err((read(&err.message).unwrap(), err.exit_code))
        }
    }

    #[test]
    fn endpoint_of_reads_the_ref_binding() {
        assert_eq!(
            endpoint(
                &["--port", "0", "--ref", "Terminal.server=127.0.0.1:2323"],
                "Terminal.server"
            ),
            Ok(("127.0.0.1".to_string(), 2323))
        );
        assert_eq!(
            endpoint(&[], "Terminal.server"),
            Err(("no binding for Terminal.server".to_string(), 1))
        );
        let (message, code) =
            endpoint(&["--ref", "Terminal.server=x"], "Terminal.server").unwrap_err();
        assert_eq!(code, 2);
        assert!(message.starts_with("--ref"), "{message}");
    }

    #[test]
    fn decode_args_skips_nulls() {
        let first = c"terminald";
        let second = c"--port";
        let argv = [first.as_ptr(), std::ptr::null(), second.as_ptr()];
        assert_eq!(decode_args(3, argv.as_ptr()), ["terminald", "--port"]);
        assert!(decode_args(2, std::ptr::null()).is_empty());
    }

    /// 21.6: `argv` keeps its bytes; the Rust target reads `args_os`, so
    /// both targets see the same byte strings.
    #[test]
    fn decode_args_keeps_bytes_that_are_not_utf8() {
        let bad = c"/bin/\xfftrue";
        let argv = [bad.as_ptr()];
        assert_eq!(
            decode_args(1, argv.as_ptr()),
            [OsStr::from_bytes(b"/bin/\xfftrue")]
        );
    }

    fn flag(args: &[&[u8]], flag: &str) -> Result<Option<String>, (String, i32)> {
        let _guard = ARGUMENTS.lock().unwrap();
        store_args(
            std::iter::once(&b"prog"[..])
                .chain(args.iter().copied())
                .map(|arg| OsStr::from_bytes(arg).to_os_string())
                .collect(),
        );
        let mut out = IgniteList::from_bytes(b"untouched");
        let mut present = -7i32;
        let mut err = IgniteIoError {
            message: IgniteList::from_bytes(b"untouched"),
            exit_code: -1,
        };
        let text = IgniteList::from_bytes(flag.as_bytes());
        let code = ignite_rt_args_flag(&text, &mut out, &mut present, &mut err);
        // SAFETY: both lists were written by the runtime in this task's arena.
        let read = |list: &IgniteList| String::from_utf8(unsafe { list.as_bytes() }.to_vec());
        if code == 0 {
            Ok((present == 1).then(|| read(&out).unwrap()))
        } else {
            assert_eq!(code, 1);
            Err((read(&err.message).unwrap(), err.exit_code))
        }
    }

    /// `args_flag` over the arguments `init` stored (21.6).
    #[test]
    fn args_flag_reads_the_flag() {
        let list: &[&[u8]] = &[b"--port", b"0", b"--tool", b"/bin/true", b"--mode=slow"];
        assert_eq!(flag(list, "--tool"), Ok(Some("/bin/true".to_string())));
        assert_eq!(flag(list, "--mode"), Ok(Some("slow".to_string())));
        assert_eq!(flag(list, "--root"), Ok(None));
        assert_eq!(
            flag(&[b"--mode", b"--tool", b"/x"], "--mode"),
            Err(("--mode needs a value".to_string(), 2))
        );
        assert_eq!(
            flag(&[b"--tool", b"a\xffb"], "--tool"),
            Err(("--tool: value a\\xffb is not UTF-8".to_string(), 2))
        );
        assert_eq!(
            flag(&[b"\xfe", b"--tool", b"x"], "--tool"),
            Ok(Some("x".to_string()))
        );
    }

    /// `command_line_launch` over the arguments `init` stored (21.19):
    /// every argument after the executable's name, in order.
    #[test]
    fn command_line_launch_reads_every_argument() {
        let launch = |args: &[&[u8]]| {
            let _guard = ARGUMENTS.lock().unwrap();
            store_args(
                std::iter::once(&b"todo"[..])
                    .chain(args.iter().copied())
                    .map(|arg| OsStr::from_bytes(arg).to_os_string())
                    .collect(),
            );
            let mut handle = -7i64;
            let mut err = IgniteIoError {
                message: IgniteList::from_bytes(b"untouched"),
                exit_code: -1,
            };
            let code = ignite_rt_command_line_launch(&mut handle, &mut err);
            (code, handle, err)
        };
        let (code, handle, _) = launch(&[b"add", b"buy milk", b"--port", b"0"]);
        assert_eq!(code, 0);
        let mut out = IgniteList::from_bytes(b"");
        ignite_rt_command_line_arguments(&mut out, handle);
        let read: Vec<String> = crate::text::texts(&out)
            .iter()
            // SAFETY: written by the runtime in this task's arena.
            .map(|text| String::from_utf8(unsafe { text.as_bytes() }.to_vec()).unwrap())
            .collect();
        assert_eq!(read, ["add", "buy milk", "--port", "0"]);

        let (code, _, err) = launch(&[b"add", b"a\xffb"]);
        assert_eq!(code, 1);
        assert_eq!(err.exit_code, 2);
        // SAFETY: written by the runtime in this task's arena.
        let message = String::from_utf8(unsafe { err.message.as_bytes() }.to_vec()).unwrap();
        assert_eq!(message, "argument 2: value a\\xffb is not UTF-8");
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
