//! The native benchmark of `quality-remote` (`docs/quality.md` row 3.3): on the dedicated
//! host, after the timing rows, the todo and terminal programs of BASE and HEAD (both
//! targets, release) and their references in plain Rust and Go (`bench/native/` of the
//! harness tree) are built, stripped, functionally checked and driven by `ignite-load matrix`
//! with one workload matrix ([`CELLS`]), server and load generator pinned to disjoint CPUs,
//! `passes` passes with the servers in ABBA order.
//!
//! Each Ignite build's value is a ratio to the plain-Rust server measured in the same pass,
//! in permille: requests per second (3.3, higher is better), p99 latency (3.7), peak RSS
//! (3.8) and the stripped binary size (3.9, lower is better), the median over the passes.
//! Go's ratio to Rust is recorded beside them as a reference (target `go/...`). The rows go
//! through the same records and release refusal as the timing rows ([`crate::remote`]), so
//! HEAD's ratio is compared with BASE's from the same run. The absolute medians per cell and
//! implementation go to `quality/bench.jsonl` ([`records`]).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Args;

use crate::json::{Json, obj};
use crate::perf::{R, ensure, io_err, path_str};
use crate::remote::Row;

/// The workload matrix: program, `ignite-load` workload, connections.
pub const CELLS: [(&str, &str, usize); 6] = [
    ("terminal", "rr", 1),
    ("terminal", "rr", 64),
    ("terminal", "connect", 8),
    ("todo", "get", 1),
    ("todo", "get", 64),
    ("todo", "patch", 16),
];

/// The servers of a run in matrix order: the Ignite builds of BASE and HEAD per target, then
/// the references. `native-rust` is the reference every ratio divides by.
pub const IMPLEMENTATIONS: [&str; 6] = [
    "base-llvm",
    "base-rust",
    "head-llvm",
    "head-rust",
    "native-rust",
    "native-go",
];

const REFERENCE: &str = "native-rust";

#[derive(Args, Debug, Clone)]
pub struct BenchArgs {
    /// The CPUs every benchmarked server runs on.
    #[arg(long, default_value = "2-5")]
    pub server_cpus: String,
    /// The CPUs of the load generator, disjoint from `--server-cpus`.
    #[arg(long, default_value = "8-31")]
    pub load_cpus: String,
    /// Measured seconds of one run.
    #[arg(long, default_value = "5")]
    pub bench_duration: String,
    /// Seconds of warm-up before each run's measurement.
    #[arg(long, default_value = "1")]
    pub bench_warmup: String,
    /// Passes of the matrix, servers in ABBA order.
    #[arg(long, default_value_t = 5)]
    pub bench_passes: usize,
}

/// One `ignite-load` result line.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub label: String,
    pub pass: usize,
    pub workload: String,
    pub connections: usize,
    pub rps: f64,
    pub p50_us: f64,
    pub p99_us: f64,
    pub peak_rss_kib: f64,
    pub errors: i64,
    pub load: f64,
}

/// The lines of `ignite-load matrix --out`.
pub fn read_samples(text: &str) -> R<Vec<Sample>> {
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let json = Json::parse(line).map_err(|e| format!("{line}: {e}"))?;
        let number = |key: &str| {
            json.field(key)
                .as_f64()
                .ok_or_else(|| format!("{line}: no number `{key}`"))
        };
        out.push(Sample {
            label: json.field("label").as_str().unwrap_or("").to_string(),
            pass: number("pass")? as usize,
            workload: json.field("workload").as_str().unwrap_or("").to_string(),
            connections: number("connections")? as usize,
            rps: number("replies_per_s")?,
            p50_us: number("p50_us")?,
            p99_us: number("p99_us")?,
            peak_rss_kib: number("peak_rss_kib")?,
            errors: number("errors")? as i64,
            load: number("load1")?,
        });
    }
    Ok(out)
}

fn cell_name(workload: &str, connections: usize) -> String {
    format!("{workload}-{connections}")
}

/// The sample of `label` in `pass` of a cell, when it ran without an error.
fn sample<'a>(
    samples: &'a [Sample],
    label: &str,
    pass: usize,
    cell: (&str, usize),
) -> Option<&'a Sample> {
    samples.iter().rev().find(|s| {
        s.label == label
            && s.pass == pass
            && s.workload == cell.0
            && s.connections == cell.1
            && s.errors == 0
    })
}

