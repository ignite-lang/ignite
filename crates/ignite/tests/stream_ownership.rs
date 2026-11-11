//! Stream ownership on the LLVM target (`docs/llvm.md` section 4): a
//! stream lives as long as the last task or binding that uses it, as the
//! `Arc` of the Rust target does. `tests/fixtures/streams/handed_on.ignite`
//! and `test_body_spawn.ignite`
//! run on both targets; `inline_server.ignite`, built with the LLVM
//! target, answers 2,000 connections on one task without `spawn` and must
//! not keep a descriptor per connection.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use ignite::Target;

use crate::common;

const TIMEOUT: Duration = Duration::from_secs(10);
const SESSIONS: usize = 2_000;
const SLACK: usize = 16;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/streams")
        .join(name)
}

/// `ignite test` of the fixture `name` on `target`; asserts exit 0 and
/// `runs` result lines of `passed` tests each.
fn ignite_test(name: &str, target: &str, passed: usize, runs: usize) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("stream-ownership")
        .join(name.trim_end_matches(".ignite"));
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(fixture(name))
        .args(["--target", target, "--timeout", "20", "--out"])
        .arg(&dir)
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .arg("--runtime")
        .arg(common::runner_archive())
        .output()
        .expect("run ignite test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let line = format!("test result: ok. {passed} passed; 0 failed;");
    assert_eq!(stdout.matches(&line).count(), runs, "{stdout}");
}

/// The review's probe3 (M1) over a pipe and over TCP, through one and two
/// tasks, and story llvm-spawn-pattern-bound-streams with the spawn in a
/// helper: a stream handed on to a second task outlives the first, and a
/// failing task closes the pattern-bound pipe end it was given.
#[test]
fn handed_on_streams_pass_on_both_targets() {
    ignite_test("handed_on.ignite", "all", 4, 2);
}

/// Story llvm-spawn-pattern-bound-streams with the spawn in the test body:
/// the end is the task's from its last use on, so the failure closes it
/// (on Rust the moves analysis moves it into the task, `docs/slice.md`
/// 20.3.1); an end used after its spawn stays the body's
/// (`test_body_spawn.ignite`).
#[test]
fn a_spawn_in_a_test_body_hands_the_pattern_bound_end_over() {
    ignite_test("test_body_spawn.ignite", "all", 2, 2);
}

#[test]
fn a_spawn_in_a_branch_or_before_a_shadowing_binding_closes_the_end_on_both_targets() {
    ignite_test("nested_spawn.ignite", "all", 2, 2);
}

/// The rest of the class of the test above: a spawn inside a `match` arm
/// or an `else` branch, an end passed as a named argument, and a branch
/// that owns a pipe of its own and still closes it at its end.
#[test]
fn a_spawn_in_a_match_arm_or_else_branch_closes_the_end_on_both_targets() {
    ignite_test("nested_spawn_arms.ignite", "all", 4, 2);
}

/// Adversary of story spawn-moves-adversary-findings: a spawn before a
/// binding that shadows the name in an inner block only, a spawn in a
/// branch that owns a pipe of its own before it, a match nested in an
/// `if`, a UFCS receiver with named arguments, an `else if` chain, and
/// the evaluation and passing order of named spawn arguments.
#[test]
fn adversary_nested_spawns_close_the_end_on_both_targets() {
    ignite_test("nested_spawn_adversary.ignite", "all", 6, 2);
}

/// Second adversary pass: the name bound again after the spawn by a match
/// arm, a for loop or a closure parameter (the end is handed over on both
/// targets), and the outer name used after an inner shadowing (the end
/// stays the body's and writable on both targets).
#[test]
fn adversary2_later_bindings_of_the_name_agree_on_both_targets() {
    ignite_test("nested_spawn_adversary2.ignite", "all", 5, 2);
}

#[test]
fn named_spawn_arguments_in_another_order_build_and_pass_on_both_targets() {
    ignite_test("named_spawn.ignite", "all", 1, 2);
}

/// A running `inlined`, killed on drop.
struct Server {
    child: Child,
    port: u16,
}

impl Server {
    fn start() -> Server {
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("llvm-inline-server");
        let options = ignite::BuildOptions {
            target: Target::Llvm,
            out: Some(out.clone()),
            runtime: Some(common::runtime_archive()),
            ..Default::default()
        };
        let source = fixture("inline_server.ignite");
        let output = ignite::build(&source, &options).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(output.executables, [out.join("bin/inlined")]);
        let mut child = Command::new(&output.executables[0])
            .args(["--port", "0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn inlined");
        let mut stderr = BufReader::new(child.stderr.take().unwrap());
        let mut first = String::new();
        stderr.read_line(&mut first).unwrap();
        let port = first
            .trim()
            .strip_prefix("listening on 127.0.0.1:")
            .unwrap_or_else(|| panic!("first line {first:?}"))
            .parse()
            .unwrap();
        Server { child, port }
    }

    /// One session: send `line`, then everything the server writes until
    /// it shuts down its write half.
    fn session(&self, line: &str) -> Vec<u8> {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        stream.write_all(line.as_bytes()).unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).unwrap();
        reply
    }

