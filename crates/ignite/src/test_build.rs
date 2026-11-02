//! `ignite test` (`docs/slice.md` sections 12.6 and 12.7): build a test
//! executable for the named files and run it with the runner's flags.
//!
//! Every file named shares one test executable per target: the standard-
//! library modules among them and every program, each program in a
//! namespace of its own (Rust: the module of its module path; LLVM: its
//! module path, which prefixes every symbol). Only two programs with one
//! module path (two files without a `package` line are both `main`) cannot
//! share one, because the runner names a test by `<module>/<name>`; the
//! second goes into a further executable. Every other module (the standard
//! library a program loads) is built normally, with its tests dropped. The
//! executables run one after another, each printing its own report.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use ignite_check::Symbols;
use ignite_syntax::SourceFile;
use ignite_syntax::ast::Program;
use ignite_types::{TestInfo, TypedModules, TypedProgram};

use crate::{
    BuildOptions, Error, ParsedFile, StdLibrary, Target, diagnostics, io_error, load_std_reachable,
    module_path, parse, parse_program, program_module, read_source, std_root, type_parsed_with,
    type_std,
};

/// Options of `ignite test`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TestOptions {
    /// Target, toolchain, standard library and output directory. `out` is
    /// the root under which each test executable gets a directory
    /// (default `target/ignite-test`).
    pub build: BuildOptions,
    /// Arguments for the runner: `[FILTER] [--exact] [--list] [--jobs N]
    /// [--timeout SECS]`.
    pub runner: Vec<String>,
}

/// What one test executable holds.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TestGroup {
    /// Program files, in command-line order; their module paths differ.
    pub programs: Vec<PathBuf>,
    /// Standard-library modules whose tests it runs, in command-line order.
    pub std_modules: Vec<String>,
    /// Module paths of the programs and the standard-library modules in
    /// command-line order: the order its tests run in.
    pub modules: Vec<String>,
}

/// Split `files` into test groups: each file under the standard library's
/// root is that module and goes into the first group; every other file is
/// a program and goes into the first group without a program of its module
/// path. Programs are read and parsed for their `package` line.
pub fn test_groups(files: &[PathBuf], library: &StdLibrary) -> Result<Vec<TestGroup>, Error> {
    group_files(files, &library.root, |file| {
        Ok(program_module(&parse(&read_source(file)?)?))
    })
}

/// [`test_groups`] over the canonical standard-library `root`, the module
/// path of a program file given by `module_of`.
fn group_files(
    files: &[PathBuf],
    root: &Path,
    mut module_of: impl FnMut(&Path) -> Result<String, Error>,
) -> Result<Vec<TestGroup>, Error> {
    let mut groups: Vec<TestGroup> = Vec::new();
    for file in files {
        let canonical = file.canonicalize().map_err(io_error(file))?;
        let (module, program) = match canonical.strip_prefix(root) {
            Ok(_) => (module_path(root, &canonical), None),
            Err(_) => (module_of(file)?, Some(file.clone())),
        };
        let index = match &program {
            None => 0,
            Some(_) => groups
                .iter()
                .position(|group| !group.modules.contains(&module))
                .unwrap_or(groups.len()),
        };
        if index == groups.len() {
            groups.push(TestGroup::default());
        }
        let group = &mut groups[index];
        if group.modules.contains(&module) {
            continue;
        }
        match program {
            Some(file) => group.programs.push(file),
            None => group.std_modules.push(module.clone()),
        }
        group.modules.push(module);
    }
    Ok(groups)
}

/// Build the test executable of every group of `files`. The standard
/// library is loaded to the modules the files reach (`docs/slice.md`
/// 16.22): every standard-library file named, every module the programs
/// and their siblings import, and their imports in turn.
pub fn build_tests(files: &[PathBuf], options: &TestOptions) -> Result<Vec<PathBuf>, Error> {
    let root = std_root(options.build.std.as_deref())?;
    let mut roots = BTreeSet::new();
    let mut parsed: BTreeMap<PathBuf, ParsedFile> = BTreeMap::new();
    for file in files {
        let canonical = file.canonicalize().map_err(io_error(file))?;
        if canonical.starts_with(&root) {
            roots.insert(module_path(&root, &canonical));
        } else if !parsed.contains_key(file) {
            let program = parse_program(file)?;
            roots.extend(program.std_modules());
            parsed.insert(file.clone(), program);
        }
    }
    let library = load_std_reachable(&root, &roots)?;
    let groups = group_files(files, &root, |file| Ok(parsed[file].module()))?;
    let (std_types, modules) = type_std(&library)?;
    let std = TypedStd {
        library: &library,
        types: &std_types,
        modules: &modules,
    };
    let mut used = BTreeSet::new();
    groups
        .iter()
        .map(|group| build_group(&std, group, &mut parsed, &options.build, &mut used))
        .collect()
}

