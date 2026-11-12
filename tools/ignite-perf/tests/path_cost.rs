//! Row 3.10 of `docs/quality.md` (`story:path-cost-counted-at-every-merge`): the frozen
//! scenarios of `bench/path-cost/`, their recorded counts, and the refusal of a merge whose
//! count of a frozen path is above its value at the last release beyond the noise band.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ignite_perf::path_cost::{
    self, Counts, Measure, OpCounts, Reference, Shown, Unit, band, callgrind_profile,
    frozen_differences, judge, parse_counts, parse_scenario, read_counts_dir, read_scenarios,
    strace_profile, write_counts,
};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/path-cost")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn measure(runs: &[u64], items: &[(&str, u64)]) -> Measure {
    let mut sorted = runs.to_vec();
    sorted.sort_unstable();
    Measure {
        value: sorted[(sorted.len() - 1) / 2],
        runs: runs.to_vec(),
        items: items.iter().map(|(n, v)| (n.to_string(), *v)).collect(),
    }
}

fn counts(reference: Reference, op: &str, instructions: Measure, syscalls: Measure) -> Counts {
    Counts {
        scenario: "todo".to_string(),
        target: "rust".to_string(),
        reference,
        ops: vec![OpCounts {
            op: op.to_string(),
            instructions,
            syscalls,
        }],
    }
}

fn release() -> Reference {
    Reference::Release("v0.16.2".to_string())
}

/// The proof the refusal works: a pair of count files in which the PATCH path of the todo
/// grew, one function by 25,000 instructions and one new by 15,000 (an allocation added to
/// the reply path), and one system call per request (`fsync`). The merge is refused on both
/// cells of `patch`, and the refusal names each function and system call with both values,
/// largest growth first; the start, inside the band, is not refused.
#[test]
fn a_frozen_path_that_grew_is_refused_naming_the_functions_and_system_calls() {
    let then = parse_counts(&fixture("release.counts")).unwrap();
    let now = parse_counts(&fixture("now.counts")).unwrap();
    let verdict = judge(&[then], &[now]);
    let report = verdict.report();
    println!("{report}");
    assert_eq!(verdict.refused.len(), 2, "{report}");
    let instructions = &verdict.refused[0];
    assert!(
        instructions.starts_with(
            "3.10 llvm/todo/patch/instructions: 400000 at v0.16.2, 440000 now, above it by more than the noise band"
        ),
        "{instructions}"
    );
    let grew = instructions
        .find("grew ignite_io::fs::Directory::write_file: 60000 → 85000")
        .unwrap_or_else(|| panic!("the grown function is named: {instructions}"));
    let new = instructions
        .find("new alloc::raw_vec::RawVec<T,A>::grow_one: under 40 at v0.16.2 → 15000")
        .unwrap_or_else(|| panic!("the new function is named: {instructions}"));
    assert!(grew < new, "largest growth first: {instructions}");
    assert!(
        !instructions.contains("serve_connection") && !instructions.contains("TcpStream::read"),
        "an unchanged function is not listed: {instructions}"
    );
    let syscalls = &verdict.refused[1];
    assert!(
        syscalls.starts_with("3.10 llvm/todo/patch/syscalls: 160 at v0.16.2, 180 now"),
        "{syscalls}"
    );
    assert!(syscalls.contains("new fsync: 0 → 20"), "{syscalls}");
    assert!(!syscalls.contains("write"), "{syscalls}");
    assert!(
        !report.contains("llvm/todo/start/instructions:"),
        "{report}"
    );
    assert!(verdict.notes.is_empty(), "{:?}", verdict.notes);
    assert!(
        verdict
            .values
            .contains(&("llvm/todo/patch/instructions".to_string(), 440000)),
        "{:?}",
        verdict.values
    );
}

