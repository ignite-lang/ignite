//! The dependencies of `ignite-io`, built while the driver works (row 6.2
//! rust of `docs/quality.md`, story `build-time-back`).
//!
//! A dev build on the Rust target spends its first part in the driver:
//! loading, typing and generating the program and the standard library.
//! Cargo's longest chain then starts with `libc`, `nix` and `ignite-io`,
//! which do not depend on the program. Before the driver parses, it writes
//! a crate that depends on `ignite-io` with the generated project's
//! profiles ([`ignite_rustgen::prebuild_manifest`]) into the build's target
//! directory and starts `cargo build -p` of the crates `ignite-io` depends
//! on ([`ignite_rustgen::SUBSTRATE_DEPENDENCIES`]): the same units the
//! program's build needs, which it then finds fresh. The program's build
//! waits for that cargo to end, so the two never share the target
//! directory's lock. A target directory that has built them before builds
//! nothing ahead, so a rebuild does no more than before. The cargo runs
//! quietly and offline: what it cannot do, the program's build does and
//! reports.
//!
//! None of it outlives the driver: a failure drops the [`Prebuild`], and so
//! does a panic as it unwinds, which stops the cargo and every process under
//! it. While it runs, SIGTERM, SIGINT and SIGHUP sent to the driver alone
//! are taken by a thread of its own ([`Watcher`]), which stops the build
//! ahead the same way and then ends the driver by the same signal; every
//! other thread of the driver blocks them for that time, the main thread
//! too, which waits for the compiling thread ([`compile_on_thread`]). The
//! cargo starts with the signal mask the driver was started with. SIGKILL
//! cannot be caught: the build ahead then runs to its end and leaves the
//! units it built, which the next build finds fresh (or waits for, on
//! cargo's lock of the target directory). A command that builds nothing
//! ahead makes none of these calls.

