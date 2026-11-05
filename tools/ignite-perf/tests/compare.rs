use ignite_perf::json::{Json, obj};
use ignite_perf::perf::{
    closure_eligible, compare_rows, failed_candidate, self_check, within_budget,
};

use crate::common;

fn row(label: &str, pair: &str, seconds: f64, load: f64, exit: i64) -> Json {
    obj(vec![
        ("unit", "warm-workspace".into()),
        ("label", label.into()),
        ("pair", pair.into()),
        ("seconds", seconds.into()),
        ("load_before", load.into()),
        ("exit", exit.into()),
    ])
}

#[test]
fn the_ratio_is_the_median_of_pair_ratios_over_kept_pairs() {
    let rows = vec![
        row("baseline", "1", 10.0, 1.0, 0),
        row("candidate", "1", 11.0, 1.0, 0),
        row("baseline", "2", 100.0, 3.0, 0),
        row("candidate", "2", 50.0, 2.5, 0),
        row("baseline", "3", 10.0, 1.0, 0),
        row("candidate", "3", 100.0, 20.0, 0),
        row("baseline", "4", 10.0, 1.0, 0),
        row("candidate", "4", 1.0, 1.0, 1),
    ];
    let value = compare_rows(&rows, "baseline", "candidate", 5.0).remove(0);
    assert_eq!(
        value.dumps(true),
        r#"{"baseline": "baseline", "before": 55.0, "candidate": 30.5, "excluded": [{"exits": [0, 0], "loads": [1.0, 20.0], "pair": "3", "reason": "load_mismatch"}, {"exits": [0, 1], "loads": [1.0, 1.0], "pair": "4", "reason": "command_failed"}], "loads": [1.0, 3.0], "ratio": 0.8, "unit": "warm-workspace", "valid_pairs": 2}"#
    );
}

#[test]
fn a_load_delta_of_exactly_the_limit_is_kept() {
    let rows = vec![
        row("baseline", "1", 1.0, 2.0, 0),
        row("candidate", "1", 1.0, 7.0, 0),
        row("baseline", "2", 1.0, 2.0, 0),
        row("candidate", "2", 1.0, 7.01, 0),
    ];
    let value = compare_rows(&rows, "baseline", "candidate", 5.0).remove(0);
    assert_eq!(value.field("valid_pairs"), &Json::Int(1));
    assert_eq!(
        value.field("excluded").item(0).field("reason").as_str(),
        Some("load_mismatch")
    );
}

#[test]
fn a_unit_without_kept_pairs_has_no_ratio() {
    let rows = vec![
        row("baseline", "1", 1.0, 1.0, 2),
        row("candidate", "1", 1.0, 1.0, 0),
    ];
    let value = compare_rows(&rows, "baseline", "candidate", 5.0).remove(0);
    assert_eq!(value.get("ratio"), None);
    assert_eq!(value.field("valid_pairs"), &Json::Int(0));
}

#[test]
fn the_budget_is_ten_percent_and_rustgen_must_recover_fully() {
    assert!(within_budget("warm-workspace", 1.10));
    assert!(!within_budget("warm-workspace", 1.1000001));
    assert!(within_budget("file-rustgen", 1.0));
    assert!(!within_budget("file-rustgen", 1.01));
}

#[test]
fn loosened_controls_are_never_eligible_for_closure() {
    let strict = common::json(&format!("{}/run.json", common::RELEASE));
    let controls = strict.field("controls");
    assert!(closure_eligible(controls, true, true));
    assert!(!closure_eligible(controls, false, true));
    assert!(!closure_eligible(controls, true, false));
    let mut loose = controls.clone();
    loose.set("load_delta", Json::Float(5.5));
    assert!(!closure_eligible(&loose, true, true));
}

#[test]
fn a_candidate_failure_blocks_even_after_green_samples() {
    assert!(failed_candidate(&[
        row("candidate", "1", 1.0, 1.0, 1),
        row("candidate", "2", 1.0, 1.0, 0)
    ]));
    assert!(!failed_candidate(&[
        row("baseline", "1", 1.0, 1.0, 1),
        row("candidate", "2", 1.0, 1.0, 0)
    ]));
}

#[test]
fn self_check_passes() {
    let scratch = common::scratch("self-check");
    self_check(&scratch).unwrap();
    assert_eq!(std::fs::read_dir(&scratch).unwrap().count(), 0);
}

/// `ignite-perf self-check` covers the apply cycle of `docs/slice.md` 22.15 and says so.
#[test]
fn self_check_names_apply_cycle() {
    let scratch = common::scratch("self-check-binary");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ignite-perf"))
        .arg("self-check")
        .env("TMPDIR", &scratch)
        .output()
        .expect("run ignite-perf self-check");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("apply-cycle"), "{stdout}");
    assert_eq!(std::fs::read_dir(&scratch).unwrap().count(), 0);
}