/// The directory name of the next group: `tests`, then `tests-2`,
/// `tests-3`, … for the groups after it in the same invocation.
fn group_dir(used: &mut BTreeSet<String>) -> String {
    let name = "tests";
    let mut dir = name.to_string();
    let mut n = 1;
    while !used.insert(dir.clone()) {
        n += 1;
        dir = format!("{name}-{n}");
    }
    dir
}

/// The file in a cargo target directory naming the generated project built
/// in it last.
const LAST_PROJECT: &str = "ignite-test-project";

/// Make cargo rebuild the generated project at `out` in `target_dir` when
/// another project was built there last. Cargo hashes a path package by its
/// name, version and workspace-relative path, so generated projects with the
/// same package identity can share one unit in a shared `--target-dir`.
/// `write_project` leaves unchanged files alone, and
/// sources older than that unit's output count as fresh, so the build would
/// keep the other project's executable. Touching the sources marks the unit
/// dirty. A repeated build of the same project stays incremental.
pub(crate) fn claim_target_dir(out: &Path, target_dir: &Path) -> Result<(), Error> {
    let marker = target_dir.join(LAST_PROJECT);
    let project = out.to_string_lossy();
    if std::fs::read_to_string(&marker).is_ok_and(|last| last == project) {
        return Ok(());
    }
    touch_sources(out, std::time::SystemTime::now())?;
    std::fs::create_dir_all(target_dir).map_err(io_error(target_dir))?;
    std::fs::write(&marker, project.as_bytes()).map_err(io_error(&marker))
}

/// Set the modification time of every file under `dir` to `now`, except
/// inside a `target` directory.
fn touch_sources(dir: &Path, now: std::time::SystemTime) -> Result<(), Error> {
    for entry in std::fs::read_dir(dir).map_err(io_error(dir))? {
        let path = entry.map_err(io_error(dir))?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "target") {
                touch_sources(&path, now)?;
            }
        } else {
            std::fs::File::options()
                .write(true)
                .open(&path)
                .and_then(|file| file.set_modified(now))
                .map_err(io_error(&path))?;
        }
    }
    Ok(())
}

/// `ignite test`: build every test executable, then run each with
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

/// The standard library, checked and typed once for every group.
struct TypedStd<'a> {
    library: &'a StdLibrary,
    types: &'a [TypedProgram],
    modules: &'a TypedModules,
}

/// One program file, checked and typed.
struct TestProgram {
    source: SourceFile,
    program: Program,
    symbols: Symbols,
    types: TypedProgram,
}

/// Add the parsed program and its sibling modules (`docs/slice.md`
/// section 14.4), checked and typed, to `programs`; a module whose path
/// `programs` already holds (a sibling also named on the command line, or
/// named by two programs) is added once.
fn type_test_programs(
    parsed: ParsedFile,
    std: &TypedStd<'_>,
    programs: &mut Vec<TestProgram>,
) -> Result<(), Error> {
    let typed = type_parsed_with(parsed, std.library, std.modules)?;
    let mut add = |source, program, symbols, types: TypedProgram| {
        if !programs.iter().any(|p| p.types.module == types.module) {
            programs.push(TestProgram {
                source,
                program,
                symbols,
                types,
            });
        }
    };
    for sibling in typed.siblings {
        add(
            sibling.source,
            sibling.program,
            sibling.symbols,
            sibling.types,
        );
    }
    add(typed.source, typed.program, typed.symbols, typed.types);
    Ok(())
}

