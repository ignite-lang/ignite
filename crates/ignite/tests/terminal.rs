//! `examples/terminal/terminal.ignite` built with `ignite::build` against the
//! standard library in `std/`, then driven as processes: the compiled
//! `terminald` over a raw socket, the compiled `tn` against `terminald`
//! and against a scripted listener, and `tn` on a pseudo-terminal.
//! `docs/slice.md` section 9, items 4 to 9. Every case runs once per
//! target (`rust::<case>`, `llvm::<case>`; `docs/llvm.md` section 8 item 3)
//! and once more per target with `--static` (`rust_static::<case>`,
//! `llvm_static::<case>`; `docs/slice.md` 23.6).

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

use crate::common::Target;

/// The deadline of this file: the longest a test waits for a line, a
/// read, a connection or an exit, each a ceiling on a wait that returns at
/// its event. Alone `llvm::a_task_that_cannot_start_closes_only_its_connection`
/// takes 0.26 to 0.64 s whole, the cold build included (5 runs,
/// 2026-11-07); it failed loaded gates of that day under the former 5 s,
/// its server started under `systemd-run --user --scope`.
const TIMEOUT: Duration = Duration::from_secs(30);
/// The prompt as `terminald` sends it: the text, then IAC GA (RFC 854 p5).
const PROMPT: &[u8] = b"terminal> \xff\xf9";
/// The prompt as `tn` prints it: the Go Ahead is a command, not text.
const SHOWN_PROMPT: &[u8] = b"terminal> ";

struct Executables {
    terminald: PathBuf,
    tn: PathBuf,
}

/// The example, built once per build of the matrix and test binary under
/// the cargo target directory: the Rust target with cargo, the LLVM target
/// with clang and the runtime archive, each also with `--static`.
fn executables(target: Target) -> &'static Executables {
    static BUILT: [OnceLock<Executables>; 4] = [const { OnceLock::new() }; 4];
    BUILT[target.index()].get_or_init(|| {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/terminal/terminal.ignite");
        let options = target.options(&Path::new(env!("CARGO_TARGET_TMPDIR")).join("terminal"));
        let output = ignite::build(&source, &options).unwrap_or_else(|e| panic!("{e}"));
        let names: Vec<&str> = output
            .executables
            .iter()
            .map(|path| path.file_stem().and_then(|s| s.to_str()).unwrap_or(""))
            .collect();
        assert_eq!(names, ["terminald", "tn"], "{:?}", output.executables);
        for path in &output.executables {
            assert!(path.is_file(), "{} was not built", path.display());
        }
        Executables {
            terminald: output.executables[0].clone(),
            tn: output.executables[1].clone(),
        }
    })
}

fn shown(bytes: &[u8]) -> String {
    bytes.escape_ascii().to_string()
}

/// A child process, killed on drop.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A running compiled `terminald --port 0`.
struct Server {
    process: Running,
    port: u16,
    /// Every line `terminald` wrote to stderr so far.
    log: Arc<Mutex<String>>,
}

impl Server {
    fn start(target: Target) -> Server {
        Server::start_under(target, &[])
    }

    /// `terminald --port 0` run as the last arguments of `wrapper`, or
    /// directly when `wrapper` is empty.
    fn start_under(target: Target, wrapper: &[&str]) -> Server {
        Server::start_program(&executables(target).terminald, wrapper)
    }

    /// [`Server::start_under`] of the `terminald` at `terminald`.
    fn start_program(terminald: &Path, wrapper: &[&str]) -> Server {
        let mut command = match wrapper.split_first() {
            None => Command::new(terminald),
            Some((program, args)) => {
                let mut command = Command::new(program);
                command.args(args).arg(terminald);
                command
            }
        };
        let mut child = command
            .args(["--port", "0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn terminald");
        let stderr = child.stderr.take().expect("piped stderr");
        let process = Running(child);
        let (ports, port) = mpsc::channel();
        let log = Arc::new(Mutex::new(String::new()));
        let lines = Arc::clone(&log);
        std::thread::spawn(move || {
            let mut stderr = std::io::BufReader::new(stderr);
            let mut line = String::new();
            while std::io::BufRead::read_line(&mut stderr, &mut line).unwrap_or(0) > 0 {
                if let Some(rest) = line.trim_end().strip_prefix("listening on 127.0.0.1:") {
                    let _ = ports.send(rest.parse::<u16>());
                }
                lines.lock().unwrap().push_str(&line);
                line.clear();
            }
        });
        let port = port
            .recv_timeout(TIMEOUT)
            .expect("terminald printed no `listening on 127.0.0.1:<port>` line")
            .expect("port in the listening line");
        Server { process, port, log }
    }

    /// Waits until stderr has a line containing `text`.
    #[track_caller]
    fn assert_logged(&self, text: &str) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let log = self.log.lock().unwrap().clone();
            if log.contains(text) {
                return;
            }
            assert!(Instant::now() < deadline, "no `{text}` in stderr: {log}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// A raw connection with the first prompt consumed.
    fn raw(&self) -> TcpStream {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        assert_eq!(
            shown(&read_exactly(&mut stream, PROMPT.len())),
            shown(PROMPT)
        );
        stream
    }

    #[track_caller]
    fn assert_alive(&mut self) {
        assert!(
            self.process.0.try_wait().expect("try_wait").is_none(),
            "terminald exited"
        );
    }
}

fn read_exactly(stream: &mut TcpStream, count: usize) -> Vec<u8> {
    let mut buffer = vec![0u8; count];
    stream.read_exact(&mut buffer).expect("data in time");
    buffer
}

/// Everything up to and including the next prompt.
fn read_through_prompt(stream: &mut TcpStream) -> Vec<u8> {
    let mut received = Vec::new();
    while !received.ends_with(PROMPT) {
        let mut chunk = [0u8; 1024];
        let count = stream
            .read(&mut chunk)
            .unwrap_or_else(|e| panic!("no prompt ({e}); got {}", shown(&received)));
        assert!(count > 0, "closed; got {}", shown(&received));
        received.extend_from_slice(&chunk[..count]);
    }
    received
}

/// Everything until the peer closes. A reset counts as a close: a peer that
/// closes with unread input sends RST instead of FIN.
fn read_until_closed(stream: &mut TcpStream) -> Vec<u8> {
    let mut received = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => return received,
            Ok(count) => received.extend_from_slice(&chunk[..count]),
            Err(e) if e.kind() == ErrorKind::ConnectionReset => return received,
            Err(e) => panic!("not closed ({e}); got {}", shown(&received)),
        }
    }
}