#[test]
fn a_count_file_reads_back_as_written() {
    for name in ["release.counts", "now.counts"] {
        let text = fixture(name);
        assert_eq!(write_counts(&parse_counts(&text).unwrap()), text, "{name}");
    }
    let first = Counts {
        reference: Reference::First("0123abcd".to_string()),
        ..parse_counts(&fixture("now.counts")).unwrap()
    };
    let text = write_counts(&first);
    assert!(text.contains("\nfirst 0123abcd\n"), "{text}");
    assert_eq!(parse_counts(&text).unwrap(), first);
    for broken in [
        "",
        "path-cost counts\nscenario todo\n",
        "path-cost counts\nscenario todo\ntarget rust\nrelease v1\nop start\ninstructions x runs 1\nsyscalls 1 runs 1\n",
        "path-cost counts\nscenario todo\ntarget rust\nrelease v1\nfn 3 main\n",
    ] {
        assert!(parse_counts(broken).is_err(), "{broken:?}");
    }
}

#[test]
fn a_cell_within_the_noise_band_passes_and_one_count_above_it_is_refused() {
    for (unit, then) in [(Unit::Instructions, 1_000_000u64), (Unit::Syscalls, 120)] {
        let slack = band(unit, then);
        for (now, refused) in [(then + slack, false), (then + slack + 1, true)] {
            let (i_then, s_then, i_now, s_now) = match unit {
                Unit::Instructions => (then, 5, now, 5),
                Unit::Syscalls => (1000, then, 1000, now),
            };
            let recorded = counts(
                release(),
                "get",
                measure(&[i_then; 5], &[]),
                measure(&[s_then; 5], &[]),
            );
            let measured = counts(
                release(),
                "get",
                measure(&[i_now], &[]),
                measure(&[s_now], &[]),
            );
            let verdict = judge(&[recorded], &[measured]);
            assert_eq!(
                !verdict.refused.is_empty(),
                refused,
                "{unit:?} {then} → {now}: {}",
                verdict.report()
            );
        }
    }
}

#[test]
fn the_refusal_lists_at_most_15_functions_largest_growth_first() {
    let names: Vec<String> = (1..=20).map(|n| format!("f{n:02}")).collect();
    let then_items: Vec<(&str, u64)> = names.iter().map(|n| (n.as_str(), 10_000)).collect();
    let now_items: Vec<(&str, u64)> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), 10_000 + 1_000 * (i as u64 + 1)))
        .collect();
    let recorded = counts(
        release(),
        "get",
        measure(&[1_000_000; 5], &then_items),
        measure(&[10; 5], &[]),
    );
    let measured = counts(
        release(),
        "get",
        measure(&[1_210_000], &now_items),
        measure(&[10], &[]),
    );
    let verdict = judge(&[recorded], &[measured]);
    assert_eq!(verdict.refused.len(), 1, "{}", verdict.report());
    let text = &verdict.refused[0];
    let listed: Vec<&str> = text
        .lines()
        .skip(1)
        .filter_map(|line| line.trim().strip_prefix("grew "))
        .collect();
    assert_eq!(listed.len(), 15, "{text}");
    assert!(listed[0].starts_with("f20: 10000 → 30000"), "{text}");
    assert!(listed[14].starts_with("f06: "), "{text}");
    assert!(text.contains("5 more"), "{text}");
}

#[test]
fn a_cell_spread_beyond_the_band_at_the_release_is_named_and_not_judged() {
    let recorded = counts(
        release(),
        "connection",
        measure(&[100_000, 100_000, 100_000, 100_000, 130_000], &[]),
        measure(&[20, 20, 20, 20, 20], &[]),
    );
    let measured = counts(
        release(),
        "connection",
        measure(&[200_000], &[]),
        measure(&[20], &[]),
    );
    let verdict = judge(&[recorded], &[measured]);
    assert!(verdict.refused.is_empty(), "{}", verdict.report());
    let note = verdict.notes.join("\n");
    assert!(
        note.contains("rust/todo/connection/instructions: not judged")
            && note.contains("100000 100000 100000 100000 130000"),
        "{note}"
    );
}

#[test]
fn a_scenario_added_after_the_freeze_is_recorded_and_compared_from_the_next_release() {
    let recorded = counts(
        Reference::First("0123abcd".to_string()),
        "get",
        measure(&[1_000_000], &[]),
        measure(&[10], &[]),
    );
    let measured = counts(
        release(),
        "get",
        measure(&[5_000_000], &[]),
        measure(&[50], &[]),
    );
    let verdict = judge(&[recorded], &[measured]);
    assert!(verdict.refused.is_empty(), "{}", verdict.report());
    let note = verdict.notes.join("\n");
    assert!(
        note.contains("rust/todo: first value, recorded at 0123abcd")
            && note.contains("compared from the next release on"),
        "{note}"
    );
}

