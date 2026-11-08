use clap::{CommandFactory, Parser};

use ignite_perf::cli::{Cli, Mode};

fn parse(words: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("ignite-perf").chain(words.iter().copied()))
}

const ROOTS: [&str; 6] = ["--sources", "/s", "--builds", "/b", "--evidence", "/e"];

#[test]
fn the_subcommands_are_those_of_the_python_tools() {
    let names: Vec<String> = Cli::command()
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    assert_eq!(
        names,
        [
            "self-check",
            "recover-baseline",
            "smoke",
            "prepare",
            "cold",
            "warm",
            "programs",
            "cli",
            "apply-cycle",
            "all",
            "summarize",
            "verify-llvm-readiness",
            "quality",
            "quality-remote"
        ]
    );
}

#[test]
fn measuring_modes_default_as_the_python_harness() {
    let Mode::Warm(args) = parse(&[&["warm"][..], &ROOTS].concat()).unwrap().mode else {
        panic!("warm");
    };
    assert_eq!(args.baseline, "origin/bench/11b-recovered");
    assert_eq!(args.candidate, "HEAD");
    assert_eq!(args.comparison, "9273349");
    assert_eq!((args.warm_pairs, args.check_pairs), (3, 15));
    assert_eq!((args.extra_warm_passes, args.cold_retries), (2, 1));
    assert_eq!(args.min_free_gib(), 20.0);
    assert!(args.repo.join("Cargo.toml").is_file());
    assert_eq!(
        args.controls().dumps(true),
        r#"{"absolute_max_load": 9, "check_pairs": 15, "cold_retries": 1, "extra_warm_passes": 2, "load_delta": 5, "warm_pairs": 3}"#
    );
    args.validate().unwrap();
}

#[test]
fn a_given_load_flag_is_recorded_as_a_float() {
    let words = [
        &["all"][..],
        &ROOTS,
        &["--load-delta", "5", "--absolute-max", "8.5"],
    ]
    .concat();
    let Mode::All(args) = parse(&words).unwrap().mode else {
        panic!("all");
    };
    assert_eq!(
        args.controls().dumps(true),
        r#"{"absolute_max_load": 8.5, "check_pairs": 15, "cold_retries": 1, "extra_warm_passes": 2, "load_delta": 5.0, "warm_pairs": 3}"#
    );
}

#[test]
fn negative_counts_parse_and_fail_validation() {
    let words = [&["cold"][..], &ROOTS, &["--cold-retries", "-1"]].concat();
    let Mode::Cold(args) = parse(&words).unwrap().mode else {
        panic!("cold");
    };
    assert_eq!(
        args.validate().unwrap_err(),
        "Retries must be nonnegative and load controls positive"
    );
    let words = [&["cli"][..], &ROOTS, &["--check-pairs", "0"]].concat();
    let Mode::Cli(args) = parse(&words).unwrap().mode else {
        panic!("cli");
    };
    assert_eq!(
        args.validate().unwrap_err(),
        "Sample counts must be positive"
    );
}

#[test]
fn the_roots_are_required() {
    assert!(parse(&["smoke", "--sources", "/s", "--builds", "/b"]).is_err());
    assert!(parse(&["recover-baseline", "--source", "/s", "--evidence", "/e"]).is_err());
}

#[test]
fn recovery_flags_default_as_the_python_harness() {
    let words = [
        "recover-baseline",
        "--source",
        "/s",
        "--manifest",
        "/m",
        "--evidence",
        "/e",
    ];
    let Mode::RecoverBaseline(args) = parse(&words).unwrap().mode else {
        panic!("recover-baseline");
    };
    assert_eq!(args.parent, "60aef38");
    assert_eq!(args.r#ref, "refs/heads/bench/11b-recovered");
    assert!(!args.publish);
}

#[test]
fn readiness_takes_the_python_verifier_flags() {
    let words = [
        "verify-llvm-readiness",
        "--cli",
        "/c",
        "--repo",
        "/r",
        "--runtime",
        "/rt",
        "--work",
        "/w",
        "--candidate",
        "abc",
        "--source-manifest",
        "/m",
    ];
    let Mode::VerifyLlvmReadiness(args) = parse(&words).unwrap().mode else {
        panic!("verify-llvm-readiness");
    };
    assert_eq!(args.timeout, 15.0);
    assert_eq!(args.candidate, "abc");
    assert!(parse(&words[..words.len() - 2]).is_err());
}
