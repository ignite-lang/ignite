//! Adversary pass 1 on story `rust-peak-memory-at-one-connection`: the
//! release build that links with the release layout
//! (`ignite_rustgen::RELEASE_LAYOUT`) and builds again without it when
//! cargo fails. `rust_memory.rs` shows the fallback for a linker that reads
//! no such script (gold); these cases are the builds that fail for another
//! reason, and what the fallback leaves behind for the builds after them.
//! Nothing here is timed.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The words of the driver's note when it builds again without the layout.
const NOTE: &str = "building without it";

/// How the functions of the backtrace symbolizer are named in a symbol, as
/// `rust_memory.rs` has it.
const SYMBOLIZER: [&str; 5] = [
    "12backtrace_rs",
    "5gimli",
    "9addr2line",
    "14rustc_demangle",
    "11miniz_oxide",
];

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/backtrace.ignite")
}

/// A fresh directory of this file's own under the test directory.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("rust-memory-adversary")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `ignite build --target rust --release` of the fixture into `dir/out` and
/// `dir/cargo`, with `extra` arguments and what `configure` does to the
/// command.
fn release_build(dir: &Path, extra: &[&str], configure: impl FnOnce(&mut Command)) -> Output {
    release_build_of(&fixture(), dir, extra, configure)
}

/// [`release_build`] of the program at `source`.
fn release_build_of(
    source: &Path,
    dir: &Path,
    extra: &[&str],
    configure: impl FnOnce(&mut Command),
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
    command
        .arg("build")
        .arg(source)
        .args(["--target", "rust", "--release", "--out"])
        .arg(dir.join("out"))
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .args(extra)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS");
    configure(&mut command);
    command.output().expect("run ignite build")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Of the functions of `executable` that have a size (`nm -S`): how many
/// are the symbolizer's, and the others that start before the last of the
/// symbolizer's ends. With the release layout there is none of the second.
fn layout_of(executable: &Path) -> (usize, Vec<String>) {
    let output = Command::new("nm")
        .args(["-S", "--defined-only"])
        .arg(executable)
        .output()
        .expect("run nm");
    assert!(output.status.success(), "nm {}", executable.display());
    let functions: Vec<(u64, u64, String)> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let [start, size, kind, name] = fields[..] else {
                return None;
            };
            let size = u64::from_str_radix(size, 16).ok()?;
            (matches!(kind, "t" | "T") && size > 0).then(|| {
                (
                    u64::from_str_radix(start, 16).expect("an address"),
                    size,
                    name.to_string(),
                )
            })
        })
        .collect();
    let symbolizer = |name: &str| SYMBOLIZER.iter().any(|part| name.contains(part));
    let end = functions
        .iter()
        .filter(|(_, _, name)| symbolizer(name))
        .map(|(start, size, _)| start + size)
        .max()
        .unwrap_or(0);
    let count = functions
        .iter()
        .filter(|(_, _, name)| symbolizer(name))
        .count();
    let others = functions
        .into_iter()
        .filter(|(start, _, name)| !symbolizer(name) && *start < end)
        .map(|(_, _, name)| name)
        .collect();
    (count, others)
}

/// A release build that cargo fails for a reason no linker script has part
/// in (here: the `--substrate` does not exist, so cargo cannot read the
/// project's dependency and nothing is compiled or linked) fails as it did
/// before the layout: cargo's error once, exit 2, and no note that says the
/// linker script failed the build. At the base commit cargo runs once and
/// its error is printed once.
#[test]
fn a_release_build_that_fails_before_any_link_fails_once_and_blames_no_linker_script() {
    let dir = scratch("no-substrate");
    let missing = dir.join("no-such-substrate");
    let output = release_build(&dir, &["--substrate", missing.to_str().unwrap()], |_| {});
    let stderr = stderr_of(&output);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert_eq!(
        stderr.matches("error: failed to load manifest").count(),
        1,
        "cargo ran more than once for one failing build:\n{stderr}"
    );
    assert!(
        !stderr.contains(NOTE),
        "the build never reached a linker, and the note says the linker script failed it:\n{stderr}"
    );
}

