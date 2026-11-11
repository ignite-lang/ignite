//! Adversary cases of `story:binary-size-back`: the worker's timers as a
//! binary heap (`sched.rs` `Timers`). Pass 1 met a heap in which the entry
//! of a task that ended stayed until such entries were half of it, and the
//! worker took the earliest entry's deadline as the time of its next wake,
//! armed or not. Since `58b5ffe0` the heap is indexed by task slot and an
//! ended task's entry is taken out at once: pass 2's cases are on that.
//!
//! Each case runs in a child process, as in `scheduler.rs`: the IO mode and
//! the worker count are read once per process. A child that does not end
//! is killed, so that a worker that panicked fails the case with what it
//! printed instead of holding the run.

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ignite_io::{Input, TcpListener, TcpStream, spawn, wait_for_tasks};

const LOCALHOST: &str = "127.0.0.1";
const LIMIT: Duration = Duration::from_secs(10);
const CHILD_LIMIT: Duration = Duration::from_secs(90);
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
    // A worker that panics leaves its tasks waiting without end, and the
    // child with them: the child is given `CHILD_LIMIT` and then killed, so
    // that the case fails with what the child printed.
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let read_stdout = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let read_stderr = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let deadline = Instant::now() + CHILD_LIMIT;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            break child.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Output {
        status,
        stdout: read_stdout.join().unwrap(),
        stderr: read_stderr.join().unwrap(),
    }
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

/// Adversary pass 2: the heap is indexed since `58b5ffe0` (one entry per
/// task slot, its position kept beside the slot's deadlines, removal and
/// an earlier arming in place). Every task that sleeps wakes once, not
/// before its deadline, and on one worker in the order of the deadlines
/// (tasks with the same deadline in the order they began to sleep), while
/// the entries of sessions that ended are taken out from under them: the
/// only entry, the earliest one, the latest one pushed, and others between
/// the sleepers' deadlines, in a shuffled order.
#[test]
fn timers_fire_in_deadline_order_after_entries_of_ended_tasks_are_removed() {
    passed(&child(
        "timer_adversary::child_sleepers_among_ended_sessions",
        &[("IGNITE_WORKERS", "1")],
    ));
}

/// The same tasks on several workers, each with a heap of its own: every
/// sleeper wakes once and not before its deadline.
#[test]
fn every_timer_fires_once_on_several_workers() {
    passed(&child(
        "timer_adversary::child_sleepers_among_ended_sessions",
        &[("IGNITE_WORKERS", "4")],
    ));
}

/// The same tasks in the IO mode `threads`, where a deadline is the
/// thread's own wait.
#[test]
fn every_timer_fires_once_in_the_mode_threads() {
    passed(&child(
        "timer_adversary::child_sleepers_among_ended_sessions",
        &[("IGNITE_IO", "threads")],
    ));
}

