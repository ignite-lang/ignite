//! Slice 7 (`docs/slice.md` section 11.6): `examples/todo/todo.ignite`
//! built with `ignite::build` and driven over a real socket as an HTTP/1.1
//! client. Every case runs once per target (`<group>::rust::<case>`,
//! `<group>::llvm::<case>`). The inline tests of `std.text` and `std.json`
//! (`task test-ignite`) replace the probe program this file used to run.

use std::collections::HashSet;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{OnceLock, mpsc};
use std::time::Duration;

use ignite::Target;

use crate::common;

const TIMEOUT: Duration = Duration::from_secs(10);

/// `source` built once per target and test binary under the cargo target
/// directory; the executable named `name`.
fn build(source: &Path, out: &str, target: Target, name: &str) -> PathBuf {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let options = match target {
        Target::Rust => ignite::BuildOptions {
            target: Target::Rust,
            out: Some(tmp.join(out)),
            ..Default::default()
        },
        Target::Llvm => ignite::BuildOptions {
            target: Target::Llvm,
            out: Some(tmp.join(format!("{out}-llvm"))),
            runtime: Some(common::runtime_archive()),
            ..Default::default()
        },
    };
    let output = ignite::build(source, &options).unwrap_or_else(|e| panic!("{e}"));
    let executable = output
        .executables
        .iter()
        .find(|path| path.file_stem().and_then(|s| s.to_str()) == Some(name))
        .unwrap_or_else(|| panic!("no {name} in {:?}", output.executables))
        .clone();
    assert!(
        executable.is_file(),
        "{} was not built",
        executable.display()
    );
    executable
}

fn cached(
    cells: &'static [OnceLock<PathBuf>; 2],
    target: Target,
    make: fn(Target) -> PathBuf,
) -> &'static Path {
    let cell = match target {
        Target::Rust => &cells[0],
        Target::Llvm => &cells[1],
    };
    cell.get_or_init(|| make(target))
}

fn todod(target: Target) -> &'static Path {
    static BUILT: [OnceLock<PathBuf>; 2] = [OnceLock::new(), OnceLock::new()];
    cached(&BUILT, target, |target| {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/todo/todo.ignite");
        build(&source, "todo", target, "todod")
    })
}

/// A child process, killed on drop.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

// ---- the todo server --------------------------------------------------------

/// A running compiled `todod --port 0`.
struct Server {
    _process: Running,
    port: u16,
}

