//! `telos` management CLI (`docs/slice.md` sections 1 and 6, `docs/llvm.md` section 1).
//!
//! ```text
//! telos check <file.telos> [--std PATH]
//! telos emit <file.telos> [--substrate PATH] [--std PATH]
//! telos build <file.telos> [--target rust|llvm] [--out DIR] [--target-dir DIR] [--substrate PATH] [--std PATH] [--clang PATH] [--runtime PATH] [--release]
//! telos test <file.telos>… [FILTER] [--target rust|llvm] [--std DIR] [--out DIR] [--exact] [--list] [--jobs N] [--timeout SECS]
//! ```
//!
//! `--std` is the standard-library root (default: the repository's `std/`
//! next to this crate). Exit statuses: 0 success, 1 diagnostics (printed to
//! stderr, one per line, in the `SourceFile::render` format, with the path
//! of the file they are in), 2 cargo or clang failed (its output passed through) or
//! invalid arguments, 3 a file could not be read or the project could not be
//! written. `telos test` exits with the test executable's status (0 every test passed,
//! 1 a test failed), 2 for diagnostics.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "telos",
    version,
    about = "Check, emit and build Telos programs"
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
        /// Path of the `telos-io` crate written into `Cargo.toml`.
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
        /// Runtime archive for `--target llvm` (default `libtelos_rt.a` next to `telos`).
        #[arg(long)]
        runtime: Option<PathBuf>,
        /// Directory of the generated project (default `target/telos/<package>`).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Cargo target directory (default `<out>/target`).
        #[arg(long)]
        target_dir: Option<PathBuf>,
        /// Path of the `telos-io` crate.
        #[arg(long)]
        substrate: Option<PathBuf>,
        /// Root of the standard library.
        #[arg(long)]
        std: Option<PathBuf>,
        #[arg(long)]
        release: bool,
    },
    /// Build the tests of the named files and run them (`docs/slice.md` section 12.7).
    /// Arguments ending in `.telos` are files; another one is the filter.
    Test {
        /// `FILE.telos…` and at most one `FILTER`.
        #[arg(required = true)]
        args: Vec<String>,
        /// `rust` (default) or `llvm`.
        #[arg(long, value_enum, default_value = "rust")]
        target: TargetArg,
        /// Root of the standard library.
        #[arg(long)]
        std: Option<PathBuf>,
        /// Root of the test builds (default `target/telos-test`).
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
    },
}

/// Exit status of `telos test` for diagnostics and invalid arguments.
const TEST_COMPILE_ERROR: i32 = 2;

#[derive(Clone, Copy, clap::ValueEnum)]
enum TargetArg {
    Rust,
    Llvm,
}

/// Stack of the compiling thread: the parser, checker, type checker and
/// generators walk expressions recursively, and a chain of 10,000 `+` or
/// interpolations nests that deep (`docs/slice.md` section 3).
const STACK: usize = 256 << 20;

fn main() {
    let compiler = std::thread::Builder::new()
        .name("telos".into())
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
    } = cli.command
    {
        let (files, filters): (Vec<String>, Vec<String>) =
            args.into_iter().partition(|arg| arg.ends_with(".telos"));
        if files.is_empty() || filters.len() > 1 {
            eprintln!("error: telos test needs FILE.telos… and at most one FILTER");
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
        let options = telos::TestOptions {
            build: telos::BuildOptions {
                target: match target {
                    TargetArg::Rust => telos::Target::Rust,
                    TargetArg::Llvm => telos::Target::Llvm,
                },
                clang,
                runtime,
                out,
                target_dir,
                std,
                ..telos::BuildOptions::default()
            },
            runner,
        };
        let files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
        match telos::test(&files, &options) {
            Ok(status) => std::process::exit(status),
            Err(error) => {
                match &error {
                    telos::Error::Diagnostics { .. } => eprintln!("{error}"),
                    _ => eprintln!("error: {error}"),
                }
                std::process::exit(match error {
                    telos::Error::Diagnostics { .. } => TEST_COMPILE_ERROR,
                    other => other.exit_status(),
                });
            }
        }
    }
    let result = match cli.command {
        Commands::Test { .. } => unreachable!("handled above"),
        Commands::Check { file, std } => telos::check_with_std(&file, std.as_deref()),
        Commands::Emit {
            file,
            substrate,
            std,
        } => telos::emit_with_std(&file, substrate.as_deref(), std.as_deref())
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
        } => {
            let options = telos::BuildOptions {
                target: match target {
                    TargetArg::Rust => telos::Target::Rust,
                    TargetArg::Llvm => telos::Target::Llvm,
                },
                clang,
                runtime,
                out,
                target_dir,
                substrate,
                release,
                std,
            };
            telos::build(&file, &options).map(|output| {
                for executable in &output.executables {
                    println!("built {}", executable.display());
                }
            })
        }
    };
    if let Err(error) = result {
        match &error {
            telos::Error::Diagnostics { .. } => eprintln!("{error}"),
            _ => eprintln!("error: {error}"),
        }
        std::process::exit(error.exit_status());
    }
}

fn print_project(prefix: &str, project: &telos_rustgen::GeneratedProject) {
    println!("// ==== {prefix}Cargo.toml ====");
    print!("{}", project.cargo_toml);
    println!("// ==== {prefix}src/lib.rs ====");
    print!("{}", project.lib_rs);
    for executable in &project.executables {
        println!("// ==== {prefix}src/bin/{}.rs ====", executable.name);
        print!("{}", executable.source);
    }
}

fn print_projects(emitted: &telos::Emitted) {
    print_project("", &emitted.program);
    print_project("std/", &emitted.std);
}
