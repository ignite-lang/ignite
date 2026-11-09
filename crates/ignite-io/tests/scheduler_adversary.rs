//! Adversary cases of `story:readiness-connect-latency`: stacks kept per
//! worker (`sched.rs` `take_stack`, `give_stack`, `Worker::retire`) and
//! the counters `ignite_io::scheduler_counts()`.
//!
//! Every case runs in a child process, as in `scheduler.rs`: the IO mode
//! and the worker count are read once per process, and the counters are
//! process-wide.

use std::process::{Command, Output};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ignite_io::census::{self, Kind};
use ignite_io::{
    Channel, Input, LINGER, SchedulerCounts, TcpListener, scheduler_counts, spawn, try_spawn,
    wait_for_tasks,
};

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

/// The unit's own identity: every spawn's stack is mapped, from a worker's
/// cache or from the pool.
fn stacks_found(counts: &SchedulerCounts) -> u64 {
    counts.stacks_allocated + counts.stacks_from_cache + counts.stacks_from_pool
}

#[allow(unconditional_recursion)]
fn deeper(depth: u64, done: &mpsc::Sender<u64>) -> u64 {
    let frame = std::hint::black_box([depth; 64]);
    if depth == u64::MAX {
        let _ = done.send(depth);
    }
    deeper(depth + 1, done) + frame[3]
}

/// An overflowing task that waits for `go` before it recurses, so that the
/// spawning thread reads the counts first.
fn overflow_after(go: ignite_io::Receiver<bool>) -> impl FnOnce() + Send + 'static {
    move || {
        let _ = go.receive();
        let (done, _finished) = mpsc::channel();
        deeper(0, &done);
    }
}

fn aborted_on_overflow(output: &Output, line: &str) {
    use std::os::unix::process::ExitStatusExt;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains(line),
        "the overflowing task did not get the stack the case built ({line}):\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("has overflowed its stack"),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(output.status.signal(), Some(6), "{:?}", output.status);
}

/// Attack 3: a task whose stack another task left in its worker's cache
/// still hits a guard page that `on_fault` recognises.
#[test]
fn an_overflow_on_a_stack_from_the_workers_cache_is_reported_and_aborts() {
    let output = child(
        "scheduler_adversary::child_overflow_cached",
        &[("IGNITE_WORKERS", "1")],
    );
    aborted_on_overflow(&output, "stack from cache: 1");
}

#[test]
fn child_overflow_cached() {
    if !in_child() {
        return;
    }
    spawn(|| {});
    wait_for_tasks();
    let (go, wait) = Channel::bounded::<bool>(1);
    let before = scheduler_counts();
    spawn(overflow_after(wait));
    let after = scheduler_counts();
    println!(
        "stack from cache: {}",
        after.stacks_from_cache - before.stacks_from_cache
    );
    go.send(true);
    std::thread::sleep(LIMIT);
    panic!("the overflow did not end the process");
}

/// Attack 3: a task whose stack came back through the shared pool (its
/// worker's cache was full) still hits a recognised guard page.
#[test]
fn an_overflow_on_a_stack_from_the_shared_pool_is_reported_and_aborts() {
    let output = child(
        "scheduler_adversary::child_overflow_pooled",
        &[("IGNITE_WORKERS", "1")],
    );
    aborted_on_overflow(&output, "stack from pool: 1");
}

#[test]
fn child_overflow_pooled() {
    if !in_child() {
        return;
    }
    // Three tasks at once on the one worker: when they end, two stacks stay
    // in its cache and the third goes to the pool.
    let (release, held) = Channel::bounded::<bool>(3);
    for _ in 0..3 {
        let held = held.clone();
        spawn(move || {
            let _ = held.receive();
        });
    }
    for _ in 0..3 {
        release.send(true);
    }
    wait_for_tasks();
    let parked = scheduler_counts();
    assert_eq!(parked.stacks_to_pool, 1, "{parked:?}");
    // Two hold the cached stacks, the third takes the pooled one.
    let (keep, kept) = Channel::bounded::<bool>(2);
    for _ in 0..2 {
        let kept = kept.clone();
        spawn(move || {
            let _ = kept.receive();
        });
    }
    let (go, wait) = Channel::bounded::<bool>(1);
    let before = scheduler_counts();
    spawn(overflow_after(wait));
    let after = scheduler_counts();
    println!(
        "stack from pool: {}",
        after.stacks_from_pool - before.stacks_from_pool
    );
    go.send(true);
    std::thread::sleep(LIMIT);
    drop(keep);
    panic!("the overflow did not end the process");
}

