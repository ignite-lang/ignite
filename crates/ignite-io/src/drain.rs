//! Drain on SIGTERM (`docs/slice.md` 16.13).
//!
//! [`install`] blocks SIGTERM in the calling thread, so every thread
//! started afterwards inherits the block, and starts the signal thread,
//! which takes SIGTERM with `sigwait`. The first SIGTERM begins the drain
//! ([`begin`]): every bound [`crate::TcpListener`] is shut down for
//! reading, which wakes a pending `accept`, then closed and announced on
//! stderr as `released <address>:<port>`, in bind order; from then on
//! `accept` fails with "draining" ([`DRAINING`]). [`crate::run`] (the Rust
//! target) and the runtime's `exit` and `fail` entries (the LLVM target)
//! then wait for the live tasks ([`crate::wait_for_tasks`]) once `main`
//! returns and end the process with status 0. A second SIGTERM ends the
//! process at once with the signal's default action, so the parent sees a
//! process killed by SIGTERM, not a clean exit.
//!
//! A listener bound with [`crate::TcpListener::bind_kept`] (a resource
//! with `keep_through_drain = true`; story `drain-keeps-listeners`) is
//! never registered here: it is not released, accepts until the process
//! exits, and a task blocked in its `accept` or in a read of a stream it
//! accepted does not hold up the wait for live tasks.
//!
//! Outside `ignite run` the wait for live tasks has no bound: a session
//! that never ends keeps the process alive, and nothing in the substrate
//! ends it. Only the driver's deadline bounds the drain
//! (`DrainRollout.deadline`, then SIGKILL; A6).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Once, PoisonError, Weak};

use nix::sys::signal::{SigSet, SigmaskHow, Signal, pthread_sigmask};

use crate::net::Slot;

/// The message of every `accept` refused by the drain.
pub const DRAINING: &str = "draining";

static MODE: AtomicBool = AtomicBool::new(false);
static INSTALL: Once = Once::new();
static LISTENERS: Mutex<Vec<Weak<Slot>>> = Mutex::new(Vec::new());

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Whether the process is draining.
pub fn draining() -> bool {
    MODE.load(Ordering::SeqCst)
}

/// A listener bound, not kept; the drain releases it. The registry holds a weak
/// reference: a listener dropped by the program is simply gone.
pub(crate) fn register(slot: &Arc<Slot>) {
    let mut listeners = lock(&LISTENERS);
    listeners.retain(|weak| weak.strong_count() > 0);
    listeners.push(Arc::downgrade(slot));
}

/// Reset SIGCHLD to its default action (`crate::child::default_sigchld`),
/// block SIGTERM in the calling thread (and so in every thread it starts
/// from now on) and start the signal thread, once per process. A later
/// call does nothing. Called by [`crate::run`] and by the LLVM runtime's
/// `init`, before any task thread exists.
pub fn install() {
    INSTALL.call_once(|| {
        crate::child::default_sigchld();
        let mut set = SigSet::empty();
        set.add(Signal::SIGTERM);
        if pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&set), None).is_err() {
            return;
        }
        let _ = std::thread::Builder::new()
            .name("drain".to_string())
            .spawn(move || {
                if set.wait().is_err() {
                    return;
                }
                begin();
                let _ = set.wait();
                crate::run_exit_hooks();
                crate::flush();
                crate::terminal::restore();
                let _ = pthread_sigmask(SigmaskHow::SIG_UNBLOCK, Some(&set), None);
                let _ = nix::sys::signal::raise(Signal::SIGTERM);
                std::process::exit(128 + Signal::SIGTERM as i32);
            });
    });
}

/// Begin the drain: set the mode, then release every listener and log
/// its `released` line.
pub(crate) fn begin() {
    MODE.store(true, Ordering::SeqCst);
    for line in release_all() {
        crate::log(&line);
    }
}

/// Release every registered listener still alive; their `released
/// <address>:<port>` lines in bind order.
pub(crate) fn release_all() -> Vec<String> {
    let slots: Vec<Arc<Slot>> = lock(&LISTENERS)
        .drain(..)
        .filter_map(|weak| weak.upgrade())
        .collect();
    slots.iter().filter_map(|slot| slot.release()).collect()
}