    fn open_fds(&self) -> usize {
        std::fs::read_dir(format!("/proc/{}/fd", self.child.id()))
            .unwrap()
            .count()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The review's L1: 2,000 connections answered on one task without
/// `spawn` leave the server's descriptor count within 16 of its start.
#[test]
fn inline_sessions_keep_the_descriptor_count() {
    let mut server = Server::start();
    assert_eq!(server.session("warm\n"), b"ok warm\n");
    let start = server.open_fds();
    for _ in 0..SESSIONS {
        assert_eq!(server.session("hi\n"), b"ok hi\n");
    }
    let deadline = Instant::now() + TIMEOUT;
    while server.open_fds() > start + SLACK && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let open = server.open_fds();
    assert!(
        open <= start + SLACK,
        "{open} descriptors open after {SESSIONS} sessions, {start} before"
    );
    assert!(server.child.try_wait().expect("try_wait").is_none());
}

/// A running `todod` of `examples/todo` built for the LLVM target, in a
/// working directory of its own, killed on drop.
struct Todod {
    child: Child,
    port: u16,
    /// Kept open, so that a line the server logs later has a reader.
    _stderr: BufReader<std::process::ChildStderr>,
}

impl Todod {
    fn start() -> Todod {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("stream-ownership")
            .join(format!("todod-work-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut child = Command::new(crate::todo::todod(common::Target::Llvm))
            .args(["--port", "0"])
            .current_dir(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn todod");
        let mut stderr = BufReader::new(child.stderr.take().unwrap());
        let mut first = String::new();
        stderr.read_line(&mut first).unwrap();
        let port = first
            .trim()
            .strip_prefix("listening on 127.0.0.1:")
            .unwrap_or_else(|| panic!("first line {first:?}"))
            .parse()
            .unwrap();
        Todod {
            child,
            port,
            _stderr: stderr,
        }
    }

    /// One connection: `GET /health`, the head of the answer read, then
    /// closed by the client.
    fn health(&self) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .unwrap();
        let mut reply = Vec::new();
        let mut chunk = [0u8; 4096];
        while !reply.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut chunk).expect("read from todod");
            assert!(n > 0, "todod closed before a whole head: {reply:?}");
            reply.extend_from_slice(&chunk[..n]);
        }
        assert!(
            reply.starts_with(b"HTTP/1.1 200"),
            "{}",
            String::from_utf8_lossy(&reply)
        );
    }

    /// The sockets among the server's open descriptors: its listener and
    /// the connections it has not closed. (A worker's epoll descriptor is
    /// not one, so a worker that starts later does not count.)
    fn sockets(&self) -> usize {
        std::fs::read_dir(format!("/proc/{}/fd", self.child.id()))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                std::fs::read_link(entry.path())
                    .is_ok_and(|target| target.to_string_lossy().starts_with("socket:"))
            })
            .count()
    }
}

impl Drop for Todod {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A connection the HTTP server of `std.protocol.http` accepted and served
/// in a task of its own is closed when the client has closed it and the
/// task has ended: 40 connections, each one request answered and then
/// closed by the client, leave the server with the sockets it had before.
/// `serve_listener` binds the accepted stream in a `match` arm
/// (`Ok(stream) => { ...; spawn serve_connection(stream, ...) }`), and the
/// LLVM target gives the task an owner of the handle without releasing the
/// one `accept` made: the task's release leaves one owner, and the socket
/// stays open until the process ends. The Rust target closes it; so did the
/// LLVM target at v0.15.0, whose loop bound the stream with `let`.
#[test]
fn connections_served_in_a_task_of_the_http_server_are_closed() {
    const CONNECTIONS: usize = 40;
    let mut server = Todod::start();
    let start = server.sockets();
    for _ in 0..CONNECTIONS {
        server.health();
    }
    let deadline = Instant::now() + TIMEOUT;
    while server.sockets() > start && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let open = server.sockets();
    assert_eq!(
        open, start,
        "{open} sockets open after {CONNECTIONS} connections served and closed, {start} before"
    );
    assert!(server.child.try_wait().expect("try_wait").is_none());
}