fn send_line(stream: &mut TcpStream, line: &str) {
    stream.write_all(line.as_bytes()).unwrap();
    stream.write_all(b"\r\n").unwrap();
}

/// Lines sent after the first prompt and the reply to each.
const STEPS: &[(&str, &[u8])] = &[
    (
        "help",
        b"help\r\necho <text>\r\nstatus\r\nquit\r\nterminal> \xff\xf9",
    ),
    ("echo hello", b"hello\r\nterminal> \xff\xf9"),
    ("status", b"ready\r\nterminal> \xff\xf9"),
    (
        "frobnicate x",
        b"unknown command: frobnicate\r\nterminal> \xff\xf9",
    ),
    ("", b"terminal> \xff\xf9"),
    ("echo", b"\r\nterminal> \xff\xf9"),
    ("  echo  x", b" x\r\nterminal> \xff\xf9"),
];

fn transcript_on_raw_socket(target: Target) {
    let mut server = Server::start(target);
    let mut stream = server.raw();
    for (line, expected) in STEPS {
        send_line(&mut stream, line);
        assert_eq!(
            shown(&read_through_prompt(&mut stream)),
            shown(expected),
            "reply to {line:?}"
        );
    }
    send_line(&mut stream, "quit");
    assert_eq!(shown(&read_until_closed(&mut stream)), shown(b"bye\r\n"));
    server.assert_alive();
}

