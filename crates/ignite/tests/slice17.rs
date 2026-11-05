//! Slice 17 (`docs/slice.md` section 21) end to end: the fixtures under
//! `tests/fixtures/slice17/` built with `ignite::build` once per target and
//! test binary, started as real processes and driven over a real socket.
//! Every case runs once per target (`slice17::rust::<case>`,
//! `slice17::llvm::<case>`).
//!
//! Children of a program (rows 13, 14 and 17 of 21.0): `signal_mask.ignite`
//! shows what a child of a server inherits, `orphans.ignite` that children
//! end with the program, however it ends.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use ignite::Target;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

use crate::common;

/// The longest a test waits for a line, a reply or an exit.
const TIMEOUT: Duration = Duration::from_secs(15);

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/slice17")
        .join(name)
}

fn tmp(name: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slice17")
        .join(name)
}

fn target_name(target: Target) -> &'static str {
    match target {
        Target::Rust => "rust",
        Target::Llvm => "llvm",
    }
}

/// `source` built once per target into this test binary's own directory;
/// the executables by file name.
fn build(source: &str, target: Target) -> Vec<PathBuf> {
    let out = tmp(format!("build-{source}-{}", target_name(target)));
    let options = match target {
        Target::Rust => ignite::BuildOptions {
            target: Target::Rust,
            out: Some(out),
            ..Default::default()
        },
        Target::Llvm => ignite::BuildOptions {
            target: Target::Llvm,
            out: Some(out),
            runtime: Some(common::runtime_archive()),
            ..Default::default()
        },
    };
    let output = ignite::build(&fixture(source), &options).unwrap_or_else(|e| panic!("{e}"));
    output.executables
}

/// The executable `name` of the fixture `source`, built once per target.
fn executable(source: &'static str, name: &str, target: Target) -> PathBuf {
    static BUILT: Mutex<Vec<(&'static str, &'static str, Vec<PathBuf>)>> = Mutex::new(Vec::new());
    let key = target_name(target);
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    if !built.iter().any(|(s, t, _)| *s == source && *t == key) {
        let executables = build(source, target);
        built.push((source, key, executables));
    }
    let (_, _, executables) = built
        .iter()
        .find(|(s, t, _)| *s == source && *t == key)
        .expect("built just now");
    executables
        .iter()
        .find(|path| path.file_stem().and_then(|s| s.to_str()) == Some(name))
        .unwrap_or_else(|| panic!("no {name} in {executables:?}"))
        .clone()
}

/// An empty working directory of its own for one case and target.
fn workdir(case: &str, target: Target) -> PathBuf {
    let dir = tmp(format!("work-{case}-{}", target_name(target)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the working directory");
    dir
}

/// Lines of a stream as they arrive.
fn lines_of(stream: impl Read + Send + 'static) -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

/// The next line that starts with `prefix`, without it, within [`TIMEOUT`].
fn wait_line(lines: &Receiver<String>, prefix: &str) -> String {
    let deadline = Instant::now() + TIMEOUT;
    let mut seen = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match lines.recv_timeout(left) {
            Ok(line) => match line.strip_prefix(prefix) {
                Some(rest) => return rest.to_string(),
                None => seen.push(line),
            },
            Err(error) => panic!("no `{prefix}` line ({error}); seen:\n{}", seen.join("\n")),
        }
    }
}

fn wait_exit(child: &mut Child, what: &str) -> ExitStatus {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            return status;
        }
        assert!(
            started.elapsed() < TIMEOUT,
            "{what} did not exit within {TIMEOUT:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A server started on a free port in its own working directory; killed
/// on drop.
struct Server {
    child: Child,
    stderr: Receiver<String>,
    port: u16,
}

impl Server {
    fn start(command: &mut Command) -> Server {
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("start {command:?}: {e}"));
        let stderr = lines_of(child.stderr.take().expect("stderr"));
        let listening = wait_line(&stderr, "listening on 127.0.0.1:");
        let port = listening
            .parse()
            .unwrap_or_else(|e| panic!("{listening}: {e}"));
        Server {
            child,
            stderr,
            port,
        }
    }

    fn pid(&self) -> i32 {
        self.child.id() as i32
    }

    fn signal(&self, signal: Signal) {
        kill(Pid::from_raw(self.pid()), signal).unwrap_or_else(|e| panic!("{signal}: {e}"));
    }

    /// `GET path` on a new connection: the status and the body.
    fn get(&self, path: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        stream.set_read_timeout(Some(TIMEOUT)).expect("timeout");
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .expect("write the request");
        let mut seen = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            if let Some(reply) = reply_of(&seen) {
                return reply;
            }
            let n = stream
                .read(&mut buffer)
                .unwrap_or_else(|e| panic!("read: {e}; seen {:?}", text(&seen)));
            assert!(n > 0, "eof before a whole reply; seen {:?}", text(&seen));
            seen.extend_from_slice(&buffer[..n]);
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Status and body of a whole HTTP reply framed by `Content-Length`.
fn reply_of(seen: &[u8]) -> Option<(u16, String)> {
    let end = seen.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = text(&seen[..end]);
    let status = head.split(' ').nth(1)?.parse().ok()?;
    let length: usize = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())
                .flatten()
        })
        .unwrap_or(0);
    let body = seen.get(end + 4..end + 4 + length)?;
    Some((status, text(body)))
}

/// The state letter of `/proc/<pid>/stat`, `None` when there is no such
/// process.
fn state_of(pid: i32) -> Option<char> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(')')?.1.trim_start().chars().next()
}

fn assert_alive(pid: i32) {
    let state = state_of(pid);
    assert!(
        state.is_some_and(|s| s != 'Z'),
        "child {pid} is not running: {state:?}"
    );
}

