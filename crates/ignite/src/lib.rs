//! The `ignite` driver as a library, so tests and other tools run the same
//! pipeline as the command line: load the standard library, read, parse,
//! check, generate, write, build (`docs/slice.md` sections 1, 4.1 and 6.2).
//!
//! Diagnostics it owns: `E031` a standard-library file whose `package` is
//! not its module path, `E032` an import cycle among standard-library
//! modules. Everything else comes from the stage that found it, rendered
//! with the path of the file it is in.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

mod llvm_build;
mod test_build;

pub use llvm_build::{COMPILE_FLAGS, LINK_FLAGS, RELEASE_LINK_FLAGS};
pub use test_build::{TestGroup, TestOptions, build_tests, test, test_groups};

use ignite_check::{Modules, Symbols};
use ignite_rustgen::GeneratedProject;
use ignite_syntax::SourceFile;
use ignite_syntax::ast::{Import, Program, Span};
use ignite_syntax::diagnostic::Diagnostic;
use ignite_types::{TypedModules, TypedProgram};

/// What `ignite build` compiles to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Target {
    /// Generated Rust projects built with cargo (`docs/slice.md` 6.2).
    #[default]
    Rust,
    /// LLVM IR compiled and linked by clang with `libignite_rt.a`
    /// (`docs/llvm.md` section 1). Runs neither cargo nor rustc.
    Llvm,
}

/// Native libraries `libignite_rt.a` needs (`cargo rustc -p ignite-rt
/// --print native-static-libs`).
pub const RUNTIME_LIBS: &[&str] = &[
    "-lgcc_s",
    "-lutil",
    "-lrt",
    "-lpthread",
    "-lm",
    "-ldl",
    "-lc",
];

/// Options of `ignite build`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildOptions {
    pub target: Target,
    /// clang for the LLVM target. Default: `clang` on `PATH`.
    pub clang: Option<PathBuf>,
    /// The runtime archive for the LLVM target. Default:
    /// [`default_runtime_path`].
    pub runtime: Option<PathBuf>,
    /// Directory the generated Cargo project is written to.
    /// Default: `target/ignite/<package>` under the current directory.
    pub out: Option<PathBuf>,
    /// `--target-dir` passed to cargo. Default: `<out>/target`.
    pub target_dir: Option<PathBuf>,
    /// Path of the `ignite-io` crate. Default: the crate this driver was
    /// built with (`CARGO_MANIFEST_DIR/../ignite-io`, canonicalized).
    pub substrate: Option<PathBuf>,
    pub release: bool,
    /// Root of the standard library. Default: [`default_std_path`].
    pub std: Option<PathBuf>,
    /// Target triple of the LLVM target, passed to clang as `--target`.
    /// Default: clang's own target (`docs/llvm.md` section 1).
    pub triple: Option<String>,
}

#[allow(clippy::derivable_impls)]
impl Default for BuildOptions {
    fn default() -> BuildOptions {
        BuildOptions {
            target: Target::Rust,
            clang: None,
            runtime: None,
            out: None,
            target_dir: None,
            substrate: None,
            release: false,
            std: None,
            triple: None,
        }
    }
}

/// Why a driver command failed. Maps to the exit statuses in `main.rs`.
#[derive(Debug)]
pub enum Error {
    /// Exit 1. Rendered, one per line, in `SourceFile::render` format with
    /// the path of the file the diagnostics are in (the program or a
    /// standard-library file).
    Diagnostics {
        source: SourceFile,
        diagnostics: Vec<Diagnostic>,
    },
    /// Exit 2. Cargo's own output was passed through to stderr.
    Cargo { status: i32 },
    /// Exit 2. clang's own output was passed through to stderr.
    Clang { status: i32 },
    /// Exit 3.
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Diagnostics {
                source,
                diagnostics,
            } => f.write_str(&source.render_all(diagnostics)),
            Error::Cargo { status } => write!(f, "cargo build failed with status {status}"),
            Error::Clang { status } => write!(f, "clang failed with status {status}"),
            Error::Io { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    pub fn exit_status(&self) -> i32 {
        match self {
            Error::Diagnostics { .. } => 1,
            Error::Cargo { .. } | Error::Clang { .. } => 2,
            Error::Io { .. } => 3,
        }
    }
}

fn diagnostics(source: &SourceFile, diagnostics: Vec<Diagnostic>) -> Error {
    Error::Diagnostics {
        source: source.clone(),
        diagnostics,
    }
}

/// What `ignite build` produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildOutput {
    pub project_dir: PathBuf,
    /// One path per executable, in declaration order.
    pub executables: Vec<PathBuf>,
}