#[test]
fn child_sleepers_among_ended_sessions() {
    if !in_child() {
        return;
    }
    const SLEEPERS: usize = 48;
    const ENDING: usize = 24;
    const KEEPER: usize = usize::MAX;
    const LEAD: u64 = 2000;
    const STEP: Duration = Duration::from_millis(40);
    let ordered = std::env::var_os("IGNITE_IO").is_none()
        && std::env::var("IGNITE_WORKERS").as_deref() == Ok("1");
    let (ended, endings) = mpsc::channel::<(usize, bool)>();
    let (began, begun) = mpsc::channel::<()>();
    let session = |index: usize, idle: i64| {
        let (stream, peer) = pair();
        if idle > 0 {
            stream.set_idle_timeout(idle).unwrap();
        }
        let ended = ended.clone();
        let began = began.clone();
        spawn(move || {
            let _ = began.send(());
            while let Ok(Input::Data(_)) = stream.read(16) {}
            let _ = ended.send((index, stream.timed_out()));
        });
        peer
    };
    // The `count` sessions spawned last began to read and every worker
    // sleeps: their reads wait, the timers armed.
    let wait = |count: usize| {
        for _ in 0..count {
            begun.recv_timeout(LIMIT).expect("a session began to read");
        }
        assert!(
            until(|| workers().iter().all(|(state, _)| *state == 'S')),
            "the workers never slept: {:?}",
            workers()
        );
    };
    // A session without a deadline keeps the worker's other tasks from
    // being alone on it (a task alone waits on the worker's thread and
    // arms nothing).
    let keeper = session(KEEPER, 0);
    // The only entry of the heap, removed.
    let only = session(ENDING, 60_000);
    wait(2);
    drop(only);
    assert_eq!(
        endings.recv_timeout(LIMIT).ok(),
        Some((ENDING, false)),
        "the session with the only timer ended when its peer closed"
    );
    let start = Instant::now() + Duration::from_millis(LEAD);
    // Two sleepers per deadline, the deadlines shuffled against the order
    // the tasks start in.
    let rank = |i: usize| (i * 29) % SLEEPERS / 2;
    let deadline = |i: usize| start + STEP * rank(i) as u32;
    let (woke, wakes) = mpsc::channel();
    // The earliest entry of all: the root.
    let mut peers = vec![session(0, 1500)];
    let sleeper = |i: usize| {
        let woke = woke.clone();
        let at = deadline(i);
        spawn(move || {
            ignite_io::wait::sleep_until(at);
            let _ = woke.send((i, Instant::now()));
        });
    };
    for i in 0..SLEEPERS / 2 {
        sleeper(i);
        if peers.len() < ENDING - 1 {
            // Deadlines among the sleepers' and after them.
            let n = peers.len();
            peers.push(session(n, (LEAD + 85 * n as u64 + 20) as i64));
        }
    }
    // The latest deadline, pushed last: the last element.
    peers.push(session(ENDING - 1, 60_000));
    assert_eq!(peers.len(), ENDING);
    wait(ENDING);
    if ordered {
        assert_eq!(workers().len(), 1, "one worker expected: {:?}", workers());
    }
    let mut peers: Vec<Option<std::net::TcpStream>> = peers.into_iter().map(Some).collect();
    for n in 0..ENDING {
        drop(peers[(n * 7) % ENDING].take());
    }
    let mut seen = [false; ENDING];
    for _ in 0..ENDING {
        let (index, timed_out) = endings
            .recv_timeout(LIMIT)
            .expect("a session whose peer closed ended");
        assert!(
            !timed_out,
            "session {index} ended on its idle timeout before its peer's close was seen: \
             nothing to watch"
        );
        assert!(
            !std::mem::replace(&mut seen[index], true),
            "session {index} ended twice"
        );
    }
    // The sleepers that start now take the task slots of the sessions
    // that ended.
    for i in SLEEPERS / 2..SLEEPERS {
        sleeper(i);
    }
    assert!(
        until(|| workers().iter().all(|(state, _)| *state == 'S')),
        "the workers never slept: {:?}",
        workers()
    );
    assert!(
        Instant::now() < start,
        "the sleepers began to sleep after the first deadline: nothing to watch"
    );
    let mut order = Vec::new();
    for _ in 0..SLEEPERS {
        let Ok((i, at)) = wakes.recv_timeout(LIMIT) else {
            let missing: Vec<usize> = (0..SLEEPERS).filter(|i| !order.contains(i)).collect();
            panic!(
                "the timers of the sleeping tasks {missing:?} did not fire; woke so far: {order:?}"
            );
        };
        assert!(at >= deadline(i), "task {i} woke before its deadline");
        order.push(i);
    }
    let mut expected: Vec<usize> = (0..SLEEPERS).collect();
    if ordered {
        expected.sort_by_key(|i| (rank(*i), *i));
        assert_eq!(
            order, expected,
            "on one worker the sleepers wake in the order of their deadlines, those with the \
             same deadline in the order they began to sleep"
        );
    } else {
        let mut woken = order.clone();
        woken.sort_unstable();
        assert_eq!(woken, expected, "every sleeper wakes once");
    }
    drop(keeper);
    assert_eq!(
        endings.recv_timeout(LIMIT).ok(),
        Some((KEEPER, false)),
        "the session without a deadline ended when its peer closed"
    );
    wait_for_tasks();
    assert!(wakes.try_recv().is_err(), "a sleeper woke twice");
    assert!(endings.try_recv().is_err(), "a session ended twice");
}

