//! The IO mode `readiness` (`docs/readiness-io.md` 4.1, 4.3 to 4.6, phase
//! R1): spawned tasks run as stackful tasks on a few worker threads and a
//! task that waits parks itself, not its thread.
//!
//! The mode and the worker count are read once per process, so every case
//! runs in a child process: this test executable again, with the child
//! test of the case as the only one, `IGNITE_IO` and `IGNITE_WORKERS`
//! set as the case needs. A child test does nothing outside a child.

use std::collections::BTreeMap;
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

/// A task spawned by a task runs on its worker, with no hand-over to
/// another thread, while that worker is not busier than the others by
/// more than one task; a task that spawns many still spreads them over
/// the workers.
#[test]
fn a_task_spawned_by_a_task_stays_on_its_worker_until_it_is_busier() {
    passed(&child(
        "scheduler::child_placement",
        &[("IGNITE_WORKERS", "4")],
    ));
}

#[test]
fn child_placement() {
    if !in_child() {
        return;
    }
    let (report, reports) = mpsc::channel();
    spawn(move || {
        let parent = std::thread::current().id();
        let (tell, told) = Channel::bounded::<bool>(1);
        spawn(move || {
            tell.send(std::thread::current().id() == parent);
        });
        let paired = told.receive();
        let (each, all) = Channel::bounded::<String>(8);
        for _ in 0..8 {
            let each = each.clone();
            spawn(move || {
                ignite_io::clock::sleep(200_000_000);
                each.send(format!("{:?}", std::thread::current().id()));
            });
        }
        drop(each);
        let mut threads: Vec<String> = (0..8).filter_map(|_| all.receive()).collect();
        threads.sort();
        threads.dedup();
        report.send((paired, threads.len())).unwrap();
    });
    let (paired, spread) = reports.recv_timeout(LIMIT).expect("the parent reported");
    assert_eq!(
        paired,
        Some(true),
        "the spawned task ran on its parent's worker"
    );
    assert!(spread > 1, "8 sleeping tasks ran on {spread} worker(s)");
    wait_for_tasks();
}

/// A read after a read that emptied the socket waits for readiness first
/// (`Stream::emptied`): it still returns data that arrived before it
/// began, data that arrives while it waits, and the rest of a read that
/// filled its buffer; with the task alone on its worker and with another
/// task parked there.
#[test]
fn reads_after_an_emptying_read_return_every_byte() {
    passed(&child(
        "scheduler::child_emptying_reads",
        &[("IGNITE_WORKERS", "1")],
    ));
}

#[test]
fn child_emptying_reads() {
    use std::io::Write;
    if !in_child() {
        return;
    }
    for company in [false, true] {
        let (stream, mut peer) = pair();
        let (go, gone) = Channel::bounded::<bool>(1);
        let (read, got) = mpsc::channel();
        let (stay, stayed) = Channel::bounded::<bool>(1);
        if company {
            spawn(move || {
                stayed.receive();
            });
        }
        spawn(move || {
            while gone.receive() == Some(true) {
                let _ = read.send(stream.read(64).unwrap());
            }
        });
        let next = |expected: &[u8]| {
            go.send(true);
            assert_eq!(
                got.recv_timeout(LIMIT).expect("the read returned"),
                Input::Data(expected.to_vec())
            );
        };
        peer.write_all(b"one").unwrap();
        next(b"one");
        peer.write_all(b"two").unwrap();
        std::thread::sleep(Duration::from_millis(50));
        next(b"two");
        let late = peer.try_clone().unwrap();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            (&late).write_all(b"three").unwrap();
        });
        next(b"three");
        writer.join().unwrap();
        peer.write_all(&[b'x'; 67]).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        next(&[b'x'; 64]);
        next(b"xxx");
        go.send(false);
        stay.send(true);
    }
    wait_for_tasks();
}

/// The only task of a worker may block it in `recv` (`sched::block_alone`):
/// a task spawned meanwhile runs at once on another worker, not after that
/// read, and the read still ends at its idle timeout. With one worker the
/// task never blocks it, so the spawned task runs on the same worker.
#[test]
fn a_task_spawned_while_a_lone_task_blocks_in_a_read_runs_at_once() {
    passed(&child(
        "scheduler::child_lone_blocking_read",
        &[("IGNITE_WORKERS", "2")],
    ));
    passed(&child(
        "scheduler::child_lone_blocking_read",
        &[("IGNITE_WORKERS", "1")],
    ));
}

#[test]
fn child_lone_blocking_read() {
    use std::io::Write;
    if !in_child() {
        return;
    }
    let workers: usize = std::env::var("IGNITE_WORKERS").unwrap().parse().unwrap();
    for round in 0..2 {
        let (stream, mut peer) = pair();
        let (report, reports) = mpsc::channel();
        let reader = report.clone();
        spawn(move || {
            stream.set_idle_timeout(400).unwrap();
            let first = stream.read(16).unwrap();
            let start = Instant::now();
            let second = stream.read(16).unwrap();
            reader
                .send(format!(
                    "reader {:?} {first:?} {second:?} {} {}",
                    std::thread::current().id(),
                    stream.timed_out(),
                    start.elapsed() >= Duration::from_millis(390)
                ))
                .unwrap();
        });
        if round == 0 {
            peer.write_all(b"go").unwrap();
        }
        std::thread::sleep(Duration::from_millis(100));
        let started = Instant::now();
        spawn(move || {
            report
                .send(format!("spawned {:?}", std::thread::current().id()))
                .unwrap();
        });
        let spawned = reports.recv_timeout(LIMIT).expect("the spawned task ran");
        let waited = started.elapsed();
        assert!(spawned.starts_with("spawned"), "{spawned}");
        let reader = reports.recv_timeout(LIMIT).expect("the reader ended");
        if round == 0 {
            assert!(
                waited < Duration::from_millis(250),
                "the spawned task waited {waited:?}"
            );
            assert!(
                reader.ends_with("Data([103, 111]) Eof true true"),
                "{reader}"
            );
        }
        let spawned_on = spawned.trim_start_matches("spawned ");
        assert_eq!(
            reader.contains(spawned_on),
            workers == 1,
            "{reader} / {spawned}"
        );
        drop(peer);
        wait_for_tasks();
    }
}

