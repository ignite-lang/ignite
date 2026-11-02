//! `ignite` management CLI (`docs/slice.md` sections 1 and 6, `docs/llvm.md` section 1).
//!
//! ```text
//! ignite check <file.ignite> [--std PATH]
//! ignite emit <file.ignite> [--substrate PATH] [--std PATH]
//! ignite build <file.ignite> [--target rust|llvm] [--out DIR] [--target-dir DIR] [--substrate PATH] [--std PATH] [--clang PATH] [--runtime PATH] [--release] [--triple TRIPLE]
//! ignite test <file.ignite>… [FILTER] [--target rust|llvm] [--std DIR] [--out DIR] [--exact] [--list] [--jobs N] [--timeout SECS] [--release] [--triple TRIPLE]
//! ignite plan <file.ignite> [--assembly NAME] [--out FILE] [--fuel N] [--std PATH]
//! ignite explain <file.ignite> <path> [--assembly NAME] [--fuel N] [--std PATH]
//! ignite run <file.ignite> [--assembly NAME] [--state DIR] [--build DIR] [--target rust|llvm] [--release] [--clock real|manual] [--epoch RFC3339] [--settle DURATION] [--port RESOURCE=PORT]... [--json] [--fresh] [--std PATH] [--fuel N] [--runtime PATH] [--target-dir DIR]
//! ignite replay <file.ignite> [--assembly NAME] [--state DIR] [--std PATH] [--fuel N]
//! ```
//!
//! `--std` is the standard-library root (default: the repository's `std/`
//! next to this crate). Exit statuses: 0 success, 1 diagnostics (printed to
//! stderr, one per line, in the `SourceFile::render` format, with the path
//! of the file they are in), 2 cargo or clang failed (its output passed through) or
//! invalid arguments, 3 a file could not be read or the project could not be
//! written. `ignite test` exits with the test executable's status (0 every test passed,
//! 1 a test failed), 2 for diagnostics. `ignite plan` prints the system model
//! (`docs/slice.md` section 15.5) on stdout or writes it to `--out FILE`;
//! `ignite explain` prints the explanation of one path (section 15.8).
//! `ignite run` exits with the run's status (section 16.3: 0 a requested
//! stop, 1 a machine gave up or failed), 2 for a diagnostic before the
//! start; `ignite replay` prints the final standings.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "ignite",
    version,
    about = "Check, emit, build, plan and explain Ignite programs"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse and check a program and the standard library; no output on success.
    Check {
        file: PathBuf,
        /// Root of the standard library.
        #[arg(long)]
        std: Option<PathBuf>,
    },
    /// Print the generated Rust projects: the program, then `std/`.
    Emit {
        file: PathBuf,
        /// Path of the `ignite-io` crate written into `Cargo.toml`.
        #[arg(long)]
        substrate: Option<PathBuf>,
        /// Root of the standard library.
        #[arg(long)]
        std: Option<PathBuf>,
    },
    /// Build the program: generated Rust with cargo (`--target rust`) or LLVM IR with clang
    /// (`--target llvm`).
    Build {
        file: PathBuf,
        /// `rust` (default) or `llvm`.
        #[arg(long, value_enum, default_value = "rust")]
        target: TargetArg,
        /// clang for `--target llvm` (default `clang` on `PATH`).
        #[arg(long)]
        clang: Option<PathBuf>,
        /// Runtime archive for `--target llvm` (default `libignite_rt.a` next to `ignite`).
        #[arg(long)]
        runtime: Option<PathBuf>,
        /// Directory of the generated project (default `target/ignite/<package>`).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Cargo target directory (default `<out>/target`).
        #[arg(long)]
        target_dir: Option<PathBuf>,
        /// Path of the `ignite-io` crate.
        #[arg(long)]
        substrate: Option<PathBuf>,
        /// Root of the standard library.
        #[arg(long)]
        std: Option<PathBuf>,
        #[arg(long)]
        release: bool,
        /// Target triple for `--target llvm`, passed to clang (default: clang's own target).
        #[arg(long)]
        triple: Option<String>,
    },
    /// Build the tests of the named files and run them (`docs/slice.md` section 12.7).
    /// Arguments ending in `.ignite` are files; another one is the filter.
    Test {
        /// `FILE.ignite…` and at most one `FILTER`.
        #[arg(required = true)]
        args: Vec<String>,
        /// `rust` (default) or `llvm`.
        #[arg(long, value_enum, default_value = "rust")]
        target: TargetArg,
        /// Root of the standard library.
        #[arg(long)]
        std: Option<PathBuf>,
        /// Root of the test builds (default `target/ignite-test`).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Run only the test whose full name equals the filter.
        #[arg(long)]
        exact: bool,
        /// List the selected tests instead of running them.
        #[arg(long)]
        list: bool,
        /// Tests run at once (default: the CPU count).
        #[arg(long)]
        jobs: Option<usize>,
        /// Seconds after which a test is killed (default 60).
        #[arg(long)]
        timeout: Option<u64>,
        /// clang for `--target llvm`.
        #[arg(long)]
        clang: Option<PathBuf>,
        /// Runtime archive for `--target llvm`.
        #[arg(long)]
        runtime: Option<PathBuf>,
        /// Cargo target directory for `--target rust` (default `<out>/<name>/rust/target`).
        #[arg(long)]
        target_dir: Option<PathBuf>,
        /// Build the tests with the release profile (`-O2` on `--target llvm`).
        #[arg(long)]
        release: bool,
        /// Target triple for `--target llvm`, passed to clang (default: clang's own target).
        #[arg(long)]
        triple: Option<String>,
    },
    /// Print the system model of a program as canonical JSON (`docs/slice.md` section 15.5).
    Plan {
        file: PathBuf,
        /// The assembly to select (default: the program's only one).
        #[arg(long)]
        assembly: Option<String>,
        /// Write the model to this file instead of stdout (overwriting it).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Fuel limit per property expression (default 1,000,000 units).
        #[arg(long)]
        fuel: Option<u64>,
        /// Root of the standard library.
        #[arg(long)]
        std: Option<PathBuf>,
    },
    /// Explain one system, member, property path or assembly of a program (`docs/slice.md`
    /// section 15.8).
    Explain {
        file: PathBuf,
        /// `Terminal`, `Terminal.reachable`, `Terminal.server.recovery` or `local`.
        path: String,
        /// The assembly to select (default: the program's only one).
        #[arg(long)]
        assembly: Option<String>,
        /// Fuel limit per property expression (default 1,000,000 units).
        #[arg(long)]
        fuel: Option<u64>,
        /// Root of the standard library.
        #[arg(long)]
        std: Option<PathBuf>,
    },
    /// Run an assembly: build its members, start them, probe them and
    /// evaluate its contracts (`docs/slice.md` section 16).
    Run {
        file: PathBuf,
        /// The assembly to run (default: the program's only one).
        #[arg(long)]
        assembly: Option<String>,
        /// State directory: journal, lock and executables (default:
        /// `target/ignite-run/<assembly>/`).
        #[arg(long)]
        state: Option<PathBuf>,
        /// Build directory (default: `<state>/build/`).
        #[arg(long)]
        build: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = TargetArg::Rust)]
        target: TargetArg,
        /// Build the members with optimizations.
        #[arg(long)]
        release: bool,
        #[arg(long, value_enum, default_value_t = ClockArg::Real)]
        clock: ClockArg,
        /// Epoch of the manual clock, RFC 3339 (default `2026-01-01T00:00:00Z`).
        #[arg(long)]
        epoch: Option<String>,
        /// Settle time of the manual clock, a Duration literal (default `2 s`).
        #[arg(long)]
        settle: Option<String>,
        /// Bind a resource to a port, `RESOURCE=PORT`; port 0 binds a free port.
        #[arg(long = "port", value_name = "RESOURCE=PORT")]
        ports: Vec<String>,
        /// Print the journal records on stdout.
        #[arg(long)]
        json: bool,
        /// Rename an existing journal and start a new one.
        #[arg(long)]
        fresh: bool,
        /// Root of the standard library.
        #[arg(long)]
        std: Option<PathBuf>,
        /// Fuel limit per property expression and reducer step (default 1,000,000 units).
        #[arg(long)]
        fuel: Option<u64>,
        /// Runtime archive `libignite_rt.a` of the LLVM target.
        #[arg(long)]
        runtime: Option<PathBuf>,
        /// Cargo target directory of the Rust target (default: `<build>/target`).
        #[arg(long)]
        target_dir: Option<PathBuf>,
    },
    /// Re-step the journal of a state directory and print the final standings
    /// (`docs/slice.md` section 16.15).
    Replay {
        file: PathBuf,
        /// The assembly whose journal to replay (default: the program's only one).
        #[arg(long)]
        assembly: Option<String>,
        /// State directory holding the journal (default: `target/ignite-run/<assembly>/`).
        #[arg(long)]
        state: Option<PathBuf>,
        /// Root of the standard library.
        #[arg(long)]
        std: Option<PathBuf>,
        /// Fuel limit per reducer step (default 1,000,000 units).
        #[arg(long)]
        fuel: Option<u64>,
    },
}

