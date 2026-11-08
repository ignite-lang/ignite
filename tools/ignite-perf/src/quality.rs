//! Mode `quality` (`docs/quality.md`, `task quality`): the timing and size rows of the quality
//! metrics, measured once on the working tree with its built compiler, each with the 1-minute
//! load average at its start. Unlike the comparison modes it exports no tree and compares
//! nothing: it writes `{"values": [{metric, target, value, load}]}` for `quality::record` in
//! `crates/ignite/tests/quality.rs`, which holds the values against `quality/baseline.json`
//! and appends them to `quality/history.jsonl`.
//!
//! Rows: 6.1 the median of `--check-runs` runs of `ignite check` and of `ignite plan` of the
//! terminal design (`all/check`, `all/plan`, ms); 6.2 the median of `--build-runs` dev builds
//! of the todo example per target, a fresh output (and, for Rust, a fresh cargo target) each
//! (`rust`, `llvm`, ms); 3.2 the size of `todod` and `terminald` per target and profile
//! (`<target>/<executable>/<profile>`, bytes); 5.3 the real-clock restart of
//! `verify-llvm-readiness` (`llvm`, ms). `--timing-only` leaves out 3.2.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use clap::Args;

use crate::cli::{ReadinessArgs, default_repo};
use crate::json::{Json, obj};
use crate::perf::{R, ensure, io_err, loadavg, path_str, sha256};

#[derive(Args, Debug, Clone)]
pub struct QualityArgs {
    #[arg(long, default_value_os_t = default_repo())]
    pub repo: PathBuf,
    /// The compiler to time; default `<repo>/target/debug/ignite`.
    #[arg(long)]
    pub cli: Option<PathBuf>,
    /// The runtime archive with the test runner the LLVM builds and `ignite run` link.
    #[arg(long)]
    pub runtime: PathBuf,
    /// A directory outside the checkout this mode owns: build outputs, readiness state.
    #[arg(long)]
    pub work: PathBuf,
    /// The file the values are written to.
    #[arg(long)]
    pub out: PathBuf,
    #[arg(long, default_value_t = 15)]
    pub check_runs: usize,
    #[arg(long, default_value_t = 3)]
    pub build_runs: usize,
    /// Only the timing rows (5.3, 6.1, 6.2): leave out the sizes (3.2), as `quality-remote`
    /// measures on the dedicated host.
    #[arg(long)]
    pub timing_only: bool,
}