/// One checked standard-library module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StdModule {
    /// Module path, e.g. `std.protocol.telnet`.
    pub path: String,
    pub source: SourceFile,
    pub program: Program,
    pub symbols: Symbols,
}

/// The standard library, checked, in import order (a module comes after
/// every module it imports).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct StdLibrary {
    pub root: PathBuf,
    pub modules: Vec<StdModule>,
}

impl StdLibrary {
    /// Every module's symbols by module path, for `check_module`.
    pub fn symbols(&self) -> Modules {
        self.modules
            .iter()
            .map(|module| (module.path.clone(), module.symbols.clone()))
            .collect()
    }
}

/// Both generated projects: the program and the `ignite-std` crate that is
/// written to `<out>/std`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Emitted {
    pub program: GeneratedProject,
    pub std: GeneratedProject,
}

/// The repository's `std/` next to this driver's crate
/// (`CARGO_MANIFEST_DIR/../../std`, canonicalized when it exists).
pub fn default_std_path() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../std");
    path.canonicalize().unwrap_or(path)
}

/// The `ignite-io` crate this driver was built next to.
pub fn default_substrate_path() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ignite-io");
    path.canonicalize().unwrap_or(path)
}

fn io_error(path: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |error| Error::Io {
        path: path.to_path_buf(),
        error,
    }
}

/// Every `.ignite` file under `dir`, sorted.
fn ignite_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Error> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(io_error(dir))?
        .map(|entry| entry.map(|e| e.path()).map_err(io_error(dir)))
        .collect::<Result<_, _>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            ignite_files(&path, files)?;
        } else if path.extension().is_some_and(|e| e == "ignite") {
            files.push(path);
        }
    }
    Ok(())
}

/// `<root>/a/b.ignite` is `std.a.b`.
fn module_path(root: &Path, file: &Path) -> String {
    let relative = file.strip_prefix(root).unwrap_or(file).with_extension("");
    let mut path = String::from("std");
    for component in relative.components() {
        path.push('.');
        path.push_str(&component.as_os_str().to_string_lossy());
    }
    path
}