fn send_in_pieces(stream: &mut TcpStream, pieces: &[&[u8]]) {
    for piece in pieces {
        stream.write_all(piece).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn fragmented_input_is_reassembled(target: Target) {
    let server = Server::start(target);
    let mut stream = server.raw();
    send_in_pieces(&mut stream, &[b"ec", b"ho hi\r", b"\n"]);
    assert_eq!(
        shown(&read_through_prompt(&mut stream)),
        shown(b"hi\r\nterminal> \xff\xf9")
    );
    send_in_pieces(&mut stream, &[b"\xff", b"\xfd", b"\x01"]);
    assert_eq!(read_exactly(&mut stream, 3), [255, 252, 1]);
}

fn do_echo_is_refused_with_wont(target: Target) {
    let server = Server::start(target);
    let mut stream = server.raw();
    stream.write_all(&[255, 253, 1]).unwrap();
    assert_eq!(read_exactly(&mut stream, 3), [255, 252, 1]);
}

fn will_naws_is_refused_with_dont(target: Target) {
    let server = Server::start(target);
    let mut stream = server.raw();
    stream.write_all(&[255, 251, 31]).unwrap();
    assert_eq!(read_exactly(&mut stream, 3), [255, 254, 31]);
}

/// The line is `echo a\xffb`; the console receives it as lossy UTF-8 text,
/// so the lone byte 255 comes back as U+FFFD.
fn iac_iac_is_a_literal_byte(target: Target) {
    let server = Server::start(target);
    let mut stream = server.raw();
    stream.write_all(b"echo a\xff\xffb\r\n").unwrap();
    assert_eq!(
        shown(&read_through_prompt(&mut stream)),
        shown(&["a\u{FFFD}b\r\n".as_bytes(), PROMPT].concat())
    );
}

fn cr_nul_ends_a_line(target: Target) {
    let server = Server::start(target);
    let mut stream = server.raw();
    stream.write_all(b"echo x\r\0").unwrap();
    assert_eq!(
        shown(&read_through_prompt(&mut stream)),
        shown(b"x\r\nterminal> \xff\xf9")
    );
    send_line(&mut stream, "status");
    assert_eq!(
        shown(&read_through_prompt(&mut stream)),
        shown(b"ready\r\nterminal> \xff\xf9")
    );
}

fn are_you_there_is_answered(target: Target) {
    let server = Server::start(target);
    let mut stream = server.raw();
    stream.write_all(&[255, 246]).unwrap();
    assert_eq!(
        shown(&read_exactly(&mut stream, 9)),
        shown(b"[yes]\r\n\xff\xf9")
    );
}

fn subnegotiation_is_skipped(target: Target) {
    let server = Server::start(target);
    let mut stream = server.raw();
    stream
        .write_all(&[255, 250, 24, 0, b'x', b't', b'e', b'r', b'm', 255, 240])
        .unwrap();
    send_line(&mut stream, "status");
    assert_eq!(
        shown(&read_through_prompt(&mut stream)),
        shown(b"ready\r\nterminal> \xff\xf9")
    );
}

fn subnegotiation_of_1024_bytes_is_skipped(target: Target) {
    let server = Server::start(target);
    let mut stream = server.raw();
    let mut bytes = vec![255, 250];
    bytes.extend(std::iter::repeat_n(b'x', 1023));
    bytes.extend([255, 240]);
    stream.write_all(&bytes).unwrap();
    send_line(&mut stream, "status");
    assert_eq!(
        shown(&read_through_prompt(&mut stream)),
        shown(b"ready\r\nterminal> \xff\xf9")
    );
}

fn subnegotiation_over_1024_bytes_closes(target: Target) {
    let mut server = Server::start(target);
    let mut stream = server.raw();
    let mut bytes = vec![255, 250];
    bytes.extend(std::iter::repeat_n(b'x', 1025));
    stream.write_all(&bytes).unwrap();
    assert_eq!(shown(&read_until_closed(&mut stream)), "");
    server.assert_alive();
}

fn interrupt_process_closes(target: Target) {
    let mut server = Server::start(target);
    let mut stream = server.raw();
    stream.write_all(&[255, 244]).unwrap();
    assert_eq!(shown(&read_until_closed(&mut stream)), "");
    server.assert_alive();
}

fn overlong_line_is_refused_and_closed(target: Target) {
    let mut server = Server::start(target);
    let mut stream = server.raw();
    stream.write_all(&[b'a'; 4097]).unwrap();
    assert_eq!(
        shown(&read_until_closed(&mut stream)),
        shown(b"line too long\r\n")
    );
    server.assert_alive();
}

fn line_of_4096_bytes_is_accepted(target: Target) {
    let server = Server::start(target);
    let mut stream = server.raw();
    let mut line = b"echo ".to_vec();
    line.resize(4096, b'a');
    stream.write_all(&line).unwrap();
    stream.write_all(b"\r\n").unwrap();
    let mut expected = vec![b'a'; 4091];
    expected.extend_from_slice(b"\r\nterminal> \xff\xf9");
    assert_eq!(shown(&read_through_prompt(&mut stream)), shown(&expected));
}

/// A running compiled `tn --port <port>` with piped standard streams.
struct Client {
    process: Running,
    stdout: mpsc::Receiver<Vec<u8>>,
}

impl Client {
    fn start(target: Target, port: u16) -> Client {
        let mut child = Command::new(&executables(target).tn)
            .args(["--port", &port.to_string()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn tn");
        let mut stdout = child.stdout.take().expect("piped stdout");
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.read_to_end(&mut bytes);
            let _ = send.send(bytes);
        });
        Client {
            process: Running(child),
            stdout: receive,
        }
    }

    fn write_stdin(&mut self, bytes: &[u8]) {
        let stdin = self.process.0.stdin.as_mut().expect("stdin open");
        stdin.write_all(bytes).unwrap();
        stdin.flush().unwrap();
    }

    fn close_stdin(&mut self) {
        drop(self.process.0.stdin.take());
    }

    /// Exit status and everything written to standard output.
    fn finish(mut self) -> (Option<i32>, Vec<u8>) {
        let deadline = Instant::now() + TIMEOUT;
        let status = loop {
            if let Some(status) = self.process.0.try_wait().expect("try_wait") {
                break status;
            }
            assert!(Instant::now() < deadline, "tn did not exit");
            std::thread::sleep(Duration::from_millis(20));
        };
        let stdout = self
            .stdout
            .recv_timeout(TIMEOUT)
            .expect("tn standard output closed");
        (status.code(), stdout)
    }
}

/// A listener the test plays the server on; returns the accepted stream.
fn accept(listener: &TcpListener) -> TcpStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(TIMEOUT)).unwrap();
                return stream;
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "tn did not connect");
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("accept: {e}"),
        }
    }
}

fn listener() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    (listener, port)
}

fn tn_with_piped_input_prints_the_transcript(target: Target) {
    let mut server = Server::start(target);
    let mut client = Client::start(target, server.port);
    client.write_stdin(b"help\necho hello\nstatus\nquit\n");
    client.close_stdin();
    let (status, stdout) = client.finish();
    assert_eq!(
        shown(&stdout),
        shown(
            b"terminal> help\necho <text>\nstatus\nquit\n\
              terminal> hello\nterminal> ready\nterminal> bye\n"
        )
    );
    assert_eq!(status, Some(0));
    server.assert_alive();
}

fn tn_refuses_options_and_strips_commands(target: Target) {
    let (listener, port) = listener();
    let mut client = Client::start(target, port);
    let mut stream = accept(&listener);
    stream.write_all(&[255, 253, 1, 255, 251, 31]).unwrap();
    assert_eq!(read_exactly(&mut stream, 6), [255, 252, 1, 255, 254, 31]);
    stream
        .write_all(b"a\xff\xffb\xff\xfa\x18\x01\xff\xf0c\r\n")
        .unwrap();
    client.write_stdin(b"x\n");
    assert_eq!(shown(&read_exactly(&mut stream, 3)), shown(b"x\r\n"));
    drop(stream);
    let (status, stdout) = client.finish();
    assert_eq!(shown(&stdout), shown(b"a\xffbc\n"));
    assert_eq!(status, Some(0));
}

fn tn_closes_its_side_when_input_ends_and_exits_when_the_server_closes(target: Target) {
    let (listener, port) = listener();
    let mut client = Client::start(target, port);
    let mut stream = accept(&listener);
    client.write_stdin(b"one\n");
    client.close_stdin();
    assert_eq!(shown(&read_until_closed(&mut stream)), shown(b"one\r\n"));
    stream.write_all(b"bye\r\n").unwrap();
    drop(stream);
    let (status, stdout) = client.finish();
    assert_eq!(shown(&stdout), shown(b"bye\n"));
    assert_eq!(status, Some(0));
}

