//! The release archive of `task dist` on the Rust target: the substrate
//! `ignite-io` packed under `<prefix>/share/ignite/substrate`, found by a
//! driver at `<prefix>/bin/ignite` before the source tree it was built
//! from, and `ignite test --substrate`.
//!
//! [`pack_substrate`] is the code `task dist` packs the substrate with:
//! `task dist` runs `dist::the_substrate_packs_into_a_workspace_of_its_own`
//! with `IGNITE_DIST_SHARE`, so the layout these tests build is the
//! archive's. After packing, it runs
//! `dist::the_archive_builds_and_tests_on_the_rust_target_without_the_repository`
//! with `IGNITE_DIST_ARCHIVE` on the archive it wrote: the released driver
//! holds no path of this tree, and from an empty directory it builds and
//! tests a package on the Rust target. Without those variables, as in
//! `task gate`, both tests run on a layout of their own.
//!
//! Crates come from cargo's local cache, never the network: every build
//! here runs with `CARGO_NET_OFFLINE=true`, from the lock the archive
//! carries, whose packages a workspace build (`cargo test -p ignite-io`
//! in `task gate`, `cargo build -p ignite` in `task dist`) has fetched.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn cargo() -> std::ffi::OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

/// The substrate's directory under an archive's `share/ignite`.
const SUBSTRATE: &str = "substrate";

/// Write the substrate of the archive under `share` (its `share/ignite`):
/// `substrate/crates/ignite-io` (the crate's tracked files; it names no
/// other crate by path), `substrate/Cargo.toml`, a workspace of that one
/// member written for the archive with the `[workspace.package]` and the
/// registry entries of `[workspace.dependencies]` of the repository's
/// manifest, which the crate inherits, and `substrate/Cargo.lock`, the
/// repository's lock with the entries that workspace does not use dropped
/// by cargo (offline: no version moves). A program's build seeds its lock
/// from it (`<substrate>/../../Cargo.lock`, `docs/slice.md` 6.2), and
/// `cargo fetch --locked` of it fetches every crate a program can need
/// (`dist/toolchain/Dockerfile`).
pub fn pack_substrate(repository: &Path, share: &Path) {
    let root = share.join(SUBSTRATE);
    let _ = std::fs::remove_dir_all(&root);
    let listed = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(["ls-files", "-z", "--", "crates/ignite-io"])
        .output()
        .expect("start git");
    assert!(
        listed.status.success(),
        "git ls-files: {}",
        text(&listed.stderr)
    );
    let files: Vec<&[u8]> = listed
        .stdout
        .split(|&byte| byte == 0)
        .filter(|name| !name.is_empty())
        .collect();
    assert!(!files.is_empty(), "crates/ignite-io has no tracked file");
    for name in files {
        let name = Path::new(std::str::from_utf8(name).expect("a UTF-8 file name"));
        let to = root.join(name);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(repository.join(name), &to)
            .unwrap_or_else(|error| panic!("copy {}: {error}", name.display()));
    }
    let manifest = std::fs::read_to_string(repository.join("Cargo.toml")).unwrap();
    std::fs::write(root.join("Cargo.toml"), substrate_manifest(&manifest)).unwrap();
    std::fs::copy(repository.join("Cargo.lock"), root.join("Cargo.lock")).unwrap();
    let trimmed = Command::new(cargo())
        .args(["metadata", "--quiet", "--offline", "--format-version", "1"])
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .output()
        .expect("start cargo");
    assert!(
        trimmed.status.success(),
        "cargo metadata of the substrate: {}",
        text(&trimmed.stderr)
    );
}

