//! Shared by the apply suites (`docs/slice.md` 17.7): the deadlines, the one
//! build directory per target and the lock around preparing into it.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

/// The ceiling of a wait that ends at an observed event: a client's exit,
/// a host's socket, a readiness, a convergence. The wait returns at the
/// event, so the ceiling costs nothing when the event comes and only bounds
/// a hang. A first apply of the terminal took over 15 s beside 21 other
/// apply tests at load 16 (slice 13, U5).
pub const LIVE: Duration = Duration::from_secs(120);

/// The ceiling of one `plan --for-apply`, counted once [`build_lock`] is
/// held: the first preparation of a target compiles the substrate.
pub const BUILD: Duration = Duration::from_secs(600);

static RUST: Mutex<()> = Mutex::new(());
static LLVM: Mutex<()> = Mutex::new(());

/// Held around one `plan --for-apply` of `target` that builds. The states of
/// every apply suite share one build directory per target ([`shared_build`],
/// O41). The driver locks that directory while it builds, but the host
/// digests and copies the executable out of the shared output after the
/// driver's lock is released, so a second preparation must not overwrite
/// the output in between.
pub fn build_lock(target: &str) -> MutexGuard<'static, ()> {
    let lock = if target == "llvm" { &LLVM } else { &RUST };
    lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Link `state/build` to the one build directory of the apply suites, so
/// each target's substrate is compiled once for all of them.
pub fn shared_build(state: &Path) {
    let shared = Path::new(env!("CARGO_TARGET_TMPDIR")).join("apply-build");
    std::fs::create_dir_all(&shared).unwrap();
    std::os::unix::fs::symlink(&shared, state.join("build")).unwrap();
}

/// The `plan` arguments that select `target`; the LLVM target links the
/// runtime archive built beside this test binary.
pub fn target_args(target: &str) -> Vec<String> {
    let mut args = vec!["--target".to_string(), target.to_string()];
    if target == "llvm" {
        args.push("--runtime".into());
        args.push(super::runtime_archive().display().to_string());
    }
    args
}