/// A cargo that fails once for the project and then works (as a compiler
/// the kernel killed for memory, a full disk or a registry that did not
/// answer make it) must not turn the release layout off for the target
/// directory: the next release build into it, with nothing failing, links
/// its executable with the symbolizer in one run at the start of the code,
/// as a build into a fresh directory does.
#[test]
fn a_release_build_after_one_that_cargo_failed_once_still_links_with_the_layout() {
    let dir = scratch("failed-once");
    let manifest = dir.join("out").join("Cargo.toml");
    let failed = dir.join("failed-once");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let wrapper = dir.join("cargo-once");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  if [ \"$arg\" = '{}' ] && [ ! -e '{}' ]; then\n    : > '{}'\n    echo 'error: this cargo fails its first build of the project' >&2\n    exit 101\n  fi\ndone\nexec '{}' \"$@\"\n",
            manifest.display(),
            failed.display(),
            failed.display(),
            Path::new(&cargo).display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();

    let first = release_build(&dir, &[], |command| {
        command.env("CARGO", &wrapper);
    });
    assert!(
        failed.is_file(),
        "the failing cargo was not asked to build the project:\n{}",
        stderr_of(&first)
    );

    let second = release_build(&dir, &[], |_| {});
    let stderr = stderr_of(&second);
    assert!(second.status.success(), "{stderr}");
    let executable = dir.join("cargo/release/overflow");
    let (symbolizer, others) = layout_of(&executable);
    assert!(
        symbolizer > 0,
        "no function of the symbolizer by name in {}",
        executable.display()
    );
    assert!(
        others.is_empty(),
        "after one failed build the next release build links without the layout: {} functions \
         that are not the symbolizer's lie before or among its {symbolizer}; the first build \
         said:\n{}\nthe second:\n{stderr}",
        others.len(),
        stderr_of(&first)
    );
}

/// The functions of `executable` are laid out as the release layout has
/// them: the symbolizer's are there by name and no other function starts
/// before the last of them ends.
fn assert_layout(executable: &Path, context: &str) {
    let (symbolizer, others) = layout_of(executable);
    assert!(
        symbolizer > 0,
        "no function of the symbolizer by name in {}\n{context}",
        executable.display()
    );
    assert!(
        others.is_empty(),
        "{}: {} functions that are not the symbolizer's lie before or among its {symbolizer}\n{context}",
        executable.display(),
        others.len()
    );
}

/// `docs/slice.md` 6: `ignite build` gives cargo the link arguments itself,
/// "which overrides the build script: cargo neither compiles nor runs it".
/// So a release build by the driver leaves no build directory of the
/// project's own package in the target directory (cargo makes one per
/// package whose build script it compiles), and the executable still has
/// the layout. Green when written; a driver that does not pass the
/// `--config` builds and runs `layout.rs` on every cold release build, and
/// no other test notices (mutant `noconfig` of this pass).
#[test]
fn the_drivers_release_build_compiles_no_build_script_of_the_project() {
    let dir = scratch("no-build-script");
    let built = release_build(&dir, &[], |_| {});
    let stderr = stderr_of(&built);
    assert!(built.status.success(), "{stderr}");
    assert_layout(&dir.join("cargo/release/overflow"), &stderr);
    let own: Vec<String> = std::fs::read_dir(dir.join("cargo/release/build"))
        .expect("the build directories of the dependencies")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.starts_with("dev_backtrace-"))
        .collect();
    assert!(
        own.is_empty(),
        "cargo compiled or ran the project's build script: {own:?}\n{stderr}"
    );
}

/// A static release build (`--static`, musl, which GNU ld links) has the
/// layout too. Green when written; a driver that leaves the layout out of
/// a build with a target triple fails no other test (mutant `nostatic` of
/// this pass).
#[test]
fn a_static_release_build_links_with_the_layout() {
    let dir = scratch("static");
    let built = release_build(&dir, &["--static"], |_| {});
    let stderr = stderr_of(&built);
    assert!(built.status.success(), "{stderr}");
    let executable = dir
        .join("cargo")
        .join(format!("{}-unknown-linux-musl", std::env::consts::ARCH))
        .join("release/overflow");
    assert_layout(&executable, &stderr);
}

/// `docs/slice.md` 6: the build script of a release project "is for a
/// `cargo build --release` by hand in `DIR`". A program with no executable
/// (a package of functions only) is a release project too, and cargo by
/// hand builds it, as it did before the layout: at the base commit the
/// same two commands end with status 0.
#[test]
fn cargo_by_hand_builds_the_release_project_of_a_program_without_an_executable() {
    let dir = scratch("library-only");
    let source = dir.join("library.ignite");
    std::fs::write(
        &source,
        "package adversary_library\n\nfn add(a: Int, b: Int) -> Int {\n  return a + b\n}\n",
    )
    .unwrap();
    let built = release_build_of(&source, &dir, &[], |_| {});
    assert!(built.status.success(), "{}", stderr_of(&built));

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let by_hand = Command::new(cargo)
        .args(["build", "--release", "--offline", "--manifest-path"])
        .arg(dir.join("out").join("Cargo.toml"))
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .output()
        .expect("run cargo");
    assert!(
        by_hand.status.success(),
        "cargo build --release in the generated project:\n{}",
        stderr_of(&by_hand)
    );
}