/// `docs/readiness-io.md` 4.1 as landed: "a connection accepted in `main`
/// and served on a worker takes no pool lock and maps no stack". Built
/// deterministically with 2 workers: worker 1 holds a free stack in its
/// cache and one task; the connection accepted in `main` next starts
/// worker 2 (`choose` spreads while a worker may start), whose stack
/// comes from the pool or a fresh map (`new_stack(None)`), never from
/// worker 1's cache.
#[test]
fn a_connection_accepted_in_main_maps_no_stack_while_one_is_free() {
    passed(&child(
        "scheduler_adversary::child_accept_while_a_stack_is_free",
        &[("IGNITE_WORKERS", "2")],
    ));
}

#[test]
fn child_accept_while_a_stack_is_free() {
    use std::io::{Read, Write};
    if !in_child() {
        return;
    }
    // Worker 1 runs a task and the task it spawns (it stays on its worker
    // while another may start): both stacks end in worker 1's cache.
    spawn(|| spawn(|| {}));
    wait_for_tasks();
    let warm = scheduler_counts();
    assert_eq!(
        (warm.workers_started, warm.stacks_allocated),
        (1, 2),
        "{warm:?}"
    );
    // A session parked on worker 1 takes one of them; one stays free there.
    let (release, held) = Channel::bounded::<bool>(1);
    spawn(move || {
        let _ = held.receive();
    });
    let parked = scheduler_counts();
    assert_eq!(
        parked.stacks_from_cache - warm.stacks_from_cache,
        1,
        "{parked:?}"
    );
    // A connection accepted in `main`.
    let listener = TcpListener::bind(LOCALHOST.to_string(), 0).unwrap();
    let port = listener.local_port() as u16;
    let peer = std::thread::spawn(move || {
        let mut peer = std::net::TcpStream::connect((LOCALHOST, port)).unwrap();
        peer.set_read_timeout(Some(LIMIT)).unwrap();
        peer.write_all(b"ping").unwrap();
        let mut reply = [0u8; 4];
        peer.read_exact(&mut reply).unwrap();
        assert_eq!(&reply, b"ping");
    });
    let stream = listener.accept().unwrap();
    spawn(move || {
        if let Ok(Input::Data(bytes)) = stream.read(64) {
            let _ = stream.write_all(bytes);
        }
    });
    let served = scheduler_counts();
    peer.join().unwrap();
    release.send(true);
    wait_for_tasks();
    println!("warm {warm:?}\nparked {parked:?}\nserved {served:?}");
    assert_eq!(
        (
            served.stacks_allocated - parked.stacks_allocated,
            served.stacks_from_pool - parked.stacks_from_pool,
        ),
        (0, 0),
        "the connection mapped a stack or took the pool while worker 1 held a free one\nparked {parked:?}\nserved {served:?}"
    );
}

/// Attack 1: spawns at the moment the one worker retires (after
/// [`LINGER`] idle), from `main` and from a second thread at once. Every
/// task runs, every spawn's stack is accounted for, and the workers end.
#[test]
fn spawns_racing_a_retiring_worker_lose_no_task() {
    passed(&child(
        "scheduler_adversary::child_retire_race",
        &[("IGNITE_WORKERS", "1")],
    ));
}