/// One measured value.
struct Measured {
    metric: &'static str,
    target: String,
    value: i64,
    load: f64,
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

/// Run `command` in `cwd`; its wall time in ms and its stdout, or an error with its output.
fn timed(command: &[String], cwd: &Path) -> R<(f64, String)> {
    let started = Instant::now();
    let output = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(cwd)
        .env("CARGO_INCREMENTAL", "0")
        .output()
        .map_err(io_err(&command[0]))?;
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    ensure(
        output.status.success(),
        format!(
            "{} failed: {}\n{}",
            command.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    Ok((ms, String::from_utf8_lossy(&output.stdout).into_owned()))
}

/// The executables `ignite build` reports (`built <path>` lines).
fn built(stdout: &str) -> Vec<PathBuf> {
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("built "))
        .map(PathBuf::from)
        .collect()
}

fn size_of(paths: &[PathBuf], name: &str) -> R<i64> {
    let path = paths
        .iter()
        .find(|p| p.file_name().is_some_and(|n| n == name))
        .ok_or_else(|| format!("no {name} in {paths:?}"))?;
    Ok(fs::metadata(path).map_err(io_err(path.display()))?.len() as i64)
}

impl QualityArgs {
    fn cli(&self) -> PathBuf {
        self.cli
            .clone()
            .unwrap_or_else(|| self.repo.join("target/debug/ignite"))
    }

    /// The `ignite build` command of `source` on `target` into `out`.
    fn build(&self, source: &str, target: &str, release: bool, out: &Path) -> Vec<String> {
        let mut command = vec![
            path_str(&self.cli()),
            "build".to_string(),
            path_str(&self.repo.join(source)),
            "--target".to_string(),
            target.to_string(),
            "--out".to_string(),
            path_str(out),
        ];
        if target == "llvm" {
            command.extend(["--runtime".to_string(), path_str(&self.runtime)]);
        } else {
            command.extend(["--target-dir".to_string(), path_str(&out.join("target"))]);
        }
        if release {
            command.push("--release".to_string());
        }
        command
    }
}

/// 6.1: check and plan of the design file.
fn cli_rows(args: &QualityArgs, out: &mut Vec<Measured>) -> R<()> {
    let design = path_str(&args.repo.join("examples/terminal/terminal.design.ignite"));
    let plan_out = path_str(&args.work.join("plan.json"));
    let cli = path_str(&args.cli());
    for (name, command) in [
        (
            "all/check",
            vec![cli.clone(), "check".into(), design.clone()],
        ),
        (
            "all/plan",
            vec![
                cli.clone(),
                "plan".into(),
                design.clone(),
                "--out".into(),
                plan_out,
            ],
        ),
    ] {
        let load = loadavg()?;
        let mut samples = Vec::new();
        for _ in 0..args.check_runs {
            samples.push(timed(&command, &args.repo)?.0);
        }
        out.push(Measured {
            metric: "6.1",
            target: name.to_string(),
            value: median(samples).round() as i64,
            load,
        });
    }
    Ok(())
}

/// 6.2 and 3.2: the todo's dev builds per target, then the sizes of both examples.
fn program_rows(args: &QualityArgs, out: &mut Vec<Measured>) -> R<()> {
    for target in ["llvm", "rust"] {
        let load = loadavg()?;
        let mut samples = Vec::new();
        for sample in 0..args.build_runs {
            let dir = args.work.join(format!("todo-{target}-{sample}"));
            let _ = fs::remove_dir_all(&dir);
            let command = args.build("examples/todo/todo.ignite", target, false, &dir);
            samples.push(timed(&command, &args.repo)?.0);
            let _ = fs::remove_dir_all(&dir);
        }
        out.push(Measured {
            metric: "6.2",
            target: target.to_string(),
            value: median(samples).round() as i64,
            load,
        });
    }
    if args.timing_only {
        return Ok(());
    }
    for (source, executable) in [
        ("examples/todo/todo.ignite", "todod"),
        ("examples/terminal/terminal.ignite", "terminald"),
    ] {
        for target in ["llvm", "rust"] {
            for (profile, release) in [("dev", false), ("release", true)] {
                let load = loadavg()?;
                let dir = args
                    .work
                    .join(format!("size-{executable}-{target}-{profile}"));
                let _ = fs::remove_dir_all(&dir);
                let (_, stdout) = timed(&args.build(source, target, release, &dir), &args.repo)?;
                let value = size_of(&built(&stdout), executable)?;
                let _ = fs::remove_dir_all(&dir);
                out.push(Measured {
                    metric: "3.2",
                    target: format!("{target}/{executable}/{profile}"),
                    value,
                    load,
                });
            }
        }
    }
    Ok(())
}

/// The delay of `readiness::real_scenario`'s refusal `Real restart N ns
/// outside 100–500 ms`.
pub fn outside_bound(message: &str) -> Option<i64> {
    message
        .split_once("Real restart ")?
        .1
        .split_once(" ns outside ")?
        .0
        .parse()
        .ok()
}

/// 5.3: `verify-llvm-readiness` over the working tree, its real-clock restart.
fn readiness_row(args: &QualityArgs, out: &mut Vec<Measured>) -> R<()> {
    let work = args.work.join("readiness");
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&args.work).map_err(io_err(args.work.display()))?;
    let design = "examples/terminal/terminal.design.ignite";
    let bytes = fs::read(args.repo.join(design)).map_err(io_err(design))?;
    let manifest = args.work.join("readiness-manifest.json");
    fs::write(
        &manifest,
        obj(vec![(design, Json::from(sha256(&bytes)))]).dumps(true),
    )
    .map_err(io_err(manifest.display()))?;
    let load = loadavg()?;
    let verified = crate::readiness::verify(&ReadinessArgs {
        cli: args.cli(),
        repo: args.repo.clone(),
        runtime: args.runtime.clone(),
        work: work.clone(),
        candidate: "working tree".to_string(),
        source_manifest: manifest,
        timeout: 15.0,
    });
    let ns = match verified {
        Ok(_) => {
            let report = crate::perf::read_json(&work.join("report.json"))?;
            report
                .field("scenarios")
                .field("real")
                .field("real_restart_ns")
                .as_i64()
                .ok_or("readiness: no real_restart_ns")?
        }
        // A restart outside the readiness bound of 100 to 500 ms is still a
        // measured restart: 5.3 is a timing row on the shared workstation,
        // recorded and gating nothing (docs/quality.md), so the value is
        // kept and the run goes on. Every other readiness failure stops it.
        Err(error) => match outside_bound(&error.to_string()) {
            Some(ns) => {
                eprintln!("5.3: {error} (load {load:.2}); recorded, not gated");
                ns
            }
            None => return Err(error),
        },
    };
    out.push(Measured {
        metric: "5.3",
        target: "llvm".to_string(),
        value: (ns as f64 / 1e6).round() as i64,
        load,
    });
    Ok(())
}

/// Measure every row and write them to `args.out`.
pub fn measure(args: &QualityArgs) -> R<()> {
    ensure(
        !args.work.starts_with(&args.repo),
        "--work must be outside the checkout",
    )?;
    fs::create_dir_all(&args.work).map_err(io_err(args.work.display()))?;
    let mut rows = Vec::new();
    cli_rows(args, &mut rows)?;
    program_rows(args, &mut rows)?;
    readiness_row(args, &mut rows)?;
    let values = rows
        .iter()
        .map(|m| {
            obj(vec![
                ("load", Json::from(format!("{:.2}", m.load))),
                ("metric", Json::from(m.metric)),
                ("target", Json::from(m.target.as_str())),
                ("value", Json::Int(m.value)),
            ])
        })
        .collect();
    let text = obj(vec![("values", Json::Arr(values))]).dumps_indent(true, 2) + "\n";
    if let Some(dir) = args.out.parent() {
        fs::create_dir_all(dir).map_err(io_err(dir.display()))?;
    }
    fs::write(&args.out, text).map_err(io_err(args.out.display()))?;
    for m in &rows {
        println!("{} {} {} (load {:.2})", m.metric, m.target, m.value, m.load);
    }
    Ok(())
}
