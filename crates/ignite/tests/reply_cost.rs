//! Story `http-reply-cost-back` (rows 3.3 and 3.8 `todo/get-1` of
//! `docs/quality.md`): an HTTP reply of the todo example allocates no more
//! than at v0.15.0, and a server whose application has no `reply` starts no
//! task for held requests.
//!
//! Allocations are counted outside the process, so that a v0.15.0 binary
//! is counted by the same code: `valgrind --tool=memcheck
//! --trace-malloc=yes` logs every `malloc`, `calloc`, `realloc` and `free`
//! of the server, and the lines the log gains over a run of replies are
//! the run's calls. `valgrind` is a prerequisite of these tests, as
//! `strace` is of `scheduler::` in `crates/ignite-io`; without it they
//! fail and name it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::common::Target;
use crate::todo;

/// The longest a wait of this file takes: the listening line, one reply,
/// the log of a run coming to rest. Each returns at its event.
const LIMIT: Duration = Duration::from_secs(60);

/// Replies before the count, so that first-use allocations (the store's
/// load, buffers that grow once) are not counted.
const WARM: usize = 50;

/// Replies counted.
const COUNTED: usize = 200;

/// v0.15.0's allocations (`malloc`, `calloc`, `memalign`) per `GET
/// /todos/1` reply of `todod` on the Rust target, by
/// [`count_a_given_todod`] over its release and its dev build: 37.000 in
/// both (`realloc` 3.000). The diagnosis of the story counted the same
/// 37 with an `LD_PRELOAD` counter of the release build.
const V0_15_0_RUST: f64 = 37.0;

/// The same on the LLVM target, release and dev build: 1.000 (`realloc`
/// 0).
const V0_15_0_LLVM: f64 = 1.0;

/// What a run counted: allocations and reallocations per reply.
struct Counted {
    allocations: f64,
    reallocations: f64,
}

/// A child process, killed on drop.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `valgrind` runs here; otherwise allocations cannot be counted, and the
/// test says which prerequisite is missing.
fn valgrind_works() {
    let status = Command::new("valgrind")
        .args([
            "--tool=memcheck",
            "--trace-malloc=yes",
            "--log-file=/dev/null",
            "true",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("valgrind is needed to count allocations and did not run: {e}"));
    assert!(status.success(), "valgrind did not run `true`: {status:?}");
}

/// A new empty directory under the test's scratch directory.
fn scratch(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "reply-cost-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `command` started in `dir`; the port of its `listening on
/// 127.0.0.1:<port>` line.
fn start(mut command: Command, dir: &Path) -> (Running, u16) {
    let mut child = command
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the server");
    let stderr = child.stderr.take().expect("piped stderr");
    let running = Running(child);
    let (ports, port) = mpsc::channel();
    std::thread::spawn(move || {
        let mut stderr = BufReader::new(stderr);
        let mut line = String::new();
        while stderr.read_line(&mut line).unwrap_or(0) > 0 {
            if let Some(rest) = line.trim_end().strip_prefix("listening on 127.0.0.1:") {
                let _ = ports.send(rest.parse::<u16>());
            }
            line.clear();
        }
    });
    let port = port
        .recv_timeout(LIMIT)
        .expect("no `listening on 127.0.0.1:<port>` line")
        .expect("port in the listening line");
    (running, port)
}

/// One keep-alive HTTP/1.1 connection that sends a request and reads its
/// whole answer: the status and the body.
struct Client {
    stream: TcpStream,
    buffer: Vec<u8>,
}

impl Client {
    fn connect(port: u16) -> Client {
        let stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        stream.set_nodelay(true).unwrap();
        stream.set_read_timeout(Some(LIMIT)).unwrap();
        Client {
            stream,
            buffer: Vec::new(),
        }
    }

    fn send(&mut self, request: &[u8]) -> (u16, String) {
        self.stream.write_all(request).unwrap();
        loop {
            if let Some(end) = self.buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&self.buffer[..end]).into_owned();
                let length: usize = head
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().ok())?
                    })
                    .unwrap_or(0);
                if self.buffer.len() >= end + 4 + length {
                    let body = String::from_utf8_lossy(&self.buffer[end + 4..end + 4 + length])
                        .into_owned();
                    self.buffer.drain(..end + 4 + length);
                    let status = head
                        .get(9..12)
                        .and_then(|code| code.parse().ok())
                        .unwrap_or_else(|| panic!("status line of {head:?}"));
                    return (status, body);
                }
            }
            let mut chunk = [0u8; 65536];
            let n = self.stream.read(&mut chunk).expect("read the answer");
            assert!(n > 0, "closed before a whole answer: {:?}", self.buffer);
            self.buffer.extend_from_slice(&chunk[..n]);
        }
    }
}

/// The allocation calls in a `--trace-malloc=yes` log so far, and the
/// reallocations, once the log has come to rest (no line for 300 ms).
fn calls_at_rest(log: &Path) -> (u64, u64) {
    let deadline = Instant::now() + LIMIT;
    let mut last = None;
    loop {
        std::thread::sleep(Duration::from_millis(300));
        let text = std::fs::read_to_string(log).unwrap_or_default();
        let mut allocations = 0;
        let mut reallocations = 0;
        for line in text.lines() {
            let Some(call) = line
                .strip_prefix("--")
                .and_then(|rest| rest.split_once("-- "))
                .map(|(_, call)| call)
            else {
                continue;
            };
            let name = call.split('(').next().unwrap_or("");
            match name {
                "malloc" | "calloc" | "memalign" | "posix_memalign" | "aligned_alloc"
                | "valloc" => allocations += 1,
                "realloc" => reallocations += 1,
                _ => {}
            }
        }
        let now = (allocations, reallocations, text.len());
        if last == Some(now) {
            return (allocations, reallocations);
        }
        assert!(
            Instant::now() < deadline,
            "the allocation log of {} did not come to rest",
            log.display()
        );
        last = Some(now);
    }
}

