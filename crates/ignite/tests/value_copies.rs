//! Unit `value-copies-harness` (`docs/value-copies.md` 4.4, item 1): the
//! synthetic workload of the note, `fixtures/value_copies.ignite`, whose
//! inline tests are units of the differential corpus. This test appends
//! [`EXECUTABLES`] to it, which build the tables at size s1 (33 parents,
//! 128 middles, 222 children) and answer one request (`s1-r1`) or two
//! (`s1-r2`), so that the second run less the first is one request without
//! the table build. The executables stay out of the fixture: the
//! differential corpus would count their `main`s as units the evaluator
//! cannot run (metric 1.6).
//!
//! Allocations and bytes are those of the `total heap usage` line of
//! `valgrind --tool=memcheck`, the instrument of the note, a prerequisite
//! of this test as of `borrowed_reads::`. The LLVM target allocates from
//! its task's arena, which `malloc` does not see; its executables run for
//! their digests only, which must be the Rust target's.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::common::Target;
use crate::todo;

/// The most allocations one request at s1 may make on the Rust target
/// since unit `rust-shared-payload-lists`: a row is a payload list shared
/// by its copies (`docs/value-copies.md` 3.4, design D). The count at
/// `1dfc2937`, before the change of representation, was 607,779
/// (`docs/value-copies.md` 2.1).
const MOST_PER_REQUEST: u64 = 52_000;

/// The executables appended to the fixture.
const EXECUTABLES: &str = r#"
type Run {
  executable: Text
  parents: Int
  middles: Int
  children: Int
  requests: Int
}

fn Run.main(self: Run) -> Result<Unit, IoError> {
  let sizes = Sizes { parents = self.parents, middles = self.middles, children = self.children, fields = 8, depth = 3 }
  log("{self.requests} requests: {run(sizes, self.requests)}")
  return Ok(Unit)
}

system ValueCopies {
  component s1r1: Run { executable = "s1-r1", parents = 33, middles = 128, children = 222, requests = 1 }
  component s1r2: Run { executable = "s1-r2", parents = 33, middles = 128, children = 222, requests = 2 }
}
"#;

/// The executable `name` of the fixture with [`EXECUTABLES`] appended,
/// built for `target`.
fn executable(target: Target, name: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/value_copies.ignite");
    let text = std::fs::read_to_string(&fixture).expect("the fixture");
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("value-copies-source-{target:?}"));
    std::fs::create_dir_all(&dir).expect("the source directory");
    let source = dir.join("value_copies.ignite");
    std::fs::write(&source, text + EXECUTABLES).expect("write the source");
    todo::build(&source, &format!("value-copies-{target:?}"), target, name)
}

/// The standard error of `command` run to its end, which must succeed.
fn logged(mut command: Command) -> String {
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("run the executable");
    let text = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "{:?}: {text}", output.status);
    text.trim().to_string()
}

/// The allocations and bytes of `binary` run to its end under memcheck
/// (its `total heap usage` line), and its log line.
fn heap(binary: &Path) -> (u64, u64, String) {
    let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "value-copies-{}-{}.log",
        binary.file_name().and_then(|n| n.to_str()).unwrap_or("run"),
        std::process::id()
    ));
    let mut command = Command::new("valgrind");
    command
        .arg("--tool=memcheck")
        .arg(format!("--log-file={}", log.display()))
        .arg(binary);
    let line = logged(command);
    let text = std::fs::read_to_string(&log).expect("the valgrind log");
    let _ = std::fs::remove_file(&log);
    let usage = text
        .lines()
        .find_map(|line| line.split_once("total heap usage: ").map(|(_, rest)| rest))
        .unwrap_or_else(|| panic!("no heap usage in the valgrind log:\n{text}"));
    let number = |part: &str| -> u64 {
        part.split_whitespace()
            .next()
            .and_then(|n| n.replace(',', "").parse().ok())
            .unwrap_or_else(|| panic!("a count in {usage:?}"))
    };
    let parts: Vec<&str> = usage.split(", ").collect();
    (number(parts[0]), number(parts[2]), line)
}

/// One request of the workload at s1 allocates no more on the Rust target
/// than [`MOST_PER_REQUEST`], and the LLVM executables print the digests
/// the Rust ones print.
#[test]
fn one_request_of_the_workload_allocates_no_more_than_the_bar() {
    let status = Command::new("valgrind")
        .args(["--tool=memcheck", "--log-file=/dev/null", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("valgrind is needed to count allocations and did not run: {e}"));
    assert!(status.success(), "valgrind did not run `true`: {status:?}");
    let (one, one_bytes, one_line) = heap(&executable(Target::Rust, "s1-r1"));
    let (two, two_bytes, two_line) = heap(&executable(Target::Rust, "s1-r2"));
    let per_request = two.saturating_sub(one);
    let bytes = two_bytes.saturating_sub(one_bytes);
    println!(
        "Rust, s1: {one} and {two} allocations, {per_request} per request \
         ({bytes} bytes per request)"
    );
    assert!(
        per_request <= MOST_PER_REQUEST,
        "one request at s1 allocates {per_request} times on the Rust target, more than \
         {MOST_PER_REQUEST} ({one} allocations for one request, {two} for two)"
    );
    assert_eq!(
        logged(Command::new(executable(Target::Llvm, "s1-r1"))),
        one_line,
        "LLVM, one request"
    );
    assert_eq!(
        logged(Command::new(executable(Target::Llvm, "s1-r2"))),
        two_line,
        "LLVM, two requests"
    );
}