/// The module an import refers to: `import a.b.{X}` and `import a.b.X`
/// both refer to `a.b`.
fn imported_module(import: &Import) -> String {
    let segments = &import.path.segments;
    let count = if import.names.is_empty() {
        segments.len().saturating_sub(1)
    } else {
        segments.len()
    };
    segments[..count]
        .iter()
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn read_source(file: &Path) -> Result<SourceFile, Error> {
    let text = std::fs::read_to_string(file).map_err(io_error(file))?;
    Ok(SourceFile::new(file.display().to_string(), text))
}

fn parse(source: &SourceFile) -> Result<Program, Error> {
    ignite_syntax::parse(source).map_err(|d| diagnostics(source, d))
}

/// A parsed standard-library file before checking.
struct Parsed {
    path: String,
    source: SourceFile,
    program: Program,
}

/// Load, parse and check every `.ignite` file under `root`. Files are
/// checked in import order with the modules checked before them.
pub fn load_std(root: &Path) -> Result<StdLibrary, Error> {
    let root = root.canonicalize().map_err(io_error(root))?;
    let mut files = Vec::new();
    ignite_files(&root, &mut files)?;

    let mut parsed: BTreeMap<String, Parsed> = BTreeMap::new();
    for file in files {
        let source = read_source(&file)?;
        let program = parse(&source)?;
        let path = module_path(&root, &file);
        check_package(&path, &source, &program)?;
        parsed.insert(
            path.clone(),
            Parsed {
                path,
                source,
                program,
            },
        );
    }

    let order = import_order(&parsed)?;
    let mut checked: Modules = Modules::new();
    let mut modules = Vec::new();
    for path in order {
        let Parsed {
            path,
            source,
            program,
        } = parsed.remove(&path).expect("ordered modules are parsed");
        let symbols =
            ignite_check::check_module(&program, &checked).map_err(|d| diagnostics(&source, d))?;
        checked.insert(path.clone(), symbols.clone());
        modules.push(StdModule {
            path,
            source,
            program,
            symbols,
        });
    }
    Ok(StdLibrary { root, modules })
}

/// E031 unless the file declares `package <path>`.
fn check_package(path: &str, source: &SourceFile, program: &Program) -> Result<(), Error> {
    let diagnostic = match &program.package {
        Some(package) if package.dotted() == path => return Ok(()),
        Some(package) => Diagnostic::new(
            "E031",
            package.span,
            format!(
                "package `{}` does not match the module path `{path}` of this file",
                package.dotted()
            ),
        ),
        None => Diagnostic::new(
            "E031",
            Span::new(0, 0),
            format!("a standard-library file must declare `package {path}`"),
        ),
    };
    Err(diagnostics(source, vec![diagnostic]))
}

/// Module paths in import order, smallest path first among the ready ones;
/// E032 when imports among the modules form a cycle.
fn import_order(parsed: &BTreeMap<String, Parsed>) -> Result<Vec<String>, Error> {
    let edges: BTreeMap<&str, BTreeSet<&str>> = parsed
        .iter()
        .map(|(path, module)| {
            let targets = module
                .program
                .imports
                .iter()
                .map(imported_module)
                .filter_map(|m| parsed.get_key_value(&m).map(|(k, _)| k.as_str()))
                .collect();
            (path.as_str(), targets)
        })
        .collect();
    let mut done: BTreeSet<&str> = BTreeSet::new();
    let mut order = Vec::new();
    while done.len() < edges.len() {
        let ready = edges
            .iter()
            .find(|(path, targets)| !done.contains(*path) && targets.is_subset(&done))
            .map(|(path, _)| *path);
        match ready {
            Some(path) => {
                done.insert(path);
                order.push(path.to_string());
            }
            None => return Err(cycle_error(parsed, &edges, &done)),
        }
    }
    Ok(order)
}

/// E032 at the import that closes the first cycle found among the modules
/// not yet ordered.
fn cycle_error(
    parsed: &BTreeMap<String, Parsed>,
    edges: &BTreeMap<&str, BTreeSet<&str>>,
    done: &BTreeSet<&str>,
) -> Error {
    let pending = |path: &&str| !done.contains(path);
    let start = edges
        .keys()
        .copied()
        .find(pending)
        .expect("a cycle has a module");
    let mut walk: Vec<&str> = vec![start];
    let cycle = loop {
        let current = *walk.last().expect("walk is not empty");
        let next = edges[current]
            .iter()
            .copied()
            .find(pending)
            .expect("every pending module imports a pending module");
        if let Some(at) = walk.iter().position(|p| *p == next) {
            let mut cycle = walk[at..].to_vec();
            cycle.push(next);
            break cycle;
        }
        walk.push(next);
    };
    let (from, to) = (cycle[0], cycle[1]);
    let module = &parsed[from];
    let span = module
        .program
        .imports
        .iter()
        .find(|import| imported_module(import) == to)
        .map_or(Span::new(0, 0), |import| import.span);
    diagnostics(
        &module.source,
        vec![Diagnostic::new(
            "E032",
            span,
            format!("import cycle: {}", cycle.join(" -> ")),
        )],
    )
}

/// The standard library at `std`, or at [`default_std_path`].
fn std_library(std: Option<&Path>) -> Result<StdLibrary, Error> {
    match std {
        Some(root) => load_std(root),
        None => load_std(&default_std_path()),
    }
}

/// The module path of a program: its package, or `main` without one (as
/// `ignite_types` names it).
fn program_module(program: &Program) -> String {
    program
        .package
        .as_ref()
        .map_or_else(|| "main".to_string(), |package| package.dotted())
}

/// Every standard-library module's source, the program's and its
/// siblings', by module path: the generators print `assert` and `?`
/// positions from them.
fn sources(library: &StdLibrary, typed: &TypedFile) -> BTreeMap<String, SourceFile> {
    let mut sources: BTreeMap<String, SourceFile> = library
        .modules
        .iter()
        .map(|module| (module.path.clone(), module.source.clone()))
        .collect();
    for sibling in &typed.siblings {
        sources.insert(sibling.path.clone(), sibling.source.clone());
    }
    sources.insert(typed.types.module.clone(), typed.source.clone());
    sources
}

/// Type every standard-library module in import order
/// (`docs/llvm.md` section 2): each module's types, and every module's
/// exports by module path.
pub fn type_std(library: &StdLibrary) -> Result<(Vec<TypedProgram>, TypedModules), Error> {
    let mut modules = TypedModules::new();
    let mut typed = Vec::new();
    for module in &library.modules {
        let types = ignite_types::type_module(&module.program, &module.symbols, &modules)
            .map_err(|d| diagnostics(&module.source, d))?;
        modules.insert(module.path.clone(), types.exports());
        typed.push(types);
    }
    Ok((typed, modules))
}

/// A module of the program's package other than the program itself
/// (`docs/slice.md` section 14.4): the file `<name>.ignite` beside the
/// program whose `package` is `<program package>.<name>`, checked and
/// typed.
pub(crate) struct Sibling {
    pub path: String,
    pub source: SourceFile,
    pub program: Program,
    pub symbols: Symbols,
    pub types: TypedProgram,
}

/// A program file, checked and typed against the typed standard library
/// and its sibling modules.
pub(crate) struct TypedFile {
    pub source: SourceFile,
    pub program: Program,
    pub symbols: Symbols,
    pub types: TypedProgram,
    /// In import order.
    pub siblings: Vec<Sibling>,
}

/// [`TypedFile`] with the standard library's types.
struct Typed {
    file: TypedFile,
    std_types: Vec<TypedProgram>,
}

/// Whether the file text declares `package <path>` as its first
/// declaration, read without parsing so that an unrelated file beside the
/// program is never parsed.
fn declares_package(text: &str, path: &str) -> bool {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("//"))
        .and_then(|line| line.strip_prefix("package "))
        .is_some_and(|rest| rest.trim() == path)
}

