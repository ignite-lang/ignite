//! Adversary pass on story
//! `move-analysis-cost-of-fields-set-in-if-without-else`: a form the count
//! test does not write. N statements `if k > i { r.f<2i> = i } else {
//! r.f<2i+1> = i }` set a different field in each branch, so neither
//! branch's live entry for `r` covers the other's and the live set doubles
//! with every statement, before the story and after it. Before the story
//! the join was the union of two sets, linear in their size; the reduced
//! join (`Live::join` in `crates/ignite-types/src/moves.rs`) compares every
//! entry of one side with every entry of the other at the same place, so
//! each statement costs the square of the set it doubles.
//!
//! Counted as `moves_cost` counts: `valgrind --tool=callgrind`, the
//! instructions of `ignite check`. No test here reads a clock.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The statements of the smaller program.
const SMALL: usize = 8;

/// The statements of the larger program: two more.
const LARGE: usize = SMALL + 2;

/// The largest ratio of the instructions at [`LARGE`] to those at
/// [`SMALL`]: two more statements multiply the live set by 4, and a join
/// linear in the set it joins, as at `cb9c3e4e` (55,522,900 against
/// 262,887,994 instructions, ratio 4.73), stays within twice that. A join
/// quadratic in the set multiplies by 16.
const BOUND: f64 = 8.0;

/// The program with `n` statements setting one of two fields each.
fn program(n: usize) -> String {
    let mut text = String::from("package tests.moves_cost_adversary\n\ntype R {\n");
    for i in 0..2 * n {
        text += &format!("  f{i}: Int\n");
    }
    text += "}\n\nfn fill(k: Int) -> R {\n  var r = R { ";
    text += &(0..2 * n)
        .map(|i| format!("f{i} = 0"))
        .collect::<Vec<_>>()
        .join(", ");
    text += " }\n";
    for i in 0..n {
        text += &format!(
            "  if k > {i} {{ r.f{} = {i} }} else {{ r.f{} = {i} }}\n",
            2 * i,
            2 * i + 1
        );
    }
    text += "  return r\n}\n";
    text
}

fn scratch(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "moves-cost-adversary-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The instructions `ignite check` runs on the program with `n`
/// statements.
fn instructions(n: usize) -> u64 {
    let dir = scratch("instructions");
    let source = dir.join(format!("split{n}.ignite"));
    std::fs::write(&source, program(n)).unwrap();
    let output = Command::new("valgrind")
        .arg("--tool=callgrind")
        .arg(format!(
            "--callgrind-out-file={}",
            dir.join("callgrind.out").display()
        ))
        .arg(env!("CARGO_BIN_EXE_ignite"))
        .arg("check")
        .arg(&source)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("valgrind is needed to count and did not run: {e}"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "ignite check {}: {:?}\n{stderr}",
        source.display(),
        output.status
    );
    stderr
        .lines()
        .find_map(|line| {
            let digits: String = line
                .split_once("Collected :")?
                .1
                .trim_start()
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            digits.parse().ok()
        })
        .unwrap_or_else(|| panic!("no `Collected :` in valgrind's report:\n{stderr}"))
}

/// A field set in each branch of N `if`s with `else`: two more statements
/// cost what the doubling of the live set cost before the story, not its
/// square.
#[test]
fn fields_split_over_if_and_else_cost_no_more_than_the_doubling() {
    let small = instructions(SMALL);
    let large = instructions(LARGE);
    let ratio = large as f64 / small as f64;
    println!(
        "instructions {large} at {LARGE} statements against {small} at {SMALL}, ratio {ratio:.2}, bound {BOUND}"
    );
    assert!(
        ratio <= BOUND,
        "instructions {large} at {LARGE} statements against {small} at {SMALL}, ratio {ratio:.2}, bound {BOUND}"
    );
}
