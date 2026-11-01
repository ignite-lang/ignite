//! `telos test` (`docs/slice.md` sections 12.6 and 12.7): build a test
//! executable for the named files and run it with the runner's flags.
//!
//! Each program file gets a test executable of its own, holding that
//! program's tests; the standard-library modules among the files share one,
//! built over an empty program. Every other module (the standard library a
//! program loads) is built normally, with its tests dropped. The executables
//! run one after another, each printing its own report, in the order of the
//! first file of each.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use telos_syntax::SourceFile;
use telos_types::{TestInfo, TypedProgram};

use crate::{
    BuildOptions, Error, StdLibrary, Target, diagnostics, front_end_source, io_error, read_source,
    std_library, type_std,
};

/// Options of `telos test`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TestOptions {
    /// Target, toolchain, standard library and output directory. `out` is
    /// the root under which each test executable gets a directory
    /// (default `target/telos-test`).
    pub build: BuildOptions,
    /// Arguments for the runner: `[FILTER] [--exact] [--list] [--jobs N]
    /// [--timeout SECS]`.
    pub runner: Vec<String>,
}

/// What one test executable holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestGroup {
    /// The program file, or `None` for the standard-library modules alone.
    pub program: Option<PathBuf>,
    /// Standard-library modules whose tests it runs, in command-line order.
    pub std_modules: Vec<String>,
}

/// Split `files` into test groups: each file under the standard library's
/// root is that module; every other file is a program.
pub fn test_groups(files: &[PathBuf], library: &StdLibrary) -> Result<Vec<TestGroup>, Error> {
    let mut groups = Vec::new();
    let mut std_group: Option<usize> = None;
    for file in files {
        let canonical = file.canonicalize().map_err(io_error(file))?;
        match canonical.strip_prefix(&library.root) {
            Ok(_) => {
                let module = crate::module_path(&library.root, &canonical);
                let index = *std_group.get_or_insert_with(|| {
                    groups.push(TestGroup {
                        program: None,
                        std_modules: Vec::new(),
                    });
                    groups.len() - 1
                });
                if !groups[index].std_modules.contains(&module) {
                    groups[index].std_modules.push(module);
                }
            }
            Err(_) => groups.push(TestGroup {
                program: Some(file.clone()),
                std_modules: Vec::new(),
            }),
        }
    }
    Ok(groups)
}

/// Build the test executable of every group of `files`.
pub fn build_tests(files: &[PathBuf], options: &TestOptions) -> Result<Vec<PathBuf>, Error> {
    let library = std_library(options.build.std.as_deref())?;
    test_groups(files, &library)?
        .iter()
        .map(|group| build_group(&library, group, &options.build))
        .collect()
}

/// `telos test`: build every test executable, then run each with
/// `options.runner`, its output inherited. Returns the exit status: 0 when
/// every executable exited 0, else the largest status.
pub fn test(files: &[PathBuf], options: &TestOptions) -> Result<i32, Error> {
    let executables = build_tests(files, options)?;
    let mut status = 0;
    for executable in executables {
        let code = Command::new(&executable)
            .args(&options.runner)
            .status()
            .map_err(io_error(&executable))?
            .code()
            .unwrap_or(1);
        status = status.max(code);
    }
    Ok(status)
}

/// The empty program a group of standard-library modules is built over.
fn empty_program() -> SourceFile {
    SourceFile::new("<tests>", "")
}

