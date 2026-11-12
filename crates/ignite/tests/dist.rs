//! The release archive of `task dist` on the Rust target: the substrate
//! `ignite-io` packed under `<prefix>/share/ignite/substrate`, found by a
//! driver at `<prefix>/bin/ignite` before the source tree it was built
//! from, and `--substrate` of `ignite test`, `conformance` and `run`.
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
//! carries. Packing and its check need only the crates a workspace build
//! for this host (`cargo test -p ignite-io` in `task gate`, `cargo build
//! -p ignite` in `task dist`) has fetched, none of another platform.

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
/// `substrate/crates/ignite-io` (the crate's tracked files but its
/// integration tests, `tests/`, which no program's build compiles; it
/// names no other crate by path), `substrate/Cargo.toml`, a workspace of that one
/// member written for the archive with the `[workspace.package]` and the
/// registry entries of `[workspace.dependencies]` of the repository's
/// manifest, which the crate inherits, and `substrate/Cargo.lock`, the
/// repository's lock with the entries that workspace does not use dropped
/// by `cargo update --workspace --offline`, which moves no version and
/// downloads nothing. A program's build seeds its lock from it
/// (`<substrate>/../../Cargo.lock`, `docs/slice.md` 6.2), and `cargo fetch
/// --locked` of it fetches every crate a program can need
/// (`dist/toolchain/Dockerfile`).
pub fn pack_substrate(repository: &Path, share: &Path) {
    pack_substrate_with(repository, share, None).unwrap_or_else(|error| panic!("{error}"));
}

/// Cargo, with `home` as its `CARGO_HOME` when given.
fn cargo_in(home: Option<&Path>) -> Command {
    let mut command = Command::new(cargo());
    if let Some(home) = home {
        command.env("CARGO_HOME", home);
    }
    command
}

/// Run `command`; its stderr, prefixed with `what`, when it fails.
fn succeeds(what: &str, mut command: Command) -> Result<(), String> {
    let output = command.output().expect("start cargo");
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("{what}: {}", text(&output.stderr)))
    }
}

/// [`pack_substrate`] with cargo's home `home`; cargo's error when cargo
/// fails.
fn pack_substrate_with(repository: &Path, share: &Path, home: Option<&Path>) -> Result<(), String> {
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
        .filter(|name| !name.is_empty() && !name.starts_with(b"crates/ignite-io/tests/"))
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
    let mut trim = cargo_in(home);
    trim.args(["update", "--workspace", "--offline", "--quiet"])
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"));
    succeeds("cargo update --workspace of the substrate", trim)
}

/// The check of a packed substrate at `root` (its `substrate/`), with
/// cargo's home `home`: its lock is exact (`--locked`), and every crate a
/// program's build for this host can need, `ignite-io` with all its
/// features, is in the cache (`--offline`). Crates of other platforms and
/// the crate's dev-dependencies are not needed: a build fetches neither.
fn fetch_packed(root: &Path, home: Option<&Path>) -> Result<(), String> {
    let mut tree = cargo_in(home);
    tree.args(["tree", "--locked", "--offline", "--quiet", "--all-features"])
        .args(["--edges", "normal,build", "--target", &host()])
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .stdout(std::process::Stdio::null());
    succeeds("cargo tree --locked of the packed substrate", tree)
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
/// exactly what it uses and moves no version, and [`fetch_packed`]
/// passes on it. Into
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
    assert!(root.join("crates/ignite-io/src/lib.rs").is_file());
    assert!(
        !root.join("crates/ignite-io/tests").exists(),
        "the archive carries the substrate's integration tests"
    );
    fetch_packed(&root, None).unwrap_or_else(|error| panic!("{error}"));
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

/// The cargo home of this run.
fn cargo_home() -> PathBuf {
    std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap()).join(".cargo"))
}

/// The host's target triple, `host:` of `rustc -vV`.
fn host() -> String {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc).arg("-vV").output().unwrap();
    text(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .expect("rustc -vV names the host")
        .to_string()
}

/// The Rust toolchain's root, `rustc --print sysroot`.
fn sysroot() -> String {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    text(&output.stdout).trim().to_string()
}