#[test]
fn a_cell_without_a_recorded_value_is_refused() {
    let measured = counts(release(), "get", measure(&[1], &[]), measure(&[1], &[]));
    let verdict = judge(&[], std::slice::from_ref(&measured));
    assert_eq!(verdict.refused.len(), 1);
    assert!(
        verdict.refused[0].contains("rust/todo: no recorded counts"),
        "{}",
        verdict.report()
    );
    let other = counts(release(), "put", measure(&[1], &[]), measure(&[1], &[]));
    let verdict = judge(&[other], &[measured]);
    assert!(
        verdict
            .refused
            .iter()
            .any(|r| r.contains("rust/todo/get: no value at v0.16.2"))
            && verdict
                .refused
                .iter()
                .any(|r| r.contains("rust/todo/put: recorded at v0.16.2 and not measured")),
        "{}",
        verdict.report()
    );
}

#[test]
fn callgrind_self_cost_leaves_out_the_cost_of_calls() {
    let text = "# callgrind format\nversion: 1\ncreator: callgrind-3.25.1\npart: 2\n\npositions: line\nevents: Ir\nsummary: 1000\n\n\
ob=/usr/lib/libc.so.6\nfl=???\nfn=main\n0 100\ncob=/srv/todod\ncfi=???\ncfn=work::h0123456789abcdef\ncalls=1 0 \n0 800\n0 50\n\n\
ob=/srv/todod\nfl=src/x.rs\nfn=work::h0123456789abcdef\n0 790\nfi=src/y.rs\n0 10\n\nfn=0x00000000000017e0\n0 50\n\ntotals: 1000\n";
    let profile = callgrind_profile(text).unwrap();
    assert_eq!(profile.total, 1000);
    let expected: BTreeMap<String, u64> = [
        ("main".to_string(), 150),
        ("work".to_string(), 800),
        ("??? todod".to_string(), 50),
    ]
    .into_iter()
    .collect();
    assert_eq!(profile.items, expected);
    assert!(
        callgrind_profile("events: Ir\nfn=main\n0 5\n").is_err(),
        "no totals line"
    );
}

#[test]
fn a_system_call_counts_once_at_the_line_that_begins_it() {
    let traced = "101 execve(\"/srv/todod\", [\"todod\"], 0x7ffd /* 1 var */) = 0\n\
101 brk(NULL)                         = 0x55d\n\
101 clone3({flags=CLONE_VM|CLONE_THREAD, ...} <unfinished ...>\n\
102 futex(0x7f, FUTEX_WAIT_PRIVATE, 2, NULL <unfinished ...>\n\
101 <... clone3 resumed> => {parent_tid=[102]}, 88) = 102\n\
102 <... futex resumed>)              = 0\n\
102 --- SIGPIPE {si_signo=SIGPIPE} ---\n\
102 +++ exited with 0 +++\n\
101 write(2, \"listening on 127.0.0.1:4\"..., 30) = 30\n\
101 epoll_wait(3, ";
    let profile = strace_profile(traced);
    assert_eq!(profile.total, 5, "{:?}", profile.items);
    assert_eq!(profile.items.get("futex"), Some(&1));
    assert_eq!(profile.items.get("clone3"), Some(&1));
    assert_eq!(
        profile.items.get("epoll_wait"),
        None,
        "a line still being written"
    );
    let untraced =
        "openat(AT_FDCWD, \"x\", O_RDONLY) = 3\nread(3, \"\", 4096) = 0\n+++ exited with 0 +++\n";
    let profile = strace_profile(untraced);
    assert_eq!(profile.total, 2, "{:?}", profile.items);
}

const SCRIPT: &str = "frozen v0.16.2\nfile todo.ignite examples/todo/todo.ignite\nserver todo.ignite todod\ntargets llvm rust\nwarm 1\nrepeat 2\nop get 200 GET /todos/1\n";