/// Exit status of `ignite test` for diagnostics and invalid arguments.
const TEST_COMPILE_ERROR: i32 = 2;

/// Exit status of `ignite run` for a diagnostic before the start
/// (`docs/slice.md` 16.3).
const RUN_DIAGNOSTIC: i32 = 2;

#[derive(Clone, Copy, clap::ValueEnum)]
enum TargetArg {
    Rust,
    Llvm,
}

impl From<TargetArg> for ignite::Target {
    fn from(target: TargetArg) -> ignite::Target {
        match target {
            TargetArg::Rust => ignite::Target::Rust,
            TargetArg::Llvm => ignite::Target::Llvm,
        }
    }
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ClockArg {
    Real,
    Manual,
}

/// `RESOURCE=PORT` of `--port`: `Err` is the usage text.
fn parse_port(arg: &str) -> Result<(String, u16), String> {
    let (resource, port) = arg
        .split_once('=')
        .ok_or_else(|| format!("--port needs RESOURCE=PORT, got `{arg}`"))?;
    let port = port
        .parse::<u16>()
        .map_err(|_| format!("--port {resource}: `{port}` is not a port in 0..=65535"))?;
    Ok((resource.to_string(), port))
}

/// Stack of the compiling thread: the parser, checker, type checker and
/// generators walk expressions recursively, and a chain of 10,000 `+` or
/// interpolations nests that deep (`docs/slice.md` section 3).
const STACK: usize = 256 << 20;

fn main() {
    let compiler = std::thread::Builder::new()
        .name("ignite".into())
        .stack_size(STACK)
        .spawn(run)
        .expect("spawn the compiling thread");
    if compiler.join().is_err() {
        std::process::exit(101);
    }
}

fn run() {
    let cli = Cli::parse();
    if let Commands::Test {
        args,
        target,
        std,
        out,
        exact,
        list,
        jobs,
        timeout,
        clang,
        runtime,
        target_dir,
        release,
        triple,
    } = cli.command
    {
        let (files, filters): (Vec<String>, Vec<String>) =
            args.into_iter().partition(|arg| arg.ends_with(".ignite"));
        if files.is_empty() || filters.len() > 1 {
            eprintln!("error: ignite test needs FILE.ignite… and at most one FILTER");
            std::process::exit(TEST_COMPILE_ERROR);
        }
        let mut runner = filters;
        if exact {
            runner.push("--exact".into());
        }
        if list {
            runner.push("--list".into());
        }
        if let Some(jobs) = jobs {
            runner.extend(["--jobs".into(), jobs.to_string()]);
        }
        if let Some(timeout) = timeout {
            runner.extend(["--timeout".into(), timeout.to_string()]);
        }
        let options = ignite::TestOptions {
            build: ignite::BuildOptions {
                target: match target {
                    TargetArg::Rust => ignite::Target::Rust,
                    TargetArg::Llvm => ignite::Target::Llvm,
                },
                clang,
                runtime,
                out,
                target_dir,
                std,
                release,
                triple,
                ..ignite::BuildOptions::default()
            },
            runner,
        };
        let files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
        match ignite::test(&files, &options) {
            Ok(status) => std::process::exit(status),
            Err(error) => {
                match &error {
                    ignite::Error::Diagnostics { .. } => eprintln!("{error}"),
                    _ => eprintln!("error: {error}"),
                }
                std::process::exit(match error {
                    ignite::Error::Diagnostics { .. } => TEST_COMPILE_ERROR,
                    other => other.exit_status(),
                });
            }
        }
    }
    if let Commands::Run {
        file,
        assembly,
        state,
        build,
        target,
        release,
        clock,
        epoch,
        settle,
        ports,
        json,
        fresh,
        std,
        fuel,
        runtime,
        target_dir,
    } = cli.command
    {
        let ports = match ports.iter().map(|arg| parse_port(arg)).collect() {
            Ok(ports) => ports,
            Err(message) => {
                eprintln!("error: {message}");
                std::process::exit(RUN_DIAGNOSTIC);
            }
        };
        if let Some(text) = &epoch
            && ignite::run::parse_rfc3339(text).is_none()
        {
            eprintln!("error: --epoch `{text}` is no RFC 3339 instant");
            std::process::exit(RUN_DIAGNOSTIC);
        }
        if let Some(text) = &settle
            && ignite::run::clock::parse_duration(text).is_none()
        {
            eprintln!("error: --settle `{text}` is no Duration literal");
            std::process::exit(RUN_DIAGNOSTIC);
        }
        let options = ignite::RunOptions {
            assembly,
            state,
            build,
            target: target.into(),
            release,
            clock: match clock {
                ClockArg::Real => ignite::ClockMode::Real,
                ClockArg::Manual => ignite::ClockMode::Manual,
            },
            epoch,
            settle,
            ports,
            json,
            fresh,
            std,
            fuel,
            runtime,
            target_dir,
        };
        match ignite::run(&file, &options) {
            Ok(status) => std::process::exit(status),
            Err(error) => {
                match &error {
                    ignite::Error::Diagnostics { .. } => eprintln!("{error}"),
                    _ => eprintln!("error: {error}"),
                }
                std::process::exit(match error {
                    ignite::Error::Diagnostics { .. } => RUN_DIAGNOSTIC,
                    other => other.exit_status(),
                });
            }
        }
    }
    let result = match cli.command {
        Commands::Test { .. } | Commands::Run { .. } => unreachable!("handled above"),
        Commands::Check { file, std } => ignite::check_with_std(&file, std.as_deref()),
        Commands::Emit {
            file,
            substrate,
            std,
        } => ignite::emit_with_std(&file, substrate.as_deref(), std.as_deref())
            .map(|emitted| print_projects(&emitted)),
        Commands::Build {
            file,
            target,
            clang,
            runtime,
            out,
            target_dir,
            substrate,
            std,
            release,
            triple,
        } => {
            let options = ignite::BuildOptions {
                target: match target {
                    TargetArg::Rust => ignite::Target::Rust,
                    TargetArg::Llvm => ignite::Target::Llvm,
                },
                clang,
                runtime,
                out,
                target_dir,
                substrate,
                release,
                std,
                triple,
            };
            ignite::build(&file, &options).map(|output| {
                for executable in &output.executables {
                    println!("built {}", executable.display());
                }
            })
        }
        Commands::Plan {
            file,
            assembly,
            out,
            fuel,
            std,
        } => {
            let options = ignite::PlanOptions {
                assembly,
                fuel,
                std,
            };
            ignite::plan(&file, &options).and_then(|model| match out {
                Some(out) => std::fs::write(&out, &model)
                    .map_err(|error| ignite::Error::Io { path: out, error }),
                None => {
                    print!("{model}");
                    Ok(())
                }
            })
        }
        Commands::Explain {
            file,
            path,
            assembly,
            fuel,
            std,
        } => {
            let options = ignite::PlanOptions {
                assembly,
                fuel,
                std,
            };
            ignite::explain(&file, &path, &options).map(|text| print!("{text}"))
        }
        Commands::Replay {
            file,
            assembly,
            state,
            std,
            fuel,
        } => {
            let options = ignite::ReplayOptions {
                assembly,
                state,
                std,
                fuel,
            };
            ignite::replay(&file, &options).map(|standings| {
                for standing in &standings {
                    println!("{} {}", standing.contract, standing.value.debug_text());
                }
            })
        }
    };
    if let Err(error) = result {
        match &error {
            ignite::Error::Diagnostics { .. } => eprintln!("{error}"),
            _ => eprintln!("error: {error}"),
        }
        std::process::exit(error.exit_status());
    }
}

fn print_project(prefix: &str, project: &ignite_rustgen::GeneratedProject) {
    println!("// ==== {prefix}Cargo.toml ====");
    print!("{}", project.cargo_toml);
    println!("// ==== {prefix}src/lib.rs ====");
    print!("{}", project.lib_rs);
    for executable in &project.executables {
        println!("// ==== {prefix}src/bin/{}.rs ====", executable.name);
        print!("{}", executable.source);
    }
}

fn print_projects(emitted: &ignite::Emitted) {
    print_project("", &emitted.program);
    print_project("std/", &emitted.std);
}
