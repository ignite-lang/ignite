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
/// read at once, not at the idle timeout; so is the urgent byte a read
/// stopped before. One worker, the reader parked in the reactor (another
/// task keeps it from waiting alone), and the peer writing on the same
/// worker, so both arrive before the worker looks at its reactor.
#[test]
fn what_a_short_read_left_behind_is_read_at_once() {
    for case in ["end", "urgent"] {
        passed(&child(
            "scheduler::child_short_read",
            &[("IGNITE_WORKERS", "1"), ("SCHEDULER_CASE", case)],
        ));
    }
}

#[test]
fn child_short_read() {
    if !in_child() {
        return;
    }
    let urgent = std::env::var("SCHEDULER_CASE").as_deref() == Ok("urgent");
    let listener = TcpListener::bind(LOCALHOST.to_string(), 0).unwrap();
    let peer = TcpStream::connect(LOCALHOST.to_string(), listener.local_port()).unwrap();
    let stream = listener.accept().unwrap();
    let (keep, kept) = Channel::bounded::<bool>(1);
    spawn(move || {
        kept.receive();
    });
    let (report, reports) = mpsc::channel();
    spawn(move || {
        stream.set_idle_timeout(2_000).unwrap();
        let first = stream.read(4096).unwrap();
        let second = stream.read(4096).unwrap();
        report
            .send(format!("{first:?} {second:?} {}", stream.timed_out()))
            .unwrap();
    });
    spawn(move || {
        // Parked on a timer until the reader parks in its first read.
        ignite_io::wait::sleep(Duration::from_millis(100));
        peer.write_all(b"ping".to_vec()).unwrap();
        if urgent {
            peer.write_urgent(vec![255, 242]).unwrap();
        } else {
            peer.close().unwrap();
        }
    });
    let read = reports.recv_timeout(LIMIT).expect("the reader ended");
    let expected = if urgent {
        "Data([112, 105, 110, 103, 255]) Data([242]) false"
    } else {
        "Data([112, 105, 110, 103]) Eof false"
    };
    assert_eq!(read, expected);
    drop(keep);
    wait_for_tasks();
}
