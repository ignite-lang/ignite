//! Adversary cases for `ignite --version` (story `version-names-commit`):
//! a driver built from a scratch git checkout of this workspace (a clone
//! with `.git` a file through `--separate-git-dir`, after `git gc` has
//! packed its refs), rebuilt after a git operation and asked what it was
//! built from.

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
        .output()
        .expect("cargo");
    assert!(output.status.success(), "cargo build: {output:?}");
    let output = Command::new(target.join("debug/ignite"))
        .arg("--version")
        .output()
        .expect("ignite --version");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A clean clone at the release tag prints the release; one more commit on
/// the branch must print that commit, not the release again. After `git gc`
/// (`pack-refs --all`, which `gc` and `gc --auto` run) the branch ref lives
/// in `packed-refs` only, so `build.rs` drops the branch's loose ref file from
/// its watch list (it does not exist), and a commit that leaves the index
/// alone (`--allow-empty`, `reset --soft`, a message-only `--amend`) writes
/// a new loose ref nothing watches: the driver of a later commit says it is
/// the release.
#[test]
fn a_commit_after_the_release_tag_with_packed_refs_is_not_the_release() {
    let release = env!("CARGO_PKG_VERSION");
    let root = target_dir().join("version-adversary");
    let origin = root.join("origin");
    let clone = root.join("clone");
    let gitdir = root.join("clone.git");
    for dir in [&origin, &clone, &gitdir] {
        let _ = std::fs::remove_dir_all(dir);
    }
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
    git(&origin, &["commit", "-qm", "release"]);
    git(
        &origin,
        &["tag", "-a", &format!("v{release}"), "-m", "release"],
    );
    git(
        &root,
        &[
            "clone",
            "-q",
            "--separate-git-dir",
            gitdir.to_str().unwrap(),
            origin.to_str().unwrap(),
            clone.to_str().unwrap(),
        ],
    );
    git(&clone, &["pack-refs", "--all"]);
    let target = root.join("target");

    // Twice: the first build of a target directory writes the runtime
    // archive that build.rs also watches, so the second build reruns the
    // script once whatever git did; from then on only git's files count.
    for _ in 0..2 {
        assert_eq!(
            build_and_ask(&clone, &target),
            format!("ignite {release}"),
            "a clean clone at the release tag"
        );
    }

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
