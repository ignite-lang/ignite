//! Second adversary pass on `ignite --version` (story
//! `version-names-commit`), against the correction that watches the
//! worktree's `logs/HEAD`, `HEAD`, the branch's loose ref, the release tag's
//! loose ref and `reftable/tables.list`. Each case builds the driver of a
//! scratch git checkout of this workspace, changes the checkout with git and
//! rebuilds with plain `cargo build`, as a developer does, and compares what
//! `--version` prints with what a fresh build of the same tree prints.

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The cargo target directory this test binary was built into.
fn target_dir() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_ignite"))
        .ancestors()
        .nth(2)
        .expect("target directory")
        .to_path_buf()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=adversary",
            "-c",
            "user.email=adversary@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git");
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Builds the driver of the checkout `src` into `target` and returns what
/// its `--version` prints.
fn build_and_ask(src: &Path, target: &Path) -> String {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .args(["build", "--offline", "-p", "ignite"])
        .current_dir(src)
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_INCREMENTAL", "0")
        .env_remove("IGNITE_RELEASE_COMMIT")
        .output()
        .expect("cargo");
    assert!(output.status.success(), "cargo build: {output:?}");
    let output = Command::new(target.join("debug/ignite"))
        .arg("--version")
        .output()
        .expect("ignite --version");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Held for the whole of a case: the cases share one target directory, and
/// another case's build rewrites the runtime archive `build.rs` watches,
/// which reruns the script and hides what git alone does.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A git repository `<root>/<name>/origin` holding the driver's sources in
/// one commit, and the cargo target directory every case shares (one per
/// case would compile every dependency once per case).
fn origin(name: &str) -> (PathBuf, PathBuf) {
    let root = target_dir().join("version-adversary-2");
    let case = root.join(name);
    let _ = std::fs::remove_dir_all(&case);
    let origin = case.join("origin");
    let files = git(
        &workspace(),
        &[
            "ls-files",
            "--",
            "Cargo.toml",
            "Cargo.lock",
            "crates",
            "tools",
        ],
    );
    for file in files.lines() {
        let from = workspace().join(file);
        if !from.is_file() {
            continue;
        }
        let to = origin.join(file);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(&from, &to).unwrap();
    }
    git(&origin, &["init", "-q"]);
    git(&origin, &["add", "-A"]);
    git(&origin, &["commit", "-qm", "base"]);
    (origin, root.join("target"))
}

/// Builds twice: the first build of a target directory writes the runtime
/// archive that build.rs also watches, so the second build reruns the script
/// once whatever git did; from then on only git's files and the sources
/// count.
fn settle(src: &Path, target: &Path) -> String {
    build_and_ask(src, target);
    build_and_ask(src, target)
}

/// A checkout without a reflog (`core.logAllRefUpdates=false`, no `logs/`)
/// whose refs are packed: `build.rs` watches `logs/HEAD` and the branch's
/// loose ref, neither of which exists, so the clean release build stays
/// `ignite X.Y.Z` after a commit on the branch.
#[test]
fn a_commit_after_the_release_without_a_reflog_and_a_packed_branch_is_not_the_release() {
    let release = env!("CARGO_PKG_VERSION");
    let _serial = serial();
    let (origin, target) = origin("no-reflog");
    git(
        &origin,
        &["tag", "-a", &format!("v{release}"), "-m", "release"],
    );
    let clone = origin.with_file_name("clone");
    git(
        origin.parent().unwrap(),
        &[
            "clone",
            "-q",
            "-c",
            "core.logAllRefUpdates=false",
            origin.to_str().unwrap(),
            clone.to_str().unwrap(),
        ],
    );
    let _ = std::fs::remove_dir_all(clone.join(".git/logs"));
    git(&clone, &["pack-refs", "--all"]);
    assert!(!clone.join(".git/refs/heads/main").exists());
    assert_eq!(
        settle(&clone, &target),
        format!("ignite {release}"),
        "a clean clone at the release tag"
    );

    git(
        &clone,
        &["commit", "-q", "--allow-empty", "-m", "after the release"],
    );
    let head = git(&clone, &["rev-parse", "HEAD"]);
    assert_eq!(
        build_and_ask(&clone, &target),
        format!("ignite {release}+g{}", &head[..12]),
        "the driver of a commit after the tag names that commit"
    );
}

/// The release tag made after the last build: a fresh build of the clean,
/// tagged checkout prints `ignite X.Y.Z`, but a rebuild of the checkout that
/// was built before the tag existed keeps `+g<commit>`, since the tag's loose
/// ref did not exist and was not watched. One tree, two answers, decided by
/// build history.
#[test]
fn a_tag_made_after_the_last_build_is_the_release_on_a_plain_rebuild() {
    let release = env!("CARGO_PKG_VERSION");
    let _serial = serial();
    let (origin, target) = origin("tag-after-build");
    let head = git(&origin, &["rev-parse", "HEAD"]);
    assert_eq!(
        settle(&origin, &target),
        format!("ignite {release}+g{}", &head[..12]),
        "an untagged clean checkout"
    );
    git(
        &origin,
        &["tag", "-a", &format!("v{release}"), "-m", "release"],
    );
    assert_eq!(
        build_and_ask(&origin, &target),
        format!("ignite {release}"),
        "the same clean checkout, now at the release tag"
    );
}

/// `.dirty` after the edit is reverted by a mode change back: git counts the
/// executable bit of a tracked file (`core.fileMode`, the default on Linux),
/// so `chmod +x` on a source makes the build `.dirty`, and `chmod -x` makes
/// the tree clean again without changing any mtime cargo compares, so the
/// rebuild keeps `.dirty`.
#[test]
fn a_reverted_mode_change_leaves_no_stale_dirty() {
    use std::os::unix::fs::PermissionsExt;
    let release = env!("CARGO_PKG_VERSION");
    let _serial = serial();
    let (origin, target) = origin("mode-revert");
    let head = git(&origin, &["rev-parse", "HEAD"]);
    let file = origin.join("crates/ignite/src/lib.rs");
    let mode = |bits| std::fs::set_permissions(&file, std::fs::Permissions::from_mode(bits));

    mode(0o755).unwrap();
    assert_eq!(
        settle(&origin, &target),
        format!("ignite {release}+g{}.dirty", &head[..12]),
        "a source whose mode differs from the commit"
    );
    mode(0o644).unwrap();
    assert_eq!(git(&origin, &["status", "--porcelain"]), "");
    assert_eq!(
        build_and_ask(&origin, &target),
        format!("ignite {release}+g{}", &head[..12]),
        "the mode restored, the tree is clean"
    );
}