use std::ffi::{CString, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use nix::errno::Errno;
use nix::fcntl::OFlag;
use nix::spawn::{PosixSpawnAttr, PosixSpawnFileActions, PosixSpawnFlags, posix_spawnp};
use nix::sys::pthread::{Pthread, pthread_kill, pthread_self};
use nix::sys::signal::{SigSet, SigmaskHow, Signal, kill, raise};
use nix::sys::stat::Mode;
use nix::sys::wait::{Id, WaitPidFlag, WaitStatus, waitid, waitpid};
use nix::unistd::Pid;

/// The directory in the target directory that holds the crate built from.
pub(crate) const DIR: &str = "ignite-prebuild";

/// A running `cargo build` of the dependencies; [`Prebuild::finish`] waits
/// for it, dropping it first stops it and every process it started.
pub(crate) struct Prebuild {
    child: Option<Pid>,
    watcher: Option<Watcher>,
}

impl Prebuild {
    /// Nothing built ahead.
    pub(crate) fn none() -> Prebuild {
        Prebuild {
            child: None,
            watcher: None,
        }
    }

    /// Start the build of the dependencies of `ignite-io` at `substrate`
    /// into `target_dir` for the project at `out`, unless that target
    /// directory has built them before (`nix`'s fingerprint is there) or
    /// the build is static (`triple`). The lock is the project's when it has
    /// one, else the substrate workspace's (as [`crate::seed_lock`] seeds
    /// the project). Nothing is started when any of that fails.
    pub(crate) fn start(
        substrate: &Path,
        out: &Path,
        target_dir: &Path,
        release: bool,
        triple: Option<&str>,
    ) -> Prebuild {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        Prebuild::start_with(cargo, substrate, out, target_dir, release, triple)
    }

    /// [`Prebuild::start`] with `cargo` as the cargo program.
    fn start_with(
        cargo: OsString,
        substrate: &Path,
        out: &Path,
        target_dir: &Path,
        release: bool,
        triple: Option<&str>,
    ) -> Prebuild {
        if triple.is_some() || built_before(target_dir, release) {
            return Prebuild::none();
        }
        let dir = target_dir.join(DIR);
        let lock = [out.join("Cargo.lock"), substrate.join("../../Cargo.lock")]
            .into_iter()
            .find(|lock| lock.is_file());
        let written = std::fs::create_dir_all(&dir)
            .and_then(|()| {
                std::fs::write(
                    dir.join("Cargo.toml"),
                    ignite_rustgen::prebuild_manifest(substrate),
                )
            })
            .and_then(|()| std::fs::write(dir.join("lib.rs"), ""))
            .and_then(|()| match &lock {
                Some(lock) => std::fs::copy(lock, dir.join("Cargo.lock")).map(drop),
                None => Ok(()),
            });
        if written.is_err() {
            return Prebuild::none();
        }
        let mut args: Vec<OsString> = vec![
            cargo,
            "build".into(),
            "--manifest-path".into(),
            dir.join("Cargo.toml").into(),
            "--target-dir".into(),
            target_dir.into(),
            "--quiet".into(),
            "--offline".into(),
        ];
        for package in ignite_rustgen::SUBSTRATE_DEPENDENCIES {
            args.extend(["-p".into(), (*package).into()]);
        }
        if release {
            args.push("--release".into());
        }
        let mut environment: Vec<OsString> = std::env::vars_os()
            .map(|(mut name, value)| {
                name.push("=");
                name.push(value);
                name
            })
            .collect();
        for (name, value) in crate::c_debug_flags() {
            environment.push(format!("{name}={value}").into());
        }
        let started_with = SigSet::thread_get_mask().unwrap_or_else(|_| SigSet::empty());
        let watcher = Watcher::start(&started_with);
        // Held across the spawn: a signal taken meanwhile waits for the
        // cargo to be known, so it is stopped too.
        let mut running = running();
        let child = spawn(&args, &environment, &started_with);
        *running = child;
        drop(running);
        Prebuild { child, watcher }
    }

    /// Wait for the build to end, whatever its outcome: the program's build
    /// compiles what it left undone.
    pub(crate) fn finish(mut self) {
        if let Some(child) = self.child.take() {
            // Ended but not reaped, so its id is still its own while the
            // watcher may stop it.
            while waitid(Id::Pid(child), WaitPidFlag::WEXITED | WaitPidFlag::WNOWAIT)
                == Err(Errno::EINTR)
            {}
            *running() = None;
            reap(child);
        }
        if let Some(watcher) = self.watcher.take() {
            watcher.stop();
        }
    }
}

impl Drop for Prebuild {
    /// The driver failed or panics before the program's build: stop the
    /// cargo and every process under it, so none outlives the driver.
    fn drop(&mut self) {
        if let Some(child) = self.child.take() {
            *running() = None;
            if matches!(
                waitpid(child, Some(WaitPidFlag::WNOHANG)),
                Ok(WaitStatus::StillAlive)
            ) {
                stop_tree(child);
                let _ = kill(child, Signal::SIGKILL);
                reap(child);
            }
        }
        if let Some(watcher) = self.watcher.take() {
            watcher.stop();
        }
    }
}

/// Start `args` (the program first, looked up on `PATH`) with
/// `environment`, the standard streams on `/dev/null` and `mask` as its
/// signal mask, the one the driver was started with: the thread that
/// starts it blocks the ending signals then, and a child of
/// `std::process::Command` would inherit that thread's mask. SIGPIPE,
/// which the Rust runtime ignores, gets its default action, as
/// `std::process::Command` gives it.
fn spawn(args: &[OsString], environment: &[OsString], mask: &SigSet) -> Option<Pid> {
    let c_string = |text: &OsString| CString::new(text.as_bytes()).ok();
    let args: Vec<CString> = args.iter().map(c_string).collect::<Option<_>>()?;
    let environment: Vec<CString> = environment.iter().filter_map(c_string).collect();
    let mut attributes = PosixSpawnAttr::init().ok()?;
    attributes.set_sigmask(mask).ok()?;
    attributes
        .set_sigdefault(&SigSet::from(Signal::SIGPIPE))
        .ok()?;
    attributes
        .set_flags(PosixSpawnFlags::POSIX_SPAWN_SETSIGMASK | PosixSpawnFlags::POSIX_SPAWN_SETSIGDEF)
        .ok()?;
    let mut actions = PosixSpawnFileActions::init().ok()?;
    for (fd, flag) in [
        (0, OFlag::O_RDONLY),
        (1, OFlag::O_WRONLY),
        (2, OFlag::O_WRONLY),
    ] {
        actions
            .add_open(fd, "/dev/null", flag, Mode::empty())
            .ok()?;
    }
    posix_spawnp(args.first()?, &actions, &attributes, &args, &environment).ok()
}

/// Wait for `child` to end and take its status.
fn reap(child: Pid) {
    while waitpid(child, None) == Err(Errno::EINTR) {}
}

/// The cargo of the build ahead while it runs, for [`Watcher`].
static RUNNING: Mutex<Option<Pid>> = Mutex::new(None);

fn running() -> MutexGuard<'static, Option<Pid>> {
    RUNNING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The signals that end a driver by default and that it takes itself
/// while it builds ahead.
const ENDING: [Signal; 3] = [Signal::SIGTERM, Signal::SIGINT, Signal::SIGHUP];

/// The signal that wakes the [`Watcher`] to end it: none of [`ENDING`],
/// so one of those the watcher takes is always one sent to the driver, and
/// ignored by default.
const WAKE: Signal = Signal::SIGURG;

/// What the compiling thread asks of the driver's main thread, which waits
/// for it ([`compile_on_thread`]).
enum Request {
    /// Block these signals, then say so.
    Block(SigSet, Sender<()>),
    /// Set the signal mask back to the one before the first block.
    Restore,
    /// The compiling thread is done.
    Ended,
}

/// The main thread's end of [`Request`]s, when it waits in
/// [`compile_on_thread`].
static MAIN_THREAD: Mutex<Option<Sender<Request>>> = Mutex::new(None);

/// Send `request` to the main thread; false when none waits.
fn ask(request: Request) -> bool {
    let main = MAIN_THREAD
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    main.is_some_and(|main| main.send(request).is_ok())
}

/// Says [`Request::Ended`] when the compiling thread returns or unwinds.
struct Ended(Sender<Request>);

impl Drop for Ended {
    fn drop(&mut self) {
        let _ = self.0.send(Request::Ended);
    }
}

/// Run `compile` on a thread of `stack` bytes, the compiling thread, and
/// wait for it in the calling thread, the driver's main thread, which
/// meanwhile blocks the ending signals while a build ahead runs and only
/// then, when the [`Watcher`] asks: a signal sent to the driver then never
/// lands on it. The thread's outcome, `Err` when it panicked.
pub fn compile_on_thread(
    stack: usize,
    compile: impl FnOnce() + Send + 'static,
) -> std::thread::Result<()> {
    let (send, receive) = channel();
    *MAIN_THREAD.lock().unwrap_or_else(PoisonError::into_inner) = Some(send.clone());
    let compiler = std::thread::Builder::new()
        .name("ignite".into())
        .stack_size(stack)
        .spawn(move || {
            let _ended = Ended(send);
            compile();
        })
        .expect("spawn the compiling thread");
    let mut started_with: Option<SigSet> = None;
    for request in &receive {
        match request {
            Request::Block(set, done) => {
                if let Ok(mask) = set.thread_swap_mask(SigmaskHow::SIG_BLOCK) {
                    started_with.get_or_insert(mask);
                }
                let _ = done.send(());
            }
            Request::Restore => {
                if let Some(mask) = started_with.take() {
                    let _ = mask.thread_set_mask();
                }
            }
            Request::Ended => break,
        }
    }
    compiler.join()
}

/// A thread that waits for the ending signals while a build ahead runs:
/// the thread that starts the build and the main thread block them for
/// that time, so a signal sent to the driver comes to the watcher, which
/// stops the build ahead, unblocks the signal in itself and raises it: the
/// driver ends by it as its default action would have ended it.
struct Watcher {
    thread: JoinHandle<()>,
    id: Pthread,
    stopping: Arc<AtomicBool>,
    /// The mask of the thread that started it, set again when it stops.
    started_with: SigSet,
}

impl Watcher {
    /// Block the ending signals in the main thread and the calling thread,
    /// whose mask is `started_with`, and start the watcher; none when every
    /// one is ignored (`nohup`, a background job) or blocked already (they
    /// stay so) or no thread starts.
    fn start(started_with: &SigSet) -> Option<Watcher> {
        let ignored = ignored_signals();
        let mut set = SigSet::empty();
        for signal in ENDING {
            if ignored & (1u64 << (signal as i32 - 1)) == 0 && !started_with.contains(signal) {
                set.add(signal);
            }
        }
        if !ENDING.into_iter().any(|signal| set.contains(signal)) {
            return None;
        }
        let (done, acknowledged) = channel();
        if ask(Request::Block(set, done)) {
            let _ = acknowledged.recv();
        }
        let mut waited = set;
        waited.add(WAKE);
        let started_with = *started_with;
        let blocked = waited.thread_block().is_ok();
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stopping);
        let (send, receive) = channel();
        let thread = blocked
            .then(|| {
                std::thread::Builder::new()
                    .name("signals".to_string())
                    .spawn(move || {
                        let _ = send.send(pthread_self());
                        loop {
                            let Ok(signal) = waited.wait() else {
                                return;
                            };
                            if signal == WAKE {
                                if stop.load(Ordering::Acquire) {
                                    return;
                                }
                                continue;
                            }
                            // Sent to the driver, also while the watcher
                            // is being stopped: the driver ends by it.
                            if let Some(root) = running().take() {
                                stop_tree(root);
                                let _ = kill(root, Signal::SIGKILL);
                            }
                            let _ = set.thread_unblock();
                            let _ = raise(signal);
                            std::process::exit(128 + signal as i32);
                        }
                    })
                    .ok()
            })
            .flatten();
        let started = thread.and_then(|thread| {
            let id = receive.recv().ok()?;
            Some((thread, id))
        });
        let Some((thread, id)) = started else {
            let _ = started_with.thread_set_mask();
            ask(Request::Restore);
            return None;
        };
        Some(Watcher {
            thread,
            id,
            stopping,
            started_with,
        })
    }

    /// End the watcher ([`WAKE`] sent to it alone, after the flag that
    /// makes it return) and set the masks of the thread that started it
    /// and of the main thread back: an ending signal sent meanwhile is
    /// pending and takes its default action or the driver's handler then.
    fn stop(self) {
        self.stopping.store(true, Ordering::Release);
        if !self.thread.is_finished() {
            let _ = pthread_kill(self.id, WAKE);
        }
        let _ = self.thread.join();
        let _ = self.started_with.thread_set_mask();
        ask(Request::Restore);
    }
}

