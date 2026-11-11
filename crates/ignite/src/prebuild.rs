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
//! ahead the same way and then ends the driver by the same signal; the
//! driver's main thread, which only waits for the compiling thread, keeps
//! them blocked for that ([`block_ending_signals`]). SIGKILL cannot be
//! caught: the build ahead then runs to its end and leaves the units it
//! built, which the next build finds fresh (or waits for, on cargo's lock
//! of the target directory).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use nix::sys::pthread::{Pthread, pthread_kill, pthread_self};
use nix::sys::signal::{SigSet, SigmaskHow, Signal, kill, pthread_sigmask, raise};
use nix::unistd::Pid;

/// The directory in the target directory that holds the crate built from.
pub(crate) const DIR: &str = "ignite-prebuild";

/// A running `cargo build` of the dependencies; [`Prebuild::finish`] waits
/// for it, dropping it first stops it and every process it started.
pub(crate) struct Prebuild {
    child: Option<Child>,
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
        let mut command = Command::new(cargo);
        crate::without_c_debug_information(&mut command);
        command
            .arg("build")
            .arg("--manifest-path")
            .arg(dir.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(target_dir)
            .args(["--quiet", "--offline"]);
        for package in ignite_rustgen::SUBSTRATE_DEPENDENCIES {
            command.args(["-p", package]);
        }
        if release {
            command.arg("--release");
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let watcher = Watcher::start();
        // Held across the spawn: a signal taken meanwhile waits for the
        // cargo to be known, so it is stopped too.
        let mut running = running();
        let child = command.spawn().ok();
        *running = child.as_ref().map(Child::id);
        drop(running);
        Prebuild { child, watcher }
    }

    /// Wait for the build to end, whatever its outcome: the program's build
    /// compiles what it left undone.
    pub(crate) fn finish(mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.wait();
            *running() = None;
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
        if let Some(mut child) = self.child.take() {
            *running() = None;
            if !matches!(child.try_wait(), Ok(Some(_))) {
                stop_tree(child.id());
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(watcher) = self.watcher.take() {
            watcher.stop();
        }
    }
}

/// The cargo of the build ahead while it runs, for [`Watcher`].
static RUNNING: Mutex<Option<u32>> = Mutex::new(None);

fn running() -> MutexGuard<'static, Option<u32>> {
    RUNNING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The signals that end a driver by default and that it takes itself
/// while it builds ahead.
const ENDING: [Signal; 3] = [Signal::SIGTERM, Signal::SIGINT, Signal::SIGHUP];

/// [`ENDING`] but the signals this process ignores (`nohup`, a background
/// job), which stay ignored.
fn ending_signals() -> SigSet {
    let ignored = ignored_signals();
    let mut set = SigSet::empty();
    for signal in ENDING {
        if ignored & (1u64 << (signal as i32 - 1)) == 0 {
            set.add(signal);
        }
    }
    set
}

/// Block the ending signals in the calling thread, for good: the driver's
/// main thread, before it starts the compiling thread, which unblocks them
/// in itself ([`unblock_ending_signals`]). A signal sent to the driver
/// then reaches the compiling thread or a thread it started, never the main
/// thread, which only waits: while a build ahead runs every thread blocks
/// them but its [`Watcher`], which takes them. The processes the driver
/// starts get an empty mask from `std::process::Command`.
pub fn block_ending_signals() {
    let _ = pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&ending_signals()), None);
}

/// Undo [`block_ending_signals`] in the calling thread.
pub fn unblock_ending_signals() {
    let _ = pthread_sigmask(SigmaskHow::SIG_UNBLOCK, Some(&ending_signals()), None);
}

/// A thread that waits for the ending signals while a build ahead runs:
/// the thread that starts the build blocks them for that time, so a signal
/// sent to the driver comes to the watcher, which stops the build ahead,
/// unblocks the signal in itself and raises it: the driver ends by it as
/// its default action would have ended it.
struct Watcher {
    thread: JoinHandle<()>,
    id: Pthread,
    stopping: Arc<AtomicBool>,
    set: SigSet,
}

impl Watcher {
    /// Block the ending signals in the calling thread and start the
    /// watcher; none when every one is ignored or no thread starts.
    fn start() -> Option<Watcher> {
        let set = ending_signals();
        ENDING
            .into_iter()
            .any(|signal| set.contains(signal))
            .then_some(())?;
        pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&set), None).ok()?;
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&stopping);
        let (send, receive) = std::sync::mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("signals".to_string())
            .spawn(move || {
                let _ = send.send(pthread_self());
                loop {
                    let Ok(signal) = set.wait() else {
                        return;
                    };
                    if stop.load(Ordering::Acquire) {
                        return;
                    }
                    if !set.contains(signal) {
                        continue;
                    }
                    if let Some(root) = running().take() {
                        stop_tree(root);
                        let _ = kill(pid(root), Signal::SIGKILL);
                    }
                    let _ = pthread_sigmask(SigmaskHow::SIG_UNBLOCK, Some(&set), None);
                    let _ = raise(signal);
                    std::process::exit(128 + signal as i32);
                }
            });
        let started = thread.ok().and_then(|thread| {
            let id = receive.recv().ok()?;
            Some((thread, id))
        });
        let Some((thread, id)) = started else {
            let _ = pthread_sigmask(SigmaskHow::SIG_UNBLOCK, Some(&set), None);
            return None;
        };
        Some(Watcher {
            thread,
            id,
            stopping,
            set,
        })
    }

    /// End the watcher (a signal of its set sent to it alone, after the
    /// flag that makes it return) and unblock the ending signals in the
    /// calling thread, the one that started it: one sent meanwhile is
    /// pending and takes its default action or the driver's handler then.
    fn stop(self) {
        self.stopping.store(true, Ordering::Release);
        if !self.thread.is_finished()
            && let Some(wake) = ENDING.into_iter().find(|signal| self.set.contains(*signal))
        {
            let _ = pthread_kill(self.id, wake);
        }
        let _ = self.thread.join();
        let _ = pthread_sigmask(SigmaskHow::SIG_UNBLOCK, Some(&self.set), None);
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

fn pid(id: u32) -> Pid {
    Pid::from_raw(i32::try_from(id).unwrap_or(i32::MAX))
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
/// then kill them all: a stopped process starts no other, so none escapes
/// (`/proc/<pid>/task/<tid>/children`).
fn stop_tree(root: u32) {
    let _ = kill(pid(root), Signal::SIGSTOP);
    let mut tree: Vec<u32> = Vec::new();
    let mut parents = vec![root];
    while let Some(parent) = parents.pop() {
        for child in children(parent) {
            if !tree.contains(&child) {
                let _ = kill(pid(child), Signal::SIGSTOP);
                tree.push(child);
                parents.push(child);
            }
        }
    }
    for id in tree.iter().rev() {
        let _ = kill(pid(*id), Signal::SIGKILL);
    }
}

/// The children of process `id`, from every one of its threads.
fn children(id: u32) -> Vec<u32> {
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
                .collect::<Vec<u32>>()
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
}