/// Parse, check and type the sibling modules of the program at `file` in
/// import order, each seeing the standard library and the siblings before
/// it; a program without a `package` has none. Returns the siblings with
/// the symbols and types the program is checked against.
fn load_siblings(
    file: &Path,
    program: &Program,
    library: &StdLibrary,
    std_types: &TypedModules,
) -> Result<(Vec<Sibling>, Modules, TypedModules), Error> {
    let mut modules = library.symbols();
    let mut typed = std_types.clone();
    let Some(package) = program.package.as_ref().map(|p| p.dotted()) else {
        return Ok((Vec::new(), modules, typed));
    };
    let dir = match file.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    let own = file.canonicalize().ok();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(io_error(dir))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()
        .map_err(io_error(dir))?;
    entries.sort();
    let mut parsed: BTreeMap<String, Parsed> = BTreeMap::new();
    for entry in entries {
        if entry.extension().is_none_or(|ext| ext != "ignite")
            || !entry.is_file()
            || entry.canonicalize().ok() == own
        {
            continue;
        }
        let Some(stem) = entry.file_stem() else {
            continue;
        };
        let path = format!("{package}.{}", stem.to_string_lossy());
        let source = read_source(&entry)?;
        if !declares_package(&source.text, &path) {
            continue;
        }
        let program = parse(&source)?;
        parsed.insert(
            path.clone(),
            Parsed {
                path,
                source,
                program,
            },
        );
    }
    let mut siblings = Vec::new();
    for path in import_order(&parsed)? {
        let Parsed {
            path,
            source,
            program,
        } = parsed.remove(&path).expect("ordered modules are parsed");
        let symbols =
            ignite_check::check_module(&program, &modules).map_err(|d| diagnostics(&source, d))?;
        let types = ignite_types::type_module(&program, &symbols, &typed)
            .map_err(|d| diagnostics(&source, d))?;
        modules.insert(path.clone(), symbols.clone());
        typed.insert(path.clone(), types.exports());
        siblings.push(Sibling {
            path,
            source,
            program,
            symbols,
            types,
        });
    }
    Ok((siblings, modules, typed))
}

/// Read, parse, check and type the program at `file` and its sibling
/// modules against the typed standard library `std_types`.
pub(crate) fn type_program_with(
    file: &Path,
    library: &StdLibrary,
    std_types: &TypedModules,
) -> Result<TypedFile, Error> {
    let source = read_source(file)?;
    let program = parse(&source)?;
    let (siblings, modules, typed) = load_siblings(file, &program, library, std_types)?;
    let symbols =
        ignite_check::check_module(&program, &modules).map_err(|d| diagnostics(&source, d))?;
    let types = ignite_types::type_module(&program, &symbols, &typed)
        .map_err(|d| diagnostics(&source, d))?;
    Ok(TypedFile {
        source,
        program,
        symbols,
        types,
        siblings,
    })
}

fn type_program(file: &Path, library: &StdLibrary) -> Result<Typed, Error> {
    let (std_types, modules) = type_std(library)?;
    let file = type_program_with(file, library, &modules)?;
    Ok(Typed { file, std_types })
}