/// The archive's substrate workspace manifest for the repository's
/// `manifest`: one member, `crates/ignite-io`, and the tables it inherits
/// from, without the entries that name a crate by path (none of which
/// the archive carries).
fn substrate_manifest(manifest: &str) -> String {
    let mut written =
        String::from("[workspace]\nresolver = \"3\"\nmembers = [\"crates/ignite-io\"]\n");
    let mut table = "";
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            table = trimmed;
            if matches!(table, "[workspace.package]" | "[workspace.dependencies]") {
                written.push('\n');
                written.push_str(trimmed);
                written.push('\n');
            }
            continue;
        }
        let kept = match table {
            "[workspace.package]" => true,
            "[workspace.dependencies]" => !trimmed.contains("path ="),
            _ => false,
        };
        if kept && !trimmed.is_empty() && !trimmed.starts_with('#') {
            written.push_str(line);
            written.push('\n');
        }
    }
    written
}

/// [`pack_substrate`] writes a workspace of its own whose lock holds
/// exactly what it uses: `cargo fetch --locked --offline` of it succeeds,
/// as the toolchain image's fetch does with the network. Into
/// `IGNITE_DIST_SHARE` (the archive's `share/ignite`, relative to the
/// workspace) when it is set, which makes this test `task dist`'s packing
/// step, else into a directory of its own.
#[test]
fn the_substrate_packs_into_a_workspace_of_its_own() {
    let share = match std::env::var_os("IGNITE_DIST_SHARE") {
        Some(share) => workspace().join(share),
        None => tmp("dist-pack").join("share/ignite"),
    };
    pack_substrate(&workspace(), &share);
    let root = share.join(SUBSTRATE);
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(
        manifest.starts_with("[workspace]\nresolver = \"3\"\nmembers = [\"crates/ignite-io\"]\n"),
        "{manifest}"
    );
    assert!(!manifest.contains("path ="), "{manifest}");
    let fetched = Command::new(cargo())
        .args(["fetch", "--locked", "--offline", "--quiet"])
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .output()
        .expect("start cargo");
    assert!(
        fetched.status.success(),
        "cargo fetch --locked of the packed substrate: {}",
        text(&fetched.stderr)
    );
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).unwrap();
    let repository = std::fs::read_to_string(workspace().join("Cargo.lock")).unwrap();
    assert!(lock.contains("name = \"ignite-io\""), "{lock}");
    assert!(!lock.contains("name = \"ignite-syntax\""), "{lock}");
    for package in lock.split("\n[[package]]\n").skip(1) {
        let entry = package.split("\ndependencies").next().unwrap();
        assert!(
            repository.contains(entry),
            "the packed lock moved a version:\n{entry}"
        );
    }
}

/// A program with an executable, a std import and a test.
const HELLO: &str = r#"package hello

import std.text.split

type Greeter {
  greeting: Text
  executable: Text
}

fn greet(name: Text) -> Text = "Hello, {name}!"

fn words(line: Text) -> List<Text> = line.split(" ")

fn Greeter.main(self: Greeter) -> Result<(), IoError> {
  Stdout.write_all("{greet(self.greeting)}\n".to_bytes())
}

system Hello {
  component greeter: Greeter {
    greeting = "Ignite"
    executable = "hello"
  }
}

test "greet puts the name in the sentence" {
  assert greet("Ignite") == "Hello, Ignite!"
}

test "words splits at spaces" {
  assert words("a b") == ["a", "b"]
}
"#;

/// Run `driver` in `dir` without the network, retrying an exec that fails
/// with ETXTBSY (a linked driver's inode written an instant before by
/// another test thread's copy).
fn run(driver: &Path, args: &[&std::ffi::OsStr], dir: &Path) -> Output {
    let mut attempts = 0;
    loop {
        let output = Command::new(driver)
            .args(args)
            .current_dir(dir)
            .env("CARGO_NET_OFFLINE", "true")
            .output();
        match output {
            Err(error) if error.raw_os_error() == Some(26) && attempts < 100 => {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            other => return other.unwrap(),
        }
    }
}

/// The `Cargo.toml` files under `dir`, outside cargo's target directories.
fn manifests(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "target") {
                manifests(&path, found);
            }
        } else if path.file_name().is_some_and(|name| name == "Cargo.toml") {
            found.push(path);
        }
    }
}

