//! Task threads (`docs/slice.md` section 7): [`spawn`] runs each task on an
//! OS thread of its own for as long as the task runs, and reuses threads
//! that finished a task within [`LINGER`] instead of starting new ones.
//!
//! A worker that finished a task puts itself on the idle stack and parks.
//! `spawn` pops the most recently idle worker (its stack and caches are
//! warm), puts the task in the worker's slot and unparks it; only when the
//! stack is empty does it start a thread. A worker that waited [`LINGER`]
//! without a task takes itself off the stack under the stack's lock and
//! ends; if a `spawn` popped it first, that `spawn` fills (or has filled)
//! its slot, so the worker waits for the task and runs it. A task that
//! fails or panics (`docs/slice.md` section 14.8) is caught at the worker:
//! `task ended with error: <report>` is logged, and the worker takes its
//! next task as after any other task.
//!
//! Reuse keeps a thread's thread-locals across tasks. Every task ends with
//! [`crate::flush`], so the substrate's pending writes are empty in between;
//! the LLVM runtime ends every task with `task_end`, which resets the arena.
//!
//! Freed memory. glibc keeps freed heap pages mapped, in one allocator
//! arena per thread that allocated them, so a process whose tasks run on
//! many threads keeps the high-water mark of every thread's arena. A
//! `Store` that drops values and an LLVM task whose arena grew past 1 MiB
//! call [`released_memory`]; the worker of the next task that ends then
//! calls `malloc_trim(0)`, which returns the free pages of every arena to
//! the kernel. A task that freed nothing pays one atomic load.
//!
//! The trim walks every free chunk of every arena, holding each arena's
//! lock in turn: on a fragmented heap of 1 to 2 GiB it took 68 to 138 ms,
//! and 12 to 25 ms again when nothing was left to return (glibc 2.44,
//! 2026-11-04). So one worker trims at a time, a task's end trims at most
//! once per [`TRIM_INTERVAL`], and a worker that ends after lingering
//! trims what is still marked, so the last release is not left behind.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, TryLockError};
use std::thread::Thread;
use std::time::{Duration, Instant};

use crate::IoError;

/// How long a worker without a task waits for one before it ends.
pub const LINGER: Duration = Duration::from_secs(1);

type Job = Box<dyn FnOnce() + Send + 'static>;

/// Tasks handed to [`spawn`] that have not ended: counted from the call
/// until the task returned or failed. The drain (`docs/slice.md` 16.13)
/// waits for zero.
static LIVE: Mutex<usize> = Mutex::new(0);
static ENDED: Condvar = Condvar::new();

fn task_started() {
    *lock(&LIVE) += 1;
}

fn task_ended() {
    let mut live = lock(&LIVE);
    *live = live.saturating_sub(1);
    ENDED.notify_all();
}

/// The number of tasks started and not yet ended.
pub fn live_tasks() -> usize {
    *lock(&LIVE)
}

/// Wait until every task started with [`spawn`] has ended. No bound: a
/// task that never ends keeps the caller waiting (`docs/slice.md` 16.13,
/// A6: outside `ignite run` only the driver's deadline ends a drain).
pub fn wait_for_tasks() {
    let mut live = lock(&LIVE);
    while *live > 0 {
        live = ENDED.wait(live).unwrap_or_else(PoisonError::into_inner);
    }
}

/// An idle worker: its thread and the slot `spawn` puts its next task in.
struct Worker {
    thread: Thread,
    next: Mutex<Option<Job>>,
}

/// Idle workers, most recently idle last.
static IDLE: Mutex<Vec<Arc<Worker>>> = Mutex::new(Vec::new());

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Start `task` on an idle worker or a new OS thread and return at once. A
/// failure or panic in the task ends only the task: its report is logged
/// as `task ended with error: <report>` and the process continues. When
/// the operating system cannot start a thread, `task` is dropped without
/// running, as [`try_spawn`] describes: a task that owns a stream closes it.
pub fn spawn<F>(task: F)
where
    F: FnOnce() + Send + 'static,
{
    let _ = try_spawn(task);
}

