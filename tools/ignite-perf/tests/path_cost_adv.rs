//! Adversary cases for row 3.10 (`ignite-perf path-cost`, `docs/quality.md` "The path cost at
//! every merge"): costs that pass, a frozen check that misses an edit, and inputs the readers
//! accept that they should refuse.

use std::path::{Path, PathBuf};
use std::process::Command;

use ignite_perf::path_cost::{
    self, Counts, Measure, PathCostArgs, Reference, Shown, frozen_differences,
    frozen_in_repository, judge, judge_measuring_again, parse_counts, parse_scenario,
    read_counts_dir, read_scenarios,
};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("path-cost-adv-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The recorded counts of `scenario` on `target` in this repository.
fn recorded(scenario: &str, target: &str) -> Counts {
    read_counts_dir(&repo().join(path_cost::COUNTS))
        .unwrap()
        .into_iter()
        .find(|c| c.scenario == scenario && c.target == target)
        .unwrap_or_else(|| panic!("no counts of {scenario}-{target}"))
}

/// What one merge's run measures when nothing changed: each recorded value, once.
fn unchanged(counts: &Counts) -> Counts {
    let mut now = counts.clone();
    now.reference = Reference::Release("now".to_string());
    for op in &mut now.ops {
        for m in [&mut op.instructions, &mut op.syscalls] {
            m.runs = vec![m.value];
        }
    }
    now
}

fn op_mut<'a>(counts: &'a mut Counts, op: &str) -> &'a mut path_cost::OpCounts {
    counts.ops.iter_mut().find(|o| o.op == op).unwrap()
}

fn grow(m: &mut Measure, item: &str, by: u64) {
    m.value += by;
    m.runs = vec![m.value];
    *m.items.entry(item.to_string()).or_default() += by;
}

/// One more heap allocation per PATCH request on the Rust target: in the PATCH dump of the
/// recorded run, `malloc` costs 58.3 and `free` 64.9 instructions per call (1,780 calls each,
/// 103,709 and 115,512 inclusive), so 20 requests add 1,166 to `malloc` and 1,298 to `free`.
/// The noise band of the cell is 8,005 (1 % of 800,543) while its five recorded runs spread
/// 195; the cell passes. The implementor's own proof fixture names the same change ("an
/// allocation added to the reply path") and gives it 40,000 instructions.
#[test]
fn adv_one_extra_allocation_per_patch_request_is_refused() {
    let then = recorded("todo", "rust");
    let mut now = unchanged(&then);
    let patch = op_mut(&mut now, "patch");
    grow(&mut patch.instructions, "malloc", 1_166);
    grow(&mut patch.instructions, "free", 1_298);
    let verdict = judge(&[then], &[now]);
    assert!(
        verdict
            .refused
            .iter()
            .any(|r| r.contains("rust/todo/patch/instructions") && r.contains("malloc")),
        "one allocation per request on the PATCH path (2,464 instructions per batch) passed:\n{}",
        verdict.report()
    );
}

/// A cell left out of the judgement because its runs at the release spread beyond the band
/// (`llvm/todo/connection`) is not bounded at all: the connection path costing half as much
/// again passes. A bound such as the highest recorded run plus the band would refuse it, and
/// would also refuse the same rise after one run of a count file is edited by hand to push a
/// judged cell over its band.
#[test]
fn adv_a_not_judged_cell_that_rises_by_half_is_refused() {
    let then = recorded("todo", "llvm");
    let mut now = unchanged(&then);
    let connection = op_mut(&mut now, "connection");
    let (i, s) = (connection.instructions.value, connection.syscalls.value);
    grow(
        &mut connection.instructions,
        "ignite_io::net::accept",
        i / 2,
    );
    grow(&mut connection.syscalls, "accept4", s / 2);
    let verdict = judge(&[then], &[now]);
    assert!(
        verdict
            .refused
            .iter()
            .any(|r| r.contains("llvm/todo/connection/instructions")),
        "the connection path at 150 % passed:\n{}",
        verdict.report()
    );
}