/// Asserts that every generated project under `out` names `substrate` as
/// its `ignite-io` and none names another.
fn assert_projects_use(out: &Path, substrate: &Path) {
    let mut found = Vec::new();
    manifests(out, &mut found);
    let named = format!("ignite-io = {{ path = \"{}\"", substrate.display());
    let using: Vec<&PathBuf> = found
        .iter()
        .filter(|manifest| {
            std::fs::read_to_string(manifest)
                .unwrap()
                .contains("ignite-io = {")
        })
        .collect();
    assert!(!using.is_empty(), "no project under {}", out.display());
    for manifest in using {
        let content = std::fs::read_to_string(manifest).unwrap();
        assert!(
            content.contains(&named),
            "{} does not name {}:\n{content}",
            manifest.display(),
            substrate.display()
        );
    }
}

/// `ignite build` and `ignite test` on the Rust target with the driver
/// at `<prefix>/bin/ignite` and nothing but its defaults, in an empty
/// `work` directory: both use `<prefix>/share/ignite/substrate`.
fn builds_and_tests_from(prefix: &Path, work: &Path) {
    let prefix = prefix.canonicalize().unwrap();
    let driver = prefix.join("bin/ignite");
    let substrate = prefix
        .join("share/ignite")
        .join(SUBSTRATE)
        .join("crates/ignite-io");
    let source = work.join("hello.ignite");
    std::fs::write(&source, HELLO).unwrap();

    let out = work.join("build");
    let built = run(
        &driver,
        &[
            "build".as_ref(),
            source.as_os_str(),
            "--target".as_ref(),
            "rust".as_ref(),
            "--out".as_ref(),
            out.as_os_str(),
        ],
        work,
    );
    assert!(
        built.status.success(),
        "build: {}{}",
        text(&built.stdout),
        text(&built.stderr)
    );
    assert_projects_use(&out, &substrate);
    assert!(
        text(&built.stderr).contains("Compiling ignite-io v"),
        "cargo compiled no ignite-io: {}",
        text(&built.stderr)
    );
    assert!(
        text(&built.stderr).contains(&format!("({})", substrate.display())),
        "cargo compiled another ignite-io than {}: {}",
        substrate.display(),
        text(&built.stderr)
    );
    let hello = Command::new(out.join("target/debug/hello"))
        .output()
        .unwrap();
    assert_eq!(text(&hello.stdout), "Hello, Ignite!\n");

    let tests = work.join("test");
    let tested = run(
        &driver,
        &[
            "test".as_ref(),
            source.as_os_str(),
            "--target".as_ref(),
            "rust".as_ref(),
            "--out".as_ref(),
            tests.as_os_str(),
        ],
        work,
    );
    assert!(
        tested.status.success(),
        "test: {}{}",
        text(&tested.stdout),
        text(&tested.stderr)
    );
    assert!(
        text(&tested.stdout).contains("2 passed; 0 failed"),
        "{}",
        text(&tested.stdout)
    );
    assert_projects_use(&tests, &substrate);
}

/// Copy the files under `from` to `to`.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

/// `ignite test --substrate PATH` builds the Rust tests against PATH, as
/// `ignite build --substrate PATH` does: a missing crate there fails the
/// build, and cargo's error names it.
#[test]
fn ignite_test_takes_substrate() {
    let dir = tmp("dist-test-substrate");
    let source = dir.join("hello.ignite");
    std::fs::write(&source, HELLO).unwrap();
    let missing = dir.join("nowhere/ignite-io");
    let tested = run(
        Path::new(env!("CARGO_BIN_EXE_ignite")),
        &[
            "test".as_ref(),
            source.as_os_str(),
            "--target".as_ref(),
            "rust".as_ref(),
            "--out".as_ref(),
            dir.join("out").as_os_str(),
            "--substrate".as_ref(),
            missing.as_os_str(),
        ],
        &dir,
    );
    assert!(!tested.status.success(), "{}", text(&tested.stdout));
    assert!(
        text(&tested.stderr).contains(&missing.join("Cargo.toml").display().to_string()),
        "the error does not name {}: {}",
        missing.display(),
        text(&tested.stderr)
    );
}

