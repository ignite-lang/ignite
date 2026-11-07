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
///
/// Alone (5 runs each, 2026-11-07, loads 5 to 27 from other sessions),
/// whole: `apply_way_out::llvm_recover_ends_a_live_instance` 124.70 to
/// 152.22 s, `apply_admission::changed_codegen_revision_with_an_unchanged_model_replaces_the_hosted_server`
/// 71.49 to 78.89 s, `slice18::llvm::todo_survives_sigkill_and_a_rollout`
/// 63.44 to 74.77 s, `slice18::llvm::a_plan_saved_before_a_source_change_is_refused`
/// 66.69 to 84.80 s. Their longest single apply is 12.58 to 15.79 s alone
/// (2 runs each), and loaded gates of that day took one past the former
/// 120 s, so the ceiling is over twice the slowest whole test.
pub const LIVE: Duration = Duration::from_secs(360);

/// The ceiling of one `plan --for-apply`, counted once [`build_lock`] is
/// held: the first preparation of a target compiles the substrate. The
/// longest plan of the four tests above takes 17.07 to 22.31 s alone
/// (2 runs each, 2026-11-07).
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
