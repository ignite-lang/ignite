use clap::Parser;

use ignite_perf::cli::{Cli, Mode};
use ignite_perf::{perf, readiness};

fn main() {
    let cli = Cli::parse();
    let (name, args) = match cli.mode {
        Mode::SelfCheck => std::process::exit(readiness::exit_status(perf::self_check(
            &std::env::temp_dir(),
        ))),
        Mode::RecoverBaseline(args) => {
            std::process::exit(readiness::exit_status(perf::recover(&args)))
        }
        Mode::VerifyLlvmReadiness(args) => {
            std::process::exit(readiness::exit_status(readiness::verify(&args)))
        }
        Mode::Smoke(args) => ("smoke", args),
        Mode::Prepare(args) => ("prepare", args),
        Mode::Cold(args) => ("cold", args),
        Mode::Warm(args) => ("warm", args),
        Mode::Programs(args) => ("programs", args),
        Mode::Cli(args) => ("cli", args),
        Mode::All(args) => ("all", args),
        Mode::Summarize(args) => ("summarize", args),
    };
    let code = match perf::run_mode(name, args) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    };
    std::process::exit(code);
}
