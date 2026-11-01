//! `Stdin`, `Stdout` and `run` in a child process.
//!
//! The test binary runs itself again with `TELOS_IO_CHILD` naming a
//! scenario; the scenario test does the work only in the child, where the
//! parent controls standard input and output.

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use telos_io::{Input, IoError, Stdin, Stdout};

const CHILD: &str = "TELOS_IO_CHILD";

fn scenario() -> Option<String> {
    std::env::var(CHILD).ok()
}

/// This test binary, running only `test` with `TELOS_IO_CHILD=test`.
fn child(test: &str) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args([test, "--exact", "--test-threads=1", "--nocapture"])
        .env(CHILD, test)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

/// Wait up to `limit`; kill the child and panic when it is still running.
fn wait(mut child: Child, limit: Duration) -> (bool, String) {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let mut stdout = String::new();
            child
                .stdout
                .take()
                .unwrap()
                .read_to_string(&mut stdout)
                .unwrap();
            return (status.success(), stdout);
        }
        if started.elapsed() > limit {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("child still running after {limit:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn child_pending_stdin_read() {
    if scenario().as_deref() != Some("child_pending_stdin_read") {
        return;
    }
    let status = telos_io::run(|| {
        telos_io::spawn(|| {
            let _ = Stdin::read(16);
        });
        std::thread::sleep(Duration::from_millis(100));
        Ok(())
    });
    assert_eq!(status, 0);
}

#[test]
fn run_returns_while_a_stdin_read_is_pending() {
    if scenario().is_some() {
        return;
    }
    let mut process = child("child_pending_stdin_read");
    let stdin = process.stdin.take();
    let (success, _) = wait(process, Duration::from_secs(20));
    drop(stdin);
    assert!(success);
}

#[test]
fn child_echo() {
    if scenario().as_deref() != Some("child_echo") {
        return;
    }
    let status = telos_io::run(|| {
        loop {
            match Stdin::read(3)? {
                Input::Data(bytes) => {
                    if bytes.len() > 3 {
                        return Err(IoError::new("read exceeded maximum"));
                    }
                    let mut framed = b"[".to_vec();
                    framed.extend(bytes);
                    framed.push(b']');
                    Stdout::write_all(framed)?;
                }
                Input::Eof => {
                    Stdout::write_all(b"<eof>".to_vec())?;
                    return Ok(());
                }
            }
        }
    });
    assert_eq!(status, 0);
}

#[test]
fn stdin_reads_up_to_maximum_and_ends_with_eof() {
    if scenario().is_some() {
        return;
    }
    let mut process = child("child_echo");
    let mut stdin = process.stdin.take().unwrap();
    stdin.write_all(b"abcdefg").unwrap();
    drop(stdin);
    let (success, stdout) = wait(process, Duration::from_secs(20));
    assert!(success, "{stdout}");
    let start = stdout.find('[').expect("no framed output");
    let end = stdout.find("<eof>").expect("no end of file");
    let framed = &stdout[start..end];
    let joined: String = framed
        .split(['[', ']'])
        .filter(|chunk| !chunk.is_empty())
        .collect();
    assert_eq!(joined, "abcdefg", "{stdout}");
}