/// [`spawn`], reporting a thread that could not be started. Then `task` is
/// dropped without running, and the first failure after a successful
/// start logs `cannot start a task: <reason>`; the process continues.
pub fn try_spawn<F>(task: F) -> Result<(), IoError>
where
    F: FnOnce() + Send + 'static,
{
    let job: Job = Box::new(task);
    task_started();
    let worker = lock(&IDLE).pop();
    if let Some(worker) = worker {
        *lock(&worker.next) = Some(job);
        worker.thread.unpark();
        return Ok(());
    }
    match std::thread::Builder::new().spawn(move || work(job)) {
        Ok(_) => {
            FAILING.store(false, Ordering::Relaxed);
            Ok(())
        }
        Err(error) => {
            task_ended();
            let error = IoError::new(format!("cannot start a task: {error}"));
            if !FAILING.swap(true, Ordering::Relaxed) {
                crate::log(&error.to_string());
            }
            Err(error)
        }
    }
}

/// The shortest time from one trim to the next trim at a task's end.
const TRIM_INTERVAL: Duration = Duration::from_secs(1);

/// Memory went back to the allocator since the last trim.
static RELEASED: AtomicBool = AtomicBool::new(false);

/// When the last trim ended; held by the worker that trims.
static LAST_TRIM: Mutex<Option<Instant>> = Mutex::new(None);