/// Generate both projects; diagnostics of a standard-library module or a
/// sibling are rendered with its file.
fn generate(file: &Path, library: &StdLibrary, substrate: PathBuf) -> Result<Emitted, Error> {
    let typed = type_program(file, library)?;
    let options = ignite_rustgen::Options {
        substrate_path: substrate,
        sources: sources(library, &typed.file),
        ..ignite_rustgen::Options::default()
    };
    let std_modules: Vec<ignite_rustgen::TypedStdModule<'_>> = library
        .modules
        .iter()
        .zip(&typed.std_types)
        .map(|(m, t)| (m.path.clone(), &m.program, &m.symbols, t))
        .collect();
    let std = ignite_rustgen::generate_std_typed(&std_modules, &options).map_err(|errors| {
        let (path, found) = errors.into_iter().next().expect("an error has a module");
        let module = library
            .modules
            .iter()
            .find(|m| m.path == path)
            .expect("errors name loaded modules");
        diagnostics(&module.source, found)
    })?;
    let file = &typed.file;
    let siblings: Vec<ignite_rustgen::SiblingModule<'_>> = file
        .siblings
        .iter()
        .map(|s| (s.path.clone(), &s.program, &s.symbols, &s.types))
        .collect();
    let program = ignite_rustgen::generate_package(
        &file.program,
        &file.symbols,
        &file.types,
        &siblings,
        &options,
    )
    .map_err(|errors| {
        let (path, found) = errors.into_iter().next().expect("an error has a module");
        let source = file
            .siblings
            .iter()
            .find(|s| s.path == path)
            .map_or(&file.source, |s| &s.source);
        diagnostics(source, found)
    })?;
    Ok(Emitted { program, std })
}

/// `ignite check`: load the standard library, then parse, check and type
/// the program and its sibling modules against it; `Ok` means no
/// diagnostics. The type checker owns the system rules (`E020`, `E104` to
/// `E109`), so nothing is generated.
pub fn check(file: &Path) -> Result<(), Error> {
    check_with_std(file, None)
}

/// [`check`] with the standard library at `std` (default
/// [`default_std_path`]).
pub fn check_with_std(file: &Path, std: Option<&Path>) -> Result<(), Error> {
    let library = std_library(std)?;
    type_program(file, &library)?;
    Ok(())
}

/// `ignite emit`: the generated program project without writing or
/// building it, with the default standard library.
pub fn emit(file: &Path, substrate: Option<&Path>) -> Result<GeneratedProject, Error> {
    emit_with_std(file, substrate, None).map(|emitted| emitted.program)
}

/// `ignite emit`: both generated projects, with the standard library at
/// `std` (default [`default_std_path`]).
pub fn emit_with_std(
    file: &Path,
    substrate: Option<&Path>,
    std: Option<&Path>,
) -> Result<Emitted, Error> {
    let library = std_library(std)?;
    let substrate = substrate
        .map(Path::to_path_buf)
        .unwrap_or_else(default_substrate_path);
    generate(file, &library, substrate)
}

/// `ignite build`: everything, then `cargo build`. The program is written to
/// `out` and the standard library to `out/std`.
///
/// Cargo is `$CARGO` when set, else `cargo` from `PATH`; its output is
/// inherited. A note on stderr precedes it, saying that compiler messages
/// refer to the generated code.
pub fn build(file: &Path, options: &BuildOptions) -> Result<BuildOutput, Error> {
    if options.target == Target::Llvm {
        return llvm_build::build(file, options);
    }
    let library = std_library(options.std.as_deref())?;
    let substrate = options
        .substrate
        .clone()
        .unwrap_or_else(default_substrate_path);
    let emitted = generate(file, &library, substrate)?;
    let project = emitted.program;
    let out = options.out.clone().unwrap_or_else(|| {
        Path::new("target")
            .join("ignite")
            .join(&project.package_name)
    });
    ignite_rustgen::write_project(&project, &out).map_err(io_error(&out))?;
    let std_out = out.join("std");
    ignite_rustgen::write_project(&emitted.std, &std_out).map_err(io_error(&std_out))?;
    let target_dir = options
        .target_dir
        .clone()
        .unwrap_or_else(|| out.join("target"));
    eprintln!(
        "note: building the generated Rust project in {}; compiler messages refer to generated code",
        out.display()
    );
    cargo_build(&out, &target_dir, options.release, false)?;
    let profile = if options.release { "release" } else { "debug" };
    let executables = project
        .executables
        .iter()
        .map(|executable| {
            target_dir.join(profile).join(format!(
                "{}{}",
                executable.name,
                std::env::consts::EXE_SUFFIX
            ))
        })
        .collect();
    Ok(BuildOutput {
        project_dir: out,
        executables,
    })
}

