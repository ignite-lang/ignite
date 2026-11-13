//! Story `move-analysis-cost-of-fields-set-in-if-without-else`: `ignite
//! check` types a fn that sets N fields of one `var` record, each in its
//! own `if` without `else`, with work and memory that grow with N as they
//! do for the same fn written with `else` branches, an `else if` chain,
//! `match` arms or nested `if`s. The last-use moves analysis
//! (`crates/ignite-types/src/moves.rs`) once doubled its live set with
//! every such `if`.
//!
//! Counted outside the process, as `reply_cost` counts allocations:
//! `valgrind --tool=callgrind` counts the instructions of `ignite check`,
//! `valgrind --tool=dhat` its peak of live heap bytes (`At t-gmax`).
//! `valgrind` is a prerequisite of these tests; without it they fail and
//! name it. No test here reads a clock.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The fields of the smaller program of each pair.
const SMALL: usize = 8;

/// The fields of the larger program: twice [`SMALL`].
const LARGE: usize = 2 * SMALL;

/// The largest ratio of a count at [`LARGE`] to the count at [`SMALL`]:
/// what a cubic in N gives when N doubles (2^3). The doubling of the live
/// set gives about 2^8 between 8 and 16 fields.
const BOUND: f64 = 8.0;

/// The fields of the program that type-checks under [`DATA_LIMIT`].
const MANY: usize = 64;

/// The compile thread's stack (`STACK` in `crates/ignite/src/main.rs`):
/// mapped once, it counts against the data limit of the process.
const STACK: u64 = 256 << 20;

/// The data limit (`RLIMIT_DATA`) of `ignite check` on [`MANY`] fields:
/// 256 MiB above the compile thread's stack.
const DATA_LIMIT: u64 = STACK + (256 << 20);

/// How the generated fn sets its fields.
#[derive(Clone, Copy, Debug)]
enum Form {
    /// `if k > i { r.fi = i }`, one `if` without `else` per field.
    IfWithoutElse,
    /// `if k > i { r.fi = i } else { r.fi = 0 - i }`.
    IfElse,
    /// `if k == 0 { r.f0 = 1 } else if k == 1 { r.f1 = 1 } …`.
    ElseIf,
    /// `match k { 0 => { r.f0 = 1 } … _ => { r.f0 = 2 } }`.
    Match,
    /// `if k > 0 { r.f0 = 0 if k > 1 { r.f1 = 1 … } }`.
    Nested,
}

/// The program of `form` with `n` fields: a record `R` and a fn `fill` that
/// sets each field of a `var r` once.
fn program(form: Form, n: usize) -> String {
    let mut text = String::from("package tests.moves_cost\n\ntype R {\n");
    for i in 0..n {
        text += &format!("  f{i}: Int\n");
    }
    text += "}\n\nfn fill(k: Int) -> R {\n  var r = R { ";
    text += &(0..n)
        .map(|i| format!("f{i} = 0"))
        .collect::<Vec<_>>()
        .join(", ");
    text += " }\n";
    match form {
        Form::IfWithoutElse => {
            for i in 0..n {
                text += &format!("  if k > {i} {{ r.f{i} = {i} }}\n");
            }
        }
        Form::IfElse => {
            for i in 0..n {
                text += &format!("  if k > {i} {{ r.f{i} = {i} }} else {{ r.f{i} = 0 - {i} }}\n");
            }
        }
        Form::ElseIf => {
            text += "  if k == 0 { r.f0 = 1 }";
            for i in 1..n {
                text += &format!(" else if k == {i} {{ r.f{i} = 1 }}");
            }
            text += "\n";
        }
        Form::Match => {
            text += "  match k {\n";
            for i in 0..n {
                text += &format!("    {i} => {{ r.f{i} = 1 }}\n");
            }
            text += "    _ => { r.f0 = 2 }\n  }\n";
        }
        Form::Nested => {
            for i in 0..n {
                text += &format!("  if k > {i} {{ r.f{i} = {i}\n");
            }
            for _ in 0..n {
                text += "  }\n";
            }
        }
    }
    text += "  return r\n}\n";
    text
}

