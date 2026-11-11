//! `ignite-perf quality-remote` without AWS: the calls of a dry run, the spot-then-on-demand
//! order, the cleanup on a failure after the launch, and the records it writes. A scripted
//! shell answers every command the mode runs, so no test reaches the network.

use std::path::{Path, PathBuf};

use clap::Parser;
use ignite_perf::cli::{Cli, Mode};
use ignite_perf::json::Json;
use ignite_perf::remote::{self, Ran, Remote, Row, Settings, Shell};

/// A shell whose answers come from `answer`; it records every argv and advances a clock on
/// `sleep`.
struct Scripted<F: FnMut(&[String]) -> Ran> {
    answer: F,
    calls: Vec<Vec<String>>,
    clock: f64,
}

impl<F: FnMut(&[String]) -> Ran> Shell for Scripted<F> {
    fn run(&mut self, argv: &[String]) -> Ran {
        self.calls.push(argv.to_vec());
        (self.answer)(argv)
    }
    fn sleep(&mut self, secs: u64) {
        self.clock += secs as f64;
    }
    fn now(&mut self) -> f64 {
        self.clock
    }
    fn log(&mut self, _line: &str) {}
}

fn ok(stdout: &str) -> Ran {
    Ran {
        code: 0,
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

fn failed(stderr: &str) -> Ran {
    Ran {
        code: 254,
        stdout: String::new(),
        stderr: stderr.to_string(),
    }
}

fn has(argv: &[String], word: &str) -> bool {
    argv.iter().any(|a| a == word)
}

/// The read-only answers of a region with two zones, `z-b` the cheaper spot zone.
fn read_only(argv: &[String]) -> Option<Ran> {
    let answer = if has(argv, "get-caller-identity") {
        r#"{"Arn": "arn:aws:sts::000000000000:assumed-role/r/x", "Account": "000000000000"}"#
    } else if argv[0] == "curl" {
        "203.0.113.5\n"
    } else if has(argv, "describe-vpcs") {
        r#"{"Vpcs": [{"VpcId": "vpc-1"}]}"#
    } else if has(argv, "describe-subnets") {
        r#"{"Subnets": [{"AvailabilityZone": "z-a", "SubnetId": "s-a"}, {"AvailabilityZone": "z-b", "SubnetId": "s-b"}]}"#
    } else if has(argv, "describe-instance-type-offerings") {
        r#"{"InstanceTypeOfferings": [{"Location": "z-a"}, {"Location": "z-b"}]}"#
    } else if has(argv, "describe-spot-price-history") {
        r#"{"SpotPriceHistory": [{"AvailabilityZone": "z-a", "SpotPrice": "1.40"}, {"AvailabilityZone": "z-b", "SpotPrice": "0.96"}]}"#
    } else if has(argv, "get-parameter") {
        r#"{"Parameter": {"Value": "ami-1"}}"#
    } else if has(argv, "describe-images") {
        r#"{"Images": [{"RootDeviceName": "/dev/sda1", "Name": "ubuntu-noble"}]}"#
    } else {
        return None;
    };
    Some(ok(answer))
}

fn settings(state: &Path, dry_run: bool) -> Settings {
    Settings {
        run_id: "iq-test".to_string(),
        instance_type: "c7i.metal-24xl".to_string(),
        profile: "p".to_string(),
        region: Some("r".to_string()),
        max_hours: 3.0,
        passes: 3,
        check_runs: 15,
        build_runs: 3,
        bench_passes: 5,
        dry_run,
        state: state.to_path_buf(),
        repo: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        base: "b".repeat(40),
        head: "h".repeat(40),
        harness: "p".repeat(40),
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("remote-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// EC2 calls that create, change or delete.
fn mutating(argv: &[String]) -> bool {
    argv.iter().any(|a| {
        a.starts_with("create-")
            || a.starts_with("delete-")
            || a.starts_with("authorize-")
            || a.starts_with("revoke-")
            || a == "run-instances"
            || a == "terminate-instances"
    })
}

#[test]
fn the_dry_run_makes_every_mutating_call_with_dry_run() {
    let state = scratch("dry");
    let mut shell = Scripted {
        answer: |argv: &[String]| {
            if has(argv, "--dry-run") {
                return failed(
                    "An error occurred (DryRunOperation) when calling: would have succeeded",
                );
            }
            read_only(argv).unwrap_or_else(|| panic!("a dry run made {argv:?}"))
        },
        calls: Vec::new(),
        clock: 0.0,
    };
    let mut run = Remote::new(settings(&state, true), &mut shell);
    let outcome = run.run().unwrap();
    assert_eq!(outcome.status, 0);
    let planned = run.planned().to_vec();
    drop(run);
    for argv in shell.calls.iter().filter(|a| a[0] == "aws") {
        assert_eq!(
            &argv[..5],
            ["aws", "--profile", "p", "--region", "r"],
            "{argv:?}"
        );
        assert!(
            !mutating(argv) || has(argv, "--dry-run"),
            "a dry run made a mutating call without --dry-run: {argv:?}"
        );
    }
    let launch = shell
        .calls
        .iter()
        .find(|a| has(a, "run-instances"))
        .expect("run-instances --dry-run");
    let joined = launch.join(" ");
    for wanted in [
        "--instance-type c7i.metal-24xl",
        "--image-id ami-1",
        "--subnet-id s-b",
        "--instance-initiated-shutdown-behavior terminate",
        "\"MarketType\":\"spot\"",
        "Key=ignite-quality-run,Value=iq-test",
        "--dry-run",
    ] {
        assert!(
            joined.contains(wanted),
            "run-instances lacks {wanted}: {joined}"
        );
    }
    let user_data = std::fs::read_to_string(state.join("user-data.sh")).unwrap();
    assert!(user_data.contains("shutdown -h +180"), "{user_data}");
    for step in [
        "create-security-group",
        "authorize-security-group-ingress",
        "create-key-pair",
        "run-instances",
        "terminate-instances",
        "delete-key-pair",
        "delete-security-group",
    ] {
        assert!(
            planned.iter().any(|a| has(a, step)),
            "the plan names no {step}: {planned:?}"
        );
    }
}

#[test]
fn on_demand_only_when_no_zone_has_spot_capacity() {
    let state = scratch("capacity");
    let mut shell = Scripted {
        answer: |argv: &[String]| {
            if has(argv, "run-instances") && argv.join(" ").contains("spot") {
                return failed(
                    "An error occurred (InsufficientInstanceCapacity) when calling RunInstances",
                );
            }
            if has(argv, "--dry-run") {
                return failed("An error occurred (DryRunOperation)");
            }
            read_only(argv).unwrap()
        },
        calls: Vec::new(),
        clock: 0.0,
    };
    Remote::new(settings(&state, true), &mut shell)
        .run()
        .unwrap();
    let launches: Vec<String> = shell
        .calls
        .iter()
        .filter(|a| has(a, "run-instances"))
        .map(|a| {
            let subnet = a[a.iter().position(|w| w == "--subnet-id").unwrap() + 1].clone();
            let market = if a.join(" ").contains("spot") {
                "spot"
            } else {
                "on-demand"
            };
            format!("{subnet} {market}")
        })
        .collect();
    assert_eq!(launches, ["s-b spot", "s-a spot", "s-b on-demand"]);
}

#[test]
fn a_failure_after_the_launch_terminates_and_deletes_what_the_run_made() {
    let state = scratch("failure");
    let mut terminated = false;
    let mut shell = Scripted {
        answer: move |argv: &[String]| {
            if let Some(ran) = read_only(argv) {
                return ran;
            }
            if has(argv, "create-security-group") {
                return ok(r#"{"GroupId": "sg-1"}"#);
            }
            if has(argv, "create-key-pair") {
                return ok(r#"{"KeyMaterial": "not a key"}"#);
            }
            if has(argv, "run-instances") {
                return ok(r#"{"Instances": [{"InstanceId": "i-1"}]}"#);
            }
            if has(argv, "terminate-instances") {
                terminated = true;
                return ok("{}");
            }
            if has(argv, "describe-instances") {
                let state = if terminated { "terminated" } else { "running" };
                let filtered = has(argv, "--filters");
                if filtered && !terminated {
                    return ok(&format!(
                        r#"{{"Reservations": [{{"Instances": [{{"InstanceId": "i-1", "State": {{"Name": "{state}"}}}}]}}]}}"#
                    ));
                }
                return ok(&format!(
                    r#"{{"Reservations": [{{"Instances": [{{"InstanceId": "i-1", "State": {{"Name": "{state}"}}, "PublicIpAddress": "198.51.100.7", "LaunchTime": "2026-11-08T10:00:00+00:00"}}]}}]}}"#
                ));
            }
            if has(argv, "describe-security-groups") || has(argv, "describe-key-pairs") {
                return ok(r#"{"SecurityGroups": [], "KeyPairs": []}"#);
            }
            if argv[0] == "ssh" {
                return failed("ssh: connect to host 198.51.100.7 port 22: Connection refused");
            }
            ok("{}")
        },
        calls: Vec::new(),
        clock: 0.0,
    };
    let error = Remote::new(settings(&state, false), &mut shell)
        .run()
        .unwrap_err();
    assert!(error.contains("ssh"), "{error}");
    let steps: Vec<&str> = shell
        .calls
        .iter()
        .filter_map(|a| {
            [
                "terminate-instances",
                "delete-key-pair",
                "delete-security-group",
            ]
            .into_iter()
            .find(|step| has(a, step))
        })
        .collect();
    assert_eq!(
        steps,
        [
            "terminate-instances",
            "delete-key-pair",
            "delete-security-group"
        ]
    );
    let last = shell.calls.last().unwrap().join(" ");
    assert!(
        last.contains("describe-") && last.contains("tag:ignite-quality-run,Values=iq-test"),
        "the run does not end with the leftover check by its tag: {last}"
    );
}

#[test]
fn a_real_run_needs_a_profile_and_names_none() {
    let Mode::QualityRemote(args) = Cli::try_parse_from([
        "ignite-perf",
        "quality-remote",
        "--base",
        "HEAD",
        "--head",
        "HEAD",
    ])
    .unwrap()
    .mode
    else {
        panic!("quality-remote");
    };
    assert_eq!(args.profile, None);
    assert_eq!(args.instance_type, "c7i.metal-24xl");
    assert_eq!(args.max_hours, 3.0);
    let error = remote::profile(&args, None).unwrap_err();
    assert!(error.contains("IGNITE_PERF_AWS_PROFILE"), "{error}");
    assert_eq!(remote::profile(&args, Some("q".into())).unwrap(), "q");
}

fn row(metric: &str, target: &str, base: &[i64], head: &[i64]) -> Row {
    Row {
        metric: metric.to_string(),
        target: target.to_string(),
        base: base.to_vec(),
        head: head.to_vec(),
        head_loads: vec![0.5; head.len()],
    }
}

#[test]
fn the_records_name_the_host_by_type_and_keep_the_instance_out() {
    let machine = Json::parse(
        r#"{"instance_type": "c7i.metal-24xl", "cpu": "Intel(R) Xeon(R) Platinum 8488C", "kernel": "6.8.0", "smt": "off", "turbo": "off", "market": "spot", "usd_per_hour": "0.9631", "instance_hours": "0.8000", "cost_usd": "0.7705"}"#,
    )
    .unwrap();
    let rows = [
        row("6.1", "all/check", &[60, 62, 61], &[61, 59, 63]),
        row("6.2", "rust", &[2000, 2010, 1990], &[2300, 2290, 2310]),
    ];
    let lines = remote::history_lines(&rows, &machine, "b0", "h1", "run", "2026-11-08");
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0],
        r#"{"base":"b0","base_value":61,"commit":"h1","date":"2026-11-08","host":"ec2:c7i.metal-24xl","load":"0.50","machine":{"cost_usd":"0.7705","cpu":"Intel(R) Xeon(R) Platinum 8488C","instance_hours":"0.8000","instance_type":"c7i.metal-24xl","kernel":"6.8.0","market":"spot","smt":"off","turbo":"off","usd_per_hour":"0.9631"},"metric":"6.1","run":"run","target":"all/check","value":61}"#
    );
    let baseline = Json::parse(
        r#"{"metrics": {"6.1": {"direction": "lower", "tolerance": "10%"}, "6.2": {"direction": "lower", "tolerance": "10%"}}}"#,
    )
    .unwrap();
    let refused = remote::refusals(&baseline, &rows).unwrap();
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(
        refused[0].starts_with("6.2 rust: 2000 ms at the base, 2300 ms"),
        "{refused:?}"
    );
}

#[test]
fn the_host_runs_both_trees_interleaved_after_a_discarded_warm_up() {
    assert_eq!(
        remote::schedule(3),
        [
            ("head", 0),
            ("base", 1),
            ("head", 1),
            ("head", 2),
            ("base", 2),
            ("base", 3),
            ("head", 3)
        ]
    );
    let command = remote::host_command(3, 15, 3, 5);
    assert!(
        command.contains("ignite-perf quality-host")
            && command.contains("--passes 3 --check-runs 15 --build-runs 3")
            && command.contains("setsid nohup"),
        "{command}"
    );
    let Mode::QualityHost(args) = Cli::try_parse_from([
        "ignite-perf",
        "quality-host",
        "--root",
        "/q",
        "--passes",
        "3",
    ])
    .unwrap()
    .mode
    else {
        panic!("quality-host");
    };
    assert_eq!((args.passes, args.check_runs, args.build_runs), (3, 15, 3));
}

/// A release row is refused beyond the tolerance (as before), and also within it when HEAD's
/// median is worse and every HEAD sample is worse than every BASE sample: then the difference
/// is shown, not noise. Overlapping samples leave the decision to the tolerance; a row with one
/// sample each is refused on any worse value. Both directions.
#[test]
fn a_row_worse_in_every_sample_is_refused_within_the_tolerance() {
    let baseline = Json::parse(
        r#"{"metrics": {"6.2": {"direction": "lower", "tolerance": "10%"}, "3.3": {"direction": "higher", "tolerance": "10%", "unit": "permille"}}}"#,
    )
    .unwrap();
    let refused = |row: Row| remote::refusals(&baseline, &[row]).unwrap();

    let base = [1000, 1004, 1008, 1002, 1006];
    let disjoint = refused(row("6.2", "rust", &base, &[1030, 1034, 1038, 1032, 1036]));
    assert!(
        disjoint.len() == 1
            && disjoint[0].starts_with("6.2 rust: 1004 ms at the base, 1034 ms now")
            && disjoint[0].contains("every sample"),
        "+3 % with disjoint samples was not refused: {disjoint:?}"
    );
    let overlap = refused(row("6.2", "rust", &base, &[1000, 1034, 1038, 1032, 1036]));
    assert!(
        overlap.is_empty(),
        "+3 % with overlapping samples was refused: {overlap:?}"
    );
    let one = refused(row("6.2", "rust", &[100], &[101]));
    assert_eq!(one.len(), 1, "one sample one unit worse: {one:?}");
    assert!(refused(row("6.2", "rust", &[100], &[100])).is_empty());
    assert!(refused(row("6.2", "rust", &base, &base)).is_empty());
    assert!(refused(row("6.2", "rust", &[101], &[100])).is_empty());

    let base = [1000, 996, 992, 998, 994];
    let lower = refused(row(
        "3.3",
        "llvm/todo/get-1",
        &base,
        &[970, 966, 962, 968, 964],
    ));
    assert!(
        lower.len() == 1
            && lower[0].starts_with("3.3 llvm/todo/get-1: 0.996× at the base, 0.966× now")
            && lower[0].contains("every sample"),
        "−3 % of a higher-is-better row with disjoint samples was not refused: {lower:?}"
    );
    let overlap = refused(row(
        "3.3",
        "llvm/todo/get-1",
        &base,
        &[1000, 966, 962, 968, 964],
    ));
    assert!(overlap.is_empty(), "{overlap:?}");
    let one = refused(row("3.3", "llvm/todo/get-1", &[900], &[899]));
    assert_eq!(one.len(), 1, "one sample one unit worse: {one:?}");
    assert!(refused(row("3.3", "llvm/todo/get-1", &[900], &[900])).is_empty());
    assert!(refused(row("3.3", "llvm/todo/get-1", &[900], &[901])).is_empty());
}

/// Samples that touch (HEAD's best equal to BASE's worst) overlap: not every HEAD sample is worse
/// than every BASE sample, so within the tolerance the row is left to it and not refused. Both
/// directions. Holds `>` and `<` of the disjoint-samples rule against `>=` and `<=`.
#[test]
fn a_row_whose_samples_touch_the_base_is_left_to_the_tolerance() {
    let baseline = Json::parse(
        r#"{"metrics": {"6.2": {"direction": "lower", "tolerance": "10%"}, "3.3": {"direction": "higher", "tolerance": "10%", "unit": "permille"}}}"#,
    )
    .unwrap();
    let refused = |row: Row| remote::refusals(&baseline, &[row]).unwrap();
    let lower = refused(row(
        "6.2",
        "rust",
        &[1000, 1004, 1008, 1002, 1006],
        &[1008, 1034, 1038, 1032, 1036],
    ));
    assert!(lower.is_empty(), "touching samples refused: {lower:?}");
    let higher = refused(row(
        "3.3",
        "llvm/todo/get-1",
        &[1000, 996, 992, 998, 994],
        &[992, 966, 962, 968, 964],
    ));
    assert!(higher.is_empty(), "touching samples refused: {higher:?}");
}

/// The baseline of the size-row tests: 3.9 marked as the size row (`"release_growth":
/// "decision"`), `rust/todo` with `decision` on its bar when one is given, `llvm/todo` without;
/// 3.8 with the same decision on its bar and no mark; the timing row 6.2.
fn size_baseline(decision: Option<&str>) -> Json {
    let decided = decision.map_or(String::new(), |d| format!(r#""decision": "{d}", "#));
    Json::parse(&format!(
        r#"{{"metrics": {{
            "3.9": {{"direction": "lower", "tolerance": "5%", "unit": "permille", "release_growth": "decision",
                    "bars": {{"rust/todo": {{{decided}"value": 864}}, "llvm/todo": {{"value": 869}}}}}},
            "3.8": {{"direction": "lower", "tolerance": "10%", "unit": "permille",
                    "bars": {{"rust/todo/get-1": {{{decided}"value": 900}}}}}},
            "6.2": {{"direction": "lower", "tolerance": "10%", "unit": "ms"}}}}}}"#
    ))
    .unwrap()
}

/// `AGENTS.md` § Releases (2026-11-11): a binary may grow when a release adds code. A cell of
/// the size row 3.9 worse than BASE is not refused when its bar in `quality/baseline.json`
/// carries a new `decision`, one BASE's baseline does not hold (its bar there had none, or
/// another); the run lists it with BASE's value, HEAD's and the decision.
#[test]
fn a_grown_cell_of_the_size_row_passes_by_a_decision_and_is_listed() {
    let decision = "story:x (2026-11-11): the TLS client session adds the bytes";
    let baseline = size_baseline(Some(decision));
    let grown = row("3.9", "rust/todo", &[723], &[864]);
    for base in [
        size_baseline(None),
        size_baseline(Some("story:w (2026-11-08): the scheduler's bytes")),
    ] {
        let judged = remote::judge(&baseline, Some(&base), std::slice::from_ref(&grown)).unwrap();
        assert_eq!(
            judged.refused,
            Vec::<String>::new(),
            "a size row grown by a new decision was refused"
        );
        assert_eq!(
            judged.grown,
            [format!(
                "3.9 rust/todo: 0.723× at the base, 0.864× now; decision: {decision}"
            )],
            "the grown cell is not listed"
        );
    }
    let one_more = remote::judge(
        &baseline,
        Some(&size_baseline(None)),
        &[row("3.9", "rust/todo", &[723], &[724])],
    )
    .unwrap();
    assert!(
        one_more.refused.is_empty() && one_more.grown.len() == 1,
        "{one_more:?}"
    );
}

/// A decision is taken for the code one release adds. One BASE's baseline already holds names
/// the last release's code, so it does not let a cell grow again: the cell is refused, and the
/// refusal says the decision is the last release's and a new one naming the added code is
/// needed. Held is held anywhere in BASE's baseline, trimmed: on the same bar, on another bar of
/// the row, on another row, on a tolerance, for a slow unit, for an intermittent difference.
#[test]
fn a_grown_cell_held_by_the_bases_decision_is_refused() {
    let decision = "story:w (2026-11-08): the scheduler's bytes";
    let held = |place: &str| {
        Json::parse(&format!(
            r#"{{"metrics": {{"3.9": {{"direction": "lower", "tolerance": "5%", "unit": "permille",
                "release_growth": "decision", "bars": {{"rust/todo": {{"value": 723}}}}}}}}, {place}}}"#
        ))
        .unwrap()
    };
    let other_bar = Json::parse(&format!(
        r#"{{"metrics": {{"3.9": {{"direction": "lower", "tolerance": "5%", "unit": "permille",
            "release_growth": "decision",
            "bars": {{"rust/todo": {{"value": 723}}, "llvm/todo": {{"decision": "{decision}", "value": 869}}}}}}}}}}"#
    ))
    .unwrap();
    let holds_none = held(r#""eval_slow": {}, "intermittent_decisions": {}"#);
    assert!(
        remote::judge(
            &size_baseline(Some(decision)),
            Some(&holds_none),
            &[row("3.9", "rust/todo", &[723], &[864])]
        )
        .unwrap()
        .refused
        .is_empty(),
        "a base that holds no decision refused a new one"
    );
    let on_a_tolerance = Json::parse(&format!(
        r#"{{"metrics": {{"4.2": {{"direction": "lower", "tolerance": 1, "tolerance_decision": "{decision}"}}}}}}"#
    ))
    .unwrap();
    for (place, base, now) in [
        (
            "the same bar",
            size_baseline(Some(decision)),
            decision.to_string(),
        ),
        (
            "the same bar, a space added",
            size_baseline(Some(decision)),
            format!("{decision} "),
        ),
        (
            "the same bar, padded at the base",
            size_baseline(Some(&format!(" {decision} "))),
            decision.to_string(),
        ),
        ("another bar of the row", other_bar, decision.to_string()),
        (
            "a tolerance of another row",
            on_a_tolerance,
            decision.to_string(),
        ),
        (
            "a slow unit",
            held(&format!(
                r#""eval_slow": {{"std/x.ignite::a test": "{decision}"}}"#
            )),
            decision.to_string(),
        ),
        (
            "an intermittent difference",
            held(&format!(
                r#""intermittent_decisions": {{"std/x.ignite::a test": "{decision}"}}"#
            )),
            decision.to_string(),
        ),
    ] {
        let baseline = size_baseline(Some(&now));
        let judged = remote::judge(
            &baseline,
            Some(&base),
            &[row("3.9", "rust/todo", &[723], &[864])],
        )
        .unwrap();
        assert!(
            judged.refused.len() == 1
                && judged.refused[0].starts_with("3.9 rust/todo: 0.723× at the base, 0.864× now")
                && judged.refused[0].contains(decision)
                && judged.refused[0].contains("is the last release's")
                && judged.refused[0].contains("a new `decision` naming the added code is needed"),
            "decision carried over from the base on {place}: {:?}",
            judged.refused
        );
        assert!(judged.grown.is_empty(), "{place}: {:?}", judged.grown);
    }
}