#[test]
fn child_retire_race() {
    if !in_child() {
        return;
    }
    const ROUNDS: i64 = 12;
    let (ran, runs) = mpsc::channel::<u64>();
    let mut expected = 0;
    for round in 0..ROUNDS {
        let first = ran.clone();
        spawn(move || first.send(0).unwrap());
        expected += 1;
        runs.recv_timeout(LIMIT)
            .expect("the round's first task ran");
        expected -= 1;
        wait_for_tasks();
        // Around the worker's retirement: LINGER after the task ended,
        // give or take a few milliseconds.
        let offset = Duration::from_millis((round * 2) as u64);
        std::thread::sleep(LINGER - Duration::from_millis(ROUNDS as u64) + offset);
        let other = {
            let ran = ran.clone();
            std::thread::spawn(move || {
                spawn(move || ran.send(1).unwrap());
            })
        };
        spawn({
            let ran = ran.clone();
            move || ran.send(2).unwrap()
        });
        expected += 2;
        other.join().unwrap();
        for _ in 0..2 {
            runs.recv_timeout(LIMIT)
                .expect("a task spawned while the worker retired ran");
            expected -= 1;
        }
        wait_for_tasks();
    }
    assert_eq!(expected, 0);
    let counts = scheduler_counts();
    println!("{counts:?}");
    assert_eq!(counts.spawns, 3 * ROUNDS as u64, "{counts:?}");
    assert_eq!(stacks_found(&counts), counts.spawns, "{counts:?}");
    let deadline = Instant::now() + LIMIT + LINGER;
    while census::count(Kind::Workers) != 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(census::count(Kind::Workers), 0, "a worker did not retire");
}

/// Attacks 1 and 4: a spawn whose stack cannot be mapped (the address
/// space limit reached) after its task was counted on the chosen worker.
/// The spawn reports the failure, the worker still ends, and the counts
/// keep the identity the unit's own test asserts: every spawn counted had
/// its stack found (`SchedulerCounts::spawns` is "Tasks started").
#[test]
fn a_spawn_that_cannot_map_a_stack_is_not_counted_as_started() {
    passed(&child(
        "scheduler_adversary::child_stack_map_fails",
        &[("IGNITE_WORKERS", "1")],
    ));
}

fn address_space() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let line = status
        .lines()
        .find(|line| line.starts_with("VmSize:"))
        .unwrap();
    let kib: u64 = line
        .trim_start_matches("VmSize:")
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .unwrap();
    kib * 1024
}

#[test]
fn child_stack_map_fails() {
    use nix::sys::resource::{Resource, getrlimit, setrlimit};
    if !in_child() {
        return;
    }
    // One task holds the only stack: the worker's cache and the pool are
    // empty, so the next spawn on it must map one.
    let (release, held) = Channel::bounded::<bool>(1);
    // The worker's thread has started (its own mappings are made) before
    // the address space is limited.
    let (running, started) = mpsc::channel::<()>();
    spawn(move || {
        running.send(()).unwrap();
        let _ = held.receive();
    });
    started.recv_timeout(LIMIT).expect("the holding task ran");
    let before = scheduler_counts();
    let (soft, hard) = getrlimit(Resource::RLIMIT_AS).unwrap();
    setrlimit(Resource::RLIMIT_AS, address_space() + 1024 * 1024, hard).unwrap();
    let failed = try_spawn(|| {});
    setrlimit(Resource::RLIMIT_AS, soft, hard).unwrap();
    let after = scheduler_counts();
    assert!(failed.is_err(), "a stack was mapped under the limit");
    release.send(true);
    wait_for_tasks();
    println!("before {before:?}\nafter {after:?}");
    let deadline = Instant::now() + LIMIT + LINGER;
    while census::count(Kind::Workers) != 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(census::count(Kind::Workers), 0, "the worker did not retire");
    assert_eq!(
        stacks_found(&after),
        after.spawns,
        "a spawn that started no task counted as one: {after:?}"
    );
}

/// Wait until `live` tasks are left (`census` kind `Tasks`).
fn tasks_left(live: i64) {
    let deadline = Instant::now() + LIMIT;
    while census::count(Kind::Tasks) != live && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(census::count(Kind::Tasks), live, "tasks left");
}

/// Pass 2, `docs/readiness-io.md` 4.1 as revised by ac15a522: "a
/// connection accepted in `main` and served on a worker takes no pool lock
/// and maps no stack while a worker holds a free one". Built with 2
/// workers: worker 2 holds a free stack and two tasks, worker 1 one task
/// and no free stack; the connection accepted next goes to worker 1, the
/// least loaded, and `free_stack` looks only at its cache and the pool.
#[test]
fn a_connection_accepted_in_main_maps_no_stack_while_another_worker_holds_one() {
    passed(&child(
        "scheduler_adversary::child_accept_while_another_worker_holds_a_stack",
        &[("IGNITE_WORKERS", "2")],
    ));
}

