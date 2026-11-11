//! Stream ownership on the LLVM target (`docs/llvm.md` section 4): a
//! stream lives as long as the last task or binding that uses it, as the
//! `Arc` of the Rust target does. `tests/fixtures/streams/handed_on.ignite`
//! and `test_body_spawn.ignite`
//! run on both targets; `inline_server.ignite`, built with the LLVM
//! target, answers 2,000 connections on one task without `spawn` and must
//! not keep a descriptor per connection. `closes::` counts the sockets
//! of an HTTP server (`todod`, and `fixtures/http_owner.ignite` whose
//! application holds or takes its connections) before and after 40
//! connections on both targets; `streams/match_arm_owner.ignite` holds
//! the smallest programs of the accept loop's shape.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
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

/// A running HTTP server compiled from Ignite, in a working directory of
/// its own, killed on drop: `todod` of `examples/todo`, or `ownerd` of
/// `fixtures/http_owner.ignite`.
struct HttpProcess {
    child: Child,
    port: u16,
    /// Kept open, so that a line the server logs later has a reader.
    _stderr: BufReader<std::process::ChildStderr>,
}

impl HttpProcess {
    fn start(binary: &Path) -> HttpProcess {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("stream-ownership")
            .join(format!(
                "http-work-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut child = Command::new(binary)
            .args(["--port", "0"])
            .current_dir(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("spawn {}: {e}", binary.display()));
        let mut stderr = BufReader::new(child.stderr.take().unwrap());
        let mut first = String::new();
        stderr.read_line(&mut first).unwrap();
        let port = first
            .trim()
            .strip_prefix("listening on 127.0.0.1:")
            .unwrap_or_else(|| panic!("first line {first:?}"))
            .parse()
            .unwrap();
        HttpProcess {
            child,
            port,
            _stderr: stderr,
        }
    }

    /// `todod` of `examples/todo` built for `target`.
    fn todod(target: common::Target) -> HttpProcess {
        HttpProcess::start(crate::todo::todod(target))
    }

    /// `ownerd` of `fixtures/http_owner.ignite` built for `target`, once
    /// per target and run of this test binary.
    fn ownerd(target: common::Target) -> HttpProcess {
        static BUILT: [OnceLock<PathBuf>; 4] = [const { OnceLock::new() }; 4];
        let binary = BUILT[target.index()].get_or_init(|| {
            let source =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/http_owner.ignite");
            crate::todo::build(&source, "http-owner", target, "ownerd")
        });
        HttpProcess::start(binary)
    }

    fn connect(&self) -> TcpStream {
        let stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to the server");
        stream.set_read_timeout(Some(SERVED)).unwrap();
        stream
    }

    /// One connection: `GET /health`, the head of the answer read, then
    /// closed by the client.
    fn health(&self) {
        let mut stream = self.connect();
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .unwrap();
        let reply = answer(&mut stream);
        assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
    }

    /// What each open descriptor of the server is: the target of its link
    /// in `/proc/<pid>/fd` (`socket:[..]`, `anon_inode:[eventpoll]`, a
    /// path).
    fn open(&self) -> Vec<String> {
        std::fs::read_dir(format!("/proc/{}/fd", self.child.id()))
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|entry| std::fs::read_link(entry.path()).ok())
            .map(|target| target.to_string_lossy().into_owned())
            .collect()
    }

    /// The sockets among the server's open descriptors: its listener and
    /// the connections it has not closed. (A worker's epoll descriptor is
    /// not one, so a worker that starts later does not count.)
    fn sockets(&self) -> usize {
        self.open()
            .iter()
            .filter(|target| target.starts_with("socket:"))
            .count()
    }

    /// Fails unless the server, still running, comes back to `start`
    /// sockets (its listener) after the `served` connections of `what`:
    /// each task ends on its own time after its client left. The wait ends
    /// on the read that shows `start` sockets and on no other, so the
    /// state it ends on is the state it found. The caller has an answer
    /// that says every one of its connections was accepted: until then a
    /// server at `start` sockets may have connections waiting in the
    /// listener's queue, each a socket once it is accepted.
    fn has_closed(&mut self, start: usize, served: usize, what: &str) {
        let begun = Instant::now();
        loop {
            let open = self.open();
            let sockets = open
                .iter()
                .filter(|target| target.starts_with("socket:"))
                .count();
            if sockets == start {
                break;
            }
            assert!(
                begun.elapsed() < SERVED,
                "{sockets} sockets open after {served} connections ({what}), {start} before; open: {}",
                kinds(&open)
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(self.child.try_wait().expect("try_wait").is_none());
    }
}

/// What a server has open (`open`, [`HttpProcess::open`]), by kind and
/// without the scheduler's own descriptors, for a failure's message:
/// `41 socket, 1 pipe, 2 null`. A file is named by its last component.
fn kinds(open: &[String]) -> String {
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for target in open {
        if target.starts_with("anon_inode:") {
            continue;
        }
        let kind = match target.split_once(":[") {
            Some((kind, _)) => kind,
            None => target.rsplit('/').next().unwrap_or(target),
        };
        *counts.entry(kind.to_string()).or_default() += 1;
    }
    counts
        .iter()
        .map(|(kind, count)| format!("{count} {kind}"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl Drop for HttpProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The longest a test below waits for an answer, or for the server to
/// close what it served: a ceiling on a wait that returns at its event.
const SERVED: Duration = Duration::from_secs(30);

/// The connections each test below serves.
const CONNECTIONS: usize = 40;

/// One answer read from `stream`: its head, then the body its
/// `Content-Length` announces.
fn answer(stream: &mut TcpStream) -> String {
    let mut reply = Vec::new();
    let mut byte = [0u8; 1];
    while !reply.ends_with(b"\r\n\r\n") {
        let n = stream.read(&mut byte).expect("read an answer");
        assert!(n > 0, "closed before a whole head: {reply:?}");
        reply.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&reply).into_owned();
    let length = head
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map_or(0, |(_, value)| {
            value.trim().parse::<usize>().expect("Content-Length")
        });
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).expect("read a body");
    head + &String::from_utf8_lossy(&body)
}

/// Reads `stream` to its end: the server closed the connection, or reset
/// it.
fn ended(stream: &mut TcpStream) {
    let mut rest = Vec::new();
    match stream.read_to_end(&mut rest) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {}
        Err(e) => panic!("the server did not end the connection: {e}"),
    }
}

/// A connection the HTTP server of `std.protocol.http` accepted and served
/// in a task of its own is closed when the client has closed it and the
/// task has ended: 40 connections, each one request answered and then
/// closed by the client, leave the server with the sockets it had before.
/// `serve_listener` binds the accepted stream in a `match` arm
/// (`Ok(stream) => { ...; spawn serve_connection(stream, ...) }`) whose
/// other arm returns the error it binds, and the LLVM target then gave the
/// task an owner of the handle without releasing the one `accept` made:
/// the task's release left one owner, and the socket stayed open until the
/// process ended. The Rust target closes it; so did the LLVM target at
/// v0.15.0, whose loop bound the stream with `let`.
#[test]
fn connections_served_in_a_task_of_the_http_server_are_closed() {
    let mut server = HttpProcess::todod(common::Target::Llvm);
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

/// Story `llvm-http-connection-closes-its-socket`: an HTTP server holds as
/// many sockets after 40 connections as before them (its listener),
/// however a connection ends. Each connection is opened after the one
/// before it was closed by this client: a count, not a load.
mod closes {
    use super::*;

    /// One request answered, then the client closes.
    pub fn closed_by_the_client(target: common::Target) {
        let mut server = HttpProcess::todod(target);
        let start = server.sockets();
        for _ in 0..CONNECTIONS {
            server.health();
        }
        server.has_closed(start, CONNECTIONS, "one request each, closed by the client");
    }

    /// Three requests on one connection, each answered, then the client
    /// closes.
    pub fn kept_alive(target: common::Target) {
        let mut server = HttpProcess::todod(target);
        let start = server.sockets();
        for _ in 0..CONNECTIONS {
            let mut stream = server.connect();
            for _ in 0..3 {
                stream
                    .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
                    .unwrap();
                let reply = answer(&mut stream);
                assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
                assert!(reply.contains("\r\nConnection: keep-alive\r\n"), "{reply}");
            }
        }
        server.has_closed(start, CONNECTIONS, "three requests each, kept alive");
    }

    /// One request with `Connection: close`: the server ends the
    /// connection after its answer.
    pub fn asked_to_close(target: common::Target) {
        let mut server = HttpProcess::todod(target);
        let start = server.sockets();
        for _ in 0..CONNECTIONS {
            let mut stream = server.connect();
            stream
                .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
                .unwrap();
            let reply = answer(&mut stream);
            assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
            assert!(reply.contains("\r\nConnection: close\r\n"), "{reply}");
            ended(&mut stream);
        }
        server.has_closed(start, CONNECTIONS, "`Connection: close`");
    }

    /// The client leaves inside a request: inside the head on every other
    /// connection, inside the announced body on the others. No answer says
    /// when the server has accepted such a connection, and it accepts them
    /// on its own time: its sockets fall to the listener alone and rise
    /// again while some still wait in the listener's queue. So one more
    /// connection is made after the 40 and its request answered: the
    /// queue hands connections out in the order they were made, and the
    /// answer says each of the 40 was accepted.
    pub fn dropped_inside_a_request(target: common::Target) {
        let mut server = HttpProcess::todod(target);
        let start = server.sockets();
        for round in 0..CONNECTIONS {
            let mut stream = server.connect();
            let part: &[u8] = if round % 2 == 0 {
                b"GET /health HTTP/1.1\r\nHost: 127."
            } else {
                b"POST /todos HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 100\r\n\r\n{\"title\":"
            };
            stream.write_all(part).unwrap();
        }
        server.health();
        server.has_closed(start, CONNECTIONS, "dropped inside a request");
    }

    /// The application holds each request (`Reply.Hold`) until another
    /// connection has it released; on every other connection the client
    /// has left by then. The held connection is the application's until
    /// its answer is given: 40 of them, and the 40 or more that asked for
    /// the release, leave no socket.
    pub fn held_by_the_application(target: common::Target) {
        let mut server = HttpProcess::ownerd(target);
        let start = server.sockets();
        let mut served = 0;
        for round in 0..CONNECTIONS {
            let mut held = server.connect();
            held.write_all(b"GET /hold HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
                .unwrap();
            let stays = round % 2 == 0;
            let mut held = stays.then_some(held);
            served += 1;
            // The release answers with how many held requests took it, and
            // forgets each: 0 until the server has read this round's
            // request and holds it, then 1, this round's and no other.
            let deadline = Instant::now() + SERVED;
            loop {
                let mut other = server.connect();
                other
                    .write_all(b"GET /release HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
                    .unwrap();
                let reply = answer(&mut other);
                served += 1;
                if reply.ends_with("\r\n\r\n1") {
                    break;
                }
                assert!(reply.ends_with("\r\n\r\n0"), "{reply}");
                assert!(Instant::now() < deadline, "the request was never held");
                std::thread::sleep(Duration::from_millis(10));
            }
            if let Some(held) = &mut held {
                let reply = answer(held);
                assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
                assert!(reply.ends_with("\r\n\r\nreleased"), "{reply}");
            }
        }
        server.has_closed(start, served, "held by the application, then released");
    }

    /// The application takes each connection over (`Reply.Upgrade`) and
    /// echoes on it until the client ends its side; the connection is the
    /// takeover's while it runs and is closed when it returns.
    pub fn taken_over_by_the_application(target: common::Target) {
        let mut server = HttpProcess::ownerd(target);
        let start = server.sockets();
        for _ in 0..CONNECTIONS {
            let mut stream = server.connect();
            stream
                .write_all(
                    b"GET /take HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: echo\r\nConnection: upgrade\r\n\r\n",
                )
                .unwrap();
            let reply = answer(&mut stream);
            assert!(reply.starts_with("HTTP/1.1 101"), "{reply}");
            let mut greeting = [0u8; 6];
            stream.read_exact(&mut greeting).expect("the greeting");
            assert_eq!(&greeting, b"taken\n");
            stream.write_all(b"ping").unwrap();
            let mut echoed = [0u8; 4];
            stream.read_exact(&mut echoed).expect("the echo");
            assert_eq!(&echoed, b"ping");
            assert!(
                server.sockets() >= 2,
                "the taken connection is open while its takeover runs"
            );
            stream.shutdown(std::net::Shutdown::Write).unwrap();
            ended(&mut stream);
        }
        server.has_closed(start, CONNECTIONS, "taken over by the application");
    }

    macro_rules! on_each_target {
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
        };
    }

    on_each_target!(
        closed_by_the_client,
        kept_alive,
        asked_to_close,
        dropped_inside_a_request,
        held_by_the_application,
        taken_over_by_the_application,
    );
}

/// Story `llvm-http-connection-closes-its-socket`, the smallest programs
/// of the shape (`match_arm_owner.ignite`): a `match` on `accept` whose
/// `Ok` arm spawns a task with the stream, or only uses it, beside an
/// `Err` arm that returns its error, closes the connection on both
/// targets; an arm that returns the stream hands it on open.
#[test]
fn a_stream_bound_in_a_match_arm_is_closed_on_both_targets() {
    ignite_test("match_arm_owner.ignite", "all", 3, 2);
}