/// A build fetches the crates of its own target only. In a cargo home that
/// holds every crate the substrate's build for this host uses (normal,
/// build and dev edges) and nothing else, as a first `task gate` or `task
/// dist` on a machine leaves it, [`pack_substrate`] packs and the check
/// [`fetch_packed`] makes passes: neither needs a crate file of another
/// platform. (Adversary case of unit `pkg`.)
#[test]
fn the_packed_substrate_fetches_offline_with_the_crates_a_host_build_fetched() {
    let dir = tmp("dist-host-crates");
    let share = dir.join("share/ignite");
    pack_substrate(&workspace(), &share);
    let manifest = share.join(SUBSTRATE).join("Cargo.toml");

    let tree = Command::new(cargo())
        .args([
            "tree",
            "--offline",
            "--locked",
            "--quiet",
            "--prefix",
            "none",
        ])
        .args(["--edges", "normal,build,dev", "--target", &host()])
        .arg("--manifest-path")
        .arg(&manifest)
        .output()
        .unwrap();
    assert!(tree.status.success(), "cargo tree: {}", text(&tree.stderr));
    let crates: std::collections::BTreeSet<String> = text(&tree.stdout)
        .lines()
        .filter(|line| !line.contains(" (/"))
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let name = words.next()?;
            let version = words.next()?.strip_prefix('v')?;
            Some(format!("{name}-{version}.crate"))
        })
        .collect();
    assert!(
        crates.iter().any(|name| name.starts_with("nix-")),
        "{crates:?}"
    );

    let registry = cargo_home().join("registry");
    let home = dir.join("cargo-home");
    std::fs::create_dir_all(home.join("registry/cache")).unwrap();
    std::os::unix::fs::symlink(registry.join("index"), home.join("registry/index")).unwrap();
    let mut found = std::collections::BTreeSet::new();
    for source in std::fs::read_dir(registry.join("cache")).unwrap() {
        let source = source.unwrap().path();
        let cache = home
            .join("registry/cache")
            .join(source.file_name().unwrap());
        std::fs::create_dir_all(&cache).unwrap();
        for name in &crates {
            if source.join(name).is_file() {
                std::os::unix::fs::symlink(source.join(name), cache.join(name)).unwrap();
                found.insert(name.clone());
            }
        }
    }
    assert_eq!(found, crates, "crates of the host build not in the cache");

    let share = dir.join("host-only/share/ignite");
    let failed: Vec<String> = [
        pack_substrate_with(&workspace(), &share, Some(&home)),
        fetch_packed(&share.join(SUBSTRATE), Some(&home)),
    ]
    .into_iter()
    .filter_map(Result::err)
    .collect();
    assert!(
        failed.is_empty(),
        "with the {} crates of a build for {}:\n{}",
        crates.len(),
        host(),
        failed.join("\n")
    );
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

