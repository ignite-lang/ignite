use std::path::PathBuf;

use ignite_perf::cli::ReadinessArgs;
use ignite_perf::json::Json;
use ignite_perf::readiness::{
    Controller, Recorded, SCENARIOS, effect_is, event_is, received, run_command, timeout_seen,
};

use crate::common;

#[test]
fn the_recorded_c6b22cf_runs_pass_every_scenario_with_the_reported_results() {
    let report = common::json(&format!("{}/report.json", common::READINESS));
    let work = common::evidence(common::READINESS);
    for (name, state, _, _, scenario) in SCENARIOS {
        let mut recorded = Recorded::load(&work, state).unwrap();
        let result = scenario(&mut recorded).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            result.dumps(false),
            report.field("scenarios").field(name).dumps(false),
            "{name}"
        );
    }
}

#[test]
fn a_restart_before_its_backoff_fails_the_manual_scenario() {
    let work = common::evidence(common::READINESS);
    let mut recorded = Recorded::load(&work, "manual-restart").unwrap();
    let mut early = recorded.records.clone();
    let started = early
        .iter()
        .rposition(|row| event_is(row, "Started"))
        .unwrap();
    let mut row = early[started].clone();
    let mut at = row.field("at").clone();
    at.set("ns", Json::Int(at.field("ns").as_i64().unwrap() - 1));
    row.set("at", at);
    early[started] = row;
    recorded.records = early;
    let (_, _, _, _, manual) = SCENARIOS[0];
    assert!(manual(&mut recorded).is_err());
}

#[test]
fn a_missing_consequence_fails_the_probe_scenario() {
    let work = common::evidence(common::READINESS);
    let mut recorded = Recorded::load(&work, "probe-timeout").unwrap();
    recorded
        .records
        .retain(|row| row.field("kind").as_str() != Some("consequence"));
    let (_, _, _, _, probe) = SCENARIOS[2];
    let error = probe(&mut recorded).unwrap_err();
    assert!(
        error.starts_with("Controller ended waiting for breach consequence"),
        "{error}"
    );
}

#[test]
fn predicates_match_exact_kinds_and_booleans() {
    let sample = |line: &str| {
        Json::parse(&format!(
            r#"{{"kind": "signal", "value": {{"ref": "Terminal.promptProbe.attempts", "value": {{"fields": {{"line": {line}, "slot": {{"ns": 0, "type": "Duration"}}}}, "type": "std.sli.Sample"}}}}}}"#
        ))
        .unwrap()
    };
    let signal = |received: &str| {
        sample(&format!(
            r#"{{"fields": ["{{\"fields\":{{\"received\":{received}}},\"type\":\"std.sli.ProbeAttempt\"}}"], "type": "Option", "variant": "Some"}}"#
        ))
    };
    assert!(received(&signal("false"), false));
    assert!(!received(&signal("0"), false));
    assert!(!received(&signal("true"), false));
    assert!(received(&signal("true"), true));
    let timed_out = sample(r#"{"type": "Option", "variant": "None"}"#);
    assert!(received(&timed_out, false));
    assert!(!received(&timed_out, true));
    let recorded = |received: &str| {
        Json::parse(&format!(
            r#"{{"kind": "signal", "value": {{"ref": "Terminal.promptProbe.attempts", "value": {{"fields": {{"received": {received}}}, "type": "std.sli.ProbeAttempt"}}}}}}"#
        ))
        .unwrap()
    };
    assert!(received(&recorded("true"), true));
    assert!(received(&recorded("false"), false));
    assert!(!received(&recorded("0"), false));
    let started = Json::parse(
        r#"{"kind": "event", "machine": "Terminal.server.supervisor", "value": {"fields": {"kind": {"variant": "Started", "fields": [42]}}}}"#,
    )
    .unwrap();
    assert!(event_is(&started, "Started"));
    let mut other = started.clone();
    other.set("machine", "Terminal.other".into());
    assert!(!event_is(&other, "Started"));
    let note = Json::parse(
        r#"{"kind": "effect", "value": {"variant": "Note", "fields": ["probe timeout"]}}"#,
    )
    .unwrap();
    assert!(effect_is(&note, "Note"));
    assert!(timeout_seen(&[note]));
    let mut recorded = Recorded {
        records: [started].into(),
        seen: Vec::new(),
        replay: String::new(),
        journal: String::new(),
    };
    recorded.event("Started").unwrap();
    assert_eq!(recorded.pids(), [42]);
}

#[test]
fn the_run_command_is_the_recorded_one() {
    let recorded = common::json(&format!("{}/probe-timeout-command.json", common::READINESS));
    let words: Vec<&str> = recorded
        .as_arr()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    let design = PathBuf::from(words[2]);
    let args = ReadinessArgs {
        cli: words[0].into(),
        repo: design.ancestors().nth(3).unwrap().to_path_buf(),
        runtime: words[12].into(),
        work: PathBuf::from(words[8]).parent().unwrap().to_path_buf(),
        candidate: "c6b22cf".into(),
        source_manifest: "/m".into(),
        timeout: 15.0,
    };
    let state = PathBuf::from(words[6]);
    let command = run_command(&args, &state, "manual", Some("2026-10-31T22:59:30Z"));
    assert_eq!(command, words);
}

/// `ignite-perf quality` keeps a real-clock restart outside the readiness
/// bound as its 5.3 value (a timing row gates nothing on the shared
/// workstation); any other readiness failure is no value.
#[test]
fn quality_reads_the_restart_of_a_refusal_outside_the_bound() {
    use ignite_perf::quality::outside_bound;
    assert_eq!(
        outside_bound("Real restart 743076991 ns outside 100–500 ms"),
        Some(743_076_991)
    );
    assert_eq!(
        outside_bound("readiness: Real restart 99 ns outside 100–500 ms"),
        Some(99)
    );
    assert_eq!(
        outside_bound("terminal: Resource temporarily unavailable"),
        None
    );
}