#[test]
fn child_accept_while_another_worker_holds_a_stack() {
    use std::io::{Read, Write};
    if !in_child() {
        return;
    }
    let (release, held) = Channel::bounded::<bool>(4);
    // Worker 1: one parked task.
    let first = held.clone();
    spawn(move || {
        let _ = first.receive();
    });
    tasks_left(1);
    // Worker 2 (started for it): a task that keeps one task of its own
    // parked there and lets a second end, whose stack stays in worker 2's
    // cache.
    let second = held.clone();
    spawn(move || {
        let kept = second.clone();
        spawn(move || {
            let _ = kept.receive();
        });
        spawn(|| {});
        let _ = second.receive();
    });
    tasks_left(3);
    let set = scheduler_counts();
    assert_eq!(set.workers_started, 2, "{set:?}");
    // A connection accepted in `main`.
    let listener = TcpListener::bind(LOCALHOST.to_string(), 0).unwrap();
    let port = listener.local_port() as u16;
    let peer = std::thread::spawn(move || {
        let mut peer = std::net::TcpStream::connect((LOCALHOST, port)).unwrap();
        peer.set_read_timeout(Some(LIMIT)).unwrap();
        peer.write_all(b"ping").unwrap();
        let mut reply = [0u8; 4];
        peer.read_exact(&mut reply).unwrap();
        assert_eq!(&reply, b"ping");
    });
    let stream = listener.accept().unwrap();
    spawn(move || {
        if let Ok(Input::Data(bytes)) = stream.read(64) {
            let _ = stream.write_all(bytes);
        }
    });
    let served = scheduler_counts();
    peer.join().unwrap();
    for _ in 0..3 {
        release.send(true);
    }
    wait_for_tasks();
    println!("set {set:?}\nserved {served:?}");
    assert_eq!(
        (
            served.stacks_allocated - set.stacks_allocated,
            served.stacks_from_pool - set.stacks_from_pool,
        ),
        (0, 0),
        "the connection mapped a stack or took the pool while worker 2 held a free one\nset {set:?}\nserved {served:?}"
    );
}

/// Pass 2, the take-back-and-replace path of ac15a522: many threads spawn
/// at once while tasks end and workers start, so a spawn often finds no
/// free stack, maps one unlocked and places again. Every task runs exactly
/// once, every spawn counted has its stack counted, and the workers end.
#[test]
fn concurrent_spawns_through_the_retry_place_every_task_once() {
    passed(&child(
        "scheduler_adversary::child_retry_storm",
        &[("IGNITE_WORKERS", "4")],
    ));
}

#[test]
fn child_retry_storm() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU8, Ordering};
    if !in_child() {
        return;
    }
    const SPAWNERS: usize = 8;
    const EACH: usize = 300;
    let runs: Arc<Vec<AtomicU8>> =
        Arc::new((0..SPAWNERS * EACH * 2).map(|_| AtomicU8::new(0)).collect());
    let spawners: Vec<_> = (0..SPAWNERS)
        .map(|spawner| {
            let runs = Arc::clone(&runs);
            std::thread::spawn(move || {
                for each in 0..EACH {
                    let id = 2 * (spawner * EACH + each);
                    let runs = Arc::clone(&runs);
                    spawn(move || {
                        runs[id].fetch_add(1, Ordering::SeqCst);
                        // A task spawning a task: placed from a worker.
                        let inner = Arc::clone(&runs);
                        spawn(move || {
                            inner[id + 1].fetch_add(1, Ordering::SeqCst);
                        });
                        if id % 3 == 0 {
                            ignite_io::clock::sleep(1_000_000);
                        }
                    });
                }
            })
        })
        .collect();
    for spawner in spawners {
        spawner.join().unwrap();
    }
    wait_for_tasks();
    let wrong: Vec<(usize, u8)> = runs
        .iter()
        .enumerate()
        .map(|(id, ran)| (id, ran.load(Ordering::SeqCst)))
        .filter(|&(_, ran)| ran != 1)
        .collect();
    let counts = scheduler_counts();
    println!("{counts:?}");
    assert!(wrong.is_empty(), "tasks not run exactly once: {wrong:?}");
    assert_eq!(counts.spawns, (SPAWNERS * EACH * 2) as u64, "{counts:?}");
    assert_eq!(stacks_found(&counts), counts.spawns, "{counts:?}");
    let deadline = Instant::now() + LIMIT + LINGER;
    while census::count(Kind::Workers) != 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(census::count(Kind::Workers), 0, "a worker did not retire");
}