/// A `share/ignite/substrate/crates/ignite-io` beside the driver that holds
/// no `Cargo.toml` is no substrate: the driver falls through to the next
/// place, the source tree it was built from.
#[test]
fn a_substrate_directory_without_a_manifest_is_passed_over() {
    let dir = tmp("dist-no-manifest");
    let bin = dir.join("prefix/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let driver = bin.join("ignite");
    if std::fs::hard_link(env!("CARGO_BIN_EXE_ignite"), &driver).is_err() {
        std::fs::copy(env!("CARGO_BIN_EXE_ignite"), &driver).unwrap();
    }
    let beside = dir.join("prefix/share/ignite/substrate/crates/ignite-io");
    std::fs::create_dir_all(beside.join("src")).unwrap();
    std::fs::write(beside.join("src/lib.rs"), "").unwrap();
    assert_eq!(ignite::installed_substrate_path(&driver), None);
    let source = dir.join("hello.ignite");
    std::fs::write(&source, HELLO).unwrap();
    let emitted = run(&driver, &["emit".as_ref(), source.as_os_str()], &dir);
    assert!(emitted.status.success(), "{}", text(&emitted.stderr));
    let tree = workspace().join("crates/ignite-io");
    assert!(
        text(&emitted.stdout).contains(&format!("ignite-io = {{ path = \"{}\" }}", tree.display())),
        "the driver does not fall through to {}:\n{}",
        tree.display(),
        text(&emitted.stdout)
    );
}

/// `ignite conformance --substrate PATH` and `ignite run --substrate PATH`
/// build on the Rust target against PATH, as `ignite build` does: a
/// missing crate there fails the build, and cargo's error names it.
#[test]
fn conformance_and_run_take_substrate() {
    let dir = tmp("dist-conformance-run-substrate");
    let missing = dir.join("nowhere/ignite-io");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let conformance = fixtures.join("conformance/complete.ignite");
    let gave_up = fixtures.join("run_gave_up.ignite");
    let out = dir.join("conformance");
    let state = dir.join("state");
    let commands: [Vec<&std::ffi::OsStr>; 2] = [
        vec![
            "conformance".as_ref(),
            conformance.as_os_str(),
            "--target".as_ref(),
            "rust".as_ref(),
            "--out".as_ref(),
            out.as_os_str(),
        ],
        vec![
            "run".as_ref(),
            gave_up.as_os_str(),
            "--target".as_ref(),
            "rust".as_ref(),
            "--state".as_ref(),
            state.as_os_str(),
        ],
    ];
    for mut args in commands {
        args.extend(["--substrate".as_ref(), missing.as_os_str()]);
        let output = run(Path::new(env!("CARGO_BIN_EXE_ignite")), &args, &dir);
        assert!(
            !output.status.success(),
            "{args:?}: {}",
            text(&output.stdout)
        );
        assert!(
            text(&output.stderr).contains(&missing.join("Cargo.toml").display().to_string()),
            "{args:?}: the error does not name {}: {}",
            missing.display(),
            text(&output.stderr)
        );
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
    for command in [
        None,
        Some("build"),
        Some("test"),
        Some("emit"),
        Some("run"),
        Some("conformance"),
    ] {
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

/// Every place `driver` holds the path of this tree, this crate, the cargo
/// home, the Rust toolchain or the home directory, with the text after it.
/// One kind is passed over: the source names of ring's C files under the
/// cargo home. The bitcode runtime the driver carries holds them as the
/// `source_filename` of their modules, which clang writes from the path it
/// was given and `-ffile-prefix-map` does not remap.
fn build_machine_paths(driver: &[u8]) -> Vec<String> {
    let cargo_home = cargo_home().display().to_string();
    let ring_source = |at: usize| {
        let rest = &driver[at + cargo_home.len()..];
        let Some(rest) = rest.strip_prefix(b"/registry/src/".as_slice()) else {
            return false;
        };
        let index = rest.iter().position(|byte| *byte == b'/');
        index.is_some_and(|index| rest[index + 1..].starts_with(b"ring-"))
    };
    let passed_over: Vec<usize> = driver
        .windows(cargo_home.len())
        .enumerate()
        .filter(|(at, window)| *window == cargo_home.as_bytes() && ring_source(*at))
        .map(|(at, _)| at)
        .collect();
    let mut found = Vec::new();
    for path in [
        workspace().display().to_string(),
        env!("CARGO_MANIFEST_DIR").to_string(),
        cargo_home.clone(),
        sysroot(),
        std::env::var("HOME").unwrap_or_default(),
    ] {
        if path.len() < 2 {
            continue;
        }
        for (at, window) in driver.windows(path.len()).enumerate() {
            if window == path.as_bytes() && !passed_over.contains(&at) {
                let end = driver[at..]
                    .iter()
                    .position(|byte| !byte.is_ascii_graphic())
                    .map_or(driver.len(), |length| at + length.min(160));
                found.push(String::from_utf8_lossy(&driver[at..end]).into_owned());
            }
        }
    }
    found
}

/// The archive on the Rust target. With `IGNITE_DIST_ARCHIVE` (relative to
/// the workspace), `task dist`'s check of the archive it wrote: unpacked
/// into an empty directory, its driver holds neither this tree's path nor
/// the path of this crate, so it cannot reach the repository, nor the path
/// of the cargo home, the Rust toolchain or the home directory it was
/// built with (`task dist` remaps them; [`build_machine_paths`] names the
/// one kind it passes over), and it
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
            let found = build_machine_paths(&driver);
            assert!(
                found.is_empty(),
                "the driver holds {} paths of the machine that built it:\n{}",
                found.len(),
                found.join("\n")
            );
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
