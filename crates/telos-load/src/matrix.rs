//! `telos-load matrix`: the throughput matrix of one or more `terminald`
//! builds, as `task bench-throughput` runs it.
//!
//! ```text
//! telos-load matrix --server NAME=PATH [--server NAME=PATH ...] [--runs 3]
//!                   [--cells rr:1,rr:16,...] [--port 2451] [--duration 10]
//!                   [--warmup 1] [--stock] [--out FILE]
//! ```
//!
//! Every run is one `telos-load` process (this executable) that starts the
//! server, measures and prints one JSON line. Runs are ordered run by run,
//! cell by cell, server by server, so a drift in machine load hits every
//! server alike. Before each run the matrix waits, up to two minutes, until
//! the mean runnable-task count is at most half of `nproc`. A run that still
//! starts above that is contended; it is repeated up to twice more, and the
//! last attempt counts. Each cell is the median of its runs, every metric
//! taken separately; a cell that kept a contended run is marked `*`.
//!
//! The client sets `TCP_QUICKACK` (`--quickack`) unless `--stock` is given.

use std::fs;
use std::io::Write;
use std::process::Command;
use std::time::{Duration, Instant};

const DEFAULT_CELLS: &[(&str, usize)] = &[
    ("rr", 1),
    ("rr", 16),
    ("rr", 256),
    ("pipe", 1),
    ("pipe", 16),
    ("pipe", 256),
    ("connect", 1),
    ("connect", 16),
];

const RETRIES: usize = 2;
const QUIET_WAIT: Duration = Duration::from_secs(120);

struct Options {
    servers: Vec<(String, String)>,
    runs: usize,
    cells: Vec<(String, usize)>,
    port: u16,
    duration: String,
    warmup: String,
    stock: bool,
    out: Option<String>,
}

/// One measured run: the fields of its JSON line the tables use.
#[derive(Clone, Debug, Default)]
struct Run {
    replies_per_s: f64,
    p50_us: f64,
    p99_us: f64,
    cpu_ms_per_1000: f64,
    peak_rss_kib: f64,
    threads_max: f64,
    errors: f64,
    runnable_mean: f64,
    contended: bool,
}

fn usage(message: &str) -> ! {
    eprintln!("telos-load matrix: {message}");
    eprintln!(
        "usage: telos-load matrix --server NAME=PATH [--server NAME=PATH ...] [--runs N] [--cells rr:1,pipe:16,...] [--port P] [--duration S] [--warmup S] [--stock] [--out FILE]"
    );
    std::process::exit(2);
}

fn parse_cells(text: &str) -> Vec<(String, usize)> {
    text.split(',')
        .filter(|cell| !cell.is_empty())
        .map(|cell| {
            let (workload, connections) = cell
                .split_once(':')
                .unwrap_or_else(|| usage(&format!("cell {cell}: expected WORKLOAD:CONNECTIONS")));
            if !matches!(workload, "rr" | "pipe" | "connect") {
                usage(&format!("cell {cell}: unknown workload {workload}"));
            }
            let connections = connections
                .parse()
                .ok()
                .filter(|n| *n > 0)
                .unwrap_or_else(|| usage(&format!("cell {cell}: bad connection count")));
            (workload.to_string(), connections)
        })
        .collect()
}

fn parse_options(args: impl Iterator<Item = String>) -> Options {
    let mut options = Options {
        servers: Vec::new(),
        runs: 3,
        cells: DEFAULT_CELLS
            .iter()
            .map(|(w, n)| (w.to_string(), *n))
            .collect(),
        port: 2451,
        duration: "10".to_string(),
        warmup: "1".to_string(),
        stock: false,
        out: None,
    };
    let mut args = args;
    while let Some(flag) = args.next() {
        let mut value = || {
            args.next()
                .unwrap_or_else(|| usage(&format!("{flag} needs a value")))
        };
        match flag.as_str() {
            "--server" => {
                let spec = value();
                let (name, path) = spec
                    .split_once('=')
                    .unwrap_or_else(|| usage("--server takes NAME=PATH"));
                options.servers.push((name.to_string(), path.to_string()));
            }
            "--runs" => {
                options.runs = value()
                    .parse()
                    .ok()
                    .filter(|n| *n > 0)
                    .unwrap_or_else(|| usage("bad --runs"))
            }
            "--cells" => {
                let cells = parse_cells(&value());
                if !cells.is_empty() {
                    options.cells = cells;
                }
            }
            "--port" => options.port = value().parse().unwrap_or_else(|_| usage("bad --port")),
            "--duration" => options.duration = value(),
            "--warmup" => options.warmup = value(),
            "--stock" => options.stock = true,
            "--out" => options.out = Some(value()),
            other => usage(&format!("unknown argument {other}")),
        }
    }
    if options.servers.is_empty() {
        usage("at least one --server is needed");
    }
    options
}

