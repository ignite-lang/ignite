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
        Mode::Quality(args) => {
            std::process::exit(readiness::exit_status(ignite_perf::quality::measure(&args)))
        }
        Mode::QualityRemote(args) => match ignite_perf::remote::main(&args) {
            Ok(code) => std::process::exit(code),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1)
            }
        },
        Mode::QualityHost(args) => std::process::exit(readiness::exit_status(
            ignite_perf::remote::host_main(&args),
        )),
        Mode::PathCost(args) => match ignite_perf::path_cost_run::stop_on_signal()
            .and_then(|()| ignite_perf::path_cost::main(&args))
        {
            Ok(code) => std::process::exit(code),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1)
            }
        },
        Mode::Runtime(args) => {
            let target_dir = args.repo.join(&args.target_dir);
            std::process::exit(readiness::exit_status(
                ignite_perf::bench::release_runtime(&args.repo, &target_dir, None)
                    .map(|built| println!("{}", built.archive.display())),
            ))
        }
        Mode::Smoke(args) => ("smoke", args),
        Mode::Prepare(args) => ("prepare", args),
        Mode::Cold(args) => ("cold", args),
        Mode::Warm(args) => ("warm", args),
        Mode::Programs(args) => ("programs", args),
        Mode::Cli(args) => ("cli", args),
        Mode::ApplyCycle(args) => ("apply-cycle", args),
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