/// A lone task blocks its worker in `recv` only while fewer tasks live
/// than workers may run: once a second task lives on the other worker, the
/// reader's next read waits through the reactor, its worker takes spawned
/// tasks again, and of two tasks spawned then one runs beside the reader.
#[test]
fn a_worker_whose_task_blocked_once_takes_tasks_again() {
    passed(&child(
        "scheduler::child_blocked_worker_rejoins",
        &[("IGNITE_WORKERS", "2")],
    ));
}

#[test]
fn child_blocked_worker_rejoins() {
    use std::io::Write;
    if !in_child() {
        return;
    }
    let (stream, mut peer) = pair();
    let (report, reports) = mpsc::channel();
    let reader = report.clone();
    spawn(move || {
        while let Ok(Input::Data(_)) = stream.read(16) {
            reader
                .send(format!("{:?}", std::thread::current().id()))
                .unwrap();
        }
    });
    std::thread::sleep(Duration::from_millis(100));
    let mut holds = Vec::new();
    let mut hold = |report: mpsc::Sender<String>| {
        let (keep, kept) = Channel::bounded::<bool>(1);
        holds.push(keep);
        spawn(move || {
            report
                .send(format!("{:?}", std::thread::current().id()))
                .unwrap();
            kept.receive();
        });
    };
    hold(report.clone());
    let first = reports.recv_timeout(LIMIT).expect("the first task ran");
    peer.write_all(b"one").unwrap();
    let read_on = reports.recv_timeout(LIMIT).expect("the reader read");
    assert_ne!(first, read_on, "the first task passed the blocked worker");
    std::thread::sleep(Duration::from_millis(100));
    hold(report.clone());
    hold(report);
    let later = [
        reports.recv_timeout(LIMIT).expect("a later task ran"),
        reports.recv_timeout(LIMIT).expect("a later task ran"),
    ];
    assert!(
        later.contains(&read_on),
        "no later task ran beside the reader on {read_on}: {later:?}"
    );
    drop(holds);
    drop(peer);
    wait_for_tasks();
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

/// Clients of the connect-8 shape (`ignite-load`'s workload `connect`):
/// each connects, sends one request, reads the reply and closes, again.
const CONNECT_CLIENTS: usize = 8;
/// Connections over all clients.
const CONNECTIONS: usize = 256;

/// The connection path's cost as counts (`story:readiness-connect-latency`):
/// an accept loop that spawns one task per connection, served to 8
/// concurrent clients that each connect, exchange one request and reply and
/// close, with 4 workers; the accept loop a task, and on the process' main
/// thread as a compiled server runs it.
#[test]
fn short_connections_take_stacks_from_the_worker_they_run_on() {
    for accept in ["task", "main"] {
        passed(&child(
            "scheduler::child_connect_8",
            &[("IGNITE_WORKERS", "4"), ("SCHEDULER_ACCEPT", accept)],
        ));
    }
}

#[test]
fn child_connect_8() {
    use std::io::{Read, Write};
    if !in_child() {
        return;
    }
    let on_task = std::env::var("SCHEDULER_ACCEPT").as_deref() == Ok("task");
    let listener = TcpListener::bind(LOCALHOST.to_string(), 0).unwrap();
    let port = listener.local_port() as u16;
    let accept_loop = move || {
        for _ in 0..CONNECTIONS {
            let Ok(stream) = listener.accept() else {
                return;
            };
            spawn(move || {
                if let Ok(Input::Data(bytes)) = stream.read(64) {
                    let _ = stream.write_all(bytes);
                }
            });
        }
    };
    let clients: Vec<_> = (0..CONNECT_CLIENTS)
        .map(|_| {
            std::thread::spawn(move || {
                for _ in 0..CONNECTIONS / CONNECT_CLIENTS {
                    let mut peer = std::net::TcpStream::connect((LOCALHOST, port)).unwrap();
                    peer.set_read_timeout(Some(LIMIT)).unwrap();
                    peer.write_all(b"ping").unwrap();
                    let mut reply = [0u8; 4];
                    peer.read_exact(&mut reply).unwrap();
                    assert_eq!(&reply, b"ping");
                }
            })
        })
        .collect();
    if on_task {
        spawn(accept_loop);
    } else {
        accept_loop();
    }
    for client in clients {
        client.join().unwrap();
    }
    wait_for_tasks();
    let counts = ignite_io::scheduler_counts();
    let accept = if on_task { "task" } else { "main" };
    println!("accept on {accept}: {counts:?}");
    let connections = CONNECTIONS as u64;
    assert_eq!(
        counts.spawns,
        connections + u64::from(on_task),
        "{counts:?}"
    );
    assert_eq!(counts.wakes_across, 0, "{counts:?}");
    // Stacks: a few mapped while the first connections overlap, then every
    // connection reuses one.
    assert!(
        counts.stacks_allocated <= 2 * CONNECT_CLIENTS as u64,
        "stacks mapped: {counts:?}"
    );
    assert_eq!(
        counts.stacks_allocated + counts.stacks_from_cache + counts.stacks_from_pool,
        counts.spawns,
        "{counts:?}"
    );
    // A connection served on a worker takes the stack a task left there
    // and leaves its own there: the shared pool's lock is taken for at
    // most one connection in eight, either way (before the per-worker
    // caches, 150 to 248 of 256 took and gave through the pool).
    let bound = connections / 8;
    assert!(
        counts.stacks_from_pool <= bound && counts.stacks_to_pool <= bound,
        "the shared pool's lock per connection: {counts:?}"
    );
    // Hand-overs: at most one per connection.
    assert!(counts.spawns_handed_over <= connections, "{counts:?}");
    assert!(
        counts.notifies_sent <= counts.spawns_handed_over,
        "{counts:?}"
    );
}

/// In-process clients of an in-process echo server, every one a task
/// sharing workers with the others: each is answered. An end of stream
/// that arrives with the reply raises no readiness edge of its own, so a
/// client that took its short read of the reply for an emptied socket
/// waited for one until its idle timeout (`answered 0` of 50 on one
/// worker, 2 to 7 on four, about 30 on twenty).
#[test]
fn in_process_clients_sharing_workers_are_each_answered() {
    for workers in ["1", "4"] {
        passed(&child(
            "scheduler::child_clients",
            &[("IGNITE_WORKERS", workers), ("SCHEDULER_CLIENTS", "50")],
        ));
    }
}

/// An accept loop task spawns one task per connection that reads once,
/// writes the bytes back and closes; each client task connects, writes
/// four bytes, reads until end of stream and reports on a channel that
/// the process' main thread receives within [`LIMIT`] per report.
#[test]
fn child_clients() {
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    if !in_child() {
        return;
    }
    static ACCEPTED: AtomicUsize = AtomicUsize::new(0);
    static SERVED: AtomicUsize = AtomicUsize::new(0);
    static CONNECTED: AtomicUsize = AtomicUsize::new(0);
    let count: usize = std::env::var("SCHEDULER_CLIENTS").unwrap().parse().unwrap();
    let listener = TcpListener::bind(LOCALHOST.to_string(), 0).unwrap();
    let port = listener.local_port();
    spawn(move || {
        for _ in 0..count {
            let Ok(stream) = listener.accept() else {
                return;
            };
            ACCEPTED.fetch_add(1, SeqCst);
            spawn(move || {
                if let Ok(Input::Data(bytes)) = stream.read(4096) {
                    let _ = stream.write_all(bytes);
                }
                let _ = stream.close();
                SERVED.fetch_add(1, SeqCst);
            });
        }
    });
    let (results, reported) = Channel::bounded::<bool>(count as i64);
    for _ in 0..count {
        let results = results.clone();
        spawn(move || {
            let answered = (|| {
                let stream = TcpStream::connect(LOCALHOST.to_string(), port).ok()?;
                CONNECTED.fetch_add(1, SeqCst);
                stream.set_idle_timeout(20_000).ok()?;
                stream.write_all(b"ping".to_vec()).ok()?;
                let mut out = Vec::new();
                while let Input::Data(bytes) = stream.read(4096).ok()? {
                    out.extend(bytes);
                }
                Some(out == b"ping")
            })();
            results.send(answered == Some(true));
        });
    }
    let (mut yes, mut seen) = (0, 0);
    while seen < count {
        match reported.receive_within(LIMIT.as_nanos() as i64) {
            Some(ok) => {
                seen += 1;
                yes += usize::from(ok);
            }
            None => break,
        }
    }
    assert_eq!(
        yes,
        count,
        "answered {yes}, wrong {}, none {}; connected {}, accepted {}, served {}",
        seen - yes,
        count - seen,
        CONNECTED.load(SeqCst),
        ACCEPTED.load(SeqCst),
        SERVED.load(SeqCst)
    );
}

/// A short read does not show that a socket is empty: what came in the
/// same readiness edge as the bytes it returned is still there, with no
/// edge of its own to come. The peer's end of stream behind the bytes is
/// read at once, not at the idle timeout; so is the reset behind them
/// (the peer closed with a zero linger), and the urgent byte a read
/// stopped before. One worker, the reader parked in the reactor (another
/// task keeps it from waiting alone), and the peer writing on the same
/// worker, so both arrive before the worker looks at its reactor.
#[test]
fn what_a_short_read_left_behind_is_read_at_once() {
    for case in ["end", "reset", "urgent"] {
        passed(&child(
            "scheduler::child_short_read",
            &[("IGNITE_WORKERS", "1"), ("SCHEDULER_CASE", case)],
        ));
    }
}

#[test]
fn child_short_read() {
    use std::io::Write;
    if !in_child() {
        return;
    }
    let case = std::env::var("SCHEDULER_CASE").unwrap();
    let listener = TcpListener::bind(LOCALHOST.to_string(), 0).unwrap();
    let port = listener.local_port();
    // The reset needs a zero linger, which only a plain socket sets.
    let (peer, plain) = if case == "reset" {
        let plain = std::net::TcpStream::connect((LOCALHOST, port as u16)).unwrap();
        (None, Some(plain))
    } else {
        (
            Some(TcpStream::connect(LOCALHOST.to_string(), port).unwrap()),
            None,
        )
    };
    let stream = listener.accept().unwrap();
    let (keep, kept) = Channel::bounded::<bool>(1);
    spawn(move || {
        kept.receive();
    });
    let (report, reports) = mpsc::channel();
    spawn(move || {
        stream.set_idle_timeout(2_000).unwrap();
        let read = || match stream.read(4096) {
            Ok(input) => format!("{input:?}"),
            Err(_) => String::from("Err"),
        };
        let first = read();
        let second = read();
        report
            .send(format!("{first} {second} {}", stream.timed_out()))
            .unwrap();
    });
    let expected = match case.as_str() {
        "urgent" => "Data([112, 105, 110, 103, 255]) Data([242]) false",
        "reset" => "Data([112, 105, 110, 103]) Err false",
        _ => "Data([112, 105, 110, 103]) Eof false",
    };
    spawn(move || {
        // Parked on a timer until the reader parks in its first read.
        ignite_io::wait::sleep(Duration::from_millis(100));
        match (peer, plain) {
            (Some(peer), _) if case == "urgent" => {
                peer.write_all(b"ping".to_vec()).unwrap();
                peer.write_urgent(vec![255, 242]).unwrap();
            }
            (Some(peer), _) => {
                peer.write_all(b"ping".to_vec()).unwrap();
                peer.close().unwrap();
            }
            (None, Some(mut plain)) => {
                plain.write_all(b"ping").unwrap();
                let linger = nix::libc::linger {
                    l_onoff: 1,
                    l_linger: 0,
                };
                nix::sys::socket::setsockopt(&plain, nix::sys::socket::sockopt::Linger, &linger)
                    .unwrap();
                drop(plain);
            }
            (None, None) => unreachable!("a peer"),
        }
    });
    let read = reports.recv_timeout(LIMIT).expect("the reader ended");
    assert_eq!(read, expected);
    drop(keep);
    wait_for_tasks();
}

/// A task's waits send what that task wrote, not what another task on its
/// worker wrote (`docs/slice.md` section 7: "the writing task's next
/// flush"). A writer task leaves one byte in its stream's send buffer,
/// spawns a reader of the same stream, then writes 10 MiB in 100 KiB
/// pieces to an echo server and parks for room while it holds the write
/// lock. The reader's read must not wait for that lock to send the
/// writer's byte: the writer waits for the echo server, which waits for
/// the reader. Before, the pending list was the worker thread's, so the
/// reader's flush waited for the writer's lock and neither moved.
#[test]
fn a_read_does_not_wait_for_another_task_s_write_lock() {
    for workers in ["1", "2"] {
        passed(&child(
            "scheduler::child_writer_with_a_reader_it_spawned",
            &[("IGNITE_WORKERS", workers)],
        ));
    }
}

#[test]
fn child_writer_with_a_reader_it_spawned() {
    use std::io::{Read, Write};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    if !in_child() {
        return;
    }
    const TOTAL: usize = 10 * 1024 * 1024;
    let echo = std::net::TcpListener::bind((LOCALHOST, 0)).unwrap();
    let port = i64::from(echo.local_addr().unwrap().port());
    std::thread::spawn(move || {
        let (mut socket, _) = echo.accept().unwrap();
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            match socket.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if socket.write_all(&buffer[..n]).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = socket.shutdown(std::net::Shutdown::Write);
    });
    let written = Arc::new(AtomicUsize::new(0));
    let read = Arc::new(AtomicUsize::new(0));
    let (done, outcome) = mpsc::channel::<String>();
    {
        let (written, read) = (written.clone(), read.clone());
        spawn(move || {
            let stream = TcpStream::connect(LOCALHOST.to_string(), port).unwrap();
            stream.write_all(vec![0u8]).unwrap();
            let reader = stream.clone();
            spawn(move || {
                let end = loop {
                    match reader.read(64 * 1024) {
                        Ok(Input::Data(bytes)) => {
                            read.fetch_add(bytes.len(), SeqCst);
                        }
                        Ok(Input::Eof) => break Ok(()),
                        Err(e) => break Err(e.message),
                    }
                };
                let _ = done.send(format!("{} bytes, end {end:?}", read.load(SeqCst)));
            });
            let block: Vec<u8> = (0..=250u8).cycle().take(TOTAL).collect();
            for piece in block.chunks(100 * 1024) {
                if stream.write_all(piece.to_vec()).is_err() {
                    return;
                }
                written.fetch_add(piece.len(), SeqCst);
            }
            let _ = stream.close();
        });
    }
    let outcome = outcome.recv_timeout(LIMIT).unwrap_or_else(|_| {
        format!(
            "no outcome within {LIMIT:?}: written {} of {TOTAL}, read {}",
            written.load(SeqCst),
            read.load(SeqCst)
        )
    });
    assert_eq!(outcome, format!("{} bytes, end Ok(())", TOTAL + 1));
}

/// Sessions opened before any of them is sent a request (a load
/// generator's start, idle keep-alive connections) are spread evenly over
/// the workers (`story:readiness-syscalls-per-connection`, row 3.3
/// `llvm/terminal/rr-64`): the first read of a session waits for readiness
/// and does not block its worker in `recv`, so no worker is passed while
/// the rest of the burst is placed. Each session greets first, as a Telnet
/// server does, then reads; the next session is placed only once every
/// worker sleeps. Before, the first three sessions each blocked their
/// worker and the fourth worker took the other thirteen ([1, 1, 1, 13]).
#[test]
fn a_burst_of_silent_sessions_is_spread_over_the_workers() {
    passed(&child(
        "scheduler::child_silent_burst",
        &[("IGNITE_WORKERS", "4")],
    ));
}

/// Whether every worker thread of this process sleeps (state `S` in
/// `/proc`): its tasks wait, or it has none.
fn workers_sleep() -> bool {
    let tasks = std::fs::read_dir("/proc/self/task").unwrap();
    tasks.filter_map(Result::ok).all(|task| {
        let path = task.path();
        let comm = std::fs::read_to_string(path.join("comm")).unwrap_or_default();
        let stat = std::fs::read_to_string(path.join("stat")).unwrap_or_default();
        comm.trim_end() != "worker"
            || stat
                .rsplit_once(") ")
                .is_some_and(|(_, rest)| rest.starts_with('S'))
    })
}

#[test]
fn child_silent_burst() {
    use std::io::Read;
    if !in_child() {
        return;
    }
    const BURST: usize = 16;
    let (report, reports) = mpsc::channel();
    let mut peers = Vec::new();
    for _ in 0..BURST {
        let (stream, mut peer) = pair();
        let report = report.clone();
        spawn(move || {
            report
                .send(format!("{:?}", std::thread::current().id()))
                .unwrap();
            if stream.write_all(b"> ".to_vec()).is_ok() {
                while let Ok(Input::Data(_)) = stream.read(16) {}
            }
        });
        let mut greeting = [0u8; 2];
        peer.read_exact(&mut greeting).unwrap();
        assert!(until(workers_sleep), "a worker never slept");
        peers.push(peer);
    }
    let mut sessions_on: BTreeMap<String, usize> = BTreeMap::new();
    for _ in 0..BURST {
        let worker = reports.recv_timeout(LIMIT).expect("a session started");
        *sessions_on.entry(worker).or_default() += 1;
    }
    let counts: Vec<usize> = sessions_on.values().copied().collect();
    assert_eq!(
        counts,
        vec![BURST / 4; 4],
        "sessions per worker: {sessions_on:?}"
    );
    drop(peers);
    wait_for_tasks();
}

/// The serving side of the connect workload (rows 3.7 and 3.3
/// `terminal/connect-8`, `story:readiness-syscalls-per-connection`) in
/// system calls per connection, at the default worker count (the
/// dedicated host sets none): `strace -f -yy` over the whole server
/// process, every call it makes, starting and ending threads included,
/// less a run of the same server that serves none ([`parse_trace`]). A
/// session does what `Telnet.serve` does for the workload's client: set
/// the idle timeout, send the prompt, read the line, look for urgent data,
/// send `bye`, close. The client connects, waits for the prompt, pauses
/// ([`COUNTED_PAUSE`], so the traced server waits in its read as an
/// untraced one does), sends `quit`, reads to the end and closes, again.
///
/// The bounds are v0.15.0's `terminald`, Rust target, release build,
/// counted by this same method ([`count_a_given_server`] with
/// `SCHEDULER_COUNT_SERVER` naming the binary), the lowest of ten rounds:
/// [`V0_15_0_CONCURRENT`] with 8 clients of 25 connections each (rounds
/// 13.63 to 13.88; the LLVM target's 13.44 to 13.54),
/// [`V0_15_0_SEQUENTIAL`] with 50 connections in sequence (14.24 to 14.86;
/// LLVM 14.62 to 15.30). Bounded by name besides: the threads started, at
/// most [`V0_15_0_STARTS_CONCURRENT`] and [`V0_15_0_STARTS_SEQUENTIAL`]
/// (the most of the same rounds; v0.15.0 started 8 to 11, median 9, and 1
/// to 3, median 2: a thread or worker is started when a connection comes
/// before the session it follows has ended, which the load decides, but a
/// worker that served for a second and retired started 16 and more,
/// [`a_worker_serving_sessions_back_to_back_stays`]); no epoll instance
/// (a worker whose tasks only wait alone makes no reactor); no
/// `EPOLL_CTL_DEL` of a socket (its close takes it out of epoll); no
/// registration of a socket for room to write (no session here fills its
/// send buffer).
#[test]
fn a_connection_costs_no_more_system_calls_than_at_v0_15_0() {
    strace_works();
    let mut failures = Vec::new();
    for (clients, each, bound, starts) in [
        (8, 25, V0_15_0_CONCURRENT, V0_15_0_STARTS_CONCURRENT),
        (1, 50, V0_15_0_SEQUENTIAL, V0_15_0_STARTS_SEQUENTIAL),
    ] {
        let (shape, total, served, none) = count_shape(None, clients, each, None);
        println!("v0.15.0 {bound}, {starts} threads started; {shape}");
        if total > bound {
            failures.push(format!("{shape}\nmore than v0.15.0's {bound}"));
        }
        let started = served.started.saturating_sub(none.started);
        if started > starts {
            failures.push(format!(
                "{shape}\n{started} threads started, v0.15.0 started {starts}"
            ));
        }
        let epolls = served.calls.get("epoll_create1").copied().unwrap_or(0);
        if epolls > 0 {
            failures.push(format!(
                "{shape}\n{epolls} epoll instances made, none expected: a worker whose \
                 tasks only wait alone makes no reactor"
            ));
        }
        if served.deleted > 0 {
            failures.push(format!(
                "{shape}\n{} EPOLL_CTL_DEL of a socket, none expected: a dropped stream's \
                 close takes it out of epoll",
                served.deleted
            ));
        }
        if served.armed > 0 {
            failures.push(format!(
                "{shape}\n{} registrations of a socket for room to write, none expected: \
                 no session waits for room",
                served.armed
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// v0.15.0's count per connection with 8 clients of 25 connections, by
/// [`count_a_given_server`] over its `terminald` (Rust target, release).
const V0_15_0_CONCURRENT: f64 = 13.63;
/// The same with 50 connections in sequence.
const V0_15_0_SEQUENTIAL: f64 = 14.24;
/// The threads v0.15.0's `terminald` started for 8 clients of 25
/// connections.
const V0_15_0_STARTS_CONCURRENT: u64 = 11;
/// The same for 50 connections in sequence.
const V0_15_0_STARTS_SEQUENTIAL: u64 = 3;

/// The calls `story:readiness-syscalls-per-connection` removed, each bounded
/// by name over the run serving connections less the run serving none, so
/// that one of them coming back fails even while the total stays under
/// v0.15.0's: no `setsockopt` per connection (the idle timeout is set before
/// a `recv` that blocks, and no session here blocks in one), one `accept4`
/// per connection (a `poll` comes first, no `accept4` fails), one
/// `recvfrom` per connection (the first read waits before it reads), and,
/// with sessions in sequence, each alone on its worker, no `epoll_ctl` of a
/// socket (a wake already taken does not end the lone wait and make its
/// task register). The last also on one worker (`IGNITE_WORKERS=1`), whose
/// lone tasks wait through its reactor: there a wake written by the spawn
/// that placed the task and not read back made each session's first wait
/// end at once, and the task register its socket (`EPOLL_CTL_ADD`).
#[test]
fn a_connection_makes_none_of_the_calls_the_story_removed() {
    strace_works();
    let mut failures = Vec::new();
    for (clients, each, workers) in [(8, 25, None), (1, 50, None), (1, 50, Some("1"))] {
        let none = traced_run(None, 0, 0, workers);
        let served = traced_run(None, clients, each, workers);
        let connections = (clients * each) as i64;
        let calls = |name: &str| {
            let count = |traced: &Traced| traced.calls.get(name).copied().unwrap_or(0) as i64;
            count(&served) - count(&none)
        };
        let shape = format!(
            "{clients} clients x {each} on {} workers",
            workers.unwrap_or("the default")
        );
        let registered = served.registered as i64 - none.registered as i64;
        for (name, count, most) in [
            ("setsockopt", calls("setsockopt"), 0),
            ("accept4", calls("accept4"), connections),
            ("recvfrom", calls("recvfrom"), connections),
            (
                "epoll_ctl of a socket",
                registered,
                if clients == 1 { 0 } else { i64::MAX },
            ),
        ] {
            if count > most {
                failures.push(format!("{shape}: {count} {name}, at most {most} expected"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The count of [`a_connection_costs_no_more_system_calls_than_at_v0_15_0`]
/// over a `terminald` binary named by `SCHEDULER_COUNT_SERVER` (started
/// with `--port`, ended with SIGTERM), for the shape
/// `SCHEDULER_COUNT_SHAPE` (`8x25` or `1x50`, default both): how its
/// bounds were measured, and how the LLVM target is counted. Does nothing
/// without the variable.
#[test]
fn count_a_given_server() {
    let Some(server) = std::env::var_os("SCHEDULER_COUNT_SERVER") else {
        return;
    };
    strace_works();
    let shape = std::env::var("SCHEDULER_COUNT_SHAPE").unwrap_or_default();
    for (clients, each) in [(8, 25), (1, 50)] {
        if shape.is_empty() || shape == format!("{clients}x{each}") {
            let (shape, _, _, _) = count_shape(Some(Path::new(&server)), clients, each, None);
            println!("{shape}");
        }
    }
}

/// `strace` runs and may trace a child here; otherwise the count cannot
/// be taken, and the test says which prerequisite is missing.
fn strace_works() {
    let status = Command::new("strace")
        .args(["-o", "/dev/null", "true"])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("strace is needed to count system calls and did not run: {e}"));
    assert!(
        status.success(),
        "strace cannot trace a child here (ptrace is blocked): {status:?}"
    );
}

/// What one traced run of a server made.
#[derive(Default)]
struct Traced {
    /// Calls by name.
    calls: BTreeMap<String, u64>,
    /// Threads started (`clone3`, `clone`).
    started: u64,
    /// `EPOLL_CTL_ADD`, `EPOLL_CTL_MOD` or `EPOLL_CTL_DEL` of a socket.
    registered: u64,
    /// `EPOLL_CTL_DEL` of a socket.
    deleted: u64,
    /// `EPOLL_CTL_ADD` or `EPOLL_CTL_MOD` of a socket with `EPOLLOUT`.
    armed: u64,
    /// Events an `epoll_wait` returned with room to write only.
    write_only: u64,
}

/// Whether the argument text `argument` (a descriptor as `strace -yy`
/// prints it) is a TCP socket.
fn socket(argument: &str) -> bool {
    argument
        .trim_start()
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .starts_with("<TCP")
}

/// The calls of a `strace -f -yy` trace: every call, except
/// `fcntl(F_GETFD)`, the check a debug build makes before each `close`
/// (std's `debug_assert_fd_is_open`), which the release builds compared
/// with do not make.
fn parse_trace(text: &str) -> Traced {
    let mut traced = Traced::default();
    for line in text.lines() {
        let Some((_, call)) = line.split_once(' ') else {
            continue;
        };
        let call = call.trim_start();
        if call.starts_with("epoll_wait(") || call.starts_with("<... epoll_wait resumed>") {
            traced.write_only += call.matches("{events=EPOLLOUT,").count() as u64;
        }
        if call.starts_with('<') || call.starts_with('-') || call.starts_with('+') {
            continue;
        }
        let Some((name, arguments)) = call.split_once('(') else {
            continue;
        };
        if name == "fcntl" && arguments.contains("F_GETFD") {
            continue;
        }
        if name == "clone3" || name == "clone" {
            traced.started += 1;
        }
        if name == "epoll_ctl"
            && ["EPOLL_CTL_ADD, ", "EPOLL_CTL_MOD, ", "EPOLL_CTL_DEL, "]
                .iter()
                .find_map(|operation| arguments.split_once(operation))
                .is_some_and(|(_, target)| socket(target))
        {
            traced.registered += 1;
            if arguments.contains("EPOLL_CTL_DEL") {
                traced.deleted += 1;
            } else if arguments.contains("EPOLLOUT") {
                traced.armed += 1;
            }
        }
        *traced.calls.entry(name.to_string()).or_default() += 1;
    }
    traced
}

/// Count one shape: the run serving `clients` x `each` connections less
/// the run serving none, per connection, of `server` (a `terminald`) or of
/// [`child_counted_server`] on `workers` workers (`None`: the default).
/// The shape's line with the table, the total, the serving run and the
/// run serving none.
fn count_shape(
    server: Option<&Path>,
    clients: usize,
    each: usize,
    workers: Option<&str>,
) -> (String, f64, Traced, Traced) {
    let none = traced_run(server, 0, 0, workers);
    let served = traced_run(server, clients, each, workers);
    let connections = (clients * each) as f64;
    let mut names: Vec<&String> = none.calls.keys().chain(served.calls.keys()).collect();
    names.sort();
    names.dedup();
    let mut table = String::new();
    let mut total = 0.0;
    for name in names {
        let count = |traced: &Traced| traced.calls.get(name).copied().unwrap_or(0) as f64;
        let calls = (count(&served) - count(&none)) / connections;
        total += calls;
        if calls.abs() >= 0.005 {
            table.push_str(&format!("{name:>16} {calls:6.2}\n"));
        }
    }
    let shape = format!(
        "{clients} clients x {each}: {total:.2} per connection; threads started {}; \
         EPOLL_CTL_DEL of a socket {}, registrations for room to write {}, wakes with room \
         to write only {}\n{table}",
        served.started as i64 - none.started as i64,
        served.deleted,
        served.armed,
        served.write_only
    );
    (shape, total, served, none)
}

/// Runs of [`traced_run`] in this process, to name their trace files.
static TRACES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A server under `strace`: when dropped, also while a client's failure
/// unwinds, the server and `strace` are killed if they still run and
/// waited for, and the trace file is removed.
struct Tracing {
    tracer: std::process::Child,
    trace: std::path::PathBuf,
}

impl Drop for Tracing {
    fn drop(&mut self) {
        if matches!(self.tracer.try_wait(), Ok(None)) {
            if let Some(server) = child_of(self.tracer.id()) {
                let _ = nix::sys::signal::kill(server, nix::sys::signal::Signal::SIGKILL);
            }
            let _ = self.tracer.kill();
            let _ = self.tracer.wait();
        }
        let _ = std::fs::remove_file(&self.trace);
    }
}

/// Run `server` (a `terminald`, ended with SIGTERM) or
/// [`child_counted_server`] on `workers` workers under `strace` for
/// `clients` clients of `each` connections; what it made.
fn traced_run(server: Option<&Path>, clients: usize, each: usize, workers: Option<&str>) -> Traced {
    use std::io::{BufRead, BufReader, Read};
    let trace = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "scheduler-trace-{}-{}.txt",
        std::process::id(),
        TRACES.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut command = Command::new("strace");
    command
        .args(["-f", "-yy", "-o"])
        .arg(&trace)
        .env_remove("IGNITE_IO")
        .env_remove("IGNITE_WORKERS")
        .stdout(std::process::Stdio::piped());
    let given = server.map(|server| {
        let port = std::net::TcpListener::bind((LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        command.arg(server).args(["--port", &port.to_string()]);
        port
    });
    if given.is_none() {
        command
            .arg(std::env::current_exe().unwrap())
            .args([
                "scheduler::child_counted_server",
                "--exact",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, "1")
            .env(COUNTED_CONNECTIONS, (clients * each).to_string());
        if let Some(workers) = workers {
            command.env("IGNITE_WORKERS", workers);
        }
    }
    let mut tracing = Tracing {
        tracer: command.spawn().expect("strace runs"),
        trace,
    };
    let mut out = BufReader::new(tracing.tracer.stdout.take().unwrap());
    let port = match given {
        Some(port) => {
            assert!(until(|| listening(port)), "the server never listened");
            port
        }
        None => {
            let mut line = String::new();
            loop {
                line.clear();
                assert!(
                    out.read_line(&mut line).unwrap() > 0,
                    "the server ended before it told its port"
                );
                // After the harness' `test <name> ... ` on the same line.
                if let Some((_, port)) = line.trim().rsplit_once("port ") {
                    break port.parse().unwrap();
                }
            }
        }
    };
    let workers: Vec<_> = (0..clients)
        .map(|_| {
            std::thread::spawn(move || {
                for _ in 0..each {
                    counted_session(port);
                }
            })
        })
        .collect();
    let failed: Vec<String> = workers
        .into_iter()
        .filter_map(|worker| worker.join().err())
        .map(|panic| {
            panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default()
        })
        .collect();
    assert!(failed.is_empty(), "a client failed: {failed:?}");
    if given.is_some() {
        let server = child_of(tracing.tracer.id()).expect("strace's child");
        nix::sys::signal::kill(server, nix::sys::signal::Signal::SIGTERM).unwrap();
    }
    let mut rest = String::new();
    out.read_to_string(&mut rest).unwrap();
    let status = tracing.tracer.wait().unwrap();
    assert!(
        given.is_some() || status.success(),
        "the server failed: {status:?}\n{rest}"
    );
    let text = std::fs::read_to_string(&tracing.trace).unwrap();
    let traced = parse_trace(&text);
    assert!(!traced.calls.is_empty(), "no calls traced:\n{text}");
    traced
}

/// Whether a socket of this machine listens on `port` (`/proc/net/tcp`,
/// `/proc/net/tcp6`: local port in hex, state `0A`).
fn listening(port: u16) -> bool {
    let local = format!(":{port:04X}");
    ["/proc/net/tcp", "/proc/net/tcp6"].iter().any(|table| {
        std::fs::read_to_string(table)
            .unwrap_or_default()
            .lines()
            .any(|row| {
                let fields: Vec<&str> = row.split_whitespace().collect();
                fields.len() > 3 && fields[1].ends_with(&local) && fields[3] == "0A"
            })
    })
}

/// The process `strace` (`tracer`) started, while it runs.
fn child_of(tracer: u32) -> Option<nix::unistd::Pid> {
    std::fs::read_to_string(format!("/proc/{tracer}/task/{tracer}/children"))
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
        .map(nix::unistd::Pid::from_raw)
}

/// The client's pause between the prompt and its request.
const COUNTED_PAUSE: Duration = Duration::from_millis(20);
/// The connections the counted server serves before it exits.
const COUNTED_CONNECTIONS: &str = "SCHEDULER_COUNTED_CONNECTIONS";
/// The counted server's prompt: `terminald`'s, with IAC GA.
const COUNTED_PROMPT: &[u8] = b"terminal> \xff\xf9";

/// One connection of the counted workload's client (see
/// [`a_connection_costs_no_more_system_calls_than_at_v0_15_0`]).
fn counted_session(port: u16) {
    use std::io::{Read, Write};
    let mut peer = std::net::TcpStream::connect((LOCALHOST, port)).unwrap();
    peer.set_nodelay(true).unwrap();
    peer.set_read_timeout(Some(LIMIT)).unwrap();
    let mut seen = Vec::new();
    let mut buffer = [0u8; 4096];
    let mut until_seen = |peer: &mut std::net::TcpStream, want: &[u8]| {
        seen.clear();
        while !seen.windows(want.len()).any(|w| w == want) {
            let n = peer.read(&mut buffer).unwrap();
            assert!(n > 0, "closed before {want:?}: {seen:?}");
            seen.extend_from_slice(&buffer[..n]);
        }
    };
    until_seen(&mut peer, COUNTED_PROMPT);
    std::thread::sleep(COUNTED_PAUSE);
    peer.write_all(b"quit\r\n").unwrap();
    until_seen(&mut peer, b"bye\r\n");
    while peer.read(&mut buffer).unwrap() > 0 {}
}

/// The server counted by [`traced_run`]: an accept loop on the main
/// thread, as a compiled server runs it, that spawns one session per
/// connection, `Telnet.serve`'s calls for the workload's client.
#[test]
fn child_counted_server() {
    use std::io::Write;
    if !in_child() {
        return;
    }
    let connections: usize = std::env::var(COUNTED_CONNECTIONS).unwrap().parse().unwrap();
    let listener = TcpListener::bind(LOCALHOST.to_string(), 0).unwrap();
    println!("port {}", listener.local_port());
    std::io::stdout().flush().unwrap();
    for _ in 0..connections {
        let stream = listener.accept().unwrap();
        spawn(move || {
            let _ = stream.set_idle_timeout(600_000);
            let _ = stream.write_all(COUNTED_PROMPT.to_vec());
            let mut line = Vec::new();
            while let Ok(Input::Data(bytes)) = stream.read(4096) {
                let _ = stream.urgent_pending();
                line.extend_from_slice(&bytes);
                if line.ends_with(b"\r\n") {
                    let _ = stream.write_all(b"bye\r\n".to_vec());
                    break;
                }
            }
            let _ = stream.close();
        });
    }
    wait_for_tasks();
}

/// A worker that runs one session after another, each of them waiting
/// alone on the worker's thread from its start to its end, is not without
/// a task between them for [`ignite_io::LINGER`], so it does not end and
/// no other starts, however long the sessions go on
/// (`story:readiness-syscalls-per-connection`: a worker that served for
/// [`ignite_io::LINGER`] used to retire after its next session, and the
/// spawn after it started a thread again).
#[test]
fn a_worker_serving_sessions_back_to_back_stays() {
    passed(&child("scheduler::child_back_to_back", &[]));
}

#[test]
fn child_back_to_back() {
    use std::io::Write;
    if !in_child() {
        return;
    }
    let until = Instant::now() + ignite_io::LINGER * 3 / 2;
    let mut sessions = 0;
    while Instant::now() < until {
        let (stream, mut peer) = pair();
        let (read, got) = mpsc::channel();
        spawn(move || {
            let _ = read.send(stream.read(16));
        });
        std::thread::sleep(Duration::from_millis(50));
        peer.write_all(b"hi").unwrap();
        let got = got.recv_timeout(LIMIT).expect("the session read");
        assert_eq!(got.unwrap(), Input::Data(b"hi".to_vec()));
        wait_for_tasks();
        sessions += 1;
    }
    let counts = ignite_io::scheduler_counts();
    assert_eq!(
        counts.workers_started, 1,
        "{sessions} sessions in sequence: {counts:?}"
    );
}

/// A socket already registered with a worker for reading (a reader parked
/// on it while another task shared the worker) is registered without room
/// to write (`story:readiness-syscalls-per-connection`, lazy `EPOLLOUT`).
/// A task on the same worker that then waits for room to write must still
/// be woken when the peer drains the socket: the first wait for room
/// changes that registration, it does not only make a new one.
#[test]
fn a_writer_is_woken_on_a_socket_already_registered_for_reading() {
    passed(&child(
        "scheduler::child_writer_after_a_registered_reader",
        &[("IGNITE_WORKERS", "1")],
    ));
}

#[test]
fn child_writer_after_a_registered_reader() {
    use std::io::{Read, Write};
    if !in_child() {
        return;
    }
    const TOTAL: usize = 32 * 1024 * 1024;
    let (stream, mut peer) = pair();
    // A second task on the worker, so neither the reader nor the writer
    // waits alone on the worker's thread: both register and park.
    let (keep, kept) = Channel::bounded::<bool>(1);
    spawn(move || {
        kept.receive();
    });
    let reader = stream.clone();
    let (read, got) = mpsc::channel();
    spawn(move || {
        let _ = read.send(reader.read(64));
    });
    // The reader has parked, so the socket is registered for reading.
    std::thread::sleep(Duration::from_millis(200));
    let (wrote, written) = mpsc::channel();
    spawn(move || {
        let chunk = vec![7u8; 64 * 1024];
        for _ in 0..TOTAL / chunk.len() {
            stream.write_from(&chunk).unwrap();
        }
        ignite_io::flush();
        wrote.send(()).unwrap();
    });
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        written.try_recv().is_err(),
        "the peer took {TOTAL} bytes unread"
    );
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut received = 0;
    while received < TOTAL {
        let count = peer
            .read(&mut buffer)
            .unwrap_or_else(|e| panic!("the writer stalled after {received} bytes: {e}"));
        assert!(count > 0, "end of stream after {received} bytes");
        received += count;
    }
    written
        .recv_timeout(LIMIT)
        .expect("the writer finished once the peer read");
    peer.write_all(b"hi").unwrap();
    let got = got.recv_timeout(LIMIT).expect("the reader ran");
    assert_eq!(got.unwrap(), Input::Data(b"hi".to_vec()));
    drop(keep);
    wait_for_tasks();
}
