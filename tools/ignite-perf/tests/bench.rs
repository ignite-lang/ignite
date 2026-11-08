//! The native benchmark of `quality-remote` (`docs/quality.md` row 3.3) without a host: the
//! ratios it records from `ignite-load matrix` lines and the stripped sizes, the absolute
//! records per implementation, the release refusal within one run, and the host command and
//! setup that run it.

use clap::Parser;
use ignite_perf::bench;
use ignite_perf::cli::{Cli, Mode};
use ignite_perf::json::Json;
use ignite_perf::remote::{self, Row};

/// One `ignite-load` result line of the cell `get:1`.
fn line(label: &str, pass: usize, rps: f64, p99: f64, rss: i64, errors: i64) -> String {
    format!(
        r#"{{"label":"{label}","pass":{pass},"quickack":true,"workload":"get","connections":1,"duration_s":5.000,"replies":1,"replies_per_s":{rps:.1},"reply_mb_per_s":0.1,"p50_us":{p50:.1},"p99_us":{p99:.1},"p999_us":1.0,"server_cpu_s":1.00,"cpu_ms_per_1000":1.0,"peak_rss_kib":{rss},"rss_sampled_kib":{rss},"threads_max":3,"errors":{errors},"first_error":"","server_session_errors":0,"nproc":48,"load1":1.10,"runnable_mean":0.20}}"#,
        p50 = p99 / 2.0
    )
}

/// Three passes of the cell `get:1`: the plain-Rust server at 1000 replies/s, the Ignite
/// LLVM build of HEAD at 800, 900 and 700 (ratios 0.8, 0.9, 0.7), BASE's at 1000.
fn samples() -> String {
    let mut text = String::new();
    for (pass, head) in [(1, 800.0), (2, 900.0), (3, 700.0)] {
        for (label, rps, p99, rss) in [
            ("native-rust", 1000.0, 100.0, 4000),
            ("native-go", 500.0, 300.0, 12000),
            ("head-llvm", head, 200.0, 6000),
            ("head-rust", 600.0, 150.0, 5000),
            ("base-llvm", 1000.0, 100.0, 4000),
            ("base-rust", 600.0, 150.0, 5000),
        ] {
            text.push_str(&line(label, pass, rps, p99, rss, 0));
            text.push('\n');
        }
    }
    text
}

fn sizes() -> Json {
    Json::parse(
        r#"{"native-rust/todo": 1000000, "native-go/todo": 6000000, "head-llvm/todo": 2000000, "head-rust/todo": 900000, "base-llvm/todo": 2100000, "base-rust/todo": 900000}"#,
    )
    .unwrap()
}

fn find<'a>(rows: &'a [Row], metric: &str, target: &str) -> &'a Row {
    rows.iter()
        .find(|r| r.metric == metric && r.target == target)
        .unwrap_or_else(|| panic!("no row {metric} {target} in {rows:?}"))
}

#[test]
fn the_ratios_are_ignite_over_plain_rust_per_pass() {
    let samples = bench::read_samples(&samples()).unwrap();
    let rows = bench::rows(&samples, &sizes(), 3).unwrap();
    let rps = find(&rows, "3.3", "llvm/todo/get-1");
    assert_eq!(rps.head, vec![800, 900, 700]);
    assert_eq!(rps.base, vec![1000, 1000, 1000]);
    let rust = find(&rows, "3.3", "rust/todo/get-1");
    assert_eq!(rust.head, vec![600, 600, 600]);
    let go = find(&rows, "3.3", "go/todo/get-1");
    assert_eq!(
        (go.base.clone(), go.head.clone()),
        (vec![500; 3], vec![500; 3])
    );
    assert_eq!(find(&rows, "3.7", "llvm/todo/get-1").head, vec![2000; 3]);
    assert_eq!(find(&rows, "3.8", "llvm/todo/get-1").head, vec![1500; 3]);
    let size = find(&rows, "3.9", "llvm/todo");
    assert_eq!(
        (size.base.clone(), size.head.clone()),
        (vec![2100], vec![2000])
    );
    assert_eq!(find(&rows, "3.9", "go/todo").head, vec![6000]);
    assert_eq!(rps.head_loads, vec![1.1; 3]);
}