/// `cargo build` of the generated project in `out`, `--quiet` when `quiet`.
/// Cargo is `$CARGO` when set, else `cargo` from `PATH`.
fn cargo_build(out: &Path, target_dir: &Path, release: bool, quiet: bool) -> Result<(), Error> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(&cargo);
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(out.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(target_dir);
    if release {
        command.arg("--release");
    }
    if quiet {
        command.arg("--quiet");
    }
    let status = command.status().map_err(|error| Error::Io {
        path: PathBuf::from(&cargo),
        error,
    })?;
    if !status.success() {
        return Err(Error::Cargo {
            status: status.code().unwrap_or(-1),
        });
    }
    Ok(())
}

/// Where the runtime archive is looked for, in order, for an executable at
/// `exe`: `libignite_rt.a` in its directory, then in that directory's
/// parent. The second covers a driver running inside a test binary in
/// `target/<profile>/deps`, whose archive `cargo build -p ignite-rt` writes
/// to `target/<profile>`.
pub fn runtime_candidates(exe: &Path) -> Vec<PathBuf> {
    let Some(dir) = exe.parent() else {
        return vec![PathBuf::from("libignite_rt.a")];
    };
    let mut candidates = vec![dir.join("libignite_rt.a")];
    if let Some(parent) = dir.parent() {
        candidates.push(parent.join("libignite_rt.a"));
    }
    candidates
}

/// The first of [`runtime_candidates`] for the running executable that is
/// a file, else the first candidate.
pub fn default_runtime_path() -> PathBuf {
    let candidates = std::env::current_exe()
        .map(|exe| runtime_candidates(&exe))
        .unwrap_or_else(|_| vec![PathBuf::from("libignite_rt.a")]);
    candidates
        .iter()
        .find(|candidate| candidate.is_file())
        .unwrap_or(&candidates[0])
        .clone()
}

/// The LLVM IR modules of a program: `std.ll`, `program.ll` and one
/// `main_<executable>.ll` per component, with the package name.
pub fn emit_llvm(
    file: &Path,
    std: Option<&Path>,
) -> Result<(String, ignite_llvm::Generated), Error> {
    emit_llvm_with(file, std, |_| Ok(None))
}

/// [`emit_llvm`] pinned to the target `target` returns for the package
/// name (`docs/llvm.md` section 1). Debug metadata names the source files
/// relative to the current directory.
pub(crate) fn emit_llvm_with(
    file: &Path,
    std: Option<&Path>,
    target: impl FnOnce(&str) -> Result<Option<ignite_llvm::TargetSpec>, Error>,
) -> Result<(String, ignite_llvm::Generated), Error> {
    let library = std_library(std)?;
    let typed = type_program(file, &library)?;
    let std_inputs: Vec<ignite_llvm::ModuleInput<'_>> = library
        .modules
        .iter()
        .zip(&typed.std_types)
        .map(|(module, types)| ignite_llvm::ModuleInput {
            path: &module.path,
            program: &module.program,
            symbols: &module.symbols,
            types,
        })
        .collect();
    let file = &typed.file;
    let program = ignite_llvm::ModuleInput {
        path: &file.types.module,
        program: &file.program,
        symbols: &file.symbols,
        types: &file.types,
    };
    let sources = sources(&library, file);
    let package = ignite_rustgen::package_name(&file.program);
    let options = ignite_llvm::Options {
        sources: sources
            .iter()
            .map(|(path, source)| (path.clone(), source))
            .collect(),
        target: target(&package)?,
        directory: std::env::current_dir()
            .ok()
            .map(|dir| dir.display().to_string()),
        siblings: file
            .siblings
            .iter()
            .map(|sibling| ignite_llvm::ModuleInput {
                path: &sibling.path,
                program: &sibling.program,
                symbols: &sibling.symbols,
                types: &sibling.types,
            })
            .collect(),
        ..ignite_llvm::Options::default()
    };
    let generated = ignite_llvm::generate_with(&std_inputs, &program, &options);
    Ok((package, generated))
}
