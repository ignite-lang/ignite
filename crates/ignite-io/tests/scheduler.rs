//! The IO mode `readiness` (`docs/readiness-io.md` 4.1, 4.3 to 4.6, phase
//! R1): spawned tasks run as stackful tasks on a few worker threads and a
//! task that waits parks itself, not its thread.
//!
//! The mode and the worker count are read once per process, so every case
//! runs in a child process: this test executable again, with the child
//! test of the case as the only one, `IGNITE_IO` and `IGNITE_WORKERS`
//! set as the case needs. A child test does nothing outside a child.

use std::path::Path;
use std::process::{Command, Output};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ignite_io::{Channel, Input, TcpListener, TcpStream, spawn, wait_for_tasks};

const LOCALHOST: &str = "127.0.0.1";
const LIMIT: Duration = Duration::from_secs(10);
/// Set in a child process.
const CHILD: &str = "IGNITE_SCHEDULER_CHILD";

/// Run the child test `name` of this executable in a child process with
/// `env`; its output.
fn child(name: &str, env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([name, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .env_remove("IGNITE_IO")
        .env_remove("IGNITE_WORKERS");
    for (key, value) in env {
        command.env(key, value);
    }
    command.output().unwrap()
}

fn in_child() -> bool {
    std::env::var_os(CHILD).is_some()
}

fn passed(output: &Output) {
    assert!(
        output.status.success(),
        "child failed: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the child ran no test:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

fn threads() -> usize {
    std::fs::read_dir(Path::new("/proc/self/task"))
        .unwrap()
        .count()
}

/// A connected pair: the substrate's end and a plain socket as its peer.
fn pair() -> (TcpStream, std::net::TcpStream) {
    let listener = TcpListener::bind(LOCALHOST.to_string(), 0).unwrap();
    let peer = std::net::TcpStream::connect((LOCALHOST, listener.local_port() as u16)).unwrap();
    peer.set_read_timeout(Some(LIMIT)).unwrap();
    (listener.accept().unwrap(), peer)
}

/// Wait until `done` holds, at most [`LIMIT`].
fn until(done: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + LIMIT;
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

/// Row 6 and the point of R1: 40 sessions waiting in a read take the
/// workers' threads (2 here), not one thread each; every session still
/// answers, and the workers end once the sessions ended.
#[test]
fn sessions_waiting_in_a_read_park_their_tasks_not_threads() {
    passed(&child(
        "scheduler::child_echo_sessions",
        &[("IGNITE_WORKERS", "2")],
    ));
}

/// The IO mode `threads` stays: one thread per waiting session.
#[test]
fn the_threads_mode_runs_one_thread_per_session() {
    passed(&child(
        "scheduler::child_echo_sessions",
        &[("IGNITE_IO", "threads"), ("IGNITE_WORKERS", "2")],
    ));
}

const SESSIONS: usize = 40;

#[test]
fn child_echo_sessions() {
    if !in_child() {
        return;
    }
    let readiness = std::env::var("IGNITE_IO").as_deref() != Ok("threads");
    let base = threads();
    let mut peers = Vec::new();
    for _ in 0..SESSIONS {
        let (stream, peer) = pair();
        spawn(move || {
            while let Ok(Input::Data(bytes)) = stream.read(64) {
                if stream.write_all(bytes).is_err() {
                    break;
                }
            }
        });
        peers.push(peer);
    }
    std::thread::sleep(Duration::from_millis(200));
    let running = threads();
    if readiness {
        assert!(
            running <= base + 2,
            "{running} threads with {SESSIONS} waiting sessions, {base} before"
        );
    } else {
        assert!(
            running >= base + SESSIONS,
            "{running} threads with {SESSIONS} waiting sessions, {base} before"
        );
    }
    for (index, peer) in peers.iter_mut().enumerate() {
        use std::io::{Read, Write};
        let line = format!("ping {index:03}");
        peer.write_all(line.as_bytes()).unwrap();
        let mut echoed = vec![0u8; line.len()];
        peer.read_exact(&mut echoed).unwrap();
        assert_eq!(String::from_utf8_lossy(&echoed), line);
    }
    drop(peers);
    wait_for_tasks();
    assert!(
        until(|| threads() <= base),
        "{} threads after the sessions ended, {base} before",
        threads()
    );
}

/// On one worker, a task waiting for room to send (its peer does not
/// read) holds the stream's write lock and parks; another task reading the
/// same stream still runs, and the writer finishes once the peer reads.
#[test]
fn a_task_waiting_to_send_does_not_stop_a_reader_on_its_worker() {
    passed(&child(
        "scheduler::child_writer_and_reader",
        &[("IGNITE_WORKERS", "1")],
    ));
}

#[test]
fn child_writer_and_reader() {
    use std::io::{Read, Write};
    if !in_child() {
        return;
    }
    const TOTAL: usize = 32 * 1024 * 1024;
    let (stream, mut peer) = pair();
    let writer = stream.clone();
    let (wrote, written) = mpsc::channel();
    spawn(move || {
        let chunk = vec![7u8; 64 * 1024];
        for _ in 0..TOTAL / chunk.len() {
            writer.write_from(&chunk).unwrap();
        }
        ignite_io::flush();
        wrote.send(()).unwrap();
    });
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        written.try_recv().is_err(),
        "the peer took {TOTAL} bytes unread"
    );
    let (read, got) = mpsc::channel();
    spawn(move || {
        let _ = read.send(stream.read(64));
    });
    peer.write_all(b"hi").unwrap();
    let got = got
        .recv_timeout(LIMIT)
        .expect("the reader ran while the writer waited for room");
    assert_eq!(got.unwrap(), Input::Data(b"hi".to_vec()));
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut received = 0;
    while received < TOTAL {
        let count = peer.read(&mut buffer).unwrap();
        assert!(count > 0, "end of stream after {received} bytes");
        received += count;
    }
    written
        .recv_timeout(LIMIT)
        .expect("the writer finished once the peer read");
    wait_for_tasks();
}

/// On one worker: a pause, a channel between two tasks, a receive with a
/// timeout, and a read's idle timeout all wait as timers or wait lists and
/// keep their meaning.
#[test]
fn timers_and_channels_work_between_tasks_on_one_worker() {
    passed(&child(
        "scheduler::child_timers_and_channels",
        &[("IGNITE_WORKERS", "1")],
    ));
}

#[test]
fn child_timers_and_channels() {
    if !in_child() {
        return;
    }
    let (results, collected) = mpsc::channel();
    let (sender, receiver) = Channel::bounded::<i64>(1);
    let paused = results.clone();
    spawn(move || {
        let start = Instant::now();
        ignite_io::clock::sleep(100_000_000);
        let slept = start.elapsed();
        sender.send(42);
        paused
            .send(format!("slept {}", slept >= Duration::from_millis(100)))
            .unwrap();
    });
    let received = results.clone();
    spawn(move || {
        let early = receiver.receive_within(10_000_000);
        let value = receiver.receive();
        received
            .send(format!("received {early:?} {value:?}"))
            .unwrap();
    });
    let (stream, _peer) = pair();
    let idle = results.clone();
    spawn(move || {
        stream.set_idle_timeout(150).unwrap();
        let start = Instant::now();
        let read = stream.read(16).unwrap();
        let waited = start.elapsed();
        idle.send(format!(
            "idle {:?} {} {}",
            read,
            stream.timed_out(),
            waited >= Duration::from_millis(140) && waited < LIMIT
        ))
        .unwrap();
    });
    let mut lines: Vec<String> = (0..3)
        .map(|_| collected.recv_timeout(LIMIT).expect("every task reported"))
        .collect();
    lines.sort();
    assert_eq!(
        lines,
        vec![
            "idle Eof true true".to_string(),
            "received None Some(42)".to_string(),
            "slept true".to_string(),
        ]
    );
    wait_for_tasks();
}

/// A task that overflows its stack hits its guard page: the process
/// prints Rust's overflow line and aborts, as a thread's overflow does.
#[test]
fn a_stack_overflow_in_a_task_is_reported_and_aborts() {
    use std::os::unix::process::ExitStatusExt;
    let output = child("scheduler::child_overflow", &[("IGNITE_WORKERS", "1")]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("has overflowed its stack"),
        "stderr:\n{stderr}"
    );
    assert_eq!(output.status.signal(), Some(6), "{:?}", output.status);
}

#[test]
fn child_overflow() {
    if !in_child() {
        return;
    }
    #[allow(unconditional_recursion)]
    fn deeper(depth: u64, done: &mpsc::Sender<u64>) -> u64 {
        let frame = std::hint::black_box([depth; 64]);
        if depth == u64::MAX {
            let _ = done.send(depth);
        }
        deeper(depth + 1, done) + frame[3]
    }
    let (done, finished) = mpsc::channel();
    spawn(move || {
        deeper(0, &done);
    });
    let _ = finished.recv_timeout(LIMIT);
    panic!("the overflow did not end the process");
}

/// The epoll loop is reached only through the reactor interface: no
/// source file of the substrate but `reactor.rs` names epoll's calls.
#[test]
fn no_epoll_call_outside_the_reactor() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().is_some_and(|name| name == "reactor.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (number, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if [
                "sys::epoll",
                "epoll_wait",
                "epoll_ctl",
                "epoll_create",
                "Epoll::",
            ]
            .iter()
            .any(|call| code.contains(call))
            {
                found.push(format!("{}:{}: {line}", path.display(), number + 1));
            }
        }
    }
    assert!(found.is_empty(), "epoll outside the reactor: {found:#?}");
}