/// The signals this process ignores, bit `n - 1` for signal `n`
/// (`SigIgn` of `/proc/self/status`); none when it cannot be read.
fn ignored_signals() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("SigIgn:"))
                .and_then(|mask| u64::from_str_radix(mask.trim(), 16).ok())
        })
        .unwrap_or(0)
}

/// Whether `target_dir` holds a build of `nix` for the profile.
fn built_before(target_dir: &Path, release: bool) -> bool {
    let profile = if release { "release" } else { "debug" };
    std::fs::read_dir(target_dir.join(profile).join(".fingerprint")).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().starts_with("nix-"))
    })
}

/// Stop `root` and, until no new one appears, every process under it,
/// then kill them all but `root`: a stopped process starts no other, so
/// none escapes (`/proc/<pid>/task/<tid>/children`).
fn stop_tree(root: Pid) {
    let _ = kill(root, Signal::SIGSTOP);
    let mut tree: Vec<Pid> = Vec::new();
    let mut parents = vec![root];
    while let Some(parent) = parents.pop() {
        for child in children(parent) {
            if !tree.contains(&child) {
                let _ = kill(child, Signal::SIGSTOP);
                tree.push(child);
                parents.push(child);
            }
        }
    }
    for id in tree.iter().rev() {
        let _ = kill(*id, Signal::SIGKILL);
    }
}

