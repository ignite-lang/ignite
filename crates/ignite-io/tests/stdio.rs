//! `Stdin`, `Stdout` and `run` in a child process.
//!
//! The test binary runs itself again with `IGNITE_IO_CHILD` naming a
//! scenario; the scenario test does the work only in the child, where the
//! parent controls standard input and output.

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use ignite_io::{Input, IoError, Stdin, Stdout};

const CHILD: &str = "IGNITE_IO_CHILD";

/// The harness name of `test` in this file: its path without the crate
/// name (`stdio::child_x` when the file is a module of `tests/main.rs`).
fn harness_name(test: &str) -> String {
    match module_path!().split_once("::") {
        Some((_, module)) => format!("{module}::{test}"),
        None => test.to_string(),
    }
}

fn scenario() -> Option<String> {
    std::env::var(CHILD).ok()
}

/// This test binary, running only `test` with `IGNITE_IO_CHILD=test`.
fn child(test: &str) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args([
            harness_name(test).as_str(),
            "--exact",
            "--test-threads=1",
            "--nocapture",
        ])
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
    let status = ignite_io::run(|| {
        ignite_io::spawn(|| {
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
    let status = ignite_io::run(|| {
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

/// Wait up to `limit` for the child's exit code, stdout and stderr; kill it
/// and panic when it is still running. Neither pipe is read before the
/// child exits.
fn finish(mut child: Child, limit: Duration) -> (Option<i32>, String, String) {
    let started = Instant::now();
    drop(child.stdin.take());
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let mut stdout = String::new();
            let mut stderr = String::new();
            let _ = child.stdout.take().unwrap().read_to_string(&mut stdout);
            let _ = child.stderr.take().unwrap().read_to_string(&mut stderr);
            return (status.code(), stdout, stderr);
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
fn child_failing_tasks() {
    if scenario().as_deref() != Some("child_failing_tasks") {
        return;
    }
    let status = ignite_io::run(|| {
        let (sender, receiver) = std::sync::mpsc::channel();
        let done = sender.clone();
        ignite_io::spawn(move || {
            ignite_io::spawn(move || {
                done.send(()).unwrap();
                ignite_io::fail(
                    "index",
                    "t.ignite",
                    1,
                    2,
                    "index 3 out of range for length 0",
                )
            });
            let _ = sender.send(());
            panic!("a Rust panic in a task");
        });
        receiver.recv().unwrap();
        receiver.recv().unwrap();
        std::thread::sleep(Duration::from_millis(200));
        Stdout::write_all(b"still alive\n".to_vec())
    });
    std::process::exit(status)
}

#[test]
fn a_failing_task_is_logged_and_the_process_continues() {
    if scenario().is_some() {
        return;
    }
    let (code, stdout, stderr) = finish(child("child_failing_tasks"), Duration::from_secs(20));
    assert_eq!(code, Some(0), "{stdout}\n{stderr}");
    assert!(stdout.contains("still alive\n"), "{stdout}");
    assert!(
        stderr.contains(
            "task ended with error: index failed at t.ignite:1:2: index 3 out of range for length 0\n"
        ),
        "{stderr}"
    );
    let panic = stderr
        .lines()
        .find(|line| line.starts_with("task ended with error: panic at "))
        .unwrap_or_else(|| panic!("{stderr}"));
    assert!(panic.contains("stdio.rs:"), "{panic}");
    assert!(panic.ends_with(": a Rust panic in a task"), "{panic}");
}

#[test]
fn child_main_fails() {
    if scenario().as_deref() != Some("child_main_fails") {
        return;
    }
    let status = ignite_io::run(|| {
        Stdout::write_all(b"before\n".to_vec())?;
        ignite_io::fail("+", "t.ignite", 4, 9, "9223372036854775807 + 1")
    });
    std::process::exit(status)
}

#[test]
fn a_failure_in_main_exits_101_with_its_report() {
    if scenario().is_some() {
        return;
    }
    let (code, stdout, stderr) = finish(child("child_main_fails"), Duration::from_secs(20));
    assert_eq!(
        code,
        Some(ignite_io::FAILURE_EXIT_CODE),
        "{stdout}\n{stderr}"
    );
    assert!(stdout.contains("before\n"), "{stdout}");
    assert!(
        stderr.contains("error: + failed at t.ignite:4:9: 9223372036854775807 + 1\n"),
        "{stderr}"
    );
    assert!(!stderr.contains("task ended"), "{stderr}");
}

#[test]
fn child_exit_with_a_stalled_writer() {
    if scenario().as_deref() != Some("child_exit_with_a_stalled_writer") {
        return;
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    ignite_io::spawn(move || {
        sender.send(()).unwrap();
        for _ in 0..64 {
            Stdout::write_all(vec![b'x'; 64 * 1024]).unwrap();
        }
    });
    receiver.recv().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    ignite_io::exit(7)
}

#[test]
fn exit_returns_while_a_stdout_write_is_stalled() {
    if scenario().is_some() {
        return;
    }
    let (code, _, stderr) = finish(
        child("child_exit_with_a_stalled_writer"),
        Duration::from_secs(20),
    );
    assert_eq!(code, Some(7), "{stderr}");
}
