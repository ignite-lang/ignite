//! The adapters' interface to the engine (`docs/slice.md` 16.2, 16.3): what
//! an adapter observed goes to the engine as an [`Observation`] over one
//! channel, and the engine asks for work through [`Adapters`]. Adapters
//! never decide; the engine stamps every observation with the clock's
//! `at` and turns it into a `std.control.Event` (16.6).
//!
//! Control input (16.3), one command per line on stdin, read by
//! [`read_control`]:
//!
//! | line | effect |
//! |---|---|
//! | `status` | the current standing of every contract and the state of every machine (one JSON line with `--json`) |
//! | `rollout <member>` | starts the rollout machine of the member (16.12) |
//! | `stop` | the same as SIGINT |
//! | `advance <quantity>` | manual clock only: moves time (16.4); a `Duration` literal such as `100 ms` or `2 seconds` |
//!
//! The settle rule (16.4, O29): [`Adapters::settle`] blocks until nothing
//! is in flight, at most the given limit. In flight are a spawn until the
//! child is `Started` and, when a listener is expected, `Ready` (or until
//! it failed, exited or was signalled), a `kill` until the child ended, a
//! probe attempt until its line or failure arrived, an `await_release`
//! until the `released` line, and whatever else holds an [`InFlight`]
//! guard ([`Adapters::in_flight`], the engine's build thread).
//!
//! Children's stdout and stderr go to the driver's stderr as
//! `<member>: <line>`, never to stdout (the `--json` stream); the lines
//! `listening on <address>:<port>` and `released <address>:<port>` are
//! the adapter's own signals (16.7, 16.12) and are not relayed.

use std::io::{self, BufRead, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

use super::probe;
use super::process::{self, Spawner};

/// What an adapter observed (16.6 `EventKind`, without `at`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    /// A member's child started: its generation and pid.
    Started {
        member: String,
        generation: i64,
        pid: i32,
    },
    /// The child printed `listening on <address>:<port>` (16.7, O37).
    Ready {
        member: String,
        pid: i32,
        address: String,
        port: u16,
    },
    /// The child exited with `code`.
    Exited { member: String, pid: i32, code: i32 },
    /// The child was ended by `signal`.
    Signalled {
        member: String,
        pid: i32,
        signal: i32,
    },
    /// The child could not be started.
    SpawnFailed { member: String, reason: String },
    /// A probe attempt printed `line` and exited 0; the engine decodes it.
    AttemptDone {
        member: String,
        id: i64,
        line: String,
    },
    /// A probe attempt could not run, exited with another status or
    /// printed nothing (16.8).
    AttemptFailed {
        member: String,
        id: i64,
        reason: String,
    },
    /// The old process printed `released <address>:<port>` (16.12).
    Released { member: String, pid: i32 },
    /// The engine's build thread finished `generation` of the member.
    Built { member: String, generation: i64 },
    /// The engine's build thread failed.
    BuildFailed { member: String, reason: String },
    /// One control line from stdin, trimmed (16.3).
    Control(String),
    /// SIGINT or SIGTERM reached the driver.
    Interrupt,
    /// Internal fence after all member exits have been published at stop.
    StopBoundary,
}

/// The in-flight count of the settle rule and the condition its changes
/// signal.
#[derive(Default)]
pub(super) struct Counter {
    count: Mutex<usize>,
    changed: Condvar,
}

impl Counter {
    fn lock(&self) -> MutexGuard<'_, usize> {
        self.count.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// One more operation in flight, until the guard drops.
    pub(super) fn enter(self: &Arc<Counter>) -> InFlight {
        *self.lock() += 1;
        InFlight(Arc::clone(self))
    }

    /// Wait until the count is 0, at most `limit`; whether it reached 0.
    fn settle(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        let mut count = self.lock();
        while *count > 0 {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            count = self
                .changed
                .wait_timeout(count, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        true
    }
}

/// An operation that counts as in flight while the guard is alive.
#[derive(Debug)]
pub struct InFlight(Arc<Counter>);

impl std::fmt::Debug for Counter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Counter({})", *self.lock())
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        let mut count = self.0.lock();
        *count = count.saturating_sub(1);
        self.0.changed.notify_all();
    }
}

/// The adapters of one run: the spawner thread every child comes from
/// (16.2, A3), the table of live member processes, the table of running
/// probe attempts and the in-flight count.
#[derive(Debug)]
pub struct Adapters {
    pub(super) observations: Sender<Observation>,
    pub(super) spawner: Spawner,
    pub(super) counter: Arc<Counter>,
    pub(super) processes: Arc<Mutex<process::Table>>,
    pub(super) attempts: Arc<Mutex<probe::Table>>,
}

impl Adapters {
    /// Start the spawner thread; observations go to `observations`.
    pub fn start(observations: Sender<Observation>) -> io::Result<Adapters> {
        Ok(Adapters {
            observations,
            spawner: Spawner::start()?,
            counter: Arc::new(Counter::default()),
            processes: Arc::new(Mutex::new(process::Table::default())),
            attempts: Arc::new(Mutex::new(probe::Table::default())),
        })
    }

    /// A guard that counts as in flight while alive, for work outside the
    /// adapters (the engine's build thread).
    pub fn in_flight(&self) -> InFlight {
        self.counter.enter()
    }

    /// The settle rule (16.4): wait until no operation is in flight, at
    /// most `limit`; `false` when one still runs.
    pub fn settle(&self, limit: Duration) -> bool {
        self.counter.settle(limit)
    }