/// The allocations per `GET /todos/1` reply of the todo server `binary`
/// under valgrind: one todo created, [`WARM`] replies, then [`COUNTED`]
/// replies counted, all on one keep-alive connection.
fn count_todod(binary: &Path) -> Counted {
    let dir = scratch("todod");
    let log = dir.join("trace-malloc.log");
    let mut command = Command::new("valgrind");
    command
        .args(["--tool=memcheck", "--trace-malloc=yes"])
        .arg(format!("--log-file={}", log.display()))
        .arg(binary)
        .args(["--port", "0"]);
    let (_server, port) = start(command, &dir);
    let mut client = Client::connect(port);
    let body = br#"{"title":"bench"}"#;
    let mut create = format!(
        "POST /todos HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    create.extend_from_slice(body);
    let (status, answer) = client.send(&create);
    assert_eq!(status, 201, "{answer}");
    let get = b"GET /todos/1 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
    for _ in 0..WARM {
        let (status, answer) = client.send(get);
        assert_eq!(status, 200, "{answer}");
    }
    let (allocations, reallocations) = calls_at_rest(&log);
    for _ in 0..COUNTED {
        let (status, answer) = client.send(get);
        assert_eq!(status, 200, "{answer}");
    }
    let (allocations_after, reallocations_after) = calls_at_rest(&log);
    Counted {
        allocations: (allocations_after - allocations) as f64 / COUNTED as f64,
        reallocations: (reallocations_after - reallocations) as f64 / COUNTED as f64,
    }
}

/// Rows 3.3 `rust/todo/get-1` and 3.8 `todo/get-1`: a `GET` reply of the
/// todo example allocates no more than at v0.15.0 ([`V0_15_0_RUST`],
/// [`V0_15_0_LLVM`]), its `realloc`s printed beside. The count per reply
/// is rounded: an allocation now and then that is no reply's (a list of a
/// long-lived object that doubles) does not fail it, one more per reply
/// does.
#[test]
fn a_get_reply_allocates_no_more_than_at_v0_15_0() {
    valgrind_works();
    let mut failures = Vec::new();
    for (target, bound) in [(Target::Rust, V0_15_0_RUST), (Target::Llvm, V0_15_0_LLVM)] {
        let counted = count_todod(todo::todod(target));
        println!(
            "{target:?}: {:.3} allocations and {:.3} reallocations per reply, v0.15.0 {bound}",
            counted.allocations, counted.reallocations
        );
        if counted.allocations.round() > bound {
            failures.push(format!(
                "{target:?}: {:.3} allocations per reply, v0.15.0 made {bound}",
                counted.allocations
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The count of [`a_get_reply_allocates_no_more_than_at_v0_15_0`] over a
/// `todod` binary named by `REPLY_COST_COUNT_SERVER`: how its bounds were
/// measured. Does nothing without the variable.
#[test]
fn count_a_given_todod() {
    let Some(server) = std::env::var_os("REPLY_COST_COUNT_SERVER") else {
        return;
    };
    valgrind_works();
    let counted = count_todod(Path::new(&server));
    println!(
        "{}: {:.3} allocations and {:.3} reallocations per reply",
        Path::new(&server).display(),
        counted.allocations,
        counted.reallocations
    );
}

/// `fixtures/http_census.ignite` built for `target`: `censusd` and
/// `censusreplyd`.
fn census_servers(target: Target) -> &'static (PathBuf, PathBuf) {
    static BUILT: [OnceLock<(PathBuf, PathBuf)>; 4] = [const { OnceLock::new() }; 4];
    BUILT[target.index()].get_or_init(|| {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/http_census.ignite");
        (
            todo::build(&source, "http_census", target, "censusd"),
            todo::build(&source, "http_census", target, "censusreplyd"),
        )
    })
}

/// The answer of the server `binary` to one request on its only
/// connection: its live tasks and workers, as `fixtures/http_census.ignite`
/// writes them.
fn census_of(binary: &Path) -> String {
    let dir = scratch("census");
    let mut command = Command::new(binary);
    command.args(["--port", "0"]);
    let (_server, port) = start(command, &dir);
    let mut client = Client::connect(port);
    let (status, body) = client.send(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
    assert_eq!(status, 200, "{body}");
    body
}

/// Row 3.8 `todo/get-1`: a server whose application has no `reply` holds
/// no request, so `serve_listener` starts no register of held requests
/// (`keep_holds`) and, while it answers its only connection, that
/// connection's task is its one task; a server with a `reply` keeps the
/// register as before, one task more.
#[test]
fn a_server_without_reply_starts_no_task_for_held_requests() {
    let mut failures = Vec::new();
    for target in [Target::Rust, Target::Llvm] {
        let (plain, replying) = census_servers(target);
        let handled = census_of(plain);
        let replied = census_of(replying);
        println!("{target:?}: handle: {handled}; reply: {replied}");
        let tasks = |answer: &str| {
            answer
                .strip_prefix("tasks ")
                .and_then(|rest| rest.split(' ').next())
                .and_then(|count| count.parse::<i64>().ok())
                .unwrap_or_else(|| panic!("{target:?}: not a census: {answer:?}"))
        };
        if tasks(&handled) != 1 {
            failures.push(format!(
                "{target:?}: a server without `reply` answers with {handled:?}, its \
                 connection's task alone expected"
            ));
        }
        if tasks(&replied) != 2 {
            failures.push(format!(
                "{target:?}: a server with `reply` answers with {replied:?}, its \
                 connection's task and the register of held requests expected"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