/// The same number of system calls, one of them replaced by a costlier one: a `write` per
/// PATCH request becomes an `fsync`. The total is unchanged, the user-space instructions too,
/// and a call the release never made on this path passes unnamed.
#[test]
fn adv_a_system_call_replaced_by_a_costlier_one_is_refused() {
    let then = recorded("todo", "rust");
    let mut now = unchanged(&then);
    let patch = op_mut(&mut now, "patch");
    let writes = patch.syscalls.items.remove("write").unwrap();
    patch.syscalls.items.insert("fsync".to_string(), writes);
    let verdict = judge(&[then], &[now]);
    assert!(
        verdict.refused.iter().any(|r| r.contains("fsync")),
        "20 fsync in place of 20 write passed:\n{}",
        verdict.report()
    );
}

/// The frozen check reads only the `file` lines of a scenario no release holds yet; the script
/// itself has no reference. A merge that edits `repeat 20` to `repeat 19` in `todo/scenario`
/// (every batch 5 % shorter, so 5 % of cost added per request hides) passes the check. Built
/// here as a repository with the tag, the freeze commit and the edit.
#[test]
fn adv_an_edited_scenario_script_fails_the_frozen_check() {
    let dir = scratch("script");
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args([
                "-c",
                "user.name=adv",
                "-c",
                "user.email=adv@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "tag.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(status.status.success(), "git {args:?}: {status:?}");
    };
    let write = |path: &str, text: &str| {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    let program = "package examples.todo\n";
    let script = "frozen v0.16.2\nfile todo.ignite examples/todo/todo.ignite\nserver todo.ignite todod\ntargets llvm\nwarm 10\nrepeat 20\nop get 200 GET /todos/1\n";
    git(&["init", "-q"]);
    write("examples/todo/todo.ignite", program);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "release"]);
    git(&["tag", "v0.16.2"]);
    write("bench/path-cost/todo/scenario", script);
    write("bench/path-cost/todo/todo.ignite", program);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "freeze the scenario"]);
    write(
        "bench/path-cost/todo/scenario",
        &script.replace("repeat 20", "repeat 19"),
    );
    git(&["commit", "-q", "-am", "a merge"]);

    let scenarios = read_scenarios(&dir.join(path_cost::SCENARIOS)).unwrap();
    let (errors, notes) = frozen_in_repository(&dir, &scenarios);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("bench/path-cost/todo/scenario")),
        "the script of a frozen scenario edited (repeat 20 → 19) passed the frozen check: errors {errors:?}, notes {notes:?}"
    );
}

/// A scenario whose script has no `frozen` line and whose `file` lines name paths of a
/// release: dropping the one line `frozen v0.16.2` from a scenario turns its frozen check off,
/// while its count file still says `release v0.16.2` and is judged.
#[test]
fn adv_a_scenario_without_frozen_line_that_names_tag_paths_is_checked() {
    let script = "file todo.ignite examples/todo/todo.ignite\nserver todo.ignite todod\ntargets llvm\nrepeat 2\nop get 200 GET /todos/1\n";
    let Ok(scenario) = parse_scenario("todo", script) else {
        return;
    };
    let dir = scratch("unfrozen");
    let root = dir.join("bench/path-cost");
    std::fs::create_dir_all(root.join("todo")).unwrap();
    std::fs::write(root.join("todo/scenario"), script).unwrap();
    std::fs::write(
        root.join("todo/todo.ignite"),
        "package examples.todo\n// edited\n",
    )
    .unwrap();
    let show = |tag: &str, path: &str| -> Result<Shown, String> {
        Ok(match (tag, path) {
            ("v0.16.2", "examples/todo/todo.ignite") => {
                Shown::File(b"package examples.todo\n".to_vec())
            }
            ("v0.16.2", _) => Shown::NoFile,
            _ => Shown::NoTag,
        })
    };
    let (errors, notes) = frozen_differences(&root, &[scenario], Some("v0.16.2"), &show);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        !errors.is_empty(),
        "a scenario that names a release's paths without `frozen` and differs from them passed: notes {notes:?}"
    );
}

