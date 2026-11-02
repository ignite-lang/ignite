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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::Thread;
use std::time::{Duration, Instant};

use crate::IoError;

/// How long a worker without a task waits for one before it ends.
pub const LINGER: Duration = Duration::from_secs(1);

type Job = Box<dyn FnOnce() + Send + 'static>;

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
            let error = IoError::new(format!("cannot start a task: {error}"));
            if !FAILING.swap(true, Ordering::Relaxed) {
                crate::log(&error.to_string());
            }
            Err(error)
        }
    }
}

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
        let mut reused = false;
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(10));
            let next = done.clone();
            spawn(move || next.send(std::thread::current().id()).unwrap());
            if finished.recv().unwrap() == a {
                reused = true;
                break;
            }
        }
        assert!(reused, "no task ran on the failed task's thread");
    }

    fn a_finished_worker_runs_the_next_task() {
        let (done, finished) = channel();
        let first = done.clone();
        spawn(move || first.send(std::thread::current().id()).unwrap());
        let a = finished.recv().unwrap();
        let mut reused = false;
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(10));
            let next = done.clone();
            spawn(move || next.send(std::thread::current().id()).unwrap());
            if finished.recv().unwrap() == a {
                reused = true;
                break;
            }
        }
        assert!(reused, "no task ran on the first task's thread");
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