fn tags(
    files: &'static [(&'static str, &'static str, &'static str)],
) -> impl Fn(&str, &str) -> Result<Shown, String> {
    move |tag: &str, path: &str| {
        if !files.iter().any(|(t, _, _)| *t == tag) {
            return Ok(Shown::NoTag);
        }
        Ok(files
            .iter()
            .find(|(t, p, _)| *t == tag && *p == path)
            .map_or(Shown::NoFile, |(_, _, text)| {
                Shown::File(text.as_bytes().to_vec())
            }))
    }
}

/// A frozen scenario is never edited: each of its program files is the file of the tag it
/// names, and once a release holds the scenario every file of its directory is that
/// release's. A tag that is not in the repository is said so, plainly, and fails.
#[test]
fn a_frozen_file_that_differs_from_its_tag_fails_and_a_missing_tag_is_named() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("path-cost-frozen-{}", std::process::id()));
    let root = dir.join("bench/path-cost");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(root.join("todo")).unwrap();
    std::fs::write(root.join("todo/scenario"), SCRIPT).unwrap();
    std::fs::write(root.join("todo/todo.ignite"), "package examples.todo\n").unwrap();
    let scenarios = read_scenarios(&root).unwrap();

    let same = tags(&[(
        "v0.16.2",
        "examples/todo/todo.ignite",
        "package examples.todo\n",
    )]);
    let (errors, notes) = frozen_differences(&root, &scenarios, Some("v0.16.2"), &same);
    assert!(errors.is_empty(), "{errors:?}");
    assert!(
        notes.iter().any(|n| n.contains("in no release yet")),
        "{notes:?}"
    );

    let edited = tags(&[(
        "v0.16.2",
        "examples/todo/todo.ignite",
        "package examples.todo\n// before\n",
    )]);
    let (errors, _) = frozen_differences(&root, &scenarios, Some("v0.16.2"), &edited);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].contains(
            "bench/path-cost/todo/todo.ignite differs from examples/todo/todo.ignite at v0.16.2"
        ),
        "{errors:?}"
    );

    let (errors, _) = frozen_differences(&root, &scenarios, Some("v0.16.1"), &tags(&[]));
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].contains("tag v0.16.2 is not in this repository")
            && errors[0].contains("git fetch --tags"),
        "{errors:?}"
    );

    let released = tags(&[
        ("v0.17.0", "bench/path-cost/todo/scenario", SCRIPT),
        (
            "v0.17.0",
            "bench/path-cost/todo/todo.ignite",
            "package examples.todo\n",
        ),
    ]);
    let (errors, notes) = frozen_differences(&root, &scenarios, Some("v0.17.0"), &released);
    assert!(errors.is_empty(), "{errors:?} {notes:?}");
    std::fs::write(
        root.join("todo/scenario"),
        SCRIPT.replace("repeat 2", "repeat 3"),
    )
    .unwrap();
    let scenarios = read_scenarios(&root).unwrap();
    let (errors, _) = frozen_differences(&root, &scenarios, Some("v0.17.0"), &released);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("bench/path-cost/todo/scenario differs from its file at v0.17.0")),
        "{errors:?}"
    );
    std::fs::write(
        root.join("todo/extra.ignite"),
        "package examples.todo.extra\n",
    )
    .unwrap();
    let (errors, _) = frozen_differences(&root, &scenarios, Some("v0.16.2"), &same);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("bench/path-cost/todo/extra.ignite")),
        "a file the scenario does not list: {errors:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_scenario_script_is_read_and_a_wrong_one_refused() {
    let scenario = parse_scenario("todo", SCRIPT).unwrap();
    assert_eq!(scenario.frozen.as_deref(), Some("v0.16.2"));
    assert_eq!(
        scenario.cells(),
        vec![
            ("llvm".to_string(), "start".to_string()),
            ("llvm".to_string(), "get".to_string()),
            ("rust".to_string(), "start".to_string()),
            ("rust".to_string(), "get".to_string()),
        ]
    );
    for broken in [
        "server todo.ignite todod\ntargets llvm\nwarm 1\nrepeat 2\nop get 200 GET\n",
        "server todo.ignite todod\ntargets wasm\nwarm 1\nrepeat 2\nop get 200 GET /\n",
        "server todo.ignite todod\ntargets llvm\nwarm 1\nrepeat 0\nop get 200 GET /\n",
        "server todo.ignite todod\ntargets llvm\nwarm 1\nrepeat 1\nop start 200 GET /\n",
        "server todo.ignite todod\ntargets llvm\nwarm 1\nrepeat 1\nop get 200 GET /\nop get 200 GET /\n",
        "op check all somewhere check x.ignite\n",
        "op check all driver\n",
        "frozen v0.16.2\nfile todo.ignite\nop check all driver check todo.ignite\n",
        "colour blue\n",
    ] {
        assert!(parse_scenario("x", broken).is_err(), "{broken:?}");
    }
}