/// Whether a decision is new is read from BASE's baseline. Without it (it could not be read) a
/// grown cell is refused, not passed, and the refusal says so; a cell that did not grow, and a
/// grown one without a decision, are judged as with it.
#[test]
fn a_grown_cell_is_refused_when_the_bases_baseline_cannot_be_read() {
    let decision = "story:x (2026-11-11): the TLS client session adds the bytes";
    let baseline = size_baseline(Some(decision));
    let grown = row("3.9", "rust/todo", &[723], &[864]);
    let judged = remote::judge(&baseline, None, std::slice::from_ref(&grown)).unwrap();
    assert!(
        judged.refused.len() == 1
            && judged.refused[0].starts_with("3.9 rust/todo: 0.723× at the base, 0.864× now")
            && judged.refused[0].contains("quality/baseline.json at the base could not be read")
            && judged.refused[0].contains(decision),
        "the base's baseline unread: {:?}",
        judged.refused
    );
    assert!(judged.grown.is_empty(), "{:?}", judged.grown);
    assert_eq!(
        remote::refusals(&baseline, std::slice::from_ref(&grown)).unwrap(),
        judged.refused,
        "`refusals` has no baseline of the base and passed a grown cell"
    );
    assert_eq!(
        remote::judge(&baseline, None, &[row("3.9", "rust/todo", &[723], &[723])]).unwrap(),
        remote::Judged::default(),
        "a cell that did not grow needs no baseline of the base"
    );
    let undecided = remote::judge(&baseline, None, &[row("3.9", "llvm/todo", &[720], &[869])])
        .unwrap()
        .refused;
    assert!(
        undecided.len() == 1
            && undecided[0].contains("a `decision` naming the added code is missing"),
        "{undecided:?}"
    );
}

