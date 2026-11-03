use ignite_perf::json::Json;
use ignite_perf::perf::{
    LogCounts, Measured, SummaryInputs, command_row, line_count, log_counts, progress_line,
    summarize_rows,
};

use crate::common;

fn text(row: &Json, key: &str) -> String {
    row.field(key).as_str().unwrap_or_default().to_string()
}

/// Rebuild a command record from its recorded inputs and retained log, as the harness writes it.
fn rebuild(row: &Json) -> Json {
    let log = common::read(&format!("{}/{}", common::RELEASE, text(row, "log")));
    let command: Vec<String> = row
        .field("command")
        .as_arr()
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();
    let mut rebuilt = command_row(&Measured {
        label: &text(row, "label"),
        commit: &text(row, "commit"),
        unit: &text(row, "unit"),
        pair: &text(row, "pair"),
        phase: &text(row, "phase"),
        recache: text(row, "cache_policy") == "recache",
        command: &command,
        seconds: row.field("seconds").as_f64().unwrap(),
        exit: row.field("exit").as_i64().unwrap(),
        load_before: row.field("load_before").as_f64().unwrap(),
        load_after: row.field("load_after").as_f64().unwrap(),
        log: &text(row, "log"),
        text: &log,
    });
    rebuilt.set("capture", row.field("capture").clone());
    rebuilt.set("harness_sha256", row.field("harness_sha256").clone());
    rebuilt
}

#[test]
fn command_records_are_rebuilt_byte_for_byte_from_their_logs() {
    let rows = common::records(&format!("{}/records.jsonl", common::RELEASE));
    let lines: Vec<String> = common::read(&format!("{}/records.jsonl", common::RELEASE))
        .lines()
        .map(String::from)
        .collect();
    let mut checked = (0, 0);
    for (row, line) in rows.iter().zip(&lines) {
        if row.get("log").is_none() {
            continue;
        }
        let mut rebuilt = rebuild(row);
        match row.get("executables") {
            None => {
                assert_eq!(rebuilt.dumps(true), *line);
                checked.0 += 1;
            }
            Some(recorded) => {
                let mut executables = rebuilt.field("executables").clone();
                assert_eq!(
                    executables.field("count"),
                    recorded.field("count"),
                    "{line}"
                );
                executables.set("bytes", recorded.field("bytes").clone());
                rebuilt.set("executables", executables);
                assert_eq!(rebuilt.dumps(true), *line);
                checked.1 += 1;
            }
        }
    }
    assert!(checked.0 > 150 && checked.1 > 10, "{checked:?}");
}

#[test]
fn log_patterns_count_only_passing_results_and_executable_paths() {
    let log = "\
running 3 tests
test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 148 passed; 0 failed; 0 filtered out; finished in 0.14s
test result: ok. 12 passed; 2 failed; 3 ignored
  Executable unittests src/lib.rs (target/debug/deps/a-1)
  Executable tests/main.rs (/abs (x)/deps/main-2)
  Executable nothing here
";
    assert_eq!(
        log_counts(log),
        LogCounts {
            passed: 15,
            failed: 2,
            ignored: 4,
            inline: vec![148],
            executables: vec!["target/debug/deps/a-1".into(), "x)/deps/main-2".into()],
        }
    );
}

#[test]
fn progress_lines_keep_the_python_key_order() {
    let row = Json::parse(
        r#"{"exit": 0, "label": "baseline", "load_before": 7.19482421875, "pair": "smoke", "seconds": 0.5, "unit": "smoke-compiler", "x": 1}"#,
    )
    .unwrap();
    assert_eq!(
        progress_line(&row),
        r#"{"label": "baseline", "unit": "smoke-compiler", "pair": "smoke", "seconds": 0.5, "exit": 0, "load_before": 7.19482421875}"#
    );
}

#[test]
fn program_sizes_count_lines_as_splitlines() {
    assert_eq!(line_count(""), 0);
    assert_eq!(line_count("a"), 1);
    assert_eq!(line_count("a\n"), 1);
    assert_eq!(line_count("a\nb"), 2);
    assert_eq!(line_count("a\r\nb\rc\u{2028}d\n\n"), 5);
}

#[test]
fn the_recorded_release_summary_is_reproduced_from_its_records() {
    let rows = common::records(&format!("{}/records.jsonl", common::RELEASE));
    let run = common::json(&format!("{}/run.json", common::RELEASE));
    let recorded = common::json(&format!("{}/summary.json", common::RELEASE));
    let controls = run.field("controls");
    let (summary, markdown) = summarize_rows(
        &rows,
        &SummaryInputs {
            commits: run.field("commits").clone(),
            has_comparison: run.field("refs").get("comparison").is_some(),
            controls,
            warm_pairs: controls.field("warm_pairs").as_i64().unwrap(),
            check_pairs: controls.field("check_pairs").as_i64().unwrap(),
            absolute_max_load: controls.field("absolute_max_load").as_f64().unwrap(),
            load_delta: run.field("load_delta").as_f64().unwrap(),
            frozen_baseline_matches: recorded.field("frozen_baseline_matches").truthy(),
        },
    );
    assert_eq!(
        summary.dumps_indent(true, 2) + "\n",
        common::read(&format!("{}/summary.json", common::RELEASE))
    );
    assert_eq!(
        markdown,
        common::read(&format!("{}/summary.md", common::RELEASE))
    );
}
