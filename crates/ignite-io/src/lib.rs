//! Ignite IO substrate (`docs/slice.md` section 7): the one implementation of
//! TCP, in-memory pipes, standard input and output, terminal mode, process arguments, tasks
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
pub use stdio::{Stderr, Stdin, Stdout};
pub use store::Store;
pub use task::{LINGER, released_memory, spawn, try_spawn};

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

/// The reach of a `TcpListener` (`docs/slice.md` 14.14 item 11): the
/// builtin enum `Exposure`, a value in a compiled program as any other
/// enum; the order of the variants is their declaration order, as in the
/// LLVM target and the evaluator, so it can be a `Map` key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Exposure {
    Loopback,
    Private,
    Public,
}

/// The running process.
#[derive(Clone, Copy, Debug)]
pub struct Process;

/// Largest buffer a single read allocates.
const MAX_READ: usize = 1024 * 1024;

/// Largest read that goes through the thread's kept read buffer.
const READ_KEEP: usize = 64 * 1024;

thread_local! {
    /// The thread's read buffer, zeroed as it grows and reused by every
    /// read of up to [`READ_KEEP`] bytes.
    static READ_SCRATCH: std::cell::Cell<Vec<u8>> = const { std::cell::Cell::new(Vec::new()) };
}

/// One read by `read_into` of up to `maximum` bytes (clamped to
/// `1..=1 MiB`): `Input::Data` of exactly the bytes read, or `Input::Eof`
/// for zero.
///
/// A read of up to [`READ_KEEP`] bytes goes into the thread's kept buffer
/// and the bytes are copied out, so a short read does not allocate and
/// zero a buffer of `maximum` bytes each time (`vec![0; 65536]` was a
/// `memset` of 64 KiB per read, 16 % of a key-value server's user time
/// at one client). The buffer is taken out of the thread-local while
/// `read_into` runs, so a nested read uses a fresh one. A larger read
/// allocates its own buffer, as the allocator hands out large zeroed
/// blocks as fresh pages without writing them.
fn read_input(
    maximum: i64,
    read_into: impl FnOnce(&mut [u8]) -> Result<usize, IoError>,
) -> Result<Input, IoError> {
    let size = usize::try_from(maximum.max(1))
        .unwrap_or(MAX_READ)
        .min(MAX_READ);
    if size > READ_KEEP {
        let mut buffer = vec![0u8; size];
        let count = read_into(&mut buffer)?;
        if count == 0 {
            return Ok(Input::Eof);
        }
        buffer.truncate(count);
        return Ok(Input::Data(buffer));
    }
    let mut buffer = READ_SCRATCH
        .try_with(std::cell::Cell::take)
        .unwrap_or_default();
    if buffer.len() < size {
        buffer.resize(size, 0);
    }
    let read = read_into(&mut buffer[..size]).map(|count| match count {
        0 => Input::Eof,
        count => Input::Data(buffer[..count].to_vec()),
    });
    let _ = READ_SCRATCH.try_with(|scratch| scratch.set(buffer));
    read
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
    run_exit_hooks();
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
///
/// `run` arms the drain on SIGTERM first ([`drain`]). When `main` returns
/// while the process drains, the result is not the exit status: an error
/// other than [`DRAINING`] (the refused `accept` that ends a server's
/// loop) is logged, then `run` waits for the live tasks to end
/// ([`wait_for_tasks`], no bound) and returns 0.
pub fn run<F>(main: F) -> i32
where
    F: FnOnce() -> Result<(), IoError>,
{
    drain();
    let restore = terminal::Restore;
    let result = report::catch(main);
    flush();
    drop(restore);
    if draining() {
        match &result {
            Ok(Ok(())) => {}
            Ok(Err(e)) if e.to_string() == DRAINING => {}
            Ok(Err(e)) => log(&format!("error: {e}")),
            Err(report) => log(&format!("error: {report}")),
        }
        wait_for_tasks();
        run_exit_hooks();
        flush();
        return 0;
    }
    run_exit_hooks();
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
    //! The process arguments the substrate reads (`docs/slice.md` 16.9,
    //! 21.6): `--port`, `--ref` and the flags of `Setting` and `Directory`
    //! resources. Each read scans the arguments as the OS gave them, as
    //! bytes: an argument that is not UTF-8 is never decoded lossily. It is
    //! ignored when the read does not ask for it, like any argument the
    //! read does not know, and refused when it is the value the read asks
    //! for (`<flag>: value <bytes> is not UTF-8`, the bytes as
    //! `escape_ascii` writes them, exit code 2). The `String` forms take
    //! UTF-8 arguments and share the scan.

    use std::ffi::OsString;
    use std::os::unix::ffi::OsStrExt;

    use super::IoError;

    /// Exit status of a malformed or out-of-range `--port`.
    const PORT_EXIT_CODE: i32 = 2;

    fn os(args: impl Iterator<Item = String>) -> impl Iterator<Item = OsString> {
        args.map(OsString::from)
    }

    /// `value`, the value of `flag`, as text; not UTF-8 is the error
    /// `<flag>: value <bytes> is not UTF-8` with exit code 2.
    fn text(flag: &str, value: &[u8]) -> Result<String, IoError> {
        String::from_utf8(value.to_vec()).map_err(|_| {
            IoError::new(format!(
                "{flag}: value {} is not UTF-8",
                value.escape_ascii()
            ))
            .with_exit_code(PORT_EXIT_CODE)
        })
    }

    /// The value of `arg` when it is `<flag>=V`.
    fn joined<'a>(arg: &'a [u8], flag: &str) -> Option<&'a [u8]> {
        arg.strip_prefix(flag.as_bytes())?.strip_prefix(b"=")
    }

    /// `--port N` from the process arguments, if present and valid.
    pub fn port_override() -> Option<i64> {
        port_from_os_args(std::env::args_os()).ok().flatten()
    }

    /// `--port N` from the process arguments: `Ok(None)` when absent, an
    /// error with exit code 2 when malformed or outside `0..=65535`.
    pub fn port() -> Result<Option<i64>, IoError> {
        port_from_os_args(std::env::args_os())
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
        port_from_os_args(os(args))
    }

    /// [`port_from_args`] over arguments as the OS gave them (module doc).
    pub fn port_from_os_args(args: impl Iterator<Item = OsString>) -> Result<Option<i64>, IoError> {
        let mut args = args;
        while let Some(arg) = args.next() {
            let arg = arg.as_bytes();
            if arg == b"--port" {
                return match args.next() {
                    Some(value) => parse_port(&text("--port", value.as_bytes())?).map(Some),
                    None => {
                        Err(IoError::new("--port needs a value").with_exit_code(PORT_EXIT_CODE))
                    }
                };
            }
            if let Some(value) = joined(arg, "--port") {
                return parse_port(&text("--port", value)?).map(Some);
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

    /// The `--ref <path>=<address>:<port>` (or `--ref=<path>=…`) binding
    /// for `reference` in `args` (`docs/slice.md` 16.9): its address and
    /// port. No binding with that path is the error `no binding for
    /// <path>` (exit code 1); a `--ref` without a value or a binding that
    /// is not `path=address:port` with a port in `0..=65535` is an error
    /// with exit code 2, like a malformed `--port`. Bindings are read in
    /// order and the first with the path wins.
    pub fn endpoint_from_args(
        args: impl Iterator<Item = String>,
        reference: &str,
    ) -> Result<(String, i64), IoError> {
        endpoint_from_os_args(os(args), reference)
    }

    /// [`endpoint_from_args`] over arguments as the OS gave them (module
    /// doc): a binding that is not UTF-8 is refused, since it is read.
    pub fn endpoint_from_os_args(
        args: impl Iterator<Item = OsString>,
        reference: &str,
    ) -> Result<(String, i64), IoError> {
        let mut args = args;
        while let Some(arg) = args.next() {
            let binding = if arg.as_bytes() == b"--ref" {
                match args.next() {
                    Some(value) => text("--ref", value.as_bytes())?,
                    None => {
                        return Err(
                            IoError::new("--ref needs a value").with_exit_code(PORT_EXIT_CODE)
                        );
                    }
                }
            } else if let Some(value) = joined(arg.as_bytes(), "--ref") {
                text("--ref", value)?
            } else {
                continue;
            };
            let (path, endpoint) = parse_binding(&binding)?;
            if path == reference {
                return Ok(endpoint);
            }
        }
        Err(IoError::new(format!("no binding for {reference}")))
    }

    /// The value of the first `--flag V` or `--flag=V` in `args`
    /// (`docs/slice.md` 21.6): `Ok(None)` when absent; a flag without a
    /// value, or one followed by an argument that starts with `--`, is the
    /// error `<flag> needs a value` with exit code 2. `--flag=V` takes `V`
    /// as it is, also when it is empty or starts with `--`.
    pub fn flag_from_args(
        args: impl Iterator<Item = String>,
        flag: &str,
    ) -> Result<Option<String>, IoError> {
        flag_from_os_args(os(args), flag)
    }

    /// [`flag_from_args`] over arguments as the OS gave them (module doc).
    pub fn flag_from_os_args(
        args: impl Iterator<Item = OsString>,
        flag: &str,
    ) -> Result<Option<String>, IoError> {
        let needs_value =
            || IoError::new(format!("{flag} needs a value")).with_exit_code(PORT_EXIT_CODE);
        let mut args = args;
        while let Some(arg) = args.next() {
            let arg = arg.as_bytes();
            if arg == flag.as_bytes() {
                return match args.next() {
                    Some(value) if !value.as_bytes().starts_with(b"--") => {
                        text(flag, value.as_bytes()).map(Some)
                    }
                    _ => Err(needs_value()),
                };
            }
            if let Some(value) = joined(arg, flag) {
                return text(flag, value).map(Some);
            }
        }
        Ok(None)
    }

    /// [`flag_from_args`] over the process arguments.
    pub fn flag(flag: &str) -> Result<Option<String>, IoError> {
        flag_from_os_args(std::env::args_os(), flag)
    }

    /// One command-line argument of the executable (`docs/slice.md` 21.6):
    /// the capability `Setting`, a resource the generated `main` reads once
    /// at launch. A cheap clone.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Setting {
        flag: std::sync::Arc<str>,
        value: std::sync::Arc<str>,
    }

    impl Setting {
        /// `Setting.of(flag, value)`, test-only.
        pub fn of(flag: impl AsRef<str>, value: impl AsRef<str>) -> Setting {
            Setting {
                flag: flag.as_ref().into(),
                value: value.as_ref().into(),
            }
        }

        /// The resource `Setting { flag, default }` at launch: the flag's
        /// value from the process arguments, else `default`; neither is the
        /// error `<flag>: required argument missing` with exit code 2.
        pub fn launch(flag: &str, default: Option<&str>) -> Result<Setting, IoError> {
            match (self::flag(flag)?, default) {
                (Some(value), _) => Ok(Setting::of(flag, value)),
                (None, Some(value)) => Ok(Setting::of(flag, value)),
                (None, None) => Err(IoError::new(format!("{flag}: required argument missing"))
                    .with_exit_code(PORT_EXIT_CODE)),
            }
        }

        /// The flag this setting reads.
        pub fn flag(&self) -> String {
            self.flag.to_string()
        }

        /// `setting.value()`: the value as text.
        pub fn value(&self) -> String {
            self.value.to_string()
        }
    }

    /// The whole command line of the executable (`docs/slice.md` 21.19):
    /// the capability `CommandLine`, every argument after the executable's
    /// name in order, read once at launch by the generated `main`. A cheap
    /// clone.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CommandLine {
        arguments: std::sync::Arc<[String]>,
    }

    impl CommandLine {
        /// `CommandLine.of(arguments)`, test-only.
        pub fn of(arguments: Vec<String>) -> CommandLine {
            CommandLine {
                arguments: arguments.into(),
            }
        }

        /// The resource `CommandLine` at launch: the process arguments
        /// after the executable's name.
        pub fn launch() -> Result<CommandLine, IoError> {
            CommandLine::from_os_args(std::env::args_os().skip(1))
        }

        /// `args`, the arguments after the executable's name as the OS
        /// gave them, as a command line. Every argument is read, so one
        /// that is not UTF-8 is the error `argument <n>: value <bytes> is
        /// not UTF-8` with exit code 2, `<n>` its position from 1.
        pub fn from_os_args(args: impl Iterator<Item = OsString>) -> Result<CommandLine, IoError> {
            let arguments = args
                .enumerate()
                .map(|(index, arg)| text(&format!("argument {}", index + 1), arg.as_bytes()))
                .collect::<Result<Vec<String>, IoError>>()?;
            Ok(CommandLine::of(arguments))
        }

        /// `command_line.arguments()`: the arguments, in order.
        pub fn arguments(&self) -> Vec<String> {
            self.arguments.to_vec()
        }
    }

    /// `path=address:port` split into the path and the endpoint.
    fn parse_binding(binding: &str) -> Result<(&str, (String, i64)), IoError> {
        let malformed = || {
            IoError::new(format!("--ref {binding}: expected path=address:port"))
                .with_exit_code(PORT_EXIT_CODE)
        };
        let (path, target) = binding.split_once('=').ok_or_else(malformed)?;
        let (address, port) = target.rsplit_once(':').ok_or_else(malformed)?;
        let port = port
            .parse::<i64>()
            .ok()
            .filter(|port| (0..=65535).contains(port))
            .ok_or_else(malformed)?;
        if path.is_empty() || address.is_empty() {
            return Err(malformed());
        }
        Ok((path, (address.to_string(), port)))
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

        fn endpoint(args: &[&str], reference: &str) -> Result<(String, i64), (String, i32)> {
            super::endpoint_from_args(
                std::iter::once("prog")
                    .chain(args.iter().copied())
                    .map(String::from),
                reference,
            )
            .map_err(|e| (e.message.clone(), e.exit_code()))
        }

        #[test]
        fn ref_binding_is_found_by_path() {
            assert_eq!(
                endpoint(
                    &["--ref", "Terminal.server=127.0.0.1:2323"],
                    "Terminal.server"
                ),
                Ok(("127.0.0.1".to_string(), 2323))
            );
            assert_eq!(
                endpoint(
                    &[
                        "--port",
                        "0",
                        "--ref",
                        "Other.member=10.0.0.1:1",
                        "--ref=Terminal.server=::1:65535",
                    ],
                    "Terminal.server"
                ),
                Ok(("::1".to_string(), 65535))
            );
        }

        #[test]
        fn missing_ref_binding_is_an_error() {
            assert_eq!(
                endpoint(&[], "Terminal.server"),
                Err(("no binding for Terminal.server".to_string(), 1))
            );
            assert_eq!(
                endpoint(&["--ref", "Other.member=10.0.0.1:1"], "Terminal.server"),
                Err(("no binding for Terminal.server".to_string(), 1))
            );
        }

        #[test]
        fn malformed_ref_binding_has_exit_code_two() {
            for bad in [
                &["--ref"][..],
                &["--ref", "Terminal.server"],
                &["--ref", "Terminal.server=127.0.0.1"],
                &["--ref", "Terminal.server=127.0.0.1:x"],
                &["--ref", "Terminal.server=127.0.0.1:65536"],
                &["--ref", "=127.0.0.1:1"],
                &["--ref", "Terminal.server=:1"],
            ] {
                let (message, code) = endpoint(bad, "Terminal.server").unwrap_err();
                assert_eq!(code, 2, "{bad:?}");
                assert!(message.starts_with("--ref"), "{message}");
            }
        }

        fn os(args: &[&[u8]]) -> Vec<std::ffi::OsString> {
            use std::os::unix::ffi::OsStrExt;
            std::iter::once(&b"prog"[..])
                .chain(args.iter().copied())
                .map(|arg| std::ffi::OsStr::from_bytes(arg).to_os_string())
                .collect()
        }

        fn flag(args: &[&[u8]], flag: &str) -> Result<Option<String>, (String, i32)> {
            super::flag_from_os_args(os(args).into_iter(), flag)
                .map_err(|e| (e.message.clone(), e.exit_code()))
        }

        /// 21.6: the first `--flag V` or `--flag=V` wins; arguments it does
        /// not know are ignored, so `--port` and `--ref` share the list.
        #[test]
        fn flag_reads_the_first_value() {
            let list: &[&[u8]] = &[
                b"--port",
                b"0",
                b"--tool",
                b"/bin/true",
                b"--ref",
                b"A.b=127.0.0.1:9",
                b"--tool=/bin/false",
                b"--mode=slow",
            ];
            assert_eq!(flag(list, "--tool"), Ok(Some("/bin/true".to_string())));
            assert_eq!(flag(list, "--mode"), Ok(Some("slow".to_string())));
            assert_eq!(flag(list, "--root"), Ok(None));
            assert_eq!(flag(&[], "--tool"), Ok(None));
            assert_eq!(flag(&[b"--toolbox", b"x"], "--tool"), Ok(None));
            assert_eq!(flag(&[b"--tool="], "--tool"), Ok(Some(String::new())));
            assert_eq!(
                flag(&[b"--tool=--x"], "--tool"),
                Ok(Some("--x".to_string()))
            );
            assert_eq!(
                super::flag_from_args(
                    ["prog", "--root", "/srv"].into_iter().map(String::from),
                    "--root"
                ),
                Ok(Some("/srv".to_string()))
            );
            assert_eq!(super::port_from_os_args(os(list).into_iter()), Ok(Some(0)));
        }

        /// 21.6: a flag at the end, or followed by an argument that starts
        /// with `--`, has no value.
        #[test]
        fn flag_without_a_value_has_exit_code_two() {
            let needs = Err(("--mode needs a value".to_string(), 2));
            assert_eq!(flag(&[b"--mode"], "--mode"), needs);
            assert_eq!(flag(&[b"--mode", b"--tool", b"/x"], "--mode"), needs);
            assert_eq!(
                flag(&[b"--mode", b"--tool", b"/x"], "--tool"),
                Ok(Some("/x".to_string()))
            );
        }

        /// 21.6: a value that is not UTF-8 is refused with its bytes as
        /// `escape_ascii` writes them; an argument that is not UTF-8 and
        /// that the read does not ask for is ignored, by every read.
        #[test]
        fn a_value_that_is_not_utf8_is_refused() {
            let refused = Err(("--tool: value a\\xffb is not UTF-8".to_string(), 2));
            assert_eq!(flag(&[b"--tool", b"a\xffb"], "--tool"), refused);
            assert_eq!(flag(&[b"--tool=a\xffb"], "--tool"), refused);
            let stray: &[&[u8]] = &[
                b"\xfe",
                b"--other=\xfe",
                b"--tool",
                b"x",
                b"--port",
                b"7",
                b"--ref",
                b"A.b=h:1",
            ];
            assert_eq!(flag(stray, "--tool"), Ok(Some("x".to_string())));
            assert_eq!(super::port_from_os_args(os(stray).into_iter()), Ok(Some(7)));
            assert_eq!(
                super::endpoint_from_os_args(os(stray).into_iter(), "A.b"),
                Ok(("h".to_string(), 1))
            );
            let port = super::port_from_os_args(os(&[b"--port", b"\xff"]).into_iter())
                .map_err(|e| (e.message.clone(), e.exit_code()));
            assert_eq!(
                port,
                Err(("--port: value \\xff is not UTF-8".to_string(), 2))
            );
            let reference =
                super::endpoint_from_os_args(os(&[b"--ref=A.b=\xff:1"]).into_iter(), "A.b")
                    .map_err(|e| (e.message.clone(), e.exit_code()));
            assert_eq!(
                reference,
                Err(("--ref: value A.b=\\xff:1 is not UTF-8".to_string(), 2))
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `docs/slice.md` 21.8: hooks run in registration order, each once,
    /// whichever exit path runs them first.
    #[test]
    fn exit_hooks_run_once_in_order() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static RAN: AtomicUsize = AtomicUsize::new(0);
        static FIRST: AtomicUsize = AtomicUsize::new(0);
        static SECOND: AtomicUsize = AtomicUsize::new(0);
        fn first() {
            FIRST.store(RAN.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
        }
        fn second() {
            SECOND.store(RAN.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
        }
        at_exit(first);
        at_exit(second);
        run_exit_hooks();
        run_exit_hooks();
        assert_eq!(RAN.load(Ordering::SeqCst), 2);
        assert_eq!(FIRST.load(Ordering::SeqCst), 1);
        assert_eq!(SECOND.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn setting_of_keeps_its_flag_and_value() {
        let setting = Setting::of("--tool", "/bin/sh");
        assert_eq!(setting.flag(), "--tool");
        assert_eq!(setting.value(), "/bin/sh");
        assert_eq!(setting.clone(), setting);
    }

    /// 21.19: every argument in order, the empty one too; one that is not
    /// UTF-8 is refused with its position.
    #[test]
    fn command_line_reads_every_argument() {
        use std::ffi::{OsStr, OsString};
        use std::os::unix::ffi::OsStrExt;
        let given = ["add", "buy milk", "", "--port", "0"];
        let read = CommandLine::from_os_args(given.iter().map(OsString::from)).unwrap();
        assert_eq!(read.arguments(), given);
        assert_eq!(read, CommandLine::of(given.map(String::from).to_vec()));
        assert!(CommandLine::of(Vec::new()).arguments().is_empty());
        let bad = [
            OsString::from("add"),
            OsStr::from_bytes(b"a\xffb").to_os_string(),
        ];
        let error = CommandLine::from_os_args(bad.into_iter()).unwrap_err();
        assert_eq!(error.to_string(), "argument 2: value a\\xffb is not UTF-8");
        assert_eq!(error.exit_code(), 2);
    }

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

    /// `read_input`: the data is exactly the bytes read, whatever the
    /// maximum; zero bytes is `Eof`; an error passes through; `maximum` is
    /// clamped; a nested read gets its own buffer; the thread keeps a
    /// buffer of `READ_KEEP` bytes, and a larger read does not grow it.
    #[test]
    fn a_read_returns_exactly_the_bytes_read() {
        let read = read_input(65536, |buffer| {
            assert_eq!(buffer.len(), 65536);
            buffer[..3].copy_from_slice(b"abc");
            Ok(3)
        });
        let Ok(Input::Data(data)) = read else {
            panic!("{read:?}")
        };
        assert_eq!(data, b"abc");
        assert!(data.capacity() < 65536, "{}", data.capacity());
        assert_eq!(read_input(16, |_| Ok(0)), Ok(Input::Eof));
        assert_eq!(
            read_input(16, |_| Err(IoError::new("reset"))),
            Err(IoError::new("reset"))
        );
        let one = |buffer: &mut [u8]| {
            buffer[0] = b'q';
            Ok(1)
        };
        let tiny = read_input(0, |buffer| {
            assert_eq!(buffer.len(), 1);
            one(buffer)
        });
        assert_eq!(tiny, Ok(Input::Data(b"q".to_vec())));
        let kept = || {
            READ_SCRATCH.with(|scratch| {
                let buffer = scratch.take();
                let len = buffer.len();
                scratch.set(buffer);
                len
            })
        };
        assert_eq!(kept(), 65536, "the 64 KiB buffer is kept");
        let huge = read_input(i64::MAX, |buffer| {
            assert_eq!(buffer.len(), MAX_READ);
            one(buffer)
        });
        assert_eq!(huge, Ok(Input::Data(b"q".to_vec())));
        assert_eq!(kept(), 65536, "a 1 MiB read does not grow the kept buffer");
        let empty = read_input(65537, |_| Ok(0));
        assert_eq!(empty, Ok(Input::Eof));
        let outer = read_input(8, |buffer| {
            buffer[..2].copy_from_slice(b"xy");
            let inner = read_input(8, |inner| {
                inner[..1].copy_from_slice(b"z");
                Ok(1)
            });
            assert_eq!(inner, Ok(Input::Data(b"z".to_vec())));
            Ok(2)
        });
        assert_eq!(outer, Ok(Input::Data(b"xy".to_vec())));
    }

    /// A read that panics while it holds the kept buffer leaves the
    /// thread's next read working, and the bytes returned are a copy: a
    /// later read into the buffer does not change them.
    #[test]
    fn a_read_after_a_panicking_read_works_and_data_is_a_copy() {
        let panicked = std::panic::catch_unwind(|| read_input(8, |_| panic!("inside the read")));
        assert!(panicked.is_err());
        let first = read_input(8, |buffer| {
            buffer[..2].copy_from_slice(b"ok");
            Ok(2)
        });
        assert_eq!(first, Ok(Input::Data(b"ok".to_vec())));
        let second = read_input(8, |buffer| {
            buffer[..2].copy_from_slice(b"no");
            Ok(2)
        });
        assert_eq!(second, Ok(Input::Data(b"no".to_vec())));
        assert_eq!(first, Ok(Input::Data(b"ok".to_vec())));
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

// Slice 12 (`docs/slice.md` section 16.13): the monotonic clock, `Endpoint.of`
// and the drain on SIGTERM, after the tests for the same reason. Owner: P2.
pub mod clock;
mod drain;
pub use drain::{DRAINING, draining};
pub use net::endpoint_of;
pub use task::{live_tasks, wait_for_tasks};

/// Arm the drain on SIGTERM (`docs/slice.md` 16.13, module `drain`): block
/// SIGTERM in the calling thread, so every thread started afterwards
/// inherits the block, and start the signal thread that takes it. The
/// first SIGTERM shuts the read side of every listener, closes it and
/// prints `released <address>:<port>` per listener; pending and later
/// `accept`s fail with [`DRAINING`]; once `main` returns, [`run`] waits
/// for the live tasks and returns 0. A second SIGTERM ends the process at
/// once. Idempotent; [`run`] and the LLVM runtime's `init` call it before
/// any task thread exists. Outside `ignite run` the wait for live tasks
/// has no bound (A6).
pub fn drain() {
    drain::install();
}

// Slice 14 (`docs/slice.md` 18.8): the in-memory stream pair and the
// stream interface both targets dispatch on, after the tests for the same
// reason. Owner: U2.
mod pipe;
mod stream;
pub use pipe::{PIPE_CAPACITY, Pipe, PipeStream};
pub use stream::ByteStream;

// Slice 17 (`docs/slice.md` 21.8): the capabilities `Directory`,
// `Launcher` with `ChildProcess`, `Entropy` and `Setting`, and the exit
// hooks, after the tests for the same reason. Owner of the surface: U1;
// `fs` U2, `child` U3, `entropy` U5, `args::flag_from_args` U4.
pub mod child;
pub mod entropy;
pub mod fs;
pub use args::{CommandLine, Setting};
pub use child::{ChildExit, ChildProcess, Invocation, Launcher};
pub use entropy::Entropy;
pub use fs::Directory;
// `docs/slice.md` 21.20: the capability `Environment`.
pub mod environment;
pub use environment::Environment;

static EXIT_HOOKS: std::sync::Mutex<Vec<fn()>> = std::sync::Mutex::new(Vec::new());

/// Register `hook` to run once before the process ends in order: [`exit`],
/// the end of [`run`], the end of a drain and a second SIGTERM during the
/// drain call every registered hook, in registration order, the first time
/// one of them runs (`docs/slice.md` 21.5: the live children's groups get
/// SIGKILL).
pub fn at_exit(hook: fn()) {
    EXIT_HOOKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(hook);
}

/// Run and forget every hook [`at_exit`] registered; a later call runs
/// only hooks registered since.
pub fn run_exit_hooks() {
    let hooks = std::mem::take(
        &mut *EXIT_HOOKS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    for hook in hooks {
        hook();
    }
}

/// `Int.parse(text)` of both targets: the value of decimal digits with an
/// optional leading `-`; `None` for anything else (a `+`, spaces, no
/// digits, overflow, bytes that are not UTF-8). Takes the bytes of a
/// `Text` or of `Bytes`, so `Int.parse(bytes.to_text())` needs no `Text`:
/// a lossy decode of bytes that are not UTF-8 holds U+FFFD, which never
/// parses either. Reads the bytes directly, with no UTF-8 pass: every byte
/// that is not an ASCII digit (or the leading `-`) rejects, and every byte
/// of a multi-byte sequence is one. Accumulates towards the sign, so
/// `i64::MIN` parses.
pub fn parse_int(text: impl AsRef<[u8]>) -> Option<i64> {
    let text = text.as_ref();
    let (negative, digits) = match text.split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, text),
    };
    if digits.is_empty() {
        return None;
    }
    let mut value: i64 = 0;
    for &byte in digits {
        let digit = byte.wrapping_sub(b'0');
        if digit > 9 {
            return None;
        }
        value = value.checked_mul(10)?;
        value = if negative {
            value.checked_sub(i64::from(digit))?
        } else {
            value.checked_add(i64::from(digit))?
        };
    }
    Some(value)
}

#[cfg(test)]
mod parse_int_tests {
    use super::parse_int;

    #[test]
    fn parse_int_takes_decimal_digits_and_a_leading_minus_only() {
        assert_eq!(parse_int("42"), Some(42));
        assert_eq!(parse_int(b"-7".as_slice()), Some(-7));
        assert_eq!(parse_int("-9223372036854775808"), Some(i64::MIN));
        for bad in ["+1", "", "-", " 1", "1 ", "1x", "9223372036854775808"] {
            assert_eq!(parse_int(bad), None, "{bad:?}");
        }
        assert_eq!(parse_int(vec![b'1', 0xff]), None);
        assert_eq!(
            parse_int(String::from_utf8_lossy(&[b'1', 0xff]).as_bytes()),
            None
        );
    }

    #[test]
    fn parse_int_matches_i64_from_str_without_plus() {
        for text in [
            "0",
            "-0",
            "7",
            "-7",
            "007",
            "-007",
            "00000000000000000000000000042",
            "42",
            "-42",
            "9223372036854775807",
            "-9223372036854775808",
            "9223372036854775808",
            "-9223372036854775809",
            "99999999999999999999",
            "",
            "-",
            "+",
            "+1",
            "+-1",
            "1-",
            "--1",
            " 1",
            "1 ",
            "1a",
            "a1",
            "1\0",
            "\u{0}1",
            "1_000",
            "0x10",
            "\u{663}",
            "\u{ff11}",
            "1\u{fffd}",
        ] {
            let expected = if text.starts_with('+') {
                None
            } else {
                text.parse::<i64>().ok()
            };
            assert_eq!(parse_int(text), expected, "{text:?}");
        }
        for bad in [&b"1\xff"[..], b"\xff1", b"-\x80", b"\xc0\xb1"] {
            assert_eq!(parse_int(bad), None, "{bad:?}");
        }
    }
}