/// `--record first` is for a scenario added after the freeze (`docs/quality.md`); on a frozen
/// scenario it turns the scenario's cells into "first value, compared from the next release
/// on", so nothing of it is judged until the next release. It must be refused before anything
/// is measured. (The compiler given does not exist, so a run that is not refused stops there.)
#[test]
fn adv_record_first_of_a_frozen_scenario_is_refused() {
    let dir = scratch("record-first");
    let args = PathCostArgs {
        repo: repo(),
        cli: Some(dir.join("no-ignite")),
        work: dir.join("work"),
        out: None,
        record: Some("first".to_string()),
        runs: 5,
        scenario: vec!["todo".to_string()],
    };
    let result = path_cost::main(&args);
    let _ = std::fs::remove_dir_all(&dir);
    match result {
        Err(e) if e.contains("frozen") => {}
        other => {
            panic!("--record first --scenario todo (frozen at v0.16.2) was not refused: {other:?}")
        }
    }
}

const COUNTS: &str = "path-cost counts\nscenario todo\ntarget rust\nrelease v0.16.2\nop patch\ninstructions 800543 runs 800543 800545 800543 800543 800738\nsyscalls 300 runs 300 300 300 300 300\nfn 71740 free\nsyscall 20 write\n";

/// A count file is written as "per cell the median of five runs, each run's value"; one whose
/// value is not the median of its runs is a hand edit (here: the bar of the PATCH cell raised
/// by 100,000 with its runs left alone) and is read without a word.
#[test]
fn adv_a_count_whose_value_is_not_the_median_of_its_runs_is_refused() {
    let edited = COUNTS.replace("instructions 800543 runs", "instructions 900543 runs");
    assert!(
        parse_counts(&edited).is_err(),
        "a value 100,000 above the median of its runs was read: {:?}",
        parse_counts(&edited).map(|c| c.ops[0].instructions.value)
    );
}

/// A duplicate operation or a duplicate cell line in a count file: the judge reads the first
/// operation, `quality::the_path_cost_bars_are_the_recorded_counts` the last.
#[test]
fn adv_a_duplicate_operation_or_cell_in_a_count_file_is_refused() {
    let twice_op = format!(
        "{COUNTS}op patch\ninstructions 999999 runs 999999 999999 999999 999999 999999\nsyscalls 300 runs 300 300 300 300 300\n"
    );
    let twice_cell = COUNTS.replace(
        "syscalls 300 runs",
        "instructions 999999 runs 999999 999999 999999 999999 999999\nsyscalls 300 runs",
    );
    let read: Vec<&str> = [("operation", &twice_op), ("cell", &twice_cell)]
        .into_iter()
        .filter(|(_, text)| parse_counts(text).is_ok())
        .map(|(what, _)| what)
        .collect();
    assert!(read.is_empty(), "read with a duplicate: {read:?}");
}

/// A program file named with `..` or as an absolute path: `stage` copies it from
/// `<scenario dir>/<name>` to `<work>/src/<scenario>/<name>`, and an absolute name replaces
/// both (`Path::join`), so the copy is a file onto itself, which `fs::copy` truncates.
#[test]
fn adv_a_file_name_that_leaves_the_scenario_directory_is_refused() {
    let read: Vec<&str> = ["../../x.ignite", "/etc/hostname", "sub/x.ignite"]
        .into_iter()
        .filter(|name| {
            parse_scenario(
                "x",
                &format!("file {name}\nop check all driver check x.ignite\n"),
            )
            .is_ok()
        })
        .collect();
    assert!(read.is_empty(), "file names read: {read:?}");
}