/// `ignite help` and the help of every command with `--substrate` say
/// where the substrate is looked for, in order: `--substrate`, the crate
/// beside the driver, the source tree's.
#[test]
fn help_names_where_the_substrate_is_looked_for_in_order() {
    for command in [None, Some("build"), Some("test"), Some("emit")] {
        let output = run(
            Path::new(env!("CARGO_BIN_EXE_ignite")),
            &["help".as_ref()]
                .into_iter()
                .chain(command.map(std::ffi::OsStr::new))
                .collect::<Vec<_>>(),
            Path::new(env!("CARGO_TARGET_TMPDIR")),
        );
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
        let help = text(&output.stdout);
        let at = |needle: &str| {
            help.find(needle)
                .unwrap_or_else(|| panic!("{command:?}: no {needle} in\n{help}"))
        };
        let order = [
            at("--substrate"),
            at("PREFIX/share/ignite/substrate/crates/ignite-io"),
            at("PREFIX/bin/ignite"),
            at("crates/ignite-io of the source tree"),
        ];
        assert!(
            order.windows(2).all(|pair| pair[0] < pair[1]),
            "{command:?}: not in order {order:?}:\n{help}"
        );
    }
}

/// The archive on the Rust target. With `IGNITE_DIST_ARCHIVE` (relative to
/// the workspace), `task dist`'s check of the archive it wrote: unpacked
/// into an empty directory, its driver holds neither this tree's path nor
/// the path of this crate, so it cannot reach the repository, and it
/// builds and tests a package on the Rust target from the archive's
/// substrate alone. Without it, every merge: the archive's layout around
/// this build's driver, `bin/ignite` (a hard link, which the driver does
/// not resolve as it does a symbolic one), `share/ignite/std` and the
/// substrate [`pack_substrate`] writes; that driver was built in this
/// tree and can still reach it, so the test shows the order, the
/// substrate beside the driver before the source tree's.
#[test]
fn the_archive_builds_and_tests_on_the_rust_target_without_the_repository() {
    let dir = tmp("dist-archive");
    let prefix = match std::env::var_os("IGNITE_DIST_ARCHIVE") {
        Some(archive) => {
            let unpacked = dir.join("unpacked");
            std::fs::create_dir_all(&unpacked).unwrap();
            let tar = Command::new("tar")
                .arg("-xzf")
                .arg(workspace().join(archive))
                .arg("-C")
                .arg(&unpacked)
                .output()
                .expect("start tar");
            assert!(tar.status.success(), "tar: {}", text(&tar.stderr));
            let prefix = std::fs::read_dir(&unpacked)
                .unwrap()
                .next()
                .expect("the archive holds one directory")
                .unwrap()
                .path();
            let driver = std::fs::read(prefix.join("bin/ignite")).unwrap();
            for path in [
                workspace().display().to_string(),
                env!("CARGO_MANIFEST_DIR").to_string(),
            ] {
                assert!(
                    !driver
                        .windows(path.len())
                        .any(|window| window == path.as_bytes()),
                    "the driver holds the build tree's path {path}"
                );
            }
            prefix
        }
        None => {
            let prefix = dir.join("prefix");
            let bin = prefix.join("bin");
            std::fs::create_dir_all(&bin).unwrap();
            let driver = bin.join("ignite");
            if std::fs::hard_link(env!("CARGO_BIN_EXE_ignite"), &driver).is_err() {
                std::fs::copy(env!("CARGO_BIN_EXE_ignite"), &driver).unwrap();
            }
            let share = prefix.join("share/ignite");
            copy_tree(&workspace().join("std"), &share.join("std"));
            pack_substrate(&workspace(), &share);
            prefix
        }
    };
    let work = dir.join("work");
    std::fs::create_dir_all(&work).unwrap();
    builds_and_tests_from(&prefix, &work);
}
