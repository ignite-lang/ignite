//! A thread per running session, reused: a thread that finished a session waits up to
//! [`LINGER`] for the next one before it ends, the way `ignite-io`'s task threads do, so a
//! connection costs no thread creation while finished threads are at hand.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::Thread;
use std::time::{Duration, Instant};

/// How long a thread without a session waits for one before it ends.
pub const LINGER: Duration = Duration::from_secs(1);

type Job = Box<dyn FnOnce() + Send + 'static>;

/// An idle thread and the slot its next session is put in.
struct Worker {
    thread: Thread,
    next: Mutex<Option<Job>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The idle threads, most recently idle last.
#[derive(Clone, Default)]
pub struct Workers {
    idle: Arc<Mutex<Vec<Arc<Worker>>>>,
}

impl Workers {
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of idle threads.
    #[cfg(test)]
    pub fn idle(&self) -> usize {
        lock(&self.idle).len()
    }

    /// Run `job` on the most recently idle thread, or on a new one when none is idle.
    pub fn spawn<F>(&self, job: F) -> std::io::Result<()>
    where
        F: FnOnce() + Send + 'static,
    {
        let job: Job = Box::new(job);
        if let Some(worker) = lock(&self.idle).pop() {
            *lock(&worker.next) = Some(job);
            worker.thread.unpark();
            return Ok(());
        }
        let workers = self.clone();
        std::thread::Builder::new()
            .spawn(move || workers.work(job))
            .map(|_| ())
    }

    fn work(&self, mut job: Job) {
        let me = Arc::new(Worker {
            thread: std::thread::current(),
            next: Mutex::new(None),
        });
        loop {
            job();
            lock(&self.idle).push(Arc::clone(&me));
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
                let mut idle = lock(&self.idle);
                if let Some(at) = idle.iter().position(|other| Arc::ptr_eq(other, &me)) {
                    idle.remove(at);
                    return;
                }
                drop(idle);
                break loop {
                    if let Some(next) = lock(&me.next).take() {
                        break next;
                    }
                    std::thread::park();
                };
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn a_finished_thread_runs_the_next_session() {
        let workers = Workers::new();
        let (sent, received) = mpsc::channel();
        let first = sent.clone();
        workers
            .spawn(move || first.send(std::thread::current().id()).unwrap())
            .unwrap();
        let one = received.recv().unwrap();
        while workers.idle() == 0 {
            std::thread::yield_now();
        }
        workers
            .spawn(move || sent.send(std::thread::current().id()).unwrap())
            .unwrap();
        assert_eq!(received.recv().unwrap(), one);
    }

    #[test]
    fn sessions_at_once_run_on_threads_of_their_own() {
        let workers = Workers::new();
        let (sent, received) = mpsc::channel();
        let (release, wait) = mpsc::channel::<()>();
        let wait = std::sync::Arc::new(std::sync::Mutex::new(wait));
        for _ in 0..2 {
            let sent = sent.clone();
            let wait = wait.clone();
            workers
                .spawn(move || {
                    sent.send(std::thread::current().id()).unwrap();
                    let _ = wait.lock().unwrap().recv();
                })
                .unwrap();
        }
        let (a, b) = (received.recv().unwrap(), received.recv().unwrap());
        assert_ne!(a, b);
        release.send(()).unwrap();
        release.send(()).unwrap();
    }
}