fn permille(value: f64, reference: f64) -> i64 {
    (value / reference * 1000.0).round() as i64
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    if n == 0 {
        f64::NAN
    } else if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

/// The consumer of a target (`llvm`, `rust` or `go`) and the implementation it reads in each
/// of the two trees.
const SERIES: [(&str, &str, &str); 3] = [
    ("llvm", "base-llvm", "head-llvm"),
    ("rust", "base-rust", "head-rust"),
    ("go", "native-go", "native-go"),
];

/// The ratio rows of a run: 3.3, 3.7 and 3.8 per cell, 3.9 per program. A pass in which an
/// implementation or the reference has no clean run is left out of that ratio; a row with no
/// pass left is an error.
pub fn rows(samples: &[Sample], sizes: &Json, passes: usize) -> R<Vec<Row>> {
    let mut rows = Vec::new();
    for (program, workload, connections) in CELLS {
        if !samples
            .iter()
            .any(|s| s.workload == workload && s.connections == connections)
        {
            continue;
        }
        let cell = (workload, connections);
        let name = cell_name(workload, connections);
        type Read = fn(&Sample) -> f64;
        let metrics: [(&str, Read); 3] = [
            ("3.3", |s| s.rps),
            ("3.7", |s| s.p99_us),
            ("3.8", |s| s.peak_rss_kib),
        ];
        for (metric, read) in metrics {
            for (consumer, base_label, head_label) in SERIES {
                let mut row = Row {
                    metric: metric.to_string(),
                    target: format!("{consumer}/{program}/{name}"),
                    base: Vec::new(),
                    head: Vec::new(),
                    head_loads: Vec::new(),
                };
                for pass in 1..=passes {
                    let Some(reference) = sample(samples, REFERENCE, pass, cell) else {
                        continue;
                    };
                    if let Some(base) = sample(samples, base_label, pass, cell) {
                        row.base.push(permille(read(base), read(reference)));
                    }
                    if let Some(head) = sample(samples, head_label, pass, cell) {
                        row.head.push(permille(read(head), read(reference)));
                        row.head_loads.push(head.load);
                    }
                }
                ensure(
                    !row.base.is_empty() && !row.head.is_empty(),
                    format!(
                        "{metric} {}: no pass with clean runs of {base_label}, {head_label} and {REFERENCE}",
                        row.target
                    ),
                )?;
                rows.push(row);
            }
        }
    }
    let mut programs: Vec<&str> = CELLS.iter().map(|c| c.0).collect();
    programs.dedup();
    for program in programs {
        let size = |label: &str| {
            sizes
                .field(&format!("{label}/{program}"))
                .as_i64()
                .ok_or_else(|| format!("no size of {label}/{program}"))
        };
        if sizes.get(&format!("{REFERENCE}/{program}")).is_none() {
            continue;
        }
        let reference = size(REFERENCE)? as f64;
        for (consumer, base_label, head_label) in SERIES {
            rows.push(Row {
                metric: "3.9".to_string(),
                target: format!("{consumer}/{program}"),
                base: vec![permille(size(base_label)? as f64, reference)],
                head: vec![permille(size(head_label)? as f64, reference)],
                head_loads: vec![0.0],
            });
        }
    }
    Ok(rows)
}

/// The absolute medians of a run per cell and implementation: requests per second, p50 and
/// p99 in µs, peak RSS in KiB, the stripped size in bytes, the errors over every pass.
pub fn records(samples: &[Sample], sizes: &Json, passes: usize) -> Vec<Json> {
    let mut out = Vec::new();
    for (program, workload, connections) in CELLS {
        for label in IMPLEMENTATIONS {
            let runs: Vec<&Sample> = (1..=passes)
                .filter_map(|pass| sample(samples, label, pass, (workload, connections)))
                .collect();
            let errors: i64 = samples
                .iter()
                .filter(|s| {
                    s.label == label && s.workload == workload && s.connections == connections
                })
                .map(|s| s.errors)
                .sum();
            if runs.is_empty() && errors == 0 {
                continue;
            }
            let value = |read: fn(&Sample) -> f64| {
                let values: Vec<f64> = runs.iter().map(|s| read(s)).collect();
                let m = median(&values);
                if m.is_finite() {
                    Json::Int(m.round() as i64)
                } else {
                    Json::Null
                }
            };
            out.push(obj(vec![
                ("program", Json::from(program)),
                ("cell", Json::from(cell_name(workload, connections))),
                ("implementation", Json::from(label)),
                ("rps", value(|s| s.rps)),
                ("p50_us", value(|s| s.p50_us)),
                ("p99_us", value(|s| s.p99_us)),
                ("peak_rss_kib", value(|s| s.peak_rss_kib)),
                (
                    "size_bytes",
                    sizes
                        .get(&format!("{label}/{program}"))
                        .cloned()
                        .unwrap_or(Json::Null),
                ),
                ("errors", Json::Int(errors)),
                ("passes", Json::Int(runs.len() as i64)),
            ]));
        }
    }
    out
}

/// One `quality/bench.jsonl` line per record, keys sorted.
pub fn record_lines(
    records: &[Json],
    base: &str,
    head: &str,
    run: &str,
    date: &str,
    host: &str,
) -> Vec<String> {
    records
        .iter()
        .map(|record| {
            let mut fields: Vec<(String, Json)> = record.as_obj().to_vec();
            fields.extend([
                ("base".to_string(), Json::from(base)),
                ("commit".to_string(), Json::from(head)),
                ("date".to_string(), Json::from(date)),
                ("host".to_string(), Json::from(host)),
                ("run".to_string(), Json::from(run)),
            ]);
            crate::remote::compact(&Json::Obj(fields))
        })
        .collect()
}

fn run(dir: &Path, argv: &[&str]) -> R<String> {
    let output = Command::new(argv[0])
        .args(&argv[1..])
        .current_dir(dir)
        .env("CARGO_INCREMENTAL", "0")
        .output()
        .map_err(io_err(argv[0]))?;
    ensure(
        output.status.success(),
        format!(
            "{} in {} failed: {}{}",
            argv.join(" "),
            dir.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The executable `name` among the `built <path>` lines of `ignite build`.
fn built(stdout: &str, name: &str) -> R<PathBuf> {
    stdout
        .lines()
        .filter_map(|l| l.strip_prefix("built "))
        .map(PathBuf::from)
        .find(|p| p.file_name().is_some_and(|n| n == name))
        .ok_or_else(|| format!("no {name} in {stdout:?}"))
}

/// The host part, after the timing rows: build, strip, check, then both matrices into
/// `out/bench.jsonl`, the sizes into `out/bench-sizes.json` and each matrix's table into
/// `out/bench-<program>.md`.
pub fn host(root: &Path, out: &Path, args: &BenchArgs) -> R<()> {
    let work = root.join("work/bench");
    let _ = fs::remove_dir_all(&work);
    let bin = work.join("bin");
    fs::create_dir_all(&bin).map_err(io_err(bin.display()))?;
    let harness = root.join("harness");
    run(
        &harness,
        &[
            "cargo",
            "build",
            "-q",
            "--release",
            "--locked",
            "-p",
            "ignite-load",
        ],
    )?;
    let load = harness.join("target/release/ignite-load");
    let mut binaries: Vec<(String, &str, PathBuf)> = Vec::new();
    for tree in ["base", "head"] {
        let dir = root.join(tree);
        run(
            &dir,
            &[
                "cargo",
                "build",
                "-q",
                "--release",
                "--locked",
                "-p",
                "ignite-rt",
                "--target-dir",
                "target/bench-rt",
            ],
        )?;
        let runtime = path_str(&dir.join("target/bench-rt/release/libignite_rt.a"));
        let cli = path_str(&dir.join("target/debug/ignite"));
        for (program, source, executable) in [
            ("todo", "examples/todo/todo.ignite", "todod"),
            ("terminal", "examples/terminal/terminal.ignite", "terminald"),
        ] {
            let llvm_out = path_str(&work.join(format!("{tree}-llvm-{program}")));
            let stdout = run(
                &dir,
                &[
                    &cli,
                    "build",
                    source,
                    "--target",
                    "llvm",
                    "--release",
                    "--runtime",
                    &runtime,
                    "--out",
                    &llvm_out,
                ],
            )?;
            binaries.push((format!("{tree}-llvm"), program, built(&stdout, executable)?));
            let rust_out = path_str(&work.join(format!("{tree}-rust-{program}")));
            let target_dir = path_str(&work.join(format!("{tree}-rust-target")));
            let stdout = run(
                &dir,
                &[
                    &cli,
                    "build",
                    source,
                    "--target",
                    "rust",
                    "--release",
                    "--out",
                    &rust_out,
                    "--target-dir",
                    &target_dir,
                ],
            )?;
            binaries.push((format!("{tree}-rust"), program, built(&stdout, executable)?));
        }
    }
    let native = harness.join("bench/native");
    run(
        &native.join("rust"),
        &["cargo", "build", "-q", "--release", "--locked"],
    )?;
    for program in ["todo", "terminal"] {
        binaries.push((
            "native-rust".to_string(),
            program,
            native.join("rust/target/release").join(program),
        ));
        let go_out = work.join(format!("go-{program}"));
        run(
            &native.join("go"),
            &[
                "go",
                "build",
                "-trimpath",
                "-o",
                &path_str(&go_out),
                &format!("./{program}"),
            ],
        )?;
        binaries.push(("native-go".to_string(), program, go_out));
    }
    let mut sizes = Vec::new();
    let mut servers: Vec<(String, &str, PathBuf)> = Vec::new();
    for (label, program, path) in binaries {
        let stripped = bin.join(format!("{label}-{program}"));
        fs::copy(&path, &stripped).map_err(io_err(path.display()))?;
        run(&bin, &["strip", &path_str(&stripped)])?;
        let size = fs::metadata(&stripped)
            .map_err(io_err(stripped.display()))?
            .len();
        println!("bench: {label}/{program} {size} bytes stripped");
        sizes.push((format!("{label}/{program}"), Json::Int(size as i64)));
        servers.push((label, program, stripped));
    }
    crate::perf::dump(&out.join("bench-sizes.json"), &obj(sizes))?;
    for (index, (label, program, path)) in servers.iter().enumerate() {
        let port = (2900 + index).to_string();
        let checked = run(
            &bin,
            &[
                &path_str(&load),
                "check",
                "--program",
                program,
                "--server",
                &path_str(path),
                "--port",
                &port,
                "--workdir",
                &path_str(&work.join(format!("check-{label}-{program}"))),
            ],
        )
        .map_err(|e| format!("{label}/{program} fails the functional check: {e}"))?;
        print!("bench: {checked}");
    }
    for (program, port) in [("terminal", "2451"), ("todo", "3451")] {
        let cells: Vec<String> = CELLS
            .iter()
            .filter(|c| c.0 == program)
            .map(|c| format!("{}:{}", c.1, c.2))
            .collect();
        let mut argv: Vec<String> = vec![path_str(&load), "matrix".to_string()];
        for label in IMPLEMENTATIONS {
            let (_, _, path) = servers
                .iter()
                .find(|(l, p, _)| l == label && *p == program)
                .ok_or_else(|| format!("no {label}/{program}"))?;
            argv.extend([
                "--server".to_string(),
                format!("{label}={}", path_str(path)),
            ]);
        }
        for (flag, value) in [
            ("--runs", args.bench_passes.to_string()),
            ("--cells", cells.join(",")),
            ("--port", port.to_string()),
            ("--duration", args.bench_duration.clone()),
            ("--warmup", args.bench_warmup.clone()),
            ("--out", path_str(&out.join("bench.jsonl"))),
            ("--work", path_str(&work.join("runs"))),
            ("--server-cpus", args.server_cpus.clone()),
            ("--load-cpus", args.load_cpus.clone()),
        ] {
            argv.extend([flag.to_string(), value]);
        }
        println!("bench: {program} matrix, {} passes", args.bench_passes);
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        let table = run(&bin, &words)?;
        print!("{table}");
        fs::write(out.join(format!("bench-{program}.md")), table).map_err(io_err(out.display()))?;
    }
    crate::perf::dump(
        &out.join("bench-settings.json"),
        &obj(vec![
            ("server_cpus", Json::from(args.server_cpus.as_str())),
            ("load_cpus", Json::from(args.load_cpus.as_str())),
            ("duration_s", Json::from(args.bench_duration.as_str())),
            ("warmup_s", Json::from(args.bench_warmup.as_str())),
            ("passes", Json::Int(args.bench_passes as i64)),
        ]),
    )?;
    Ok(())
}