/// Build one group's test executable and return its path.
fn build_group(
    library: &StdLibrary,
    group: &TestGroup,
    build: &BuildOptions,
) -> Result<PathBuf, Error> {
    let source = match &group.program {
        Some(file) => read_source(file)?,
        None => empty_program(),
    };
    let (std_types, modules) = type_std(library)?;
    let (source, program, symbols) = front_end_source(source, library)?;
    let types = telos_types::type_module(&program, &symbols, &modules)
        .map_err(|d| diagnostics(&source, d))?;
    let mut test_modules = group.std_modules.clone();
    if group.program.is_some() {
        test_modules.push(types.module.clone());
    }
    let typed: BTreeMap<&str, &TypedProgram> = library
        .modules
        .iter()
        .map(|m| m.path.as_str())
        .zip(&std_types)
        .chain(std::iter::once((types.module.as_str(), &types)))
        .collect();
    let tests: Vec<TestInfo> = test_modules
        .iter()
        .filter_map(|module| typed.get(module.as_str()))
        .flat_map(|types| types.tests().iter().cloned())
        .collect();
    let sources = crate::sources(library, &types.module, &source);
    let name = match &group.program {
        Some(_) => telos_rustgen::package_name(&program),
        None => "std".to_string(),
    };
    let target = match build.target {
        Target::Rust => "rust",
        Target::Llvm => "llvm",
    };
    let out = build
        .out
        .clone()
        .unwrap_or_else(|| Path::new("target").join("telos-test"))
        .join(&name)
        .join(target);
    match build.target {
        Target::Rust => {
            let options = telos_rustgen::Options {
                substrate_path: build
                    .substrate
                    .clone()
                    .unwrap_or_else(crate::default_substrate_path),
                test_modules,
                tests,
                sources,
            };
            let std_modules: Vec<telos_rustgen::TypedStdModule<'_>> = library
                .modules
                .iter()
                .zip(&std_types)
                .map(|(m, t)| (m.path.clone(), &m.program, &m.symbols, t))
                .collect();
            let std =
                telos_rustgen::generate_std_typed(&std_modules, &options).map_err(|errors| {
                    let (path, found) = errors.into_iter().next().expect("an error has a module");
                    let module = library
                        .modules
                        .iter()
                        .find(|m| m.path == path)
                        .expect("errors name loaded modules");
                    diagnostics(&module.source, found)
                })?;
            let project = telos_rustgen::generate_typed(&program, &symbols, &types, &options)
                .map_err(|d| diagnostics(&source, d))?;
            telos_rustgen::write_project(&project, &out).map_err(io_error(&out))?;
            let std_out = out.join("std");
            telos_rustgen::write_project(&std, &std_out).map_err(io_error(&std_out))?;
            let target_dir = build
                .target_dir
                .clone()
                .unwrap_or_else(|| out.join("target"));
            crate::cargo_build(&out, &target_dir, build.release, true)?;
            let profile = if build.release { "release" } else { "debug" };
            let file = format!(
                "{}{}",
                telos_rustgen::TEST_EXECUTABLE,
                std::env::consts::EXE_SUFFIX
            );
            // Every group's executable has the same name, so in a shared
            // `--target-dir` the next group's build overwrites it: keep a
            // copy in the group's own directory.
            let built = target_dir.join(profile).join(&file);
            let executable = out.join(&file);
            std::fs::copy(&built, &executable).map_err(io_error(&built))?;
            Ok(executable)
        }
        Target::Llvm => {
            let std_inputs: Vec<telos_llvm::ModuleInput<'_>> = library
                .modules
                .iter()
                .zip(&std_types)
                .map(|(module, types)| telos_llvm::ModuleInput {
                    path: &module.path,
                    program: &module.program,
                    symbols: &module.symbols,
                    types,
                })
                .collect();
            let input = telos_llvm::ModuleInput {
                path: &types.module,
                program: &program,
                symbols: &symbols,
                types: &types,
            };
            let options = telos_llvm::Options {
                test_modules,
                sources: sources.iter().map(|(p, s)| (p.clone(), s)).collect(),
            };
            let generated = telos_llvm::generate_with(&std_inputs, &input, &options);
            let runtime = build
                .runtime
                .clone()
                .unwrap_or_else(crate::default_runtime_path);
            check_runner(&runtime)?;
            let build = BuildOptions {
                out: Some(out),
                runtime: Some(runtime),
                ..build.clone()
            };
            let output = crate::llvm_build::link(&name, &generated, &build)?;
            output
                .executables
                .into_iter()
                .next()
                .ok_or_else(|| Error::Io {
                    path: output.project_dir,
                    error: std::io::Error::other("the test build produced no executable"),
                })
        }
    }
}

/// The symbol an LLVM test main calls, which only a runtime archive built
/// with the feature `runner` defines.
const RUNNER_SYMBOL: &[u8] = b"telos_rt_test_main";

/// An error naming the fix when `runtime` has no test runner: the default
/// `libtelos_rt.a` leaves it out so that programs link as they did before
/// tests existed (`docs/llvm.md` section 5).
fn check_runner(runtime: &Path) -> Result<(), Error> {
    let archive = std::fs::read(runtime).map_err(io_error(runtime))?;
    if archive
        .windows(RUNNER_SYMBOL.len())
        .any(|window| window == RUNNER_SYMBOL)
    {
        return Ok(());
    }
    Err(Error::Io {
        path: runtime.to_path_buf(),
        error: std::io::Error::other(
            "runtime archive without the test runner: build it with `cargo build -p telos-rt --features runner` and pass it with --runtime",
        ),
    })
}