/// The value of `key` in a flat JSON object written by `telos-load`.
fn field<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let pattern = format!("\"{key}\":");
    let start = json.find(&pattern)? + pattern.len();
    let rest = &json[start..];
    let end = rest.find([',', '}']).unwrap_or(rest.len());
    Some(rest[..end].trim_matches('"'))
}

fn number(json: &str, key: &str) -> f64 {
    field(json, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(f64::NAN)
}

fn runnable_now() -> f64 {
    fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|text| {
            let field = text.split_whitespace().nth(3)?;
            field.split('/').next()?.parse::<f64>().ok()
        })
        .map(|running| (running - 1.0).max(0.0))
        .unwrap_or(0.0)
}

/// Wait until the runnable-task mean over one second is at most `limit`,
/// for at most [`QUIET_WAIT`].
fn wait_for_quiet(limit: f64) {
    let started = Instant::now();
    loop {
        let mut total = 0.0;
        for _ in 0..10 {
            total += runnable_now();
            std::thread::sleep(Duration::from_millis(100));
        }
        if total / 10.0 <= limit || started.elapsed() > QUIET_WAIT {
            return;
        }
    }
}

struct Matrix {
    options: Options,
    exe: std::path::PathBuf,
    port: u16,
    limit: f64,
}

impl Matrix {
    fn next_port(&mut self) -> u16 {
        let port = self.port;
        self.port = if self.port >= self.options.port.saturating_add(500) {
            self.options.port
        } else {
            self.port + 1
        };
        port
    }