/// The children of process `id`, from every one of its threads.
fn children(id: Pid) -> Vec<Pid> {
    let tasks: PathBuf = Path::new("/proc").join(id.to_string()).join("task");
    let Ok(entries) = std::fs::read_dir(tasks) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|task| std::fs::read_to_string(task.path().join("children")).ok())
        .flat_map(|list| {
            list.split_whitespace()
                .filter_map(|id| id.parse().ok())
                .map(Pid::from_raw)
                .collect::<Vec<Pid>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::Duration;

    use super::Prebuild;

    /// Whether process `id` exists and is not a zombie.
    fn alive(id: u32) -> bool {
        std::fs::read_to_string(format!("/proc/{id}/stat")).is_ok_and(|stat| {
            !stat
                .rsplit(')')
                .next()
                .unwrap_or_default()
                .trim_start()
                .starts_with('Z')
        })
    }

    /// A driver that panics while it generates unwinds through its
    /// [`Prebuild`], which stops the cargo it started and every process
    /// under that cargo. The cargo here is a shell that starts a `sleep`
    /// and waits for it.
    #[test]
    fn a_panic_stops_the_build_ahead_and_what_it_started() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/prebuild-panic");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cargo = dir.join("cargo");
        let ids = dir.join("ids");
        std::fs::write(
            &cargo,
            format!(
                "#!/bin/sh\nsleep 60 &\necho $$ $! > {}.part\nmv {0}.part {0}\nwait\n",
                ids.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
        let unwound = std::panic::catch_unwind(|| {
            let prebuild = Prebuild::start_with(
                cargo.clone().into(),
                &dir.join("substrate/crates/ignite-io"),
                &dir.join("out"),
                &dir.join("target"),
                false,
                None,
            );
            let mut started: Vec<u32> = Vec::new();
            for _ in 0..1000 {
                if let Ok(text) = std::fs::read_to_string(&ids) {
                    started = text
                        .split_whitespace()
                        .filter_map(|id| id.parse().ok())
                        .collect();
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let _held = prebuild;
            std::panic::panic_any(started);
        });
        let started = *unwound.unwrap_err().downcast::<Vec<u32>>().unwrap();
        assert_eq!(started.len(), 2, "the shell and its sleep: {started:?}");
        let mut left = started.clone();
        for _ in 0..100 {
            left.retain(|id| alive(*id));
            if left.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(left.is_empty(), "still running after the panic: {left:?}");
    }

    /// The `SigBlk` line of `status`, a `/proc` status file.
    fn blocked(status: &str) -> String {
        std::fs::read_to_string(status)
            .unwrap()
            .lines()
            .find(|line| line.starts_with("SigBlk:"))
            .unwrap()
            .to_string()
    }

    /// The cargo of the build ahead starts with its standard streams open
    /// on `/dev/null` and the signal mask of the thread that starts the
    /// build, as it was before the build: not the one that thread has while
    /// the build runs. The cargo here is a shell that writes down what its
    /// streams are and the signals it blocks.
    #[test]
    fn the_build_ahead_starts_on_dev_null_with_the_mask_it_was_started_with() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/prebuild-start");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cargo = dir.join("cargo");
        let seen = dir.join("seen");
        std::fs::write(
            &cargo,
            format!(
                // The mask read first and by the shell itself: it blocks
                // SIGCHLD while it waits for a command it started.
                "#!/bin/sh\nexec 3> {0}.part\n\
                 while read -r line; do case $line in SigBlk*) mask=$line;; esac; done < /proc/$$/status\n\
                 for fd in 0 1 2; do readlink /proc/$$/fd/$fd >&3; done\n\
                 echo \"$mask\" >&3\nexec 3>&-\nmv {0}.part {0}\n",
                seen.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mask = blocked("/proc/thread-self/status");
        let prebuild = Prebuild::start_with(
            cargo.into(),
            &dir.join("substrate/crates/ignite-io"),
            &dir.join("out"),
            &dir.join("target"),
            false,
            None,
        );
        prebuild.finish();
        assert_eq!(blocked("/proc/thread-self/status"), mask, "the mask after");
        let seen = std::fs::read_to_string(&seen).unwrap_or_default();
        assert_eq!(
            seen.lines().collect::<Vec<_>>(),
            ["/dev/null", "/dev/null", "/dev/null", mask.as_str()]
        );
    }

    /// The variable that makes [`watcher_taking_a_signal_while_it_stops`]
    /// do its work, in a process of its own.
    const WATCHER_CASE: &str = "IGNITE_WATCHER_CASE";

    /// Run in a process of its own by
    /// [`a_signal_the_watcher_takes_while_it_stops_ends_the_driver`]: a
    /// watcher that is being stopped (its flag set) takes SIGTERM sent to
    /// it, then is stopped. Prints `ran on` if the process outlives that.
    #[test]
    fn watcher_taking_a_signal_while_it_stops() {
        if std::env::var_os(WATCHER_CASE).is_none() {
            return;
        }
        let mask = nix::sys::signal::SigSet::thread_get_mask().expect("the mask");
        let watcher = super::Watcher::start(&mask).expect("a watcher");
        watcher
            .stopping
            .store(true, std::sync::atomic::Ordering::Release);
        nix::sys::pthread::pthread_kill(watcher.id, nix::sys::signal::Signal::SIGTERM)
            .expect("signal the watcher");
        watcher.stop();
        println!("ran on");
    }

    /// An ending signal the watcher takes once its flag to stop is set,
    /// one sent to the driver just as its build ahead ends, is not
    /// swallowed: the driver ends by it, as it would a moment later.
    #[test]
    fn a_signal_the_watcher_takes_while_it_stops_ends_the_driver() {
        use std::os::unix::process::ExitStatusExt;
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "prebuild::tests::watcher_taking_a_signal_while_it_stops",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(WATCHER_CASE, "1")
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert_eq!(
            output.status.signal(),
            Some(nix::sys::signal::Signal::SIGTERM as i32),
            "{:?}; stdout: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout)
        );
    }
}
