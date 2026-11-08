//! `ignite-load check`: the functional check every program passes before it is timed, the
//! Ignite programs and their plain-Rust and Go references alike (`bench/native/`,
//! `docs/quality.md` row 3.3).
//!
//! ```text
//! ignite-load check --program todo|terminal --server PATH [--port P] [--workdir DIR]
//! ```
//!
//! The server is started as a run starts it (`--port P`, its working directory a fresh
//! `DIR`), checked over one connection, and killed. The todo check is the inline tests of
//! `examples/todo/todo.ignite` (`handle creates, lists, reads, updates and deletes`, `handle
//! answers 400, 404 and 405`) over real HTTP: each status, body and `Content-Type` exactly,
//! the `Allow` of a 405. The terminal check is the transcript of
//! `examples/terminal/commands.ignite` over TCP, every byte the server sends compared,
//! IAC GA after each prompt included.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use crate::http::{Client, request};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Program {
    Todo,
    Terminal,
}

impl Program {
    /// The program a workload drives.
    pub fn of(workload: &str) -> Option<Program> {
        match workload {
            "rr" | "pipe" | "connect" => Some(Program::Terminal),
            "get" | "patch" => Some(Program::Todo),
            _ => None,
        }
    }

    pub fn parse(name: &str) -> Option<Program> {
        match name {
            "todo" => Some(Program::Todo),
            "terminal" => Some(Program::Terminal),
            _ => None,
        }
    }
}

/// One request of the todo check and the response it must get.
pub struct Step {
    pub method: &'static str,
    pub path: &'static str,
    pub body: &'static str,
    pub status: u16,
    pub content_type: Option<&'static str>,
    pub allow: Option<&'static str>,
    pub expected: &'static str,
}

const JSON: Option<&str> = Some("application/json");
const PLAIN: Option<&str> = Some("text/plain; charset=utf-8");

const fn step(
    method: &'static str,
    path: &'static str,
    body: &'static str,
    status: u16,
    content_type: Option<&'static str>,
    expected: &'static str,
) -> Step {
    Step {
        method,
        path,
        body,
        status,
        content_type,
        allow: None,
        expected,
    }
}

/// The todo check, in order, on one fresh server.
pub fn todo_steps() -> Vec<Step> {
    let mut steps = vec![
        step("GET", "/todos", "", 200, JSON, "[]"),
        step(
            "POST",
            "/todos",
            r#"{"title":"say \"hi\""}"#,
            201,
            JSON,
            r#"{"id":1,"title":"say \"hi\"","done":false}"#,
        ),
        step(
            "POST",
            "/todos/",
            r#"{"title":"eggs","done":true}"#,
            201,
            JSON,
            r#"{"id":2,"title":"eggs","done":true}"#,
        ),
        step(
            "GET",
            "/todos",
            "",
            200,
            JSON,
            r#"[{"id":1,"title":"say \"hi\"","done":false},{"id":2,"title":"eggs","done":true}]"#,
        ),
        step(
            "GET",
            "/todos/2",
            "",
            200,
            JSON,
            r#"{"id":2,"title":"eggs","done":true}"#,
        ),
        step(
            "PATCH",
            "/todos/1",
            r#"{"done":true}"#,
            200,
            JSON,
            r#"{"id":1,"title":"say \"hi\"","done":true}"#,
        ),
        step(
            "PATCH",
            "/todos/1",
            r#"{"title":"tea","id":9}"#,
            200,
            JSON,
            r#"{"id":1,"title":"tea","done":true}"#,
        ),
        step("DELETE", "/todos/1", "", 204, None, ""),
        step("GET", "/todos/1", "", 404, PLAIN, "Not Found\n"),
        step("DELETE", "/todos/1", "", 404, PLAIN, "Not Found\n"),
        step(
            "GET",
            "/todos",
            "",
            200,
            JSON,
            r#"[{"id":2,"title":"eggs","done":true}]"#,
        ),
        step("POST", "/todos", "{not json", 400, PLAIN, "Bad Request\n"),
        step(
            "POST",
            "/todos",
            r#"{"done":true}"#,
            400,
            PLAIN,
            "Bad Request\n",
        ),
        step(
            "POST",
            "/todos",
            r#"{"title":7}"#,
            400,
            PLAIN,
            "Bad Request\n",
        ),
        step("PATCH", "/todos/9", "{}", 404, PLAIN, "Not Found\n"),
        step("PATCH", "/todos/9", "[1", 404, PLAIN, "Not Found\n"),
        step("GET", "/", "", 404, PLAIN, "Not Found\n"),
        step("GET", "/nothing", "", 404, PLAIN, "Not Found\n"),
        step("GET", "/todos/abc", "", 404, PLAIN, "Not Found\n"),
        step("GET", "/todos/1/2", "", 404, PLAIN, "Not Found\n"),
        step("PATCH", "/todos/2", "[1", 400, PLAIN, "Bad Request\n"),
        step("PUT", "/todos", "", 405, PLAIN, "Method Not Allowed\n"),
        step("POST", "/todos/2", "", 405, PLAIN, "Method Not Allowed\n"),
        step("GET", "/health", "", 200, PLAIN, "ok\n"),
        step(
            "POST",
            "/todos",
            r#"{"title":"x"}"#,
            201,
            JSON,
            r#"{"id":3,"title":"x","done":false}"#,
        ),
        step(
            "GET",
            "/todos",
            "",
            200,
            JSON,
            r#"[{"id":2,"title":"eggs","done":true},{"id":3,"title":"x","done":false}]"#,
        ),
    ];
    steps[21].allow = Some("GET, POST");
    steps[22].allow = Some("GET, PATCH, DELETE");
    steps
}

