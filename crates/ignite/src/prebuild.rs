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

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

/// The directory in the target directory that holds the crate built from.
pub(crate) const DIR: &str = "ignite-prebuild";

/// A running `cargo build` of the dependencies; [`Prebuild::finish`] waits
/// for it, dropping it first stops it and every process it started.
pub(crate) struct Prebuild {
    child: Option<Child>,
}

impl Prebuild {
    /// Nothing built ahead.
    pub(crate) fn none() -> Prebuild {
        Prebuild { child: None }
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
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut command = Command::new(cargo);
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
        Prebuild {
            child: command.spawn().ok(),
        }
    }

    /// Wait for the build to end, whatever its outcome: the program's build
    /// compiles what it left undone.
    pub(crate) fn finish(mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.wait();
        }
    }
}

impl Drop for Prebuild {
    /// The driver failed before the program's build: stop the cargo and
    /// every process under it, so none outlives the driver.
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if !matches!(child.try_wait(), Ok(Some(_))) {
                stop_tree(child.id());
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
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
    let pid = |id: u32| Pid::from_raw(i32::try_from(id).unwrap_or(0));
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