#[test]
fn a_run_with_errors_is_left_out_and_a_missing_reference_fails() {
    let mut text = samples();
    text.push_str(&line("head-llvm", 3, 10.0, 9999.0, 6000, 4));
    text.push('\n');
    let samples = bench::read_samples(&text).unwrap();
    let rows = bench::rows(&samples, &sizes(), 3).unwrap();
    assert_eq!(
        find(&rows, "3.3", "llvm/todo/get-1").head,
        vec![800, 900, 700]
    );
    let without: String = samples_text_without("native-rust");
    let samples = bench::read_samples(&without).unwrap();
    let error = bench::rows(&samples, &sizes(), 3).unwrap_err();
    assert!(error.contains("native-rust"), "{error}");
}

fn samples_text_without(label: &str) -> String {
    samples()
        .lines()
        .filter(|l| !l.contains(&format!(r#""label":"{label}""#)))
        .map(|l| format!("{l}\n"))
        .collect()
}

#[test]
fn the_records_hold_every_implementation_absolute() {
    let samples = bench::read_samples(&samples()).unwrap();
    let records = bench::records(&samples, &sizes(), 3);
    let llvm = records
        .iter()
        .find(|r| r.field("implementation").as_str() == Some("head-llvm"))
        .unwrap();
    assert_eq!(llvm.field("program").as_str(), Some("todo"));
    assert_eq!(llvm.field("cell").as_str(), Some("get-1"));
    assert_eq!(llvm.field("rps").as_i64(), Some(800));
    assert_eq!(llvm.field("p50_us").as_i64(), Some(100));
    assert_eq!(llvm.field("p99_us").as_i64(), Some(200));
    assert_eq!(llvm.field("peak_rss_kib").as_i64(), Some(6000));
    assert_eq!(llvm.field("size_bytes").as_i64(), Some(2000000));
    assert_eq!(llvm.field("errors").as_i64(), Some(0));
    assert_eq!(records.len(), 6);
    let lines = bench::record_lines(
        &records,
        "b0",
        "h1",
        "run",
        "2026-11-08",
        "ec2:c7i.metal-24xl",
    );
    assert!(lines[0].starts_with(r#"{"base":"b0","cell":"get-1","commit":"h1","date":"2026-11-08","errors":0,"host":"ec2:c7i.metal-24xl","implementation":"#), "{}", lines[0]);
}

#[test]
fn a_release_is_refused_on_a_ratio_worse_than_the_last_release_in_the_run() {
    let baseline = Json::parse(
        r#"{"metrics": {"3.3": {"direction": "higher", "tolerance": "10%", "unit": "permille"}, "3.7": {"direction": "lower", "tolerance": "20%", "unit": "permille"}}}"#,
    )
    .unwrap();
    let row = |metric: &str, target: &str, base: i64, head: i64| Row {
        metric: metric.to_string(),
        target: target.to_string(),
        base: vec![base; 3],
        head: vec![head; 3],
        head_loads: vec![1.0; 3],
    };
    let rows = [
        row("3.3", "llvm/todo/get-1", 900, 700),
        row("3.3", "rust/todo/get-1", 900, 850),
        row("3.3", "go/todo/get-1", 500, 500),
        row("3.7", "llvm/todo/get-1", 1000, 1300),
    ];
    let refused = remote::refusals(&baseline, &rows).unwrap();
    assert_eq!(refused.len(), 2, "{refused:?}");
    assert!(
        refused[0].starts_with("3.3 llvm/todo/get-1: 0.900× at the base, 0.700× now"),
        "{refused:?}"
    );
    assert!(refused[1].starts_with("3.7 llvm/todo/get-1"), "{refused:?}");
}

#[test]
fn the_host_runs_the_benchmark_after_the_timing_rows() {
    let command = remote::host_command(3, 15, 3);
    assert!(command.contains("--bench-passes 3"), "{command}");
    let Mode::QualityHost(args) =
        Cli::try_parse_from(["ignite-perf", "quality-host", "--root", "/q"])
            .unwrap()
            .mode
    else {
        panic!("quality-host");
    };
    assert!(!args.no_bench);
    assert_eq!(args.bench.server_cpus, "2-5");
    assert_eq!(args.bench.load_cpus, "8-31");
    let Mode::QualityHost(args) =
        Cli::try_parse_from(["ignite-perf", "quality-host", "--root", "/q", "--no-bench"])
            .unwrap()
            .mode
    else {
        panic!("quality-host");
    };
    assert!(args.no_bench);
    let user_data = remote::user_data(180);
    assert!(user_data.contains("golang-go"), "{user_data}");
    assert_eq!(
        bench::CELLS,
        [
            ("terminal", "rr", 1),
            ("terminal", "rr", 64),
            ("terminal", "connect", 8),
            ("todo", "get", 1),
            ("todo", "get", 64),
            ("todo", "patch", 16),
        ]
    );
}
