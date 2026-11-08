//! `docs/slice.md` 28 end to end: a built program raises its open-files
//! soft limit to the hard limit before `K.main` runs, on both targets, and
//! `Process.open_files_limit()` reads the raised value. The fixture
//! `fixtures/open_files_limit/limits.ignite` is built once per target and
//! started under `prlimit --nofile=256:65536`.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use ignite::Target;

use crate::common;

const SOURCE: &str = "limits.ignite";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/open_files_limit")
        .join(SOURCE)
}

fn tmp(name: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("open_files_limit")
        .join(name)
}

fn target_name(target: Target) -> &'static str {
    match target {
        Target::Rust => "rust",
        Target::Llvm => "llvm",
    }
}

/// The executable `limits`, built once per target.
fn executable(target: Target) -> PathBuf {
    static BUILT: Mutex<Vec<(&'static str, PathBuf)>> = Mutex::new(Vec::new());
    let key = target_name(target);
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, path)) = built.iter().find(|(t, _)| *t == key) {
        return path.clone();
    }
    let options = ignite::BuildOptions {
        target,
        out: Some(tmp(format!("build-{key}"))),
        runtime: (target == Target::Llvm).then(common::runtime_archive),
        ..Default::default()
    };
    let output = ignite::build(&fixture(), &options).unwrap_or_else(|e| panic!("{e}"));
    let path = output
        .executables
        .iter()
        .find(|path| path.file_stem().and_then(|s| s.to_str()) == Some("limits"))
        .unwrap_or_else(|| panic!("no limits in {:?}", output.executables))
        .clone();
    built.push((key, path.clone()));
    path
}

/// The soft and hard values of the `Max open files` line of a
/// `/proc/<pid>/limits` text.
fn open_files(limits: &str) -> (String, String) {
    let line = limits
        .lines()
        .find(|line| line.starts_with("Max open files"))
        .unwrap_or_else(|| panic!("no open-files line in {limits}"));
    let mut fields = line["Max open files".len()..].split_whitespace();
    let soft = fields.next().expect("soft limit").to_string();
    let hard = fields.next().expect("hard limit").to_string();
    (soft, hard)
}

macro_rules! both_targets {
    ($($case:ident),* $(,)?) => {
        mod rust {
            $(#[test]
            fn $case() {
                super::$case(ignite::Target::Rust);
            })*
        }
        mod llvm {
            $(#[test]
            fn $case() {
                super::$case(ignite::Target::Llvm);
            })*
        }
    };
}

both_targets!(the_soft_limit_is_raised_to_the_hard_limit_at_start);

/// 27: started with soft 256 and hard 65536, the process runs with soft
/// 65536 by the time `K.main` logs, and `Process.open_files_limit()`
/// reads that value. Nothing is printed besides the logged line.
fn the_soft_limit_is_raised_to_the_hard_limit_at_start(target: Target) {
    let path = executable(target);
    let mut child = Command::new("prlimit")
        .arg("--nofile=256:65536")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("prlimit {}: {e}", path.display()));
    let mut stderr = BufReader::new(child.stderr.take().expect("stderr"));
    let mut first = String::new();
    stderr.read_line(&mut first).expect("read the logged line");
    let limits = std::fs::read_to_string(format!("/proc/{}/limits", child.id()))
        .expect("read /proc/<pid>/limits");
    drop(child.stdin.take());
    let mut rest = String::new();
    std::io::Read::read_to_string(&mut stderr, &mut rest).expect("read the rest of stderr");
    let status = child.wait().expect("wait for limits");
    assert_eq!(first, "open files 65536\n", "rest of stderr: {rest}");
    assert_eq!(
        open_files(&limits),
        ("65536".to_string(), "65536".to_string()),
        "{limits}"
    );
    assert_eq!(rest, "");
    assert_eq!(status.code(), Some(0));
}

/// 27: the fixture's inline test calls `Process.open_files_limit()` on
/// both targets.
#[test]
fn open_files_limit_is_callable_in_tests_on_both_targets() {
    let dir = tmp("inline");
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(fixture())
        .args(["--target", "all", "--out"])
        .arg(&dir)
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .arg("--runtime")
        .arg(common::runner_archive())
        .output()
        .expect("run ignite test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        stdout
            .matches("test result: ok. 1 passed; 0 failed;")
            .count(),
        2,
        "{stdout}"
    );
}