/// A repository under the test target with `quality/baseline.json` holding `text` committed
/// (none when `text` is `None`); its root.
fn repository_with_baseline(name: &str, text: Option<&str>) -> PathBuf {
    let root = scratch(name);
    std::fs::create_dir_all(root.join("quality")).unwrap();
    std::fs::write(root.join("README.md"), "t\n").unwrap();
    if let Some(text) = text {
        std::fs::write(root.join("quality/baseline.json"), text).unwrap();
    }
    for args in [
        &["init", "-q"][..],
        &["add", "-A"],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "-q",
            "-m",
            "t",
        ],
    ] {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }
    root
}

/// BASE's baseline reaches the judge from the caller's repository, `git show
/// BASE:quality/baseline.json`: the committed file of that revision, not the working tree's.
/// A revision without the file, an unknown revision and a file that is no JSON are errors, so
/// the run passes no baseline and a grown cell is refused.
#[test]
fn the_bases_baseline_is_read_from_the_revision() {
    let committed = r#"{"metrics": {"3.9": {"bars": {"rust/todo": {"decision": "story:w: the bytes", "value": 723}}}}}"#;
    let root = repository_with_baseline("base-baseline", Some(committed));
    std::fs::write(root.join("quality/baseline.json"), r#"{"metrics": {}}"#).unwrap();
    assert_eq!(
        remote::baseline_at(&root, "HEAD").unwrap(),
        Json::parse(committed).unwrap(),
        "not the committed baseline of the revision"
    );
    let unknown = remote::baseline_at(&root, "v0.0.0").unwrap_err();
    assert!(
        unknown.contains("v0.0.0:quality/baseline.json"),
        "an unknown revision: {unknown}"
    );
    let without = repository_with_baseline("base-without-baseline", None);
    let missing = remote::baseline_at(&without, "HEAD").unwrap_err();
    assert!(
        missing.contains("HEAD:quality/baseline.json"),
        "a revision without the file: {missing}"
    );
    let broken = repository_with_baseline("base-broken-baseline", Some("{"));
    let unparsed = remote::baseline_at(&broken, "HEAD").unwrap_err();
    assert!(
        unparsed.contains("HEAD:quality/baseline.json"),
        "a baseline that is no JSON: {unparsed}"
    );
}

/// Growth stays deliberate: a cell of the size row worse than BASE whose bar has no `decision`
/// (none, a blank one, no bar at all) is refused, and the refusal says that a decision naming
/// the added code is missing.
#[test]
fn a_grown_cell_of_the_size_row_without_a_decision_is_refused() {
    for (baseline, target) in [
        (size_baseline(Some("story:x: the bytes")), "llvm/todo"),
        (size_baseline(Some("  ")), "rust/todo"),
        (size_baseline(None), "rust/todo"),
        (size_baseline(Some("story:x: the bytes")), "rust/terminal"),
    ] {
        let judged = remote::judge(
            &baseline,
            Some(&size_baseline(None)),
            &[row("3.9", target, &[720], &[869])],
        )
        .unwrap();
        assert!(
            judged.refused.len() == 1
                && judged.refused[0]
                    .starts_with(&format!("3.9 {target}: 0.720× at the base, 0.869× now"))
                && judged.refused[0].contains("a `decision` naming the added code is missing"),
            "{target}: {:?}",
            judged.refused
        );
        assert!(judged.grown.is_empty(), "{target}: {:?}", judged.grown);
    }
}

/// The rule is for the size row only: a timing row and a benchmark row that is no size row are
/// judged as before, a `decision` on the bar or not, and listed nowhere.
#[test]
fn a_row_that_is_no_size_row_is_refused_with_a_decision_as_before() {
    let baseline = size_baseline(Some("story:x: the bytes"));
    let rows = [
        row("3.8", "rust/todo/get-1", &[800], &[900]),
        row("6.2", "rust", &[2000], &[2300]),
    ];
    let judged = remote::judge(&baseline, Some(&size_baseline(None)), &rows).unwrap();
    assert_eq!(
        judged.refused,
        [
            r#"3.8 rust/todo/get-1: 0.800× at the base, 0.900× now (lower, "10%")"#,
            r#"6.2 rust: 2000 ms at the base, 2300 ms now (lower, "10%")"#,
        ]
    );
    assert!(judged.grown.is_empty(), "{:?}", judged.grown);
}

/// A size row at or below BASE needs nothing: no decision, no listing, no refusal.
#[test]
fn a_size_row_that_shrank_needs_no_decision() {
    let baseline = size_baseline(None);
    for head in [700, 723] {
        let judged = remote::judge(
            &baseline,
            Some(&size_baseline(None)),
            &[row("3.9", "rust/todo", &[723], &[head])],
        )
        .unwrap();
        assert_eq!(judged, remote::Judged::default(), "0.723× to {head}");
    }
}

/// The mark is validated where the run reads it: another value than `"decision"` is an error,
/// and so is the mark on a timing row or a benchmark row other than 3.9, whose time and memory
/// no decision holds.
#[test]
fn the_size_mark_is_validated_by_the_run() {
    let marked = |id: &str, mark: &str| {
        Json::parse(&format!(
            r#"{{"metrics": {{"{id}": {{"direction": "lower", "tolerance": "5%", "release_growth": {mark},
                "bars": {{"rust/todo": {{"decision": "story:x: the bytes", "value": 1}}}}}}}}}}"#
        ))
        .unwrap()
    };
    for mark in [r#""always""#, r#""""#, "true", "null"] {
        let error = remote::judge(
            &marked("3.9", mark),
            None,
            &[row("3.9", "rust/todo", &[723], &[723])],
        )
        .unwrap_err();
        assert!(
            error.contains("3.9") && error.contains("`release_growth`"),
            "release_growth {mark}: {error}"
        );
    }
    for id in ["5.3", "6.1", "6.2", "3.3", "3.7", "3.8"] {
        let error = remote::judge(
            &marked(id, r#""decision""#),
            None,
            &[row(id, "rust/todo", &[723], &[864])],
        )
        .unwrap_err();
        assert!(
            error.contains(id) && error.contains("`release_growth`"),
            "{id} marked as a size row: {error}"
        );
    }
}