    fn run_once(&mut self, server: usize, workload: &str, connections: usize) -> Run {
        wait_for_quiet(self.limit);
        let port = self.next_port();
        let (name, path) = self.options.servers[server].clone();
        let mut command = Command::new(&self.exe);
        command
            .args(["--server", &path, "--port", &port.to_string()])
            .args(["--workload", workload])
            .args(["--connections", &connections.to_string()])
            .args(["--duration", &self.options.duration])
            .args(["--warmup", &self.options.warmup])
            .args(["--label", &name]);
        if !self.options.stock {
            command.arg("--quickack");
        }
        if let Some(out) = &self.options.out {
            command.args(["--out", out]);
        }
        let output = command
            .output()
            .unwrap_or_else(|e| usage(&format!("cannot run {}: {e}", self.exe.display())));
        let stdout = String::from_utf8_lossy(&output.stdout);
        let Some(json) = stdout.lines().find(|line| line.starts_with('{')) else {
            eprintln!(
                "telos-load matrix: {name} {workload} {connections}: no result ({})",
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return Run {
                replies_per_s: f64::NAN,
                errors: 1.0,
                ..Run::default()
            };
        };
        let runnable_mean = number(json, "runnable_mean");
        let run = Run {
            replies_per_s: number(json, "replies_per_s"),
            p50_us: number(json, "p50_us"),
            p99_us: number(json, "p99_us"),
            cpu_ms_per_1000: number(json, "cpu_ms_per_1000"),
            peak_rss_kib: number(json, "peak_rss_kib"),
            threads_max: number(json, "threads_max"),
            errors: number(json, "errors"),
            runnable_mean,
            contended: runnable_mean > self.limit,
        };
        eprintln!(
            "  {name:>6} {workload:>7} {connections:>4}  {:>10}/s  runnable {:.1}{}",
            rate(run.replies_per_s),
            run.runnable_mean,
            if run.contended { "  contended" } else { "" }
        );
        run
    }

    /// One run, repeated while it was contended, up to [`RETRIES`] times.
    fn run(&mut self, server: usize, workload: &str, connections: usize) -> Run {
        let mut run = self.run_once(server, workload, connections);
        for _ in 0..RETRIES {
            if !run.contended {
                break;
            }
            run = self.run_once(server, workload, connections);
        }
        run
    }
}

fn median(values: impl Iterator<Item = f64>) -> f64 {
    let mut values: Vec<f64> = values.filter(|v| v.is_finite()).collect();
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len() % 2 == 1 {
        values[mid]
    } else {
        (values[mid - 1] + values[mid]) / 2.0
    }
}

fn rate(value: f64) -> String {
    if !value.is_finite() {
        "-".to_string()
    } else if value >= 1e6 {
        format!("{:.2} M", value / 1e6)
    } else if value >= 1e3 {
        format!("{:.1} k", value / 1e3)
    } else {
        format!("{value:.1}")
    }
}

fn micros(value: f64) -> String {
    if !value.is_finite() {
        "-".to_string()
    } else if value >= 1000.0 {
        format!("{:.2} ms", value / 1000.0)
    } else {
        format!("{value:.0} µs")
    }
}

/// One table per workload: a row per connection count and server.
fn tables(options: &Options, results: &[Vec<Vec<Run>>]) -> String {
    let mut out = String::new();
    let mut workloads: Vec<&str> = Vec::new();
    for (workload, _) in &options.cells {
        if !workloads.contains(&workload.as_str()) {
            workloads.push(workload);
        }
    }
    for workload in workloads {
        out.push_str(&format!(
            "\n{workload} (median of {}{})\n",
            options.runs,
            if options.stock {
                ", stock client"
            } else {
                ", quickack client"
            }
        ));
        out.push_str("| conns | build | replies/s | p50 | p99 | CPU ms/1000 | RSS MiB | threads | vs first |\n");
        out.push_str("|---:|---|---:|---:|---:|---:|---:|---:|---:|\n");
        for (cell, (w, connections)) in options.cells.iter().enumerate() {
            if w != workload {
                continue;
            }
            let first = median(results[cell][0].iter().map(|r| r.replies_per_s));
            for (server, (name, _)) in options.servers.iter().enumerate() {
                let runs = &results[cell][server];
                let m = |f: fn(&Run) -> f64| median(runs.iter().map(f));
                let replies = m(|r| r.replies_per_s);
                let marks = format!(
                    "{}{}",
                    if runs.iter().any(|r| r.contended) {
                        "*"
                    } else {
                        ""
                    },
                    if runs.iter().any(|r| r.errors > 0.0) {
                        " !"
                    } else {
                        ""
                    }
                );
                out.push_str(&format!(
                    "| {connections} | {name}{marks} | {} | {} | {} | {:.2} | {:.1} | {:.0} | {:.2}× |\n",
                    rate(replies),
                    micros(m(|r| r.p50_us)),
                    micros(m(|r| r.p99_us)),
                    m(|r| r.cpu_ms_per_1000),
                    m(|r| r.peak_rss_kib) / 1024.0,
                    m(|r| r.threads_max),
                    replies / first,
                ));
            }
        }
    }
    out.push_str(
        "\n`*` a run started with more runnable tasks than nproc/2; `!` a run had client errors (server session errors, such as a reset when a pipe client hangs up with replies in flight, are only in the JSON lines).\n",
    );
    out
}

pub fn main(args: impl Iterator<Item = String>) {
    let options = parse_options(args);
    let exe = std::env::current_exe().unwrap_or_else(|e| usage(&format!("own path: {e}")));
    let nproc = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let port = options.port;
    let mut matrix = Matrix {
        options,
        exe,
        port,
        limit: nproc as f64 / 2.0,
    };
    let cells = matrix.options.cells.clone();
    let servers = matrix.options.servers.len();
    let mut results: Vec<Vec<Vec<Run>>> = vec![vec![Vec::new(); servers]; cells.len()];
    for round in 1..=matrix.options.runs {
        eprintln!("run {round} of {}", matrix.options.runs);
        for (cell, (workload, connections)) in cells.iter().enumerate() {
            for (server, runs) in results[cell].iter_mut().enumerate() {
                runs.push(matrix.run(server, workload, *connections));
            }
        }
    }
    let text = tables(&matrix.options, &results);
    print!("{text}");
    let _ = std::io::stdout().flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fields_of_a_result_line() {
        let json =
            "{\"label\":\"llvm\",\"replies_per_s\":1234.5,\"errors\":0,\"first_error\":\"\"}";
        assert_eq!(field(json, "label"), Some("llvm"));
        assert_eq!(number(json, "replies_per_s"), 1234.5);
        assert_eq!(number(json, "errors"), 0.0);
        assert!(number(json, "missing").is_nan());
    }

    #[test]
    fn median_of_odd_and_even_counts() {
        assert_eq!(median([3.0, 1.0, 2.0].into_iter()), 2.0);
        assert_eq!(median([4.0, 1.0, 2.0, 3.0].into_iter()), 2.5);
        assert!(median([f64::NAN].into_iter()).is_nan());
    }

    #[test]
    fn cells_parse() {
        assert_eq!(
            parse_cells("rr:16,pipe:256"),
            vec![("rr".to_string(), 16), ("pipe".to_string(), 256)]
        );
    }
}