    /// Start `executable args` for `member` (16.7): `Started` or
    /// `SpawnFailed` before this returns; `Ready` when the child prints
    /// `listening on` (in flight until then when `expect_ready`); `Exited`
    /// or `Signalled` when it ends.
    pub fn spawn(
        &self,
        member: &str,
        generation: i64,
        executable: &Path,
        args: &[String],
        expect_ready: bool,
    ) {
        process::spawn(self, member, generation, executable, args, expect_ready);
    }

    /// `signal` to the live child `pid` (`Kill(member, pid, signal)`, 16.6).
    /// In flight until that child ends. `Err` when no child of the run
    /// with that pid is live or the signal is unknown.
    pub fn kill(&self, pid: i32, signal: i32) -> io::Result<()> {
        process::kill(self, pid, signal)
    }

    /// `Released` once the member's oldest live child has printed
    /// `released <address>:<port>` (16.12, A5); in flight until then. The
    /// adapter never connect-polls. `false` when the member has no live
    /// child, and then no `Released` follows.
    pub fn await_release(&self, member: &str) -> bool {
        process::await_release(self, member)
    }

    /// The live children of `member`: `(generation, pid)`, oldest first.
    pub fn pids(&self, member: &str) -> Vec<(i64, i32)> {
        process::pids(self, member)
    }

    /// One probe attempt (16.8, O28): `executable --ref <path>=<endpoint>`
    /// per pair of `refs`; `AttemptDone` with the first stdout line on
    /// exit 0, `AttemptFailed` otherwise; nothing after [`kill_attempt`].
    /// In flight until one of them is sent.
    ///
    /// [`kill_attempt`]: Adapters::kill_attempt
    pub fn attempt(&self, member: &str, id: i64, executable: &Path, refs: &[(String, String)]) {
        probe::attempt(self, member, id, executable, refs);
    }

    /// SIGKILL to the probe process of attempt `id` at its deadline
    /// (`Abort(member, id)`, 16.8); the attempt then sends no observation.
    pub fn kill_attempt(&self, member: &str, id: i64) {
        probe::kill_attempt(self, member, id);
    }
}

/// `<member>: <line>` to the driver's stderr in one write.
pub(super) fn relay(member: &str, line: &str) {
    let mut bytes = Vec::with_capacity(member.len() + line.len() + 3);
    bytes.extend_from_slice(member.as_bytes());
    bytes.extend_from_slice(b": ");
    bytes.extend_from_slice(line.as_bytes());
    bytes.push(b'\n');
    let _ = io::stderr().lock().write_all(&bytes);
}

/// Read control lines from stdin on a thread of its own: each non-empty
/// line, trimmed, is `Observation::Control`; the thread ends at EOF.
pub fn read_control(observations: Sender<Observation>) {
    let _ = std::thread::Builder::new()
        .name("control".to_string())
        .spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let Ok(line) = line else { break };
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if observations
                    .send(Observation::Control(line.to_string()))
                    .is_err()
                {
                    break;
                }
            }
        });
}

/// The write end of the self-pipe the signal handler writes to.
static SIGNAL_PIPE: AtomicI32 = AtomicI32::new(-1);

extern "C" fn on_signal(signal: i32) {
    let fd = SIGNAL_PIPE.load(Ordering::Acquire);
    if fd >= 0 {
        let byte = [signal as u8];
        // SAFETY: `write(2)` on a pipe is async-signal-safe; the fd stays
        // open for the life of the process.
        unsafe { nix::libc::write(fd, byte.as_ptr().cast(), 1) };
    }
}

/// `Observation::Interrupt` for every SIGINT and SIGTERM the driver gets,
/// through a self-pipe and a reader thread. Handlers, not a mask: the
/// children inherit no blocked signal (the compiled drain relies on
/// SIGTERM reaching them, 16.13).
pub fn watch_signals(observations: Sender<Observation>) -> io::Result<()> {
    let (read_end, write_end): (OwnedFd, OwnedFd) = nix::unistd::pipe()?;
    let write_end = Box::leak(Box::new(write_end));
    SIGNAL_PIPE.store(write_end.as_raw_fd(), Ordering::Release);
    let action = SigAction::new(
        SigHandler::Handler(on_signal),
        SaFlags::SA_RESTART,
        SigSet::empty(),
    );
    for signal in [Signal::SIGINT, Signal::SIGTERM] {
        // SAFETY: `on_signal` only writes one byte to the pipe.
        unsafe { sigaction(signal, &action) }?;
    }
    std::thread::Builder::new()
        .name("signals".to_string())
        .spawn(move || {
            let mut byte = [0u8; 1];
            loop {
                match nix::unistd::read(&read_end, &mut byte) {
                    Ok(0) => break,
                    Ok(_) => {
                        if observations.send(Observation::Interrupt).is_err() {
                            break;
                        }
                    }
                    Err(nix::errno::Errno::EINTR) => continue,
                    Err(_) => break,
                }
            }
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn settle_waits_for_every_guard() {
        let counter = Arc::new(Counter::default());
        assert!(counter.settle(Duration::from_millis(1)));
        let first = counter.enter();
        let second = counter.enter();
        assert!(!counter.settle(Duration::from_millis(20)));
        drop(first);
        assert!(!counter.settle(Duration::from_millis(20)));
        let waiter = Arc::clone(&counter);
        let handle = std::thread::spawn(move || waiter.settle(Duration::from_secs(5)));
        std::thread::sleep(Duration::from_millis(20));
        drop(second);
        assert!(handle.join().unwrap());
    }

    #[test]
    fn a_signal_becomes_an_interrupt() {
        let (sender, receiver) = channel();
        watch_signals(sender).unwrap();
        nix::sys::signal::raise(Signal::SIGTERM).unwrap();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(5)),
            Ok(Observation::Interrupt)
        );
        nix::sys::signal::raise(Signal::SIGINT).unwrap();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(5)),
            Ok(Observation::Interrupt)
        );
    }
}