fn tn_exits_when_input_ends_and_terminald_closes(target: Target) {
    let mut server = Server::start(target);
    let mut client = Client::start(target, server.port);
    client.close_stdin();
    let (status, stdout) = client.finish();
    assert_eq!(shown(&stdout), shown(SHOWN_PROMPT));
    assert_eq!(status, Some(0));
    server.assert_alive();
}

/// Waits up to `TIMEOUT` for `process` to exit; its exit code.
fn exit_code(process: &mut Running) -> Option<i32> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(status) = process.0.try_wait().expect("try_wait") {
            return status.code();
        }
        assert!(Instant::now() < deadline, "tn did not exit");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// `tn` with `args` and no standard input: exit code and standard error.
fn tn_exit(target: Target, args: &[&str]) -> (Option<i32>, String) {
    let mut child = Command::new(&executables(target).tn)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tn");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let mut process = Running(child);
    let code = exit_code(&mut process);
    let mut text = String::new();
    let _ = stderr.read_to_string(&mut text);
    (code, text)
}

fn tn_exits_2_for_a_malformed_port(target: Target) {
    let (code, stderr) = tn_exit(target, &["--port", "abc"]);
    assert_eq!(code, Some(2), "stderr: {stderr}");
    assert!(stderr.starts_with("error: "), "stderr: {stderr}");
}

fn tn_exits_3_when_nothing_listens(target: Target) {
    let (listener, port) = listener();
    drop(listener);
    let (code, stderr) = tn_exit(target, &["--port", &port.to_string()]);
    assert_eq!(code, Some(3), "stderr: {stderr}");
    assert!(stderr.starts_with("error: "), "stderr: {stderr}");
}

fn tn_exits_4_on_a_subnegotiation_over_1024_bytes(target: Target) {
    let (listener, port) = listener();
    let client = Client::start(target, port);
    let mut stream = accept(&listener);
    let mut bytes = b"hi\r\n\xff\xfa".to_vec();
    bytes.extend(std::iter::repeat_n(b'x', 1025));
    stream.write_all(&bytes).unwrap();
    let (status, stdout) = client.finish();
    assert_eq!(shown(&stdout), shown(b"hi\n"));
    assert_eq!(status, Some(4));
}

type Shared = std::sync::Arc<(std::sync::Mutex<Vec<u8>>, std::sync::Condvar)>;

/// A pseudo-terminal whose slave is `tn`'s standard input, output and
/// error. The test keeps the slave to read its terminal attributes and
/// collects everything written to it from the master.
struct Terminal {
    master: std::fs::File,
    slave: std::os::fd::OwnedFd,
    output: Shared,
    seen: usize,
}

