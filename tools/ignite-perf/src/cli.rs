//! Command line of `ignite-perf`: the subcommands and flags of the former
//! `scripts/performance.py`, plus `verify-llvm-readiness` with the flags of the former
//! `scripts/verify-llvm-readiness.py`.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::json::Json;

#[derive(Parser, Debug)]
#[command(
    name = "ignite-perf",
    about = "Reproduce the frozen 11b performance comparison without editing a checkout",
    infer_long_args = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub mode: Mode,
}

#[derive(Subcommand, Debug)]
pub enum Mode {
    /// Check the comparison rules on fixture records
    SelfCheck,
    /// Commit a retained source export as the benchmark ref through a private index
    RecoverBaseline(RecoverArgs),
    /// Build each compiler and the todo program; compare two plans for determinism
    Smoke(RunArgs),
    /// Warm the complete gate and build the runner runtime
    Prepare(RunArgs),
    /// Clean the owned targets and run the gate with SCCACHE_RECACHE=1
    Cold(RunArgs),
    /// Interleaved warm gate, no-run and file units
    Warm(RunArgs),
    /// Build the todo program on both targets and profiles
    Programs(RunArgs),
    /// Time check and plan of the design file
    Cli(RunArgs),
    /// Time the edit-build-restart cycle under ignite host and ignite apply
    ApplyCycle(RunArgs),
    /// smoke, cold, warm, programs and cli
    All(RunArgs),
    /// Write summary.json and summary.md from the records
    Summarize(RunArgs),
    /// Exercise frozen LLVM run/replay without changing benchmark source files
    VerifyLlvmReadiness(ReadinessArgs),
    /// Measure the timing and size rows of docs/quality.md once on the working tree
    Quality(crate::quality::QualityArgs),
    /// Measure the timing rows of docs/quality.md on a single-tenant EC2 host made for the run
    QualityRemote(crate::remote::RemoteArgs),
    /// The part of quality-remote that runs on the EC2 host
    QualityHost(crate::remote::HostArgs),
    /// Build the LLVM target's release runtime archive as every release build of it is built; print its path
    Runtime(RuntimeArgs),
}

#[derive(Args, Debug, Clone)]
pub struct RuntimeArgs {
    /// The tree whose runtime is built
    #[arg(long, default_value_os_t = default_repo())]
    pub repo: PathBuf,
    /// The cargo target directory
    #[arg(long, default_value = "target/bench-rt")]
    pub target_dir: PathBuf,
}

/// The checkout that built this binary, as `Path(__file__).resolve().parents[1]` was.
pub fn default_repo() -> PathBuf {
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    path.canonicalize().unwrap_or(path)
}

#[derive(Args, Debug, Clone)]
pub struct RecoverArgs {
    #[arg(long, default_value_os_t = default_repo())]
    pub repo: PathBuf,
    #[arg(long)]
    pub source: PathBuf,
    #[arg(long)]
    pub manifest: PathBuf,
    #[arg(long, default_value = "60aef38")]
    pub parent: String,
    #[arg(long, default_value = "refs/heads/bench/11b-recovered")]
    pub r#ref: String,
    #[arg(long)]
    pub evidence: PathBuf,
    #[arg(long)]
    pub publish: bool,
}

#[derive(Args, Debug, Clone)]
pub struct RunArgs {
    #[arg(long, default_value_os_t = default_repo())]
    pub repo: PathBuf,
    #[arg(long, default_value = "origin/bench/11b-recovered")]
    pub baseline: String,
    #[arg(long, default_value = "HEAD")]
    pub candidate: String,
    #[arg(long, default_value = "9273349")]
    pub comparison: String,
    #[arg(long)]
    pub sources: PathBuf,
    #[arg(long)]
    pub builds: PathBuf,
    #[arg(long)]
    pub evidence: PathBuf,
    #[arg(long, default_value_t = 3, allow_negative_numbers = true)]
    pub warm_pairs: i64,
    #[arg(long, default_value_t = 15, allow_negative_numbers = true)]
    pub check_pairs: i64,
    #[arg(long, default_value_t = 2, allow_negative_numbers = true)]
    pub extra_warm_passes: i64,
    #[arg(long, default_value_t = 1, allow_negative_numbers = true)]
    pub cold_retries: i64,
    #[arg(long, allow_negative_numbers = true)]
    pub load_delta: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    pub absolute_max_load: Option<f64>,
    #[arg(long, allow_negative_numbers = true)]
    pub min_free_gib: Option<f64>,
}

impl RunArgs {
    /// argparse leaves a non-string default untyped, so the Python harness recorded the
    /// defaults as the integers 5 and 9 and a given flag as a float; `run.json` keeps that.
    pub fn load_delta_json(&self) -> Json {
        self.load_delta.map_or(Json::Int(5), Json::Float)
    }

    pub fn absolute_max_load_json(&self) -> Json {
        self.absolute_max_load.map_or(Json::Int(9), Json::Float)
    }

    pub fn load_delta(&self) -> f64 {
        self.load_delta.unwrap_or(5.0)
    }

    pub fn absolute_max_load(&self) -> f64 {
        self.absolute_max_load.unwrap_or(9.0)
    }

    pub fn min_free_gib(&self) -> f64 {
        self.min_free_gib.unwrap_or(20.0)
    }

    /// The `controls` object of `run.json`, in the key order of the Python harness.
    pub fn controls(&self) -> Json {
        crate::json::obj(vec![
            ("warm_pairs", Json::Int(self.warm_pairs)),
            ("check_pairs", Json::Int(self.check_pairs)),
            ("extra_warm_passes", Json::Int(self.extra_warm_passes)),
            ("cold_retries", Json::Int(self.cold_retries)),
            ("load_delta", self.load_delta_json()),
            ("absolute_max_load", self.absolute_max_load_json()),
        ])
    }

    /// The Python harness's argument checks, before any directory is touched.
    pub fn validate(&self) -> Result<(), String> {
        if self.extra_warm_passes < 0
            || self.cold_retries < 0
            || self.load_delta() <= 0.0
            || self.absolute_max_load() <= 0.0
        {
            return Err("Retries must be nonnegative and load controls positive".into());
        }
        if self.warm_pairs < 1 || self.check_pairs < 1 {
            return Err("Sample counts must be positive".into());
        }
        Ok(())
    }
}

#[derive(Args, Debug, Clone)]
pub struct ReadinessArgs {
    #[arg(long)]
    pub cli: PathBuf,
    #[arg(long)]
    pub repo: PathBuf,
    #[arg(long)]
    pub runtime: PathBuf,
    #[arg(long)]
    pub work: PathBuf,
    #[arg(long)]
    pub candidate: String,
    #[arg(long)]
    pub source_manifest: PathBuf,
    #[arg(long, default_value_t = 15.0)]
    pub timeout: f64,
}