/// Values `task path-cost` measured on the unchanged tree of 45a093f0 (five passes one after
/// another, load average 18 to 66): `llvm/terminal/session/syscalls` 167 164 161 165 164
/// (recorded 164, band 1) and `rust/todo/connection/instructions` 213697 208798 203217 202648
/// 206783 (recorded 208357, band 2,083). Both cells are judged; the first pass was refused on
/// both, with no change to any file. A merge that adds no cost passes every time.
///
/// Rewritten in the fix pass for its mechanism: a cell above its band is measured again and
/// refused only when every measurement is above, and the recording changed (each request now
/// ends with the server settled). Each value of those passes, as its distance from the value
/// recorded then (+3 calls; +5,340 instructions), is the first measurement of a pass whose
/// measurements after it are at the recorded value: no pass is refused.
#[test]
fn adv_the_values_of_an_unchanged_tree_are_not_refused() {
    let mut refused = Vec::new();
    for (scenario, target, op, unit, deltas) in [
        (
            "terminal",
            "llvm",
            "session",
            "syscalls",
            &[3i64, 0, -3, 1, 0][..],
        ),
        (
            "todo",
            "rust",
            "connection",
            "instructions",
            &[5_340, 441, -5_140, -5_709, -1_574][..],
        ),
    ] {
        let then = recorded(scenario, target);
        for &delta in deltas {
            let mut now = unchanged(&then);
            let cell = op_mut(&mut now, op);
            let m = if unit == "syscalls" {
                &mut cell.syscalls
            } else {
                &mut cell.instructions
            };
            m.value = m.value.saturating_add_signed(delta);
            m.runs = vec![m.value];
            let verdict =
                judge_measuring_again(std::slice::from_ref(&then), vec![now], &mut |_, _| {
                    Ok(unchanged(&then))
                })
                .unwrap();
            refused.extend(verdict.refused);
        }
    }
    assert!(
        refused.is_empty(),
        "an unchanged tree refused:\n{}",
        refused.concat()
    );
}

/// The floor of the system-call band (at least one call) is what keeps a cell under 100 calls
/// from being refused at one call more: `rust/todo/get/syscalls` is 39 at v0.16.2, 1 % of it
/// is 0. No case of the suite has a cell under 100 calls, so removing `.max(SYSCALL_BAND_LEAST)`
/// from `band` leaves every case green; this one is green on the code and red on that mutant.
#[test]
fn adv_one_call_more_on_a_cell_under_100_calls_is_within_the_floor() {
    let then = recorded("todo", "rust");
    let mut one = unchanged(&then);
    grow(&mut op_mut(&mut one, "get").syscalls, "write", 1);
    let verdict = judge(std::slice::from_ref(&then), &[one]);
    assert!(
        verdict.refused.is_empty(),
        "39 → 40 calls refused:\n{}",
        verdict.report()
    );
    let mut two = unchanged(&then);
    grow(&mut op_mut(&mut two, "get").syscalls, "write", 2);
    let verdict = judge(&[then], &[two]);
    assert!(
        verdict
            .refused
            .iter()
            .any(|r| r.contains("rust/todo/get/syscalls")),
        "39 → 41 calls passed:\n{}",
        verdict.report()
    );
}

/// A recorded count near `u64::MAX` is read, and the judge then overflows computing the band
/// (`value * 10`) in a debug build, which `task path-cost` runs.
#[test]
fn adv_a_huge_recorded_count_is_refused_without_a_panic() {
    let huge = COUNTS.replace(
        "instructions 800543 runs 800543 800545 800543 800543 800738",
        "instructions 18446744073709551615 runs 18446744073709551615",
    );
    let Ok(then) = parse_counts(&huge) else {
        return;
    };
    let now = unchanged(&then);
    let judged = std::panic::catch_unwind(|| judge(std::slice::from_ref(&then), &[now]));
    assert!(judged.is_ok(), "the judge panicked on a count it read");
}