/// A read ends on the idle timeout it began with, whatever entry the task
/// has in the heap from an earlier read. With an entry a minute away, a
/// read with a timeout of 0.3 s moves the entry up in place, past the
/// entries of six other sessions; with an entry 1 s away, a read with a
/// timeout of 2.5 s leaves it, and the worker arms the task again when the
/// entry fires. Neither read ends before its timeout, and neither waits
/// for the earlier entry's deadline or without end.
#[test]
fn a_read_ends_on_its_own_idle_timeout_whatever_was_armed_before() {
    passed(&child(
        "timer_adversary::child_timeouts_change_between_reads",
        &[("IGNITE_WORKERS", "1")],
    ));
}

#[test]
fn child_timeouts_change_between_reads() {
    use std::io::Write;

    if !in_child() {
        return;
    }
    let (ended, endings) = mpsc::channel::<(&'static str, bool, Duration, bool)>();
    let waiting = |name: &'static str, idle: i64| {
        let (stream, peer) = pair();
        if idle > 0 {
            stream.set_idle_timeout(idle).unwrap();
        }
        let ended = ended.clone();
        spawn(move || {
            while let Ok(Input::Data(_)) = stream.read(16) {}
            let _ = ended.send((name, false, Duration::ZERO, stream.timed_out()));
        });
        peer
    };
    // One session without a deadline and six with entries 5 to 10 s away.
    let mut others = vec![waiting("other", 0)];
    for idle in [7000, 5000, 9000, 6000, 10_000, 8000] {
        others.push(waiting("other", idle));
    }
    // A task that reads once with the timeout `first`, gets a byte while it
    // waits, then reads with the timeout `second`: how long that read took
    // and whether it ended on the timeout.
    let changed = |name: &'static str, first: i64, second: i64| {
        let (stream, mut peer) = pair();
        stream.set_idle_timeout(first).unwrap();
        let (reading, reads) = mpsc::channel();
        let ended = ended.clone();
        spawn(move || {
            let _ = reading.send(());
            let data = matches!(stream.read(16), Ok(Input::Data(_)));
            stream.set_idle_timeout(second).unwrap();
            let began = Instant::now();
            let eof = matches!(stream.read(16), Ok(Input::Eof));
            let _ = ended.send((name, data, began.elapsed(), eof && stream.timed_out()));
        });
        reads
            .recv_timeout(LIMIT)
            .expect("the task began its first read");
        assert!(
            until(|| workers().iter().all(|(state, _)| *state == 'S')),
            "the worker never slept: {:?}",
            workers()
        );
        peer.write_all(b"x").unwrap();
        let (who, data, took, timed_out) = endings.recv_timeout(LIMIT).unwrap_or_else(|_| {
            panic!("the read of `{name}` with a timeout of {second} ms never ended")
        });
        assert_eq!(who, name, "another session ended first");
        assert!(
            data,
            "the first read of `{name}` got no data: nothing to watch"
        );
        assert!(
            timed_out,
            "the second read of `{name}` did not end on its idle timeout"
        );
        assert!(
            took >= Duration::from_millis(second as u64),
            "the second read of `{name}` ended after {took:?}, before its timeout of {second} ms"
        );
        peer
    };
    let earlier = changed("earlier", 60_000, 300);
    let later = changed("later", 1000, 2500);
    assert_eq!(workers().len(), 1, "one worker expected: {:?}", workers());
    drop((earlier, later));
    let count = others.len();
    drop(others);
    for _ in 0..count {
        let (name, _, _, timed_out) = endings
            .recv_timeout(LIMIT)
            .expect("a session whose peer closed ended");
        assert_eq!((name, timed_out), ("other", false));
    }
    wait_for_tasks();
    assert!(endings.try_recv().is_err(), "a session ended twice");
}