/// The dedicated host takes five samples (passes) of each tree by default, so two equal builds
/// have disjoint samples by chance in 1 of 252 runs (C(10, 5)), not 1 of 20 (C(6, 3)).
#[test]
fn the_host_takes_five_samples_of_each_tree() {
    let Mode::QualityRemote(remote_args) = Cli::try_parse_from([
        "ignite-perf",
        "quality-remote",
        "--base",
        "a",
        "--head",
        "b",
    ])
    .unwrap()
    .mode
    else {
        panic!("quality-remote");
    };
    let Mode::QualityHost(host_args) =
        Cli::try_parse_from(["ignite-perf", "quality-host", "--root", "/q"])
            .unwrap()
            .mode
    else {
        panic!("quality-host");
    };
    assert_eq!((remote_args.passes, host_args.passes), (5, 5));
    let measured = remote::schedule(remote_args.passes);
    let count = |tree: &str| {
        measured
            .iter()
            .filter(|(t, pass)| *t == tree && *pass > 0)
            .count()
    };
    assert_eq!((count("base"), count("head")), (5, 5));
}

/// A release run on fewer samples than five refuses to start: `--passes 0` yields no timing
/// row and `--bench-passes 0` no benchmark row, so the release would pass both unjudged. Runs
/// that are not a release (`run`, `bar`) take any count.
#[test]
fn a_release_run_takes_at_least_five_passes() {
    assert!(remote::release_samples("v0.16.0", 5, 5).is_ok());
    assert!(remote::release_samples("v0.16.0", 7, 6).is_ok());
    assert!(remote::release_samples("run", 0, 0).is_ok());
    assert!(remote::release_samples("bar", 1, 1).is_ok());
    for (passes, bench_passes, flag) in [
        (0, 5, "--passes 0"),
        (4, 5, "--passes 4"),
        (5, 0, "--bench-passes 0"),
        (5, 4, "--bench-passes 4"),
    ] {
        let refused = remote::release_samples("v0.16.0", passes, bench_passes);
        assert!(
            refused.as_ref().is_err_and(|e| e.contains(flag)),
            "a release with {flag} was not refused: {refused:?}"
        );
    }
}

/// Gate adversary pass 2: the pass check runs first in `remote::main`, before the profile, the
/// git lookups and any AWS call. The repository named does not exist, so a `main` without the
/// check fails on `git` instead, and still reaches no network.
#[test]
fn gate_adv2_quality_remote_refuses_a_one_pass_release_before_anything_else() {
    let Mode::QualityRemote(args) = Cli::try_parse_from([
        "ignite-perf",
        "quality-remote",
        "--repo",
        "/nonexistent/ignite-perf-gate-adv2",
        "--base",
        "v0.15.0",
        "--head",
        "HEAD",
        "--profile",
        "none",
        "--dry-run",
        "--run",
        "v0.16.0",
        "--passes",
        "1",
    ])
    .unwrap()
    .mode
    else {
        panic!("quality-remote");
    };
    let refused = remote::main(&args);
    assert!(
        refused.as_ref().is_err_and(|e| e.contains("--passes 1")),
        "a release run on one pass did not stop at the pass check: {refused:?}"
    );
}
