//! The waiter seam (`docs/readiness-io.md` 4.2, `ignite_io::wait`) over its
//! thread-per-task implementation: a park blocks the thread until a wake
//! or its deadline, a wake given first is kept, a wait list wakes the
//! longest waiting first, and a thread waiting for a descriptor is ended
//! by readiness or by a wake.

use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ignite_io::reactor::Interest;
use ignite_io::wait::{
    DescriptorWait, WaitList, Waiter, Woken, park, park_in, sleep, wait_descriptor, wake,
};

const LIMIT: Duration = Duration::from_secs(5);
const SHORT: Duration = Duration::from_millis(50);

/// Run `body` on a thread that hands its waiter over first; the waiter and
/// the receiver of the body's result.
fn parked<T: Send + 'static>(
    body: impl FnOnce(Waiter) -> T + Send + 'static,
) -> (Waiter, mpsc::Receiver<T>) {
    let (send, waiter) = mpsc::channel();
    let (done, result) = mpsc::channel();
    std::thread::spawn(move || {
        let me = Waiter::current();
        send.send(me.clone()).unwrap();
        let _ = done.send(body(me));
    });
    (waiter.recv_timeout(LIMIT).unwrap(), result)
}

#[test]
fn a_wake_from_another_thread_ends_a_park() {
    let (waiter, result) = parked(|me| park(&me, None));
    std::thread::sleep(SHORT);
    assert!(result.try_recv().is_err(), "the park waits for the wake");
    wake(&waiter);
    assert_eq!(result.recv_timeout(LIMIT).unwrap(), Woken::Notified);
}

#[test]
fn a_wake_before_the_park_is_kept() {
    let me = Waiter::current();
    wake(&me);
    wake(&me);
    let start = Instant::now();
    assert_eq!(park(&me, Some(start + LIMIT)), Woken::Notified);
    assert!(start.elapsed() < LIMIT);
    assert_eq!(
        park(&me, Some(Instant::now() + SHORT)),
        Woken::Deadline,
        "two wakes before a park are one"
    );
}

#[test]
fn a_park_ends_at_its_deadline() {
    let me = Waiter::current();
    let start = Instant::now();
    assert_eq!(park(&me, Some(start + SHORT)), Woken::Deadline);
    assert!(start.elapsed() >= SHORT);
}

#[test]
fn the_waiter_of_a_thread_is_one() {
    assert!(Waiter::current().same(&Waiter::current()));
    let other = std::thread::spawn(Waiter::current).join().unwrap();
    assert!(!other.same(&Waiter::current()));
}

#[test]
fn sleep_waits_its_duration_through_wakes() {
    let (waiter, result) = parked(|_| {
        let start = Instant::now();
        sleep(Duration::from_millis(100));
        start.elapsed()
    });
    wake(&waiter);
    let slept = result.recv_timeout(LIMIT).unwrap();
    assert!(slept >= Duration::from_millis(100), "slept {slept:?}");
}

/// A condition with its wait list under one lock.
#[derive(Default)]
struct Gate {
    open: usize,
    waiting: WaitList,
}

/// Wait until the gate has an opening, then take it.
fn pass(gate: &Mutex<Gate>) {
    let mut state = gate.lock().unwrap();
    while state.open == 0 {
        state = park_in(gate, state, |g| &mut g.waiting, None).0;
    }
    state.open -= 1;
}

#[test]
fn a_wait_list_wakes_one_waiter_per_opening() {
    let gate = Arc::new(Mutex::new(Gate::default()));
    let (done, passed) = mpsc::channel();
    for n in 0..3 {
        let gate = Arc::clone(&gate);
        let done = done.clone();
        std::thread::spawn(move || {
            pass(&gate);
            done.send(n).unwrap();
        });
    }
    std::thread::sleep(SHORT);
    for opened in 1..=3 {
        let mut state = gate.lock().unwrap();
        state.open += 1;
        state.waiting.wake_one();
        drop(state);
        passed.recv_timeout(LIMIT).unwrap();
        std::thread::sleep(SHORT);
        assert!(
            passed.try_recv().is_err(),
            "one waiter per opening ({opened})"
        );
    }
    assert!(gate.lock().unwrap().waiting.is_empty());
}

#[test]
fn a_timed_out_waiter_leaves_the_list() {
    let gate = Mutex::new(Gate::default());
    let state = gate.lock().unwrap();
    let (state, woken) = park_in(
        &gate,
        state,
        |g| &mut g.waiting,
        Some(Instant::now() + SHORT),
    );
    assert_eq!(woken, Woken::Deadline);
    assert!(state.waiting.is_empty(), "the waiter took itself off");
}

#[test]
fn readiness_ends_a_descriptor_wait() {
    let (watched, mut peer) = UnixStream::pair().unwrap();
    let (_waiter, result) = parked(move |_| {
        wait_descriptor(
            watched.as_raw_fd(),
            Interest::READ,
            Some(Instant::now() + LIMIT),
        )
        .unwrap()
    });
    std::thread::sleep(SHORT);
    peer.write_all(b"x").unwrap();
    match result.recv_timeout(LIMIT).unwrap() {
        DescriptorWait::Ready(ready) => assert!(ready.contains(Interest::READ)),
        other => panic!("not ready: {other:?}"),
    }
}

#[test]
fn a_wake_ends_a_descriptor_wait() {
    let (watched, _peer) = UnixStream::pair().unwrap();
    let (waiter, result) =
        parked(move |_| wait_descriptor(watched.as_raw_fd(), Interest::READ, None).unwrap());
    std::thread::sleep(SHORT);
    assert!(result.try_recv().is_err(), "nothing ended the wait yet");
    wake(&waiter);
    assert_eq!(result.recv_timeout(LIMIT).unwrap(), DescriptorWait::Woken);
}

#[test]
fn a_descriptor_wait_ends_at_its_deadline() {
    let (watched, _peer) = UnixStream::pair().unwrap();
    let start = Instant::now();
    let outcome =
        wait_descriptor(watched.as_raw_fd(), Interest::READ, Some(start + SHORT)).unwrap();
    assert_eq!(outcome, DescriptorWait::Deadline);
    assert!(start.elapsed() >= SHORT);
}

/// Every wait of the substrate goes through the waiter: no condition
/// variable is left in `src/` outside `wait.rs`.
#[test]
fn no_condition_variable_outside_the_waiter() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&source).unwrap() {
        let file = entry.unwrap().path();
        if file.file_name().is_some_and(|name| name == "wait.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&file).unwrap();
        for (number, line) in text.lines().enumerate() {
            if line.contains("Condvar") {
                found.push(format!("{}:{}", file.display(), number + 1));
            }
        }
    }
    assert!(found.is_empty(), "condition variables left: {found:?}");
}