/// The entry that takes the place of a removed one may be earlier than the
/// entry above that place: it then has to move up (`Timers::remove` sifts
/// up before it sifts down). A session with a deadline a minute away arms
/// first, then six sleepers in the order 1, 2, 4, 5, 6, 3 of their
/// deadlines: the heap is `[1, 4, 2, session, 5, 6, 3]`. When the session
/// ends, sleeper 3, the last entry, takes its place under sleeper 4. With a
/// removal that only sifts down it stays there and wakes after sleeper 4,
/// at that one's deadline; no other case of the crate saw that.
#[test]
fn the_entry_that_takes_a_removed_ones_place_moves_up_past_a_later_one() {
    passed(&child(
        "timer_adversary::child_removal_under_a_later_entry",
        &[("IGNITE_WORKERS", "1")],
    ));
}

#[test]
fn child_removal_under_a_later_entry() {
    if !in_child() {
        return;
    }
    const ORDER: [u32; 6] = [1, 2, 4, 5, 6, 3];
    const LEAD: Duration = Duration::from_millis(1500);
    const STEP: Duration = Duration::from_millis(60);
    let (began, begun) = mpsc::channel::<()>();
    let (ended, endings) = mpsc::channel::<bool>();
    // A task has armed its timer before the next one starts: one worker
    // runs it until it waits, and the next is spawned after it began.
    let session = |idle: i64| {
        let (stream, peer) = pair();
        if idle > 0 {
            stream.set_idle_timeout(idle).unwrap();
        }
        let ended = ended.clone();
        let began = began.clone();
        spawn(move || {
            let _ = began.send(());
            while let Ok(Input::Data(_)) = stream.read(16) {}
            let _ = ended.send(stream.timed_out());
        });
        begun.recv_timeout(LIMIT).expect("a session began to read");
        peer
    };
    // Without a deadline: keeps the others from being alone on the worker.
    let keeper = session(0);
    let far = session(60_000);
    let start = Instant::now() + LEAD;
    let (woke, wakes) = mpsc::channel();
    for rank in ORDER {
        let woke = woke.clone();
        let began = began.clone();
        let at = start + STEP * rank;
        spawn(move || {
            let _ = began.send(());
            ignite_io::wait::sleep_until(at);
            let _ = woke.send((rank, Instant::now() >= at));
        });
        begun.recv_timeout(LIMIT).expect("a sleeper began to sleep");
    }
    assert!(
        until(|| workers().iter().all(|(state, _)| *state == 'S')),
        "the worker never slept: {:?}",
        workers()
    );
    assert_eq!(workers().len(), 1, "one worker expected: {:?}", workers());
    drop(far);
    assert_eq!(
        endings.recv_timeout(LIMIT).ok(),
        Some(false),
        "the session a minute from its timeout ended when its peer closed"
    );
    assert!(
        Instant::now() < start,
        "the session ended after the first sleeper's deadline: nothing to watch"
    );
    let mut order = Vec::new();
    for _ in ORDER {
        let (rank, due) = wakes
            .recv_timeout(LIMIT)
            .unwrap_or_else(|_| panic!("a sleeper's timer did not fire; woke so far: {order:?}"));
        assert!(due, "sleeper {rank} woke before its deadline");
        order.push(rank);
    }
    assert_eq!(
        order,
        [1, 2, 3, 4, 5, 6],
        "the sleepers wake in the order of their deadlines"
    );
    drop(keeper);
    assert_eq!(
        endings.recv_timeout(LIMIT).ok(),
        Some(false),
        "the session without a deadline ended when its peer closed"
    );
    wait_for_tasks();
    assert!(wakes.try_recv().is_err(), "a sleeper woke twice");
}