#[cfg(test)]
thread_local! {
    /// Calls of [`released_memory`] on this thread, for tests.
    pub(crate) static RELEASES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Note that the current task gave a noticeable amount of memory back to
/// the allocator: a `Store` dropped values, or a task arena was freed. A
/// later task's end, or the end of a thread that lingered, returns the
/// free heap pages to the kernel.
pub fn released_memory() {
    #[cfg(test)]
    RELEASES.with(|n| n.set(n.get() + 1));
    if !RELEASED.load(Ordering::Relaxed) {
        RELEASED.store(true, Ordering::Relaxed);
    }
}

/// Trim the heap and clear `released` when it is set, no other worker
/// holds `last`, and the last trim ended at least `interval` ago; whether
/// it trimmed. A trim that is not due leaves `released` set.
fn trim_if_released(
    released: &AtomicBool,
    last: &Mutex<Option<Instant>>,
    interval: Duration,
) -> bool {
    if !released.load(Ordering::Relaxed) {
        return false;
    }
    let mut last = match last.try_lock() {
        Ok(last) => last,
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => return false,
    };
    if last.is_some_and(|at| at.elapsed() < interval) {
        return false;
    }
    if !released.swap(false, Ordering::Relaxed) {
        return false;
    }
    trim_heap();
    *last = Some(Instant::now());
    true
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn trim_heap() {
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }
    // SAFETY: glibc's `malloc_trim` takes no pointer and is thread-safe.
    unsafe { malloc_trim(0) };
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn trim_heap() {}

/// A thread failed to start since the last one that started.
static FAILING: AtomicBool = AtomicBool::new(false);

/// Run `job`, then the tasks `spawn` hands this thread, until it lingered.
fn work(mut job: Job) {
    let me = Arc::new(Worker {
        thread: std::thread::current(),
        next: Mutex::new(None),
    });
    loop {
        if let Err(report) = crate::report::catch(job) {
            crate::report::task_failed(&report);
        }
        crate::flush();
        task_ended();
        trim_if_released(&RELEASED, &LAST_TRIM, TRIM_INTERVAL);
        lock(&IDLE).push(Arc::clone(&me));
        let deadline = Instant::now() + LINGER;
        job = loop {
            if let Some(next) = lock(&me.next).take() {
                break next;
            }
            let now = Instant::now();
            if now < deadline {
                std::thread::park_timeout(deadline - now);
                continue;
            }
            let mut idle = lock(&IDLE);
            if let Some(at) = idle.iter().position(|other| Arc::ptr_eq(other, &me)) {
                idle.remove(at);
                drop(idle);
                trim_if_released(&RELEASED, &LAST_TRIM, Duration::ZERO);
                return;
            }
            drop(idle);
            let next = loop {
                if let Some(next) = lock(&me.next).take() {
                    break next;
                }
                std::thread::park();
            };
            break next;
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    /// One test: the idle stack is global, so a parallel test would hand
    /// its own workers to these tasks.
    #[test]
    fn workers_are_reused_and_run_every_task_once() {
        a_finished_worker_runs_the_next_task();
        a_failed_task_leaves_its_worker_to_the_next();
        every_task_runs_once_under_contention();
        the_drain_waits_for_every_live_task();
    }

    #[test]
    fn a_trim_runs_once_per_release_and_interval() {
        let released = AtomicBool::new(false);
        let last = Mutex::new(None);
        let hour = Duration::from_secs(3600);
        assert!(!trim_if_released(&released, &last, hour));
        released.store(true, Ordering::Relaxed);
        assert!(trim_if_released(&released, &last, hour));
        assert!(!released.load(Ordering::Relaxed));
        assert!(!trim_if_released(&released, &last, Duration::ZERO));

        released.store(true, Ordering::Relaxed);
        assert!(!trim_if_released(&released, &last, hour), "not due");
        assert!(
            released.load(Ordering::Relaxed),
            "a trim not due keeps the mark"
        );
        let held = last.lock().unwrap();
        assert!(
            !trim_if_released(&released, &last, Duration::ZERO),
            "another worker trims"
        );
        drop(held);
        assert!(trim_if_released(&released, &last, Duration::ZERO));
        assert!(!released.load(Ordering::Relaxed));
    }

    /// `wait_for_tasks` (the drain, 16.13) returns once the last task
    /// ended, a failed one included, and not before.
    fn the_drain_waits_for_every_live_task() {
        wait_for_tasks();
        assert_eq!(live_tasks(), 0);
        let (release, held) = channel::<()>();
        let held = Arc::new(Mutex::new(held));
        for i in 0..3 {
            let held = Arc::clone(&held);
            spawn(move || {
                lock(&held).recv().unwrap();
                if i == 2 {
                    crate::fail("assert", "t.ignite", 2, 1, "a failed task also ends");
                }
            });
        }
        assert_eq!(live_tasks(), 3);
        let (done, waited) = channel();
        std::thread::spawn(move || {
            wait_for_tasks();
            done.send(()).unwrap();
        });
        assert!(
            waited.recv_timeout(Duration::from_millis(100)).is_err(),
            "the wait ended with tasks live"
        );
        for _ in 0..3 {
            release.send(()).unwrap();
        }
        waited
            .recv_timeout(Duration::from_secs(5))
            .expect("the wait ends when the tasks end");
        assert_eq!(live_tasks(), 0);
    }

    fn a_failed_task_leaves_its_worker_to_the_next() {
        let (done, finished) = channel();
        let first = done.clone();
        spawn(move || {
            first.send(std::thread::current().id()).unwrap();
            crate::fail(
                "index",
                "t.ignite",
                1,
                1,
                "index 3 out of range for length 0",
            );
        });
        let a = finished.recv().unwrap();
        assert!(reaches(a), "no task ran on the failed task's thread");
    }

    /// How many spawns pop the idle worker on thread `id`, once it pushed
    /// itself onto [`IDLE`], waiting at most 5 s for that: the stack is
    /// LIFO, and under load a worker of another test can sit above it.
    fn depth_of_idle(id: std::thread::ThreadId) -> Option<usize> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            {
                let idle = lock(&IDLE);
                if let Some(at) = idle.iter().position(|w| w.thread.id() == id) {
                    return Some(idle.len() - at);
                }
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Whether a task runs on thread `id` again: each attempt waits until
    /// its worker is idle, then holds as many tasks at once as the workers
    /// at and above it, so every one of them takes a task. An attempt
    /// misses only when another worker went idle in between.
    fn reaches(id: std::thread::ThreadId) -> bool {
        for _ in 0..50 {
            let Some(depth) = depth_of_idle(id) else {
                return false;
            };
            let (release, held) = channel::<()>();
            let held = Arc::new(Mutex::new(held));
            let (done, ran) = channel();
            for _ in 0..depth {
                let held = Arc::clone(&held);
                let done = done.clone();
                spawn(move || {
                    done.send(std::thread::current().id()).unwrap();
                    lock(&held).recv().unwrap();
                });
            }
            let ids: Vec<_> = (0..depth).map(|_| ran.recv().unwrap()).collect();
            for _ in 0..depth {
                release.send(()).unwrap();
            }
            if ids.contains(&id) {
                return true;
            }
        }
        false
    }

    fn a_finished_worker_runs_the_next_task() {
        let (done, finished) = channel();
        let first = done.clone();
        spawn(move || first.send(std::thread::current().id()).unwrap());
        let a = finished.recv().unwrap();
        assert!(reaches(a), "no task ran on the first task's thread");
    }

    fn every_task_runs_once_under_contention() {
        let (done, finished) = channel();
        for round in 0..20 {
            for i in 0..50 {
                let done = done.clone();
                spawn(move || done.send(round * 50 + i).unwrap());
            }
        }
        let mut seen: Vec<i32> = (0..1000).map(|_| finished.recv().unwrap()).collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..1000).collect::<Vec<_>>());
    }
}