impl Server {
    fn start(target: Target) -> Server {
        let mut child = Command::new(todod(target))
            .args(["--port", "0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn todod");
        let stderr = child.stderr.take().expect("piped stderr");
        let process = Running(child);
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
            .recv_timeout(TIMEOUT)
            .expect("todod printed no `listening on 127.0.0.1:<port>` line")
            .expect("port in the listening line");
        Server {
            _process: process,
            port,
        }
    }

    fn connect(&self) -> Client {
        let stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to todod");
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        Client {
            stream,
            buffer: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// One keep-alive HTTP/1.1 connection.
struct Client {
    stream: TcpStream,
    buffer: Vec<u8>,
}

impl Client {
    fn send(&mut self, method: &str, path: &str, body: Option<&str>) -> Reply {
        let mut request = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n");
        if let Some(body) = body {
            request.push_str(&format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                body.len()
            ));
        }
        request.push_str("\r\n");
        request.push_str(body.unwrap_or(""));
        self.stream.write_all(request.as_bytes()).unwrap();
        self.reply()
    }

    fn fill(&mut self) -> bool {
        let mut chunk = [0u8; 65536];
        let n = self.stream.read(&mut chunk).expect("read from todod");
        self.buffer.extend_from_slice(&chunk[..n]);
        n > 0
    }

    fn reply(&mut self) -> Reply {
        let head_end = loop {
            if let Some(at) = self.buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                break at + 4;
            }
            assert!(
                self.fill(),
                "todod closed before a whole head: {:?}",
                self.buffer
            );
        };
        let head = String::from_utf8(self.buffer[..head_end - 4].to_vec()).unwrap();
        let mut lines = head.split("\r\n");
        let status_line = lines.next().unwrap();
        let status = status_line
            .strip_prefix("HTTP/1.1 ")
            .and_then(|rest| rest.get(..3))
            .and_then(|code| code.parse().ok())
            .unwrap_or_else(|| panic!("status line {status_line:?}"));
        let headers: Vec<(String, String)> = lines
            .map(|line| {
                let (name, value) = line.split_once(':').expect("header line");
                (name.to_string(), value.trim().to_string())
            })
            .collect();
        let length: usize = match headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        {
            Some((_, value)) => value.parse().expect("Content-Length"),
            None if status == 204 => 0,
            None => panic!("no Content-Length header in {head:?}"),
        };
        while self.buffer.len() < head_end + length {
            assert!(self.fill(), "todod closed inside a body");
        }
        let body = String::from_utf8(self.buffer[head_end..head_end + length].to_vec()).unwrap();
        self.buffer.drain(..head_end + length);
        Reply {
            status,
            headers,
            body,
        }
    }

    /// Whether the server closed the connection (after any reply was read).
    fn closed(&mut self) -> bool {
        let mut byte = [0u8; 1];
        match self.stream.read(&mut byte) {
            Ok(0) => true,
            Ok(_) => false,
            Err(e) if e.kind() == ErrorKind::ConnectionReset => true,
            Err(e) => panic!("read: {e}"),
        }
    }
}

/// The id of a todo in a JSON body that starts `{"id":<n>,`.
fn id_of(body: &str) -> i64 {
    body.strip_prefix(r#"{"id":"#)
        .and_then(|rest| rest.split(',').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no id in {body}"))
}

/// Create, list, read, update and delete over one connection.
fn crud_flow(target: Target) {
    let server = Server::start(target);
    let mut client = server.connect();
    let created = client.send("POST", "/todos", Some(r#"{"title":"milk"}"#));
    assert_eq!(created.status, 201, "{created:?}");
    assert_eq!(created.body, r#"{"id":1,"title":"milk","done":false}"#);
    assert_eq!(created.header("content-type"), Some("application/json"));
    let second = client.send("POST", "/todos", Some(r#"{"title":"eggs","done":true}"#));
    assert_eq!(second.body, r#"{"id":2,"title":"eggs","done":true}"#);
    let all = client.send("GET", "/todos", None);
    assert_eq!(all.status, 200);
    assert_eq!(
        all.body,
        r#"[{"id":1,"title":"milk","done":false},{"id":2,"title":"eggs","done":true}]"#
    );
    let one = client.send("GET", "/todos/1", None);
    assert_eq!(
        (one.status, one.body.as_str()),
        (200, created.body.as_str())
    );
    let patched = client.send("PATCH", "/todos/1", Some(r#"{"done":true}"#));
    assert_eq!(patched.status, 200);
    assert_eq!(patched.body, r#"{"id":1,"title":"milk","done":true}"#);
    let renamed = client.send("PATCH", "/todos/1", Some(r#"{"title":"oat milk"}"#));
    assert_eq!(renamed.body, r#"{"id":1,"title":"oat milk","done":true}"#);
    let deleted = client.send("DELETE", "/todos/1", None);
    assert_eq!((deleted.status, deleted.body.as_str()), (204, ""));
    assert_eq!(deleted.header("content-length"), None);
    assert_eq!(client.send("GET", "/todos/1", None).status, 404);
    assert_eq!(client.send("DELETE", "/todos/1", None).status, 404);
    let rest = client.send("GET", "/todos", None);
    assert_eq!(rest.body, r#"[{"id":2,"title":"eggs","done":true}]"#);
}

/// 400 for a malformed body or request, 404 for unknown targets and ids,
/// 405 with `Allow` for a method a target does not support.
fn error_statuses(target: Target) {
    let server = Server::start(target);
    let mut client = server.connect();
    assert_eq!(client.send("POST", "/todos", Some("{not json")).status, 400);
    assert_eq!(
        client
            .send("POST", "/todos", Some(r#"{"done":true}"#))
            .status,
        400
    );
    assert_eq!(
        client.send("POST", "/todos", Some(r#"{"title":7}"#)).status,
        400
    );
    assert_eq!(client.send("PATCH", "/todos/1", Some("{}")).status, 404);
    assert_eq!(client.send("GET", "/nothing", None).status, 404);
    assert_eq!(client.send("GET", "/todos/abc", None).status, 404);
    assert_eq!(client.send("GET", "/todos/1/2", None).status, 404);
    let put = client.send("PUT", "/todos", Some("{}"));
    assert_eq!(put.status, 405);
    assert_eq!(put.header("allow"), Some("GET, POST"));
    let post = client.send("POST", "/todos/1", Some("{}"));
    assert_eq!(post.status, 405);
    assert_eq!(post.header("allow"), Some("GET, PATCH, DELETE"));
    let created = client.send("POST", "/todos", Some(r#"{"title":"x"}"#));
    assert_eq!(created.status, 201, "errors did not break the connection");
    let patched = client.send("PATCH", "/todos/1", Some("[1"));
    assert_eq!(patched.status, 400);

    let mut garbage = server.connect();
    garbage.stream.write_all(b"NONSENSE\r\n\r\n").unwrap();
    assert_eq!(garbage.reply().status, 400);
    assert!(garbage.closed());
}

/// A body over 1 MiB is 413 and a head over 8 KiB is 431; both close.
fn limits(target: Target) {
    let server = Server::start(target);
    let mut large = server.connect();
    large
        .stream
        .write_all(b"POST /todos HTTP/1.1\r\nHost: x\r\nContent-Length: 1048577\r\n\r\n")
        .unwrap();
    assert_eq!(large.reply().status, 413);
    assert!(large.closed());

    let mut long = server.connect();
    let filler = "a".repeat(9000);
    long.stream
        .write_all(format!("GET /todos HTTP/1.1\r\nX-Filler: {filler}\r\n\r\n").as_bytes())
        .unwrap();
    assert_eq!(long.reply().status, 431);
    assert!(long.closed());

    let mut exact = server.connect();
    let body = format!(r#"{{"title":"{}"}}"#, "b".repeat(1024));
    assert_eq!(exact.send("POST", "/todos", Some(&body)).status, 201);
}

/// Three requests on one connection, the last with `Connection: close`.
fn keep_alive(target: Target) {
    let server = Server::start(target);
    let mut client = server.connect();
    assert_eq!(client.send("GET", "/todos", None).body, "[]");
    let created = client.send("POST", "/todos", Some(r#"{"title":"tea"}"#));
    assert_eq!(created.header("connection"), Some("keep-alive"));
    client
        .stream
        .write_all(b"GET /todos/1 HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    let last = client.reply();
    assert_eq!(last.status, 200);
    assert_eq!(last.header("connection"), Some("close"));
    assert!(client.closed());
}

/// 16 connections, 100 POSTs each, at the same time: 1600 distinct ids.
fn concurrent_posts(target: Target) {
    let server = Server::start(target);
    let workers: Vec<_> = (0..16)
        .map(|worker| {
            let mut client = server.connect();
            std::thread::spawn(move || {
                (0..100)
                    .map(|i| {
                        let body = format!(r#"{{"title":"w{worker} n{i}"}}"#);
                        let reply = client.send("POST", "/todos", Some(&body));
                        assert_eq!(reply.status, 201, "{reply:?}");
                        id_of(&reply.body)
                    })
                    .collect::<Vec<i64>>()
            })
        })
        .collect();
    let ids: HashSet<i64> = workers
        .into_iter()
        .flat_map(|worker| worker.join().expect("worker"))
        .collect();
    assert_eq!(ids.len(), 1600);
    assert_eq!(ids, (1..=1600).collect());
    let mut client = server.connect();
    let all = client.send("GET", "/todos", None);
    assert_eq!(all.body.matches(r#""id":"#).count(), 1600);
    client.stream.shutdown(Shutdown::Both).unwrap();
}

/// Two connections PATCH different fields of one todo at the same time,
/// 300 rounds behind a barrier; after each round both changes are there.
/// A read-modify-write outside `Store.update` loses one of them.
fn concurrent_patches(target: Target) {
    const ROUNDS: usize = 300;
    let server = Server::start(target);
    let mut client = server.connect();
    let created = client.send("POST", "/todos", Some(r#"{"title":"t"}"#));
    assert_eq!(created.status, 201, "{created:?}");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let worker = |field: &'static str| {
        let mut client = server.connect();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            let mut lost = Vec::new();
            for round in 0..ROUNDS {
                barrier.wait();
                let body = match field {
                    "title" => format!(r#"{{"title":"t{round}"}}"#),
                    _ => format!(r#"{{"done":{}}}"#, round % 2 == 1),
                };
                let reply = client.send("PATCH", "/todos/1", Some(&body));
                if reply.status != 200 {
                    lost.push(format!("round {round}: {reply:?}"));
                }
                barrier.wait();
                if field == "title" {
                    let now = client.send("GET", "/todos/1", None);
                    let expected =
                        format!(r#"{{"id":1,"title":"t{round}","done":{}}}"#, round % 2 == 1);
                    if now.body != expected {
                        lost.push(format!("round {round}: {} instead of {expected}", now.body));
                    }
                }
            }
            lost
        })
    };
    let title = worker("title");
    let done = worker("done");
    let mut lost = title.join().expect("title worker");
    lost.extend(done.join().expect("done worker"));
    assert!(lost.is_empty(), "{} lost updates: {lost:#?}", lost.len());
}

/// `raw` on a new connection, with the write half shut down after it when
/// `half_close`: the status of the reply, after which the
/// server has closed the connection.
fn rejected(server: &Server, raw: &[u8], half_close: bool) -> u16 {
    let mut client = server.connect();
    client.stream.write_all(raw).unwrap();
    if half_close {
        client.stream.shutdown(Shutdown::Write).unwrap();
    }
    let reply = client.reply();
    assert!(client.closed(), "still open after {reply:?}");
    reply.status
}

/// Heads a proxy could frame differently are 400; any
/// `Transfer-Encoding` is 501, never framed by `Content-Length`.
fn header_framing(target: Target) {
    let server = Server::start(target);
    let cases: [(&[u8], u16); 7] = [
        (
            b"POST /todos HTTP/1.1\r\nTransfer-Encoding\t: chunked\r\nContent-Length: 3\r\n\r\nabc",
            400,
        ),
        (
            b"POST /todos HTTP/1.1\r\nTransfer-Encoding : chunked\r\nContent-Length: 3\r\n\r\nabc",
            400,
        ),
        (
            b"GET /todos HTTP/1.1\r\nHost: x\r\n\tTransfer-Encoding: chunked\r\n\r\n",
            400,
        ),
        (
            b"GET /todos HTTP/1.1\r\n Host: x\r\n\r\n",
            400,
        ),
        (
            b"POST /todos HTTP/1.1\r\nX: a\nTransfer-Encoding: chunked\r\nContent-Length: 3\r\n\r\nabc",
            400,
        ),
        (b"GET /todos HTTP/1.1\nHost: x\r\n\r\n", 400),
        (
            b"POST /todos HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
            501,
        ),
    ];
    for (raw, status) in cases {
        assert_eq!(
            rejected(&server, raw, true),
            status,
            "{}",
            raw.escape_ascii()
        );
    }
}

/// A repeated `Content-Length`, or one that is not 1 to 18 digits, is 400.
fn content_length(target: Target) {
    let server = Server::start(target);
    let body = r#"{"title":"x"}"#;
    for lengths in [
        vec!["13", "13"],
        vec!["13", "5"],
        vec!["+13"],
        vec!["-1"],
        vec!["13x"],
        vec![" "],
        vec!["0x0d"],
        vec!["0000000000000000013"],
    ] {
        let mut raw = String::from("POST /todos HTTP/1.1\r\nHost: x\r\n");
        for length in &lengths {
            raw.push_str(&format!("Content-Length: {length}\r\n"));
        }
        raw.push_str("\r\n");
        raw.push_str(body);
        assert_eq!(rejected(&server, raw.as_bytes(), true), 400, "{lengths:?}");
    }
    let mut client = server.connect();
    let created = client.send("POST", "/todos", Some(body));
    assert_eq!(created.status, 201, "a valid request still works");
}

/// A rejected request with unread input behind it: the client reads the
/// whole reply (lingering close), not a reset.
fn rejection_is_read(target: Target) {
    let server = Server::start(target);
    let mut raw = b"POST /todos HTTP/1.1\r\nHost: x\r\nContent-Length: 2000000\r\n\r\n".to_vec();
    raw.extend(std::iter::repeat_n(b'a', 48 * 1024));
    assert_eq!(rejected(&server, &raw, false), 413);
    let mut bad = b"NONSENSE\r\n\r\n".to_vec();
    bad.extend(std::iter::repeat_n(b'b', 32 * 1024));
    assert_eq!(rejected(&server, &bad, false), 400);
    let mut long = format!("GET /todos HTTP/1.1\r\nX-Filler: {}", "c".repeat(9000)).into_bytes();
    long.extend(std::iter::repeat_n(b'c', 32 * 1024));
    long.extend(b"\r\n\r\n");
    assert_eq!(rejected(&server, &long, false), 431);
}

macro_rules! both_targets {
    ($group:ident; $($case:ident),* $(,)?) => {
        mod $group {
            mod rust {
                $(#[test]
                fn $case() {
                    super::super::$case(ignite::Target::Rust);
                })*
            }
            mod llvm {
                $(#[test]
                fn $case() {
                    super::super::$case(ignite::Target::Llvm);
                })*
            }
        }
    };
}

both_targets!(
    todo;
    crud_flow,
    error_statuses,
    limits,
    keep_alive,
    concurrent_posts,
    concurrent_patches,
    header_framing,
    content_length,
    rejection_is_read,
);
