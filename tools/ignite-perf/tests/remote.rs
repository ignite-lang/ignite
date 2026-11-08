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
