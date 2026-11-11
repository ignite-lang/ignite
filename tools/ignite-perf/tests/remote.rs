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
        size_baseline(Some(
            "story:w (2026-11-08): the scheduler's run queue adds the bytes",
        )),
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
    let decision = "story:w (2026-11-08): the scheduler's run queue adds the bytes";
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
        remote::judge(
            &size_baseline(None),
            None,
            &[row("3.9", "rust/todo", &[723], &[723])]
        )
        .unwrap(),
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

/// A file at BASE that is JSON and no baseline (no `metrics` object) holds no decision to
/// compare with, so it is not read as one: the reader refuses it, and the judge given such a
/// value refuses the grown cell as it does without a baseline of the base.
#[test]
fn a_file_at_the_base_that_is_no_baseline_is_not_read_as_one() {
    let decision = "story:x (2026-11-11): the TLS client session adds the bytes";
    let baseline = size_baseline(Some(decision));
    for (name, text) in [
        ("base-empty-object", "{}"),
        ("base-array", "[]"),
        ("base-metrics-array", r#"{"metrics": []}"#),
        ("base-metrics-null", r#"{"metrics": null}"#),
    ] {
        let root = repository_with_baseline(name, Some(text));
        let error = remote::baseline_at(&root, "HEAD").unwrap_err();
        assert!(
            error.contains("HEAD:quality/baseline.json") && error.contains("`metrics`"),
            "{text}: {error}"
        );
        let judged = remote::judge(
            &baseline,
            Some(&Json::parse(text).unwrap()),
            &[row("3.9", "rust/todo", &[723], &[864])],
        )
        .unwrap();
        assert!(
            judged.grown.is_empty()
                && judged.refused.len() == 1
                && judged.refused[0]
                    .contains("quality/baseline.json at the base could not be read"),
            "{text}: {judged:?}"
        );
    }
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

/// Row 6.2 llvm (story `build-time-back`): the host installs lld of the LLVM it installed, so
/// `ld.lld` lies beside the clang `/usr/local/bin/clang` resolves to and a dev build on the LLVM
/// target links with it, as on the workstation.
#[test]
fn the_host_installs_lld_beside_its_clang() {
    let user_data = remote::user_data(180);
    let clang = user_data
        .find("ln -sf \"/usr/bin/clang-$installed\" /usr/local/bin/clang")
        .expect("clang is linked");
    let lld = user_data
        .find("apt-get install -y -q --no-install-recommends \"lld-$installed\"")
        .unwrap_or_else(|| panic!("lld is not installed:\n{user_data}"));
    let checked = user_data
        .find("test -x \"/usr/lib/llvm-$installed/bin/ld.lld\"")
        .unwrap_or_else(|| panic!("ld.lld beside clang is not checked:\n{user_data}"));
    assert!(lld < checked && checked < clang, "{user_data}");
}

/// Size-rule adversary pass 1: the size row has one sample of each tree, so a cell one permille
/// above BASE, far inside the 5 % tolerance, is worse and is refused without a `decision`, as
/// one beyond the tolerance is. The unit's tests refuse an undecided cell only beyond the
/// tolerance (0.720× to 0.869×); a judge that let an undecided cell inside it pass, listed
/// nowhere, left them green (mutant R13b of the pass). A release does not grow by accident by
/// one permille either, and several cells are each judged on their own bar.
#[test]
fn sizerule_adv1_a_size_cell_inside_the_tolerance_is_refused_without_a_decision() {
    let baseline = size_baseline(Some(
        "story:x (2026-11-11): the TLS client session adds the bytes",
    ));
    let judged = remote::judge(
        &baseline,
        Some(&size_baseline(None)),
        &[
            row("3.9", "llvm/todo", &[720], &[721]),
            row("3.9", "rust/todo", &[723], &[724]),
            row("3.9", "rust/terminal", &[1304], &[1305]),
            row("3.9", "llvm/terminal", &[1406], &[1400]),
        ],
    )
    .unwrap();
    assert_eq!(
        judged.grown,
        [
            "3.9 rust/todo: 0.723× at the base, 0.724× now; decision: story:x (2026-11-11): the TLS client session adds the bytes"
        ],
        "the cells listed"
    );
    assert_eq!(judged.refused.len(), 2, "{:?}", judged.refused);
    for (refusal, target) in judged.refused.iter().zip(["llvm/todo", "rust/terminal"]) {
        assert!(
            refusal.starts_with(&format!("3.9 {target}: "))
                && refusal.contains("every sample worse than every sample of the base")
                && refusal.contains("a `decision` naming the added code is missing"),
            "{target}, one permille above the base without a decision: {refusal}"
        );
    }
}

// ---- the size rule, fix round 2 ----------------------------------------------------------

/// The decision BASE's baseline held, in the tests of how a decision reads.
const HELD: &str = "story:w (2026-11-08): the scheduler's run queue adds the bytes.";

/// A decision is read by its letters and digits, case folded: one with fewer than 40 of them
/// names nothing; one that is the last release's, or contains it, is the last release's (upper
/// case, the final dot dropped, other punctuation, a zero-width space, a sentence appended or
/// put before it). One character of it changed reads as new: the check reads that the text
/// differs, a person reads what it says. A text of the last release with fewer than 40 letters
/// and digits is no decision either and carries nothing over. The cases are those of
/// `quality::a_decision_is_read_by_its_letters_and_digits` of the gate, which holds its own
/// copy of the function to them.
#[test]
fn a_decision_is_read_by_its_letters_and_digits() {
    use remote::DecisionRead;
    let held = std::collections::BTreeSet::from([HELD.to_string()]);
    let cases = [
        (HELD.to_string(), DecisionRead::LastReleases, "itself"),
        (
            HELD.to_uppercase(),
            DecisionRead::LastReleases,
            "in upper case",
        ),
        (
            HELD.trim_end_matches('.').to_string(),
            DecisionRead::LastReleases,
            "without its final dot",
        ),
        (
            "Story:W, 2026-11-08 - the  scheduler's run-queue adds the bytes".to_string(),
            DecisionRead::LastReleases,
            "with other punctuation and spacing",
        ),
        (
            HELD.replace(' ', "\u{200b} "),
            DecisionRead::LastReleases,
            "with zero-width spaces in it",
        ),
        (
            format!("{HELD} It holds the TLS client session too."),
            DecisionRead::LastReleases,
            "with a sentence appended",
        ),
        (
            format!("Accepted again for this release: {HELD}"),
            DecisionRead::LastReleases,
            "with a sentence put before it",
        ),
        (
            HELD.replace("bytes", "bytez"),
            DecisionRead::New,
            "with one character changed",
        ),
        (
            "story:x (2026-11-11): the TLS client session adds 40 B".to_string(),
            DecisionRead::New,
            "another decision of 40 letters and digits",
        ),
        (
            "story:x (2026-11-11): the TLS client session adds 4 B".to_string(),
            DecisionRead::NamesNothing,
            "39 letters and digits",
        ),
        (
            "\u{200b}".to_string(),
            DecisionRead::NamesNothing,
            "a zero-width space",
        ),
        (".".to_string(), DecisionRead::NamesNothing, "a dot"),
        ("x".to_string(), DecisionRead::NamesNothing, "one letter"),
        (String::new(), DecisionRead::NamesNothing, "nothing"),
        (
            "-- .. -- .. -- .. -- .. -- .. -- .. -- .. -- .. -- .. -- ..".to_string(),
            DecisionRead::NamesNothing,
            "60 characters, no letter or digit",
        ),
    ];
    for (decision, read, what) in cases {
        assert_eq!(
            remote::read_decision(&decision, &held),
            read,
            "the last release's decision {what}: {decision:?}"
        );
    }
    let short = std::collections::BTreeSet::from(["x".to_string(), ".".to_string(), String::new()]);
    assert_eq!(
        remote::read_decision(
            "story:x (2026-11-11): the TLS client session adds 40 B",
            &short
        ),
        DecisionRead::New,
        "a text of the last release that names nothing carried over"
    );
}

/// The judge reads the decision of a grown cell that way: BASE's decision reworded does not
/// let the cell grow again, one character changed does, and a text that names nothing is no
/// decision, whether BASE's baseline was read or not.
#[test]
fn a_grown_cell_is_held_only_by_a_decision_new_in_its_letters_and_digits() {
    let base = size_baseline(Some(HELD));
    let judge = |decision: &str, base: Option<&Json>| {
        remote::judge(
            &size_baseline(Some(decision)),
            base,
            &[row("3.9", "rust/todo", &[723], &[864])],
        )
        .unwrap()
    };
    for decision in [
        HELD.to_uppercase(),
        HELD.trim_end_matches('.').to_string(),
        format!("{HELD} It holds the TLS client session too."),
    ] {
        let judged = judge(&decision, Some(&base));
        assert!(
            judged.grown.is_empty()
                && judged.held.is_empty()
                && judged.refused.len() == 1
                && judged.refused[0].contains("is the last release's")
                && judged.refused[0].contains("a new `decision` naming the added code is needed"),
            "{decision:?}: {judged:?}"
        );
    }
    let changed = judge(&HELD.replace("bytes", "bytez"), Some(&base));
    assert!(
        changed.refused.is_empty() && changed.grown.len() == 1,
        "one character changed: {changed:?}"
    );
    for decision in ["\u{200b}", ".", "x", "story:x: the bytes"] {
        for base in [Some(&base), None] {
            let judged = judge(decision, base);
            assert!(
                judged.grown.is_empty()
                    && judged.refused.len() == 1
                    && judged.refused[0]
                        .starts_with("3.9 rust/todo: 0.723× at the base, 0.864× now")
                    && judged.refused[0].contains("names nothing")
                    && judged.refused[0].contains("a `decision` naming the added code is missing"),
                "{decision:?}: {judged:?}"
            );
        }
    }
}

/// A decision on the bar of a size cell that is not above BASE holds nothing, and the run
/// refuses it until it is removed: a decision written for a candidate that was not cut does
/// not wait on the bar to hold a later growth nobody decided. A row that is no size row is
/// judged as before, a decision on its bar or not.
#[test]
fn a_decision_on_a_size_cell_not_above_the_base_is_refused() {
    let decision = "story:x (2026-11-11): the TLS client session adds the bytes";
    let baseline = size_baseline(Some(decision));
    let base = size_baseline(None);
    for (head, base) in [
        (723, Some(&base)),
        (700, Some(&base)),
        (723, None),
        (700, None),
    ] {
        let judged =
            remote::judge(&baseline, base, &[row("3.9", "rust/todo", &[723], &[head])]).unwrap();
        assert!(
            judged.grown.is_empty()
                && judged.held.is_empty()
                && judged.refused.len() == 1
                && judged.refused[0].starts_with("3.9 rust/todo: 0.723× at the base, 0.")
                && judged.refused[0].contains(decision)
                && judged.refused[0].contains("holds nothing")
                && judged.refused[0].contains("remove it"),
            "0.723× to {head} permille, a decision on the bar: {judged:?}"
        );
    }
    let others = remote::judge(
        &baseline,
        Some(&base),
        &[
            row("3.9", "llvm/todo", &[720], &[720]),
            row("3.8", "rust/todo/get-1", &[900], &[900]),
        ],
    )
    .unwrap();
    assert_eq!(
        others,
        remote::Judged::default(),
        "a size cell without a decision, and a row that is no size row with one"
    );
}

/// The release's record of a grown cell carries the decision that holds it, so the release's
/// row of `quality/trend.md` is generated from `quality/history.jsonl` alone; every other
/// record of the run has no `decision`.
#[test]
fn the_record_of_a_grown_cell_carries_its_decision() {
    let decision = "story:x (2026-11-11): the TLS client session adds the bytes";
    let rows = [
        row("3.9", "rust/todo", &[723], &[864]),
        row("3.9", "llvm/todo", &[720], &[720]),
    ];
    let judged = remote::judge(
        &size_baseline(Some(decision)),
        Some(&size_baseline(None)),
        &rows,
    )
    .unwrap();
    assert_eq!(
        judged.held,
        [remote::Held {
            metric: "3.9".to_string(),
            target: "rust/todo".to_string(),
            decision: decision.to_string(),
        }],
        "the cells a decision holds"
    );
    let machine = Json::parse(r#"{"instance_type": "c7i.large"}"#).unwrap();
    let lines = remote::history_lines_held(
        &rows,
        &machine,
        "b0",
        "h1",
        "v0.16.0",
        "2026-11-11",
        &judged.held,
    );
    assert!(
        lines[0].contains(&format!(
            r#""date":"2026-11-11","decision":"{decision}","host":"ec2:c7i.large""#
        )) && lines[0].contains(r#""target":"rust/todo""#),
        "the grown cell's record: {}",
        lines[0]
    );
    assert!(
        !lines[1].contains("decision"),
        "a cell that did not grow: {}",
        lines[1]
    );
    assert_eq!(
        remote::history_lines_held(&rows, &machine, "b0", "h1", "run", "2026-11-11", &[]),
        remote::history_lines(&rows, &machine, "b0", "h1", "run", "2026-11-11"),
        "the records of a run that holds no cell"
    );
}

/// The release path of a run judges through `remote::judge_release`: `quality/baseline.json`
/// of the working tree is the baseline, the committed one of the BASE revision says which
/// decisions are the last release's, and the texts it returns are what the run prints. A
/// decision BASE's baseline holds refuses the cell and lists nothing; one it does not hold is
/// listed; a BASE whose baseline cannot be read refuses the cell and says why; a run that is
/// no release is not judged.
#[test]
fn the_release_judgement_reads_the_bases_baseline_and_returns_what_it_prints() {
    let new = "story:x (2026-11-11): the TLS client session adds the bytes";
    let root = repository_with_baseline(
        "release-judgement",
        Some(&size_baseline(Some(HELD)).dumps(true)),
    );
    let rows = [row("3.9", "rust/todo", &[723], &[864])];
    let carried = remote::judge_release(&root, "HEAD", "v0.16.0", &rows).unwrap();
    assert!(
        carried.judged.grown.is_empty()
            && carried.judged.refused.len() == 1
            && carried.judged.refused[0].contains("is the last release's"),
        "the decision of the base, still on the bar: {carried:?}"
    );
    assert_eq!(carried.listed, "", "a refused cell is listed");
    assert!(
        carried.refusal.starts_with(
            "release v0.16.0 refused:\n  3.9 rust/todo: 0.723× at the base, 0.864× now"
        ) && carried.refusal.ends_with('\n'),
        "the refusal the run prints: {:?}",
        carried.refusal
    );

    std::fs::write(
        root.join("quality/baseline.json"),
        size_baseline(Some(new)).dumps(true),
    )
    .unwrap();
    let passed = remote::judge_release(&root, "HEAD", "v0.16.0", &rows).unwrap();
    assert!(
        passed.judged.refused.is_empty() && passed.judged.held.len() == 1,
        "a decision the base does not hold: {passed:?}"
    );
    assert_eq!(
        passed.listed,
        format!(
            "release v0.16.0: the size row above the base, each cell held by the decision on its bar:\n  3.9 rust/todo: 0.723× at the base, 0.864× now; decision: {new}\n"
        ),
        "the list the run prints"
    );
    assert_eq!((passed.note.as_str(), passed.refusal.as_str()), ("", ""));

    let unread = remote::judge_release(&root, "v0.0.0", "v0.16.0", &rows).unwrap();
    assert!(
        unread.judged.refused.len() == 1
            && unread.judged.refused[0]
                .contains("quality/baseline.json at the base could not be read")
            && unread.listed.is_empty(),
        "a base whose baseline cannot be read: {unread:?}"
    );
    assert!(
        unread.note.contains("v0.0.0:quality/baseline.json")
            && unread.note.contains("could not be read")
            && unread.note.ends_with('\n'),
        "why the base's baseline was not read: {:?}",
        unread.note
    );

    assert_eq!(
        remote::judge_release(&root, "HEAD", "run", &rows).unwrap(),
        remote::ReleaseJudgement::default(),
        "a run that is no release"
    );
    std::fs::remove_file(root.join("quality/baseline.json")).unwrap();
    assert!(
        remote::judge_release(&root, "HEAD", "v0.16.0", &rows).is_err(),
        "a repository without a baseline was judged"
    );
}