/// The terminal check: what the client sends and every byte the server answers; the first
/// is the prompt on connect, the last ends with the server's close.
pub fn terminal_steps() -> Vec<(&'static [u8], &'static [u8])> {
    vec![
        (b"", b"terminal> \xff\xf9"),
        (
            b"help\r\n",
            b"help\r\necho <text>\r\nstatus\r\nquit\r\nterminal> \xff\xf9",
        ),
        (b"echo hello\r\n", b"hello\r\nterminal> \xff\xf9"),
        (b"status\r\n", b"ready\r\nterminal> \xff\xf9"),
        (
            b"frobnicate x\r\n",
            b"unknown command: frobnicate\r\nterminal> \xff\xf9",
        ),
        (b"\r\n", b"terminal> \xff\xf9"),
        (b"echo  two  \r\n", b" two\r\nterminal> \xff\xf9"),
        (b"quit\r\n", b"bye\r\n"),
    ]
}

fn check_todo(address: SocketAddr) -> Result<(), String> {
    let mut client = Client::open(address).map_err(|e| format!("connect: {e}"))?;
    for step in todo_steps() {
        let name = format!("{} {} {}", step.method, step.path, step.body);
        let response = client
            .exchange(&request(step.method, step.path, step.body))
            .map_err(|e| format!("{name}: {e}"))?;
        let body = String::from_utf8_lossy(&response.body);
        let content_type = response.header("content-type");
        let allow = response.header("allow");
        if response.status != step.status
            || body != step.expected
            || content_type != step.content_type
            || (step.allow.is_some() && allow != step.allow)
        {
            return Err(format!(
                "{name}: expected {} {:?} {:?} allow {:?}, got {} {body:?} {content_type:?} allow {allow:?}",
                step.status, step.expected, step.content_type, step.allow, response.status
            ));
        }
    }
    Ok(())
}

fn check_terminal(address: SocketAddr) -> Result<(), String> {
    let mut socket = TcpStream::connect(address).map_err(|e| format!("connect: {e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let steps = terminal_steps();
    let last = steps.len() - 1;
    let mut buffer = [0u8; 4096];
    for (index, (send, expected)) in steps.into_iter().enumerate() {
        let name = if send.is_empty() {
            "connect".to_string()
        } else {
            String::from_utf8_lossy(send).trim_end().to_string()
        };
        socket
            .write_all(send)
            .map_err(|e| format!("{name}: write: {e}"))?;
        let mut got = Vec::new();
        while got.len() < expected.len() {
            match socket.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => got.extend_from_slice(&buffer[..n]),
                Err(e) => {
                    return Err(format!(
                        "{name}: read: {e}; got {:?}",
                        got.escape_ascii().to_string()
                    ));
                }
            }
        }
        if got != expected {
            return Err(format!(
                "{name}: expected {:?}, got {:?}",
                expected.escape_ascii().to_string(),
                got.escape_ascii().to_string()
            ));
        }
        if index == last {
            match socket.read(&mut buffer) {
                Ok(0) => {}
                Ok(n) => {
                    return Err(format!(
                        "{name}: data after the last answer: {:?}",
                        buffer[..n].escape_ascii().to_string()
                    ));
                }
                Err(e) => return Err(format!("{name}: no close: {e}")),
            }
        }
    }
    Ok(())
}

/// Check the server listening on `port` of 127.0.0.1.
pub fn run(program: Program, port: u16) -> Result<(), String> {
    let address: SocketAddr = ([127, 0, 0, 1], port).into();
    match program {
        Program::Todo => check_todo(address),
        Program::Terminal => check_terminal(address),
    }
}

/// `ignite-load check ...`: exit 0 when the server passes, 1 with the first difference.
pub fn main(args: impl Iterator<Item = String>) {
    let mut program = None;
    let mut server = None;
    let mut port: u16 = 2399;
    let mut workdir = None;
    let mut args = args;
    while let Some(flag) = args.next() {
        let mut value = || {
            args.next()
                .unwrap_or_else(|| crate::usage(&format!("{flag} needs a value")))
        };
        match flag.as_str() {
            "--program" => program = Some(value()),
            "--server" => server = Some(value()),
            "--port" => {
                port = value()
                    .parse()
                    .unwrap_or_else(|_| crate::usage("bad --port"))
            }
            "--workdir" => workdir = Some(value()),
            other => crate::usage(&format!("unknown argument {other}")),
        }
    }
    let program = program
        .as_deref()
        .and_then(Program::parse)
        .unwrap_or_else(|| crate::usage("--program todo|terminal is needed"));
    let server = server.unwrap_or_else(|| crate::usage("--server PATH is needed"));
    let mut started = crate::start_server(&server, port, workdir.as_deref());
    let result = run(program, port);
    started.stop();
    match result {
        Ok(()) => println!("{server}: {program:?} check passed"),
        Err(error) => {
            eprintln!("{server}: {program:?} check failed: {error}");
            std::process::exit(1);
        }
    }
}