/// Within 1 s the process `pid` is gone or a zombie.
fn assert_ends(pid: i32, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match state_of(pid) {
            None | Some('Z') => return,
            Some(state) => assert!(
                Instant::now() < deadline,
                "{what}: child {pid} still in state {state} 1 s after the program ended"
            ),
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn mask_server(case: &str, target: Target) -> Server {
    let maskd = executable("signal_mask.ignite", "maskd", target);
    Server::start(
        Command::new(maskd)
            .args(["--port", "0"])
            .current_dir(workdir(case, target)),
    )
}

/// Row 13: the drain blocks SIGTERM in every thread of the server; the
/// child starts with an empty mask, so its own SIGTERM ends it.
fn a_child_of_a_server_gets_sigterm(target: Target) {
    let server = mask_server("sigterm", target);
    let started = Instant::now();
    let (status, body) = server.get("/");
    assert_eq!((status, body.as_str()), (200, "ending 1 code 15"));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the reply took {:?}",
        started.elapsed()
    );
}

/// Row 14: a descriptor the server inherited without close-on-exec, its
/// listener, its connection, its root directory and its entropy source do
/// not reach a child.
fn a_child_inherits_no_descriptors(target: Target) {
    let maskd = executable("signal_mask.ignite", "maskd", target);
    let server = Server::start(
        Command::new("/bin/sh")
            .args(["-c", "exec 5</dev/null; exec \"$0\" --port 0"])
            .arg(maskd)
            .current_dir(workdir("fds", target)),
    );
    let inherited = std::fs::read_link(format!("/proc/{}/fd/5", server.pid()))
        .expect("the server holds descriptor 5");
    assert_eq!(inherited, Path::new("/dev/null"));
    let (status, body) = server.get("/fds");
    assert_eq!(status, 200, "{body}");
    let listed: Vec<&str> = body.lines().collect();
    assert_eq!(listed, ["0", "1", "2", "3"], "{body:?}");
}

/// Row 17: `orphan` starts `/bin/sleep 30`, prints its id and returns from
/// `main`; the child ends with it.
fn live_children_die_with_the_program(target: Target) {
    let orphan = executable("orphans.ignite", "orphan", target);
    let mut child = Command::new(orphan)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start orphan");
    let stdout: ChildStdout = child.stdout.take().expect("stdout");
    let lines = lines_of(stdout);
    let pid: i32 = lines
        .recv_timeout(TIMEOUT)
        .expect("orphan printed its child's id")
        .parse()
        .expect("an id");
    let status = wait_exit(&mut child, "orphan");
    assert_eq!(status.code(), Some(0), "{status:?}");
    assert_ends(pid, "main returned");
}

/// An `orphand` with one unwaited child; its id.
fn orphand_with_a_child(case: &str, target: Target) -> (Server, i32) {
    let orphand = executable("orphans.ignite", "orphand", target);
    let server = Server::start(
        Command::new(orphand)
            .args(["--port", "0"])
            .current_dir(workdir(case, target)),
    );
    let (status, body) = server.get("/");
    assert_eq!(status, 200, "{body}");
    let pid: i32 = body.parse().unwrap_or_else(|e| panic!("{body}: {e}"));
    assert_alive(pid);
    (server, pid)
}

/// Row 17 after one SIGTERM: the drain ends the server with status 0, and
/// its child with it.
fn orphans_after_drain(target: Target) {
    let (mut server, pid) = orphand_with_a_child("drain", target);
    server.signal(Signal::SIGTERM);
    let status = wait_exit(&mut server.child, "orphand");
    assert_eq!(status.code(), Some(0), "{status:?}");
    assert_ends(pid, "after the drain");
}

/// Row 17 after a second SIGTERM during the drain, which an idle
/// connection holds open.
fn orphans_after_second_sigterm(target: Target) {
    let (mut server, pid) = orphand_with_a_child("second", target);
    let _idle = TcpStream::connect(("127.0.0.1", server.port)).expect("connect");
    std::thread::sleep(Duration::from_millis(50));
    server.signal(Signal::SIGTERM);
    wait_line(&server.stderr, "released ");
    assert!(
        server.child.try_wait().expect("wait").is_none(),
        "the drain ended with a session open"
    );
    assert_alive(pid);
    server.signal(Signal::SIGTERM);
    let status = wait_exit(&mut server.child, "orphand");
    assert_eq!(status.signal(), Some(Signal::SIGTERM as i32), "{status:?}");
    assert_ends(pid, "after the second SIGTERM");
}

/// Row 17 after SIGKILL of the server: `PR_SET_PDEATHSIG` ends the direct
/// child.
fn orphans_after_sigkill(target: Target) {
    let (mut server, pid) = orphand_with_a_child("sigkill", target);
    server.signal(Signal::SIGKILL);
    let status = wait_exit(&mut server.child, "orphand");
    assert_eq!(status.signal(), Some(Signal::SIGKILL as i32), "{status:?}");
    assert_ends(pid, "after SIGKILL");
}

macro_rules! both_targets {
    ($($case:ident),* $(,)?) => {
        mod rust {
            $(#[test]
            fn $case() {
                super::$case(ignite::Target::Rust);
            })*
        }
        mod llvm {
            $(#[test]
            fn $case() {
                super::$case(ignite::Target::Llvm);
            })*
        }
    };
}

both_targets!(
    a_child_of_a_server_gets_sigterm,
    a_child_inherits_no_descriptors,
    live_children_die_with_the_program,
    orphans_after_drain,
    orphans_after_second_sigterm,
    orphans_after_sigkill,
);
