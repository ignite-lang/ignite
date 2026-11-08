//! Unit tests of the native-benchmark parts of `ignite-load`: CPU lists, the ABBA order of a
//! matrix, the HTTP client's response reader and both functional checks, each against a
//! scripted server on a loopback port.

use std::io::{Read, Write};
use std::net::TcpListener;

use crate::check::{self, Program};
use crate::http::{Response, parse_response};
use crate::matrix::order;
use crate::pin::parse_cpus;

#[test]
fn cpu_lists_parse() {
    assert_eq!(parse_cpus("0-3,8").unwrap(), vec![0, 1, 2, 3, 8]);
    assert_eq!(parse_cpus("5").unwrap(), vec![5]);
    assert!(parse_cpus("3-1").is_err());
    assert!(parse_cpus("x").is_err());
    assert!(parse_cpus("").is_err());
}

#[test]
fn passes_alternate_the_order_of_the_servers() {
    assert_eq!(order(1, 3), vec![0, 1, 2]);
    assert_eq!(order(2, 3), vec![2, 1, 0]);
    assert_eq!(order(3, 3), vec![0, 1, 2]);
}

#[test]
fn responses_are_read_by_their_length() {
    let text = b"HTTP/1.1 201 Created\r\ncontent-type: application/json\r\nContent-Length: 2\r\n\r\n{}HTTP/1.1";
    let (response, used) = parse_response(text).unwrap().unwrap();
    assert_eq!(
        response,
        Response {
            status: 201,
            headers: vec![
                ("content-type".to_string(), "application/json".to_string()),
                ("content-length".to_string(), "2".to_string()),
            ],
            body: b"{}".to_vec(),
        }
    );
    assert_eq!(used, text.len() - "HTTP/1.1".len());
    assert_eq!(response.header("Content-Type"), Some("application/json"));
    assert!(parse_response(&text[..20]).unwrap().is_none());
    assert!(parse_response(&text[..text.len() - 9]).unwrap().is_none());
    let empty = b"HTTP/1.1 204 No Content\r\ndate: x\r\n\r\n";
    let (response, used) = parse_response(empty).unwrap().unwrap();
    assert_eq!(
        (response.status, response.body.len(), used),
        (204, 0, empty.len())
    );
    assert!(parse_response(b"SSH-2.0\r\n\r\n").is_err());
}

/// A server that reads `requests` bytes per exchange and answers each with the next of
/// `answers`, for one connection.
fn scripted(answers: Vec<Vec<u8>>, todo: bool) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut answers = answers.into_iter().peekable();
        if !todo && let Some(first) = answers.next() {
            stream.write_all(&first).unwrap();
        }
        let mut buffer = [0u8; 4096];
        let mut pending = Vec::new();
        loop {
            let n = match stream.read(&mut buffer) {
                Ok(0) | Err(_) => return,
                Ok(n) => n,
            };
            pending.extend_from_slice(&buffer[..n]);
            while let Some(end) = complete(&pending, todo) {
                pending.drain(..end);
                match answers.next() {
                    Some(answer) => stream.write_all(&answer).unwrap(),
                    None => return,
                }
                if answers.peek().is_none() {
                    return;
                }
            }
        }
    });
    port
}

/// The length of the first whole request in `pending`: an HTTP request with its body, or a
/// terminal line.
fn complete(pending: &[u8], todo: bool) -> Option<usize> {
    if !todo {
        return pending.iter().position(|b| *b == b'\n').map(|i| i + 1);
    }
    let head = pending.windows(4).position(|w| w == b"\r\n\r\n")? + 4;
    let text = String::from_utf8_lossy(&pending[..head]).to_lowercase();
    let length = text
        .lines()
        .find_map(|l| l.strip_prefix("content-length: "))
        .map_or(0, |n| n.trim().parse().unwrap());
    (pending.len() >= head + length).then_some(head + length)
}

fn http_answer(
    status: u16,
    content_type: Option<&str>,
    extra: &[(&str, &str)],
    body: &str,
) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status} X\r\ncontent-length: {}\r\n", body.len());
    if let Some(content_type) = content_type {
        out.push_str(&format!("content-type: {content_type}\r\n"));
    }
    for (name, value) in extra {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str("\r\n");
    out.push_str(body);
    out.into_bytes()
}

fn todo_answers() -> Vec<Vec<u8>> {
    check::todo_steps()
        .iter()
        .map(|step| {
            let allow: Vec<(&str, &str)> = step.allow.iter().map(|a| ("allow", *a)).collect();
            http_answer(step.status, step.content_type, &allow, step.expected)
        })
        .collect()
}

#[test]
fn the_todo_check_passes_the_expected_answers_and_names_a_wrong_one() {
    let port = scripted(todo_answers(), true);
    check::run(Program::Todo, port).unwrap();
    let mut answers = todo_answers();
    answers[4] = http_answer(
        200,
        Some("application/json"),
        &[],
        r#"{"id":2,"title":"eggs","done":false}"#,
    );
    let port = scripted(answers, true);
    let error = check::run(Program::Todo, port).unwrap_err();
    assert!(error.contains("GET /todos/2"), "{error}");
}

fn terminal_answers() -> Vec<Vec<u8>> {
    check::terminal_steps()
        .iter()
        .map(|(_, answer)| answer.to_vec())
        .collect()
}

#[test]
fn the_terminal_check_passes_the_expected_bytes_and_names_a_wrong_one() {
    let port = scripted(terminal_answers(), false);
    check::run(Program::Terminal, port).unwrap();
    let mut answers = terminal_answers();
    answers[2] = b"hello\r\nterminal> \xff\xf8".to_vec();
    let port = scripted(answers, false);
    let error = check::run(Program::Terminal, port).unwrap_err();
    assert!(error.contains("echo hello"), "{error}");
}

#[test]
fn a_workload_names_its_program() {
    assert_eq!(Program::of("rr"), Some(Program::Terminal));
    assert_eq!(Program::of("connect"), Some(Program::Terminal));
    assert_eq!(Program::of("get"), Some(Program::Todo));
    assert_eq!(Program::of("patch"), Some(Program::Todo));
    assert_eq!(Program::of("nothing"), None);
}
