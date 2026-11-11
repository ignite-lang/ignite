//! Adversary case of `story:binary-size-back`: the worker's timers as a
//! binary heap (`sched.rs` `Timers`). The entry of a task that ended stays
//! in the heap until such entries are half of it, and the worker takes the
//! earliest entry's deadline as the time of its next wake, armed or not.
//!
//! The case runs in a child process, as in `scheduler.rs`: the IO mode and
//! the worker count are read once per process.

use std::process::{Command, Output};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ignite_io::{Input, TcpListener, TcpStream, spawn, wait_for_tasks};

const LOCALHOST: &str = "127.0.0.1";
const LIMIT: Duration = Duration::from_secs(10);
const CHILD: &str = "IGNITE_SCHEDULER_CHILD";

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

fn pair() -> (TcpStream, std::net::TcpStream) {
    let listener = TcpListener::bind(LOCALHOST.to_string(), 0).unwrap();
    let peer = std::net::TcpStream::connect((LOCALHOST, listener.local_port() as u16)).unwrap();
    (listener.accept().unwrap(), peer)
}

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

/// The worker threads of this process (`comm` is `worker`): for each, its
/// state letter and how often it gave up the processor of its own accord
/// (`voluntary_ctxt_switches`: once per sleep it woke from).
fn workers() -> Vec<(char, u64)> {
    let tasks = std::fs::read_dir("/proc/self/task").unwrap();
    tasks
        .filter_map(Result::ok)
        .filter_map(|task| {
            let path = task.path();
            let comm = std::fs::read_to_string(path.join("comm")).ok()?;
            if comm.trim_end() != "worker" {
                return None;
            }
            let stat = std::fs::read_to_string(path.join("stat")).ok()?;
            let state = stat.rsplit_once(") ")?.1.chars().next()?;
            let status = std::fs::read_to_string(path.join("status")).ok()?;
            let sleeps = status
                .lines()
                .find_map(|line| line.strip_prefix("voluntary_ctxt_switches:"))?
                .trim()
                .parse()
                .ok()?;
            Some((state, sleeps))
        })
        .collect()
}

/// A worker whose tasks all wait, the earliest armed deadline a minute
/// away, sleeps until then: it does not wake for the timer of a session
/// that has ended. Six sessions wait in a read with an idle timeout of
/// 60 s; four more, with timeouts of 0.5 to 1.1 s, end at once because
/// their peers close. In the 1.5 s after that the one worker must not
/// wake. With the timers in a `BTreeMap` (before `5433f7c6`) an ended
/// task's entry was removed and the worker slept on; with the heap the
/// four entries stay (4 of 10 is under half) and the worker wakes once for
/// each, an `epoll_wait` that returns on its timeout with no task to run.
#[test]
fn a_worker_does_not_wake_for_the_timer_of_a_session_that_ended() {
    passed(&child(
        "timer_adversary::child_ended_sessions_among_waiting_ones",
        &[("IGNITE_WORKERS", "1")],
    ));
}

#[test]
fn child_ended_sessions_among_waiting_ones() {
    if !in_child() {
        return;
    }
    const WAITING: usize = 6;
    const ENDING: [i64; 4] = [500, 700, 900, 1100];
    const WATCHED: Duration = Duration::from_millis(1500);
    let (ended, endings) = mpsc::channel();
    let session = |idle: i64, ended: Option<mpsc::Sender<()>>| {
        let (stream, peer) = pair();
        stream.set_idle_timeout(idle).unwrap();
        spawn(move || {
            while let Ok(Input::Data(_)) = stream.read(16) {}
            if let Some(ended) = ended {
                let _ = ended.send(());
            }
        });
        peer
    };
    let waiting: Vec<_> = (0..WAITING).map(|_| session(60_000, None)).collect();
    let armed = Instant::now();
    let ending: Vec<_> = ENDING
        .iter()
        .map(|idle| session(*idle, Some(ended.clone())))
        .collect();
    drop(ending);
    for _ in ENDING {
        endings
            .recv_timeout(LIMIT)
            .expect("a session whose peer closed ended");
    }
    assert!(
        until(|| workers().iter().all(|(state, _)| *state == 'S')),
        "the worker never slept: {:?}",
        workers()
    );
    let before = workers();
    let took = armed.elapsed();
    assert_eq!(before.len(), 1, "one worker expected: {before:?}");
    assert!(
        took < Duration::from_millis(ENDING[0] as u64),
        "the four sessions took {took:?} to end, past the first deadline: nothing to watch"
    );
    std::thread::sleep(WATCHED);
    let after = workers();
    assert_eq!(
        after
            .iter()
            .map(|(_, sleeps)| *sleeps)
            .collect::<Vec<u64>>(),
        before
            .iter()
            .map(|(_, sleeps)| *sleeps)
            .collect::<Vec<u64>>(),
        "the worker woke in {WATCHED:?} with {WAITING} sessions waiting, their deadline 60 s \
         away, and no other task (sleeps it woke from, before and after)"
    );
    drop(waiting);
    wait_for_tasks();
}