impl Terminal {
    fn open() -> Terminal {
        use nix::fcntl::{FcntlArg, FdFlag, fcntl};
        let pty = nix::pty::openpty(None, None).expect("openpty");
        for fd in [&pty.master, &pty.slave] {
            fcntl(fd, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC)).expect("close on exec");
        }
        let master = std::fs::File::from(pty.master);
        let mut reader = master.try_clone().expect("clone master");
        let output = Shared::default();
        let shared = output.clone();
        std::thread::spawn(move || {
            let mut chunk = [0u8; 1024];
            while let Ok(count @ 1..) = reader.read(&mut chunk) {
                let (lock, changed) = &*shared;
                lock.lock().unwrap().extend_from_slice(&chunk[..count]);
                changed.notify_all();
            }
        });
        let terminal = Terminal {
            master,
            slave: pty.slave,
            output,
            seen: 0,
        };
        terminal.assert_canonical_with_echo();
        terminal
    }

    fn local_flags(&self) -> nix::sys::termios::LocalFlags {
        nix::sys::termios::tcgetattr(&self.slave)
            .expect("tcgetattr")
            .local_flags
    }

    #[track_caller]
    fn assert_canonical_with_echo(&self) {
        use nix::sys::termios::LocalFlags;
        let flags = self.local_flags();
        assert!(
            flags.contains(LocalFlags::ICANON | LocalFlags::ECHO),
            "not canonical with echo: {flags:?}"
        );
    }

    /// `tn --port <port>` on this terminal, once it has switched the
    /// terminal to raw mode.
    fn start_tn(&self, target: Target, port: u16) -> Running {
        use nix::sys::termios::LocalFlags;
        let stdio = || Stdio::from(self.slave.try_clone().expect("clone slave"));
        let process = Running(
            Command::new(&executables(target).tn)
                .args(["--port", &port.to_string()])
                .stdin(stdio())
                .stdout(stdio())
                .stderr(stdio())
                .spawn()
                .expect("spawn tn"),
        );
        let deadline = Instant::now() + TIMEOUT;
        while self
            .local_flags()
            .intersects(LocalFlags::ICANON | LocalFlags::ECHO | LocalFlags::ISIG)
        {
            assert!(
                Instant::now() < deadline,
                "tn did not switch the terminal to raw mode"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        process
    }

    fn type_keys(&mut self, keys: &[u8]) {
        self.master.write_all(keys).expect("write to the pty");
    }

    /// Waits for `expected` in the output after what earlier calls consumed.
    #[track_caller]
    fn expect(&mut self, expected: &[u8]) {
        let (lock, changed) = &*self.output;
        let deadline = Instant::now() + TIMEOUT;
        let mut output = lock.lock().unwrap();
        loop {
            if let Some(at) = output[self.seen..]
                .windows(expected.len())
                .position(|window| window == expected)
            {
                self.seen += at + expected.len();
                return;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "expected {} after {}",
                shown(expected),
                shown(&output[self.seen..])
            );
            output = changed.wait_timeout(output, left).unwrap().0;
        }
    }
}

fn tn_on_a_terminal_echoes_and_sends_a_line(target: Target) {
    let mut server = Server::start(target);
    let mut terminal = Terminal::open();
    let _tn = terminal.start_tn(target, server.port);
    terminal.expect(SHOWN_PROMPT);
    terminal.type_keys(b"help\r");
    terminal.expect(b"help\r\nhelp\r\necho <text>\r\nstatus\r\nquit\r\nterminal> ");
    terminal.type_keys(b"echo hello\r");
    terminal.expect(b"echo hello\r\nhello\r\nterminal> ");
    server.assert_alive();
}

fn tn_on_a_terminal_erases_with_backspace(target: Target) {
    let server = Server::start(target);
    let mut terminal = Terminal::open();
    let _tn = terminal.start_tn(target, server.port);
    terminal.expect(SHOWN_PROMPT);
    terminal.type_keys(b"ecx\x08ho hi\x7f\x7fyo\r");
    terminal.expect(b"ecx\x08 \x08ho hi\x08 \x08\x08 \x08yo\r\nyo\r\nterminal> ");
}

fn tn_on_a_terminal_quits_from_the_command_prompt(target: Target) {
    let mut server = Server::start(target);
    let mut terminal = Terminal::open();
    let mut tn = terminal.start_tn(target, server.port);
    terminal.expect(SHOWN_PROMPT);
    terminal.type_keys(b"\x1d");
    terminal.expect(b"\ntelnet> ");
    terminal.type_keys(b"quit\r");
    terminal.expect(b"quit\r\n");
    assert_eq!(exit_code(&mut tn), Some(0));
    terminal.assert_canonical_with_echo();
    server.assert_alive();
}

fn tn_on_a_terminal_quits_at_once_when_the_server_keeps_the_connection(target: Target) {
    let (listener, port) = listener();
    let mut terminal = Terminal::open();
    let mut tn = terminal.start_tn(target, port);
    let mut stream = accept(&listener);
    stream.write_all(b"hold> ").unwrap();
    terminal.expect(b"hold> ");
    terminal.type_keys(b"\x1dquit\r");
    assert_eq!(exit_code(&mut tn), Some(0));
    terminal.assert_canonical_with_echo();
    assert_eq!(shown(&read_until_closed(&mut stream)), "");
}

fn tn_on_a_terminal_answers_help_unknown_and_empty_commands(target: Target) {
    let server = Server::start(target);
    let mut terminal = Terminal::open();
    let mut tn = terminal.start_tn(target, server.port);
    terminal.expect(SHOWN_PROMPT);
    terminal.type_keys(b"\x1dhelp\r");
    terminal.expect(b"\ntelnet> help\r\n");
    terminal.expect(b"close ");
    terminal.expect(b"\r\nquit ");
    terminal.expect(b"\r\nhelp ");
    terminal.type_keys(b"\x1dfrob\r");
    terminal.expect(b"\ntelnet> frob\r\nunknown command\r\n");
    terminal.type_keys(b"st\x1d\r");
    terminal.expect(b"st");
    terminal.expect(b"\ntelnet> \r\nst");
    terminal.type_keys(b"atus\r");
    terminal.expect(b"atus\r\nready\r\nterminal> ");
    terminal.type_keys(b"\x1dclose\r");
    assert_eq!(exit_code(&mut tn), Some(0));
    terminal.assert_canonical_with_echo();
}

fn tn_on_a_terminal_ends_the_session_on_ctrl_c(target: Target) {
    let mut server = Server::start(target);
    let mut terminal = Terminal::open();
    let mut tn = terminal.start_tn(target, server.port);
    terminal.expect(SHOWN_PROMPT);
    terminal.type_keys(b"stat\x03");
    assert_eq!(exit_code(&mut tn), Some(0));
    terminal.assert_canonical_with_echo();
    server.assert_alive();
}

/// Whether urgent data is ahead of `stream`'s reader (`POLLPRI`).
fn urgent_pending(stream: &TcpStream) -> bool {
    use nix::poll::{PollFd, PollFlags, poll};
    use std::os::fd::AsFd;
    let mut fds = [PollFd::new(stream.as_fd(), PollFlags::POLLPRI)];
    poll(&mut fds, 0u16).expect("poll");
    fds[0]
        .revents()
        .is_some_and(|events| events.contains(PollFlags::POLLPRI))
}

/// Ctrl+C sends IAC IP and the Synch (RFC 854 p10): IAC DM with the DM as
/// TCP urgent data. The test's socket reads urgent data inline
/// (`SO_OOBINLINE`), so the first read stops before the DM while the
/// urgent mark is pending, and the next starts with it.
fn tn_on_a_terminal_sends_ctrl_c_as_iac_ip(target: Target) {
    use std::os::fd::AsRawFd;
    let (listener, port) = listener();
    let mut terminal = Terminal::open();
    let _tn = terminal.start_tn(target, port);
    let mut stream = accept(&listener);
    let on: nix::libc::c_int = 1;
    // SAFETY: the descriptor belongs to `stream`, and `on` outlives the
    // call with the size given.
    let set = unsafe {
        nix::libc::setsockopt(
            stream.as_raw_fd(),
            nix::libc::SOL_SOCKET,
            nix::libc::SO_OOBINLINE,
            (&raw const on).cast(),
            std::mem::size_of::<nix::libc::c_int>() as nix::libc::socklen_t,
        )
    };
    assert_eq!(set, 0, "SO_OOBINLINE");
    terminal.type_keys(b"a\x03b\r");
    assert_eq!(shown(&read_exactly(&mut stream, 3)), shown(b"\xff\xf4\xff"));
    assert!(urgent_pending(&stream), "the DM is urgent data");
    assert_eq!(shown(&read_exactly(&mut stream, 4)), shown(b"\xf2b\r\n"));
    assert!(!urgent_pending(&stream), "read past the urgent mark");
}

/// Ctrl+D on a partial line sends it without a line end (RFC 854 p5);
/// Ctrl+D on an empty line closes the write half.
fn tn_on_a_terminal_closes_its_side_on_ctrl_d(target: Target) {
    let (listener, port) = listener();
    let mut terminal = Terminal::open();
    let mut tn = terminal.start_tn(target, port);
    let mut stream = accept(&listener);
    terminal.type_keys(b"one\r");
    terminal.expect(b"one\r\n");
    terminal.type_keys(b"x\x04\x7f\x04");
    assert_eq!(shown(&read_until_closed(&mut stream)), shown(b"one\r\nx"));
    stream.write_all(b"bye\r\n").unwrap();
    drop(stream);
    terminal.expect(b"bye\r\n");
    assert_eq!(exit_code(&mut tn), Some(0));
    terminal.assert_canonical_with_echo();
}

/// The calls of a `strace -f` trace so far, once it has come to rest (no
/// line for 300 ms; a worker without a task ends after a second, later
/// than that): a call counts once, at the line that begins it, and
/// `fcntl(F_GETFD)` not at all, the check a debug build of std makes
/// before each `close`, which no release build makes.
fn calls_at_rest(trace: &Path) -> Vec<String> {
    let deadline = Instant::now() + TIMEOUT;
    let mut text = String::new();
    let mut changed = Instant::now();
    while changed.elapsed() < Duration::from_millis(300) {
        assert!(Instant::now() < deadline, "the trace never came to rest");
        std::thread::sleep(Duration::from_millis(20));
        let now = std::fs::read_to_string(trace).unwrap_or_default();
        if now != text {
            text = now;
            changed = Instant::now();
        }
    }
    text.lines()
        .filter_map(|line| line.split_once(' ').map(|(_, call)| call.trim_start()))
        .filter(|call| !call.starts_with(['<', '+', '-']))
        .filter(|call| !(call.starts_with("fcntl(") && call.contains("F_GETFD")))
        .filter_map(|call| call.split_once('(').map(|(name, _)| name.to_string()))
        .collect()
}

/// The system calls `terminald` makes for the first session of its
/// process: from the client's `connect` until the session is closed and
/// the process is at rest, the session's request sent once the server
/// waits for it. By name with their counts, most first, and the total.
fn first_session_calls(target: Target) -> (usize, String) {
    let status = Command::new("strace")
        .args(["-o", "/dev/null", "true"])
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("strace is needed to count system calls and did not run: {e}"));
    assert!(
        status.success(),
        "strace cannot trace a child here: {status:?}"
    );
    let trace = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "terminal-first-session-{}-{}.txt",
        std::process::id(),
        target.index()
    ));
    let _ = std::fs::remove_file(&trace);
    let server = Server::start_under(target, &["strace", "-f", "-o", trace.to_str().unwrap()]);
    let before = calls_at_rest(&trace).len();
    let mut stream = server.raw();
    calls_at_rest(&trace);
    send_line(&mut stream, "quit");
    assert_eq!(shown(&read_until_closed(&mut stream)), shown(b"bye\r\n"));
    let calls = calls_at_rest(&trace);
    drop(server);
    let _ = std::fs::remove_file(&trace);
    let mut by_name: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for call in &calls[before..] {
        *by_name.entry(call).or_default() += 1;
    }
    let mut counts: Vec<(&str, usize)> = by_name.into_iter().collect();
    counts.sort_by_key(|(name, count)| (std::cmp::Reverse(*count), *name));
    let table: Vec<String> = counts
        .iter()
        .map(|(name, count)| format!("{name} {count}"))
        .collect();
    (calls.len() - before, table.join(", "))
}