/// A new empty directory under the test's scratch directory.
fn scratch(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "moves-cost-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `valgrind` runs here; otherwise nothing can be counted, and the test
/// says which prerequisite is missing.
fn valgrind_works() {
    let status = Command::new("valgrind")
        .args(["--tool=none", "--log-file=/dev/null", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("valgrind is needed to count and did not run: {e}"));
    assert!(status.success(), "valgrind did not run `true`: {status:?}");
}

/// The program of `form` with `n` fields, written into `dir`.
fn write_program(dir: &Path, form: Form, n: usize) -> PathBuf {
    let path = dir.join(format!("fill{n}.ignite"));
    std::fs::write(&path, program(form, n)).unwrap();
    path
}

/// `ignite check` of `source` under `valgrind --tool=<tool>` with `args`;
/// the check must pass.
fn check_under(tool: &str, args: &[String], source: &Path) -> Output {
    let output = Command::new("valgrind")
        .arg(format!("--tool={tool}"))
        .args(args)
        .arg(env!("CARGO_BIN_EXE_ignite"))
        .arg("check")
        .arg(source)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("valgrind --tool={tool}: {e}"));
    assert!(
        output.status.success(),
        "ignite check {} under {tool}: {:?}\n{}\n{}",
        source.display(),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// The number after `marker` on the first line of valgrind's report that
/// has it, digit-group commas dropped.
fn reported(output: &Output, marker: &str) -> u64 {
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr
        .lines()
        .find_map(|line| {
            let rest = line.split_once(marker)?.1.trim_start();
            let digits: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == ',')
                .filter(|c| *c != ',')
                .collect();
            digits.parse().ok()
        })
        .unwrap_or_else(|| panic!("no `{marker}` in valgrind's report:\n{stderr}"))
}

/// The instructions `ignite check` runs on the program of `form` with `n`
/// fields.
fn instructions(form: Form, n: usize) -> u64 {
    let dir = scratch("instructions");
    let source = write_program(&dir, form, n);
    let out = format!(
        "--callgrind-out-file={}",
        dir.join("callgrind.out").display()
    );
    reported(&check_under("callgrind", &[out], &source), "Collected :")
}

/// The peak of live heap bytes of `ignite check` on the program of `form`
/// with `n` fields.
fn peak_bytes(form: Form, n: usize) -> u64 {
    let dir = scratch("bytes");
    let source = write_program(&dir, form, n);
    let out = format!("--dhat-out-file={}", dir.join("dhat.json").display());
    reported(&check_under("dhat", &[out], &source), "At t-gmax:")
}

/// What one measure of `form` failed with against [`BOUND`]: its counts at
/// [`SMALL`] and [`LARGE`] fields and their ratio, printed either way.
fn within_bound(form: Form, what: &str, measure: fn(Form, usize) -> u64) -> Option<String> {
    let small = measure(form, SMALL);
    let large = measure(form, LARGE);
    let ratio = large as f64 / small as f64;
    let line = format!(
        "{form:?}: {what} {large} at {LARGE} fields against {small} at {SMALL}, ratio {ratio:.2}, bound {BOUND}"
    );
    println!("{line}");
    (ratio > BOUND).then_some(line)
}

/// The reported form: N fields each set in an `if` without `else`. Its
/// instructions and its peak heap bytes at 16 fields stay within
/// [`BOUND`] times those at 8, as a polynomial of low degree does and the
/// doubling of the live set does not.
#[test]
fn fields_set_in_if_without_else_cost_what_a_polynomial_does() {
    valgrind_works();
    if let Some(failure) = within_bound(Form::IfWithoutElse, "instructions", instructions) {
        panic!("{failure}");
    }
    if let Some(failure) = within_bound(Form::IfWithoutElse, "peak heap bytes", peak_bytes) {
        panic!("{failure}");
    }
}

/// The forms that were flat before the story (`else` branches, an `else
/// if` chain, `match` arms, nested `if`s) stay within the same bound.
#[test]
fn the_flat_forms_stay_within_the_same_bound() {
    valgrind_works();
    let mut failures = Vec::new();
    for form in [Form::IfElse, Form::ElseIf, Form::Match, Form::Nested] {
        failures.extend(within_bound(form, "instructions", instructions));
        failures.extend(within_bound(form, "peak heap bytes", peak_bytes));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// [`MANY`] fields each set in an `if` without `else` type-check with the
/// process's data limited to [`DATA_LIMIT`] (`prlimit --data`, as
/// `open_files_limit` limits descriptors): 256 MiB of heap above the
/// compile thread's stack.
#[test]
fn sixty_four_fields_type_check_under_a_memory_limit() {
    let dir = scratch("limit");
    let source = write_program(&dir, Form::IfWithoutElse, MANY);
    let output = Command::new("prlimit")
        .arg(format!("--data={DATA_LIMIT}"))
        .arg(env!("CARGO_BIN_EXE_ignite"))
        .arg("check")
        .arg(&source)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("prlimit is needed to limit the check and did not run: {e}"));
    assert!(
        output.status.success(),
        "ignite check of {MANY} fields under a data limit of {DATA_LIMIT} bytes: {:?}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