/// The scenarios of the story's first set, frozen at v0.16.2: the start of `todod` and
/// `terminald`, one GET, PATCH and DELETE of the todo, one connection opened and closed,
/// one `terminald` session, on both targets; `ignite check`, `ignite plan` and a dev build
/// of the design file.
#[test]
fn the_first_set_of_scenarios_is_frozen_at_v0_16_2() {
    let scenarios = read_scenarios(&repo().join(path_cost::SCENARIOS)).unwrap();
    let cells: Vec<String> = scenarios
        .iter()
        .flat_map(|s| {
            s.cells()
                .into_iter()
                .map(move |(target, op)| format!("{target}/{}/{op}", s.name))
        })
        .collect();
    for target in ["llvm", "rust"] {
        for op in ["start", "get", "patch", "delete", "connection"] {
            assert!(cells.contains(&format!("{target}/todo/{op}")), "{cells:?}");
        }
        for op in ["start", "session"] {
            assert!(
                cells.contains(&format!("{target}/terminal/{op}")),
                "{cells:?}"
            );
        }
        assert!(
            cells.contains(&format!("{target}/design/build")),
            "{cells:?}"
        );
    }
    assert!(cells.contains(&"all/design/check".to_string()), "{cells:?}");
    assert!(cells.contains(&"all/design/plan".to_string()), "{cells:?}");
    for name in ["todo", "terminal", "design"] {
        let scenario = scenarios.iter().find(|s| s.name == name).unwrap();
        assert_eq!(scenario.frozen.as_deref(), Some("v0.16.2"), "{name}");
    }
}

/// Every cell of every scenario has its recorded value, and every count file is a
/// scenario's: a count without a scenario is a scenario edited or removed.
#[test]
fn every_cell_of_the_scenarios_has_its_recorded_counts() {
    let root = repo().join(path_cost::SCENARIOS);
    let scenarios = read_scenarios(&root).unwrap();
    let recorded = read_counts_dir(&repo().join(path_cost::COUNTS)).unwrap();
    let mut errors = Vec::new();
    for scenario in &scenarios {
        for (target, op) in scenario.cells() {
            let found = recorded
                .iter()
                .find(|c| c.scenario == scenario.name && c.target == target)
                .is_some_and(|c| c.ops.iter().any(|o| o.op == op));
            if !found {
                errors.push(format!(
                    "{target}/{}/{op}: no recorded counts",
                    scenario.name
                ));
            }
        }
    }
    for counts in &recorded {
        let Some(scenario) = scenarios.iter().find(|s| s.name == counts.scenario) else {
            errors.push(format!("{}: counts of no scenario", counts.scenario));
            continue;
        };
        for op in &counts.ops {
            if !scenario
                .cells()
                .contains(&(counts.target.clone(), op.op.clone()))
            {
                errors.push(format!(
                    "{}/{}/{}: counts of no cell",
                    counts.target, counts.scenario, op.op
                ));
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

/// The scenarios of this repository against their tags (`git`), as `ignite-perf path-cost`
/// checks them before it measures.
#[test]
fn the_scenarios_of_the_repository_are_the_files_of_their_tags() {
    let root = repo().join(path_cost::SCENARIOS);
    let scenarios = read_scenarios(&root).unwrap();
    let (errors, notes) = path_cost::frozen_in_repository(&repo(), &scenarios);
    println!("{}", notes.join("\n"));
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
