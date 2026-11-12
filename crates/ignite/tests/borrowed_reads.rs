//! Story `std-mysql-client`, unit `mysql-cost`: a lookup that only reads
//! the elements of lists allocates nothing on the Rust target
//! (`docs/slice.md` 20.3.7). `fixtures/borrowed_reads.ignite` has one of
//! each read-only form: a tuple pattern in a `for`, an indexed element
//! passed to a borrowed parameter, matched, iterated, measured and
//! compared, an element of an element, and a `for` over a list of integers.
//! This test appends [`EXECUTABLES`] to it, which build a table of 50
//! elements and make 1,000 (`lookups-1000`) or 2,000 (`lookups-2000`)
//! lookups over it. The executables stay out of the fixture: the
//! differential corpus would count their `main`s as units the evaluator
//! cannot run (metric 1.6). The allocations the second run makes more than
//! the first are those of 1,000 lookups, counted as `reply_cost::` counts
//! them, with `valgrind --tool=memcheck --trace-malloc=yes`, a prerequisite
//! of this test as of that one.
//!
//! The LLVM target allocates from its task's arena, which `malloc` does
//! not see; the test runs its executables for the values only, which must
//! be the Rust target's.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::common::Target;
use crate::todo;

/// Lookups the second executable makes more than the first.
const EXTRA: f64 = 1000.0;

/// The two executables appended to the fixture.
const EXECUTABLES: &str = r#"
type Lookups {
  executable: Text
  count: Int
}

fn Lookups.main(self: Lookups) -> Result<Unit, IoError> {
  log("{self.count} lookups: {lookups(table(50), self.count)}")
  return Ok(Unit)
}

system BorrowedReads {
  component fewer: Lookups { executable = "lookups-1000", count = 1000 }
  component more: Lookups { executable = "lookups-2000", count = 2000 }
}
"#;

/// The two executables of the fixture built for `target`.
fn executables(target: Target) -> (PathBuf, PathBuf) {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/borrowed_reads.ignite");
    let text = std::fs::read_to_string(&fixture).expect("the fixture");
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("borrowed-reads-source-{target:?}"));
    std::fs::create_dir_all(&dir).expect("the source directory");
    let source = dir.join("borrowed_reads.ignite");
    std::fs::write(&source, text + EXECUTABLES).expect("write the source");
    let out = format!("borrowed-reads-{target:?}");
    (
        todo::build(&source, &out, target, "lookups-1000"),
        todo::build(&source, &out, target, "lookups-2000"),
    )
}

/// The standard error of `binary` run to its end, which must succeed.
fn logged(mut command: Command) -> String {
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("run the executable");
    let text = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "{:?}: {text}", output.status);
    text
}

/// The allocation calls (`malloc`, `calloc`, `memalign` and the like) of
/// `binary` run to its end under valgrind, and its log line.
fn allocations(binary: &Path) -> (u64, String) {
    let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "borrowed-reads-{}-{}.log",
        binary.file_name().and_then(|n| n.to_str()).unwrap_or("run"),
        std::process::id()
    ));
    let mut command = Command::new("valgrind");
    command
        .args(["--tool=memcheck", "--trace-malloc=yes"])
        .arg(format!("--log-file={}", log.display()))
        .arg(binary);
    let line = logged(command);
    let text = std::fs::read_to_string(&log).expect("the valgrind log");
    let count = text
        .lines()
        .filter_map(|line| {
            line.strip_prefix("--")
                .and_then(|rest| rest.split_once("-- "))
        })
        .filter(|(_, call)| {
            matches!(
                call.split('(').next().unwrap_or(""),
                "malloc" | "calloc" | "memalign" | "posix_memalign" | "aligned_alloc" | "valloc"
            )
        })
        .count() as u64;
    let _ = std::fs::remove_file(&log);
    (count, line.trim().to_string())
}

#[test]
fn a_lookup_that_only_reads_allocates_nothing_on_the_rust_target() {
    let status = Command::new("valgrind")
        .args(["--tool=memcheck", "--log-file=/dev/null", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("valgrind is needed to count allocations and did not run: {e}"));
    assert!(status.success(), "valgrind did not run `true`: {status:?}");
    let (fewer, more) = executables(Target::Rust);
    let (first, first_line) = allocations(&fewer);
    let (second, second_line) = allocations(&more);
    let per_lookup = second.saturating_sub(first) as f64 / EXTRA;
    println!("Rust: {first} and {second} allocations, {per_lookup:.3} per lookup");
    assert!(
        per_lookup < 0.01,
        "a read-only lookup allocates {per_lookup:.3} times on the Rust target \
         ({first} allocations for 1,000 lookups, {second} for 2,000)"
    );
    let (fewer_llvm, more_llvm) = executables(Target::Llvm);
    assert_eq!(
        logged(Command::new(&fewer_llvm)).trim(),
        first_line,
        "LLVM, 1,000 lookups"
    );
    assert_eq!(
        logged(Command::new(&more_llvm)).trim(),
        second_line,
        "LLVM, 2,000 lookups"
    );
}