/// `story:readiness-session-cost`: the first session of a process, the
/// worker it starts included, makes the system calls of the first session
/// of v0.15.0's `terminald` with the thread it started, and at most one
/// more: v0.15.0 made 30 on the Rust target and 25 on the LLVM target
/// (release builds on 4 CPUs; this counts the builds of the test matrix),
/// this build 29 and 26. The one more on the LLVM target is the `prctl`
/// that names the worker, by which the workers are told from a program's
/// other threads. Before, 60 and 58: what a process does once was done
/// while its first session waited (the worker limit read from the cgroup
/// files, the overflow handler, the eventfd of the lone waits), a `poll`
/// came before each `accept4`, and a worker of the LLVM target asked for
/// its alternate stack before it was given one.
#[test]
fn the_first_session_costs_at_most_one_system_call_more_than_at_v0_15_0() {
    let mut failures = Vec::new();
    for (target, v0_15_0, bound) in [(Target::Rust, 30, 30), (Target::Llvm, 25, 26)] {
        let (total, table) = first_session_calls(target);
        println!("{target:?}: {total} calls, v0.15.0 made {v0_15_0}: {table}");
        if total > bound {
            failures.push(format!(
                "{target:?}: {total} calls, at most {bound} expected (v0.15.0 made \
                 {v0_15_0}): {table}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The symbol names in `binary`, from `nm`.
fn symbols(binary: &Path) -> String {
    let output = Command::new("nm").arg(binary).output().expect("run nm");
    assert!(output.status.success(), "nm {}", binary.display());
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Both targets run the one blocking substrate `ignite-io`; neither links
/// tokio (`docs/slice.md` section 7).
#[test]
fn no_target_links_tokio() {
    for target in [Target::Rust, Target::Llvm] {
        let executables = executables(target);
        for binary in [&executables.terminald, &executables.tn] {
            let names = symbols(binary);
            assert!(
                names.contains("ignite_io"),
                "{} has no ignite_io symbols",
                binary.display()
            );
            assert!(!names.contains("tokio"), "{} links tokio", binary.display());
        }
    }
}

/// `Threads:` of `pid`, from `/proc/<pid>/status`.
fn threads(pid: u32) -> u64 {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).expect("proc status");
    status
        .lines()
        .find_map(|line| line.strip_prefix("Threads:"))
        .and_then(|rest| rest.trim().parse().ok())
        .expect("Threads in proc status")
}

/// One OS thread per task and nothing else but the drain's signal thread
/// (`docs/slice.md` 16.13, `ignite-io/src/drain.rs`, named `drain`, which
/// takes SIGTERM with `sigwait` since slice 12): an idle `terminald` is
/// its main thread and that one, a session adds one thread, and the
/// thread ends with the session.
#[test]
fn idle_terminald_runs_one_thread() {
    const IDLE: u64 = 2;
    for target in [
        Target::Rust,
        Target::Llvm,
        Target::RustStatic,
        Target::LlvmStatic,
    ] {
        let mut server = Server::start(target);
        let pid = server.process.0.id();
        assert_eq!(threads(pid), IDLE, "{target:?} idle");
        let mut stream = server.raw();
        assert_eq!(threads(pid), IDLE + 1, "{target:?} with one session");
        send_line(&mut stream, "quit");
        assert_eq!(shown(&read_until_closed(&mut stream)), shown(b"bye\r\n"));
        drop(stream);
        let deadline = Instant::now() + TIMEOUT;
        while threads(pid) != IDLE {
            assert!(
                Instant::now() < deadline,
                "{target:?}: {} threads after the session ended",
                threads(pid)
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        server.assert_alive();
    }
}

/// Under `ulimit -n 10`, `accept` runs out of descriptors (`EMFILE`):
/// `terminald` logs it, keeps serving the sessions it has, and accepts
/// again once descriptors are free (`docs/slice.md` section 7). With the
/// first session open the process holds 8: standard input, output and
/// error, the listener, `main`'s eventfd, the session's socket and its
/// worker's epoll and eventfd (IO mode `readiness`), so two descriptors
/// are left for the crowd, as there were under `ulimit -n 8` before the
/// workers.
fn accept_survives_running_out_of_descriptors(target: Target) {
    let mut server =
        Server::start_under(target, &["sh", "-c", "ulimit -n 10 && exec \"$@\"", "sh"]);
    let mut first = server.raw();
    let crowd: Vec<TcpStream> = (0..8)
        .map(|_| TcpStream::connect(("127.0.0.1", server.port)).expect("connect"))
        .collect();
    server.assert_logged("accept: ");
    server.assert_alive();
    send_line(&mut first, "echo still here");
    assert_eq!(
        shown(&read_through_prompt(&mut first)),
        shown(b"still here\r\nterminal> \xff\xf9")
    );
    drop(crowd);
    let mut later = server.raw();
    send_line(&mut later, "quit");
    assert_eq!(shown(&read_until_closed(&mut later)), shown(b"bye\r\n"));
    server.assert_alive();
}

/// Whether `systemd-run --user --scope` works here.
fn scopes_available() -> bool {
    Command::new("systemd-run")
        .args(["--user", "--scope", "--quiet", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// A connection that gets a prompt, retried until `TIMEOUT` while the
/// server closes it at once (no thread free yet).
fn session_when_a_thread_is_free(server: &Server) -> TcpStream {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let mut stream = TcpStream::connect(("127.0.0.1", server.port)).expect("connect");
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        let mut prompt = vec![0u8; PROMPT.len()];
        if stream.read_exact(&mut prompt).is_ok() {
            assert_eq!(shown(&prompt), shown(PROMPT));
            return stream;
        }
        assert!(Instant::now() < deadline, "no session within {TIMEOUT:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// With at most three threads (systemd `TasksMax=3`: the main thread, the
/// drain's signal thread of `docs/slice.md` 16.13 and one session), the
/// second connection finds no thread to start: `terminald` logs it, closes
/// that connection only and keeps serving. Skipped without systemd user
/// scopes.
fn a_task_that_cannot_start_closes_only_its_connection(target: Target) {
    if !scopes_available() {
        eprintln!("skipped: systemd-run --user --scope is not available");
        return;
    }
    let mut server = Server::start_under(
        target,
        &[
            "systemd-run",
            "--user",
            "--scope",
            "--quiet",
            "-p",
            "TasksMax=3",
        ],
    );
    let mut first = server.raw();
    let mut refused = TcpStream::connect(("127.0.0.1", server.port)).expect("connect");
    refused.set_read_timeout(Some(TIMEOUT)).unwrap();
    assert_eq!(shown(&read_until_closed(&mut refused)), "");
    server.assert_logged("cannot start a task");
    server.assert_alive();
    send_line(&mut first, "echo still here");
    assert_eq!(
        shown(&read_through_prompt(&mut first)),
        shown(b"still here\r\nterminal> \xff\xf9")
    );
    send_line(&mut first, "quit");
    assert_eq!(shown(&read_until_closed(&mut first)), shown(b"bye\r\n"));
    drop(first);
    let mut again = session_when_a_thread_is_free(&server);
    send_line(&mut again, "quit");
    assert_eq!(shown(&read_until_closed(&mut again)), shown(b"bye\r\n"));
    server.assert_alive();
}

/// Idle timeout of the `terminald` built by [`short_idle_terminald`].
const SHORT_IDLE_MS: u64 = 300;

/// The latest the silent session of
/// `a_silent_session_is_closed_after_the_idle_timeout` may close, from its
/// connect: 10 times [`SHORT_IDLE_MS`], so an idle timeout 10 times too
/// long fails, independent of [`TIMEOUT`]. Alone the close comes 300.9 to
/// 316.6 ms after the connect (5 runs on each of the 4 builds, load 10 to
/// 13, 2026-11-07).
const IDLE_CLOSE: Duration = Duration::from_millis(10 * SHORT_IDLE_MS);

/// `terminald` built against a copy of `std/` whose Telnet idle timeout is
/// [`SHORT_IDLE_MS`], once per build of the matrix.
fn short_idle_terminald(target: Target) -> &'static Path {
    static BUILT: [OnceLock<PathBuf>; 4] = [const { OnceLock::new() }; 4];
    BUILT[target.index()].get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("short-idle-{target:?}"));
        let std = tmp.join("std");
        copy_tree(&root.join("std"), &std);
        let telnet = std.join("protocol/telnet.ignite");
        let text = std::fs::read_to_string(&telnet).unwrap();
        let constant = "idle_timeout: Duration = 10 minutes";
        assert!(text.contains(constant), "telnet.ignite has no `{constant}`");
        let short = format!("idle_timeout: Duration = {SHORT_IDLE_MS} ms");
        std::fs::write(&telnet, text.replace(constant, &short)).unwrap();
        let options = ignite::BuildOptions {
            std: Some(std),
            ..target.options(&tmp.join("out"))
        };
        let source = root.join("examples/terminal/terminal.ignite");
        let output = ignite::build(&source, &options).unwrap_or_else(|e| panic!("{e}"));
        output.executables[0].clone()
    })
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A Telnet session whose client stays silent for the idle timeout ends as
/// at end of input: `terminald` closes the connection and keeps serving.
/// The production timeout is 10 minutes; this `terminald` uses a copy of
/// `std/` with [`SHORT_IDLE_MS`].
fn a_silent_session_is_closed_after_the_idle_timeout(target: Target) {
    let mut server = Server::start_program(short_idle_terminald(target), &[]);
    let started = Instant::now();
    let mut silent = server.raw();
    assert_eq!(shown(&read_until_closed(&mut silent)), "");
    let waited = started.elapsed();
    assert!(
        waited >= Duration::from_millis(SHORT_IDLE_MS) && waited < IDLE_CLOSE,
        "closed after {waited:?}, the idle timeout is {SHORT_IDLE_MS} ms"
    );
    server.assert_alive();
    let mut active = server.raw();
    send_line(&mut active, "echo still here");
    assert_eq!(
        shown(&read_through_prompt(&mut active)),
        shown(b"still here\r\nterminal> \xff\xf9")
    );
}

/// One test per build of the matrix for each case.
macro_rules! both_targets {
    ($($case:ident),* $(,)?) => {
        mod rust {
            $(#[test]
            fn $case() {
                super::$case(crate::common::Target::Rust);
            })*
        }
        mod llvm {
            $(#[test]
            fn $case() {
                super::$case(crate::common::Target::Llvm);
            })*
        }
        mod rust_static {
            $(#[test]
            fn $case() {
                super::$case(crate::common::Target::RustStatic);
            })*
        }
        mod llvm_static {
            $(#[test]
            fn $case() {
                super::$case(crate::common::Target::LlvmStatic);
            })*
        }
    };
}

both_targets! {
    transcript_on_raw_socket,
    fragmented_input_is_reassembled,
    do_echo_is_refused_with_wont,
    will_naws_is_refused_with_dont,
    iac_iac_is_a_literal_byte,
    cr_nul_ends_a_line,
    are_you_there_is_answered,
    subnegotiation_is_skipped,
    subnegotiation_of_1024_bytes_is_skipped,
    subnegotiation_over_1024_bytes_closes,
    interrupt_process_closes,
    overlong_line_is_refused_and_closed,
    line_of_4096_bytes_is_accepted,
    tn_with_piped_input_prints_the_transcript,
    tn_refuses_options_and_strips_commands,
    tn_closes_its_side_when_input_ends_and_exits_when_the_server_closes,
    tn_exits_when_input_ends_and_terminald_closes,
    tn_exits_2_for_a_malformed_port,
    tn_exits_3_when_nothing_listens,
    tn_exits_4_on_a_subnegotiation_over_1024_bytes,
    tn_on_a_terminal_echoes_and_sends_a_line,
    tn_on_a_terminal_erases_with_backspace,
    tn_on_a_terminal_quits_from_the_command_prompt,
    tn_on_a_terminal_quits_at_once_when_the_server_keeps_the_connection,
    tn_on_a_terminal_answers_help_unknown_and_empty_commands,
    tn_on_a_terminal_ends_the_session_on_ctrl_c,
    tn_on_a_terminal_sends_ctrl_c_as_iac_ip,
    tn_on_a_terminal_closes_its_side_on_ctrl_d,
    accept_survives_running_out_of_descriptors,
    a_task_that_cannot_start_closes_only_its_connection,
    a_silent_session_is_closed_after_the_idle_timeout,
}