/// Build one group's test executable and return its path.
fn build_group(
    std: &TypedStd<'_>,
    group: &TestGroup,
    parsed: &mut BTreeMap<PathBuf, ParsedFile>,
    build: &BuildOptions,
    used: &mut BTreeSet<String>,
) -> Result<PathBuf, Error> {
    let library = std.library;
    let mut programs: Vec<TestProgram> = Vec::new();
    for file in &group.programs {
        let program = parsed
            .remove(file)
            .expect("every program of a group was parsed");
        type_test_programs(program, std, &mut programs)?;
    }
    let typed: BTreeMap<&str, &TypedProgram> = library
        .modules
        .iter()
        .map(|m| m.path.as_str())
        .zip(std.types)
        .chain(programs.iter().map(|p| (p.types.module.as_str(), &p.types)))
        .collect();
    let test_modules = group.modules.clone();
    let tests: Vec<TestInfo> = test_modules
        .iter()
        .filter_map(|module| typed.get(module.as_str()))
        .flat_map(|types| types.tests().iter().cloned())
        .collect();
    let mut sources: BTreeMap<String, SourceFile> = library
        .modules
        .iter()
        .map(|module| (module.path.clone(), module.source.clone()))
        .collect();
    for program in &programs {
        sources.insert(program.types.module.clone(), program.source.clone());
    }
    let target = match build.target {
        Target::Rust => "rust",
        Target::Llvm => "llvm",
    };
    let name = group_dir(used);
    let out = build
        .out
        .clone()
        .unwrap_or_else(|| Path::new("target").join("ignite-test"))
        .join(&name)
        .join(target);
    match build.target {
        Target::Rust => {
            let options = ignite_rustgen::Options {
                substrate_path: build
                    .substrate
                    .clone()
                    .unwrap_or_else(crate::default_substrate_path),
                test_modules,
                tests,
                sources,
            };
            let std_modules: Vec<ignite_rustgen::TypedStdModule<'_>> = library
                .modules
                .iter()
                .zip(std.types)
                .map(|(m, t)| (m.path.clone(), &m.program, &m.symbols, t))
                .collect();
            let inputs: Vec<ignite_rustgen::TestProgram<'_>> = programs
                .iter()
                .map(|p| (&p.program, &p.symbols, &p.types))
                .collect();
            let project = ignite_rustgen::generate_tests(&inputs, &options).map_err(|errors| {
                let (path, found) = errors.into_iter().next().expect("an error has a module");
                let program = programs
                    .iter()
                    .find(|p| p.types.module == path)
                    .expect("errors name a program");
                diagnostics(&program.source, found)
            })?;
            let std_project = ignite_rustgen::generate_std_for(&std_modules, &options, &project)
                .map_err(|errors| {
                    let (path, found) = errors.into_iter().next().expect("an error has a module");
                    let module = library
                        .modules
                        .iter()
                        .find(|m| m.path == path)
                        .expect("errors name loaded modules");
                    diagnostics(&module.source, found)
                })?;
            ignite_rustgen::write_project(&project, &out).map_err(io_error(&out))?;
            let std_out = out.join("std");
            ignite_rustgen::write_project(&std_project, &std_out).map_err(io_error(&std_out))?;
            let target_dir = build
                .target_dir
                .clone()
                .unwrap_or_else(|| out.join("target"));
            claim_target_dir(&out, &target_dir)?;
            crate::cargo_build(&out, &target_dir, build.release, true)?;
            let profile = if build.release { "release" } else { "debug" };
            let file = format!(
                "{}{}",
                ignite_rustgen::TEST_EXECUTABLE,
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
            let std_inputs: Vec<ignite_llvm::ModuleInput<'_>> = library
                .modules
                .iter()
                .zip(std.types)
                .map(|(module, types)| ignite_llvm::ModuleInput {
                    path: &module.path,
                    program: &module.program,
                    symbols: &module.symbols,
                    types,
                })
                .collect();
            let inputs: Vec<ignite_llvm::ModuleInput<'_>> = programs
                .iter()
                .map(|p| ignite_llvm::ModuleInput {
                    path: &p.types.module,
                    program: &p.program,
                    symbols: &p.symbols,
                    types: &p.types,
                })
                .collect();
            let ir_dir = out.join("llvm");
            std::fs::create_dir_all(&ir_dir).map_err(io_error(&ir_dir))?;
            let options = ignite_llvm::Options {
                test_modules,
                sources: sources.iter().map(|(p, s)| (p.clone(), s)).collect(),
                target: Some(crate::llvm_build::target(build, &ir_dir)?),
                directory: std::env::current_dir()
                    .ok()
                    .map(|d| d.display().to_string()),
                siblings: Vec::new(),
            };
            let generated = ignite_llvm::generate_tests(&std_inputs, &inputs, &options);
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
const RUNNER_SYMBOL: &[u8] = b"ignite_rt_test_main";

/// An error naming the fix when `runtime` has no test runner: the default
/// `libignite_rt.a` leaves it out so that programs link as they did before
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
            "runtime archive without the test runner: build it with `cargo build -p ignite-rt --features runner` and pass it with --runtime",
        ),
    })
}
