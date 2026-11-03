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

pub mod apply;
mod llvm_build;
pub mod rfc;
pub mod run;
mod test_build;

pub use llvm_build::{COMPILE_FLAGS, LINK_FLAGS, RELEASE_LINK_FLAGS};
pub use run::Standing;
pub use test_build::{TestGroup, TestOptions, build_tests, test, test_groups, test_targets};

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

/// Options of `ignite plan` and `ignite explain` (`docs/slice.md` sections
/// 15.7 and 15.8).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PlanOptions {
    /// `--assembly NAME`: the assembly to select; default the program's
    /// only one (15.6).
    pub assembly: Option<String>,
    /// `--fuel N`: the fuel limit per property expression (15.3, O14);
    /// default `ignite_eval::Fuel::DEFAULT_LIMIT`.
    pub fuel: Option<u64>,
    /// Root of the standard library. Default: [`default_std_path`].
    pub std: Option<PathBuf>,
}

/// The clock of `ignite run` (`docs/slice.md` 16.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ClockMode {
    /// `--clock real`: `at` is the monotonic clock offset by the segment's
    /// start, the epoch the wall time the journal was created at.
    #[default]
    Real,
    /// `--clock manual`: time moves only on `advance`; the epoch is
    /// `--epoch`, default `2026-01-01T00:00:00Z`.
    Manual,
}

/// Options of `ignite run` (`docs/slice.md` 16.3, 16.16).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RunOptions {
    /// `--assembly NAME`: the assembly to run; default the program's only
    /// one (15.6).
    pub assembly: Option<String>,
    /// `--state DIR`: the journal, lock and `bin/<generation>/`; default
    /// `target/ignite-run/<assembly>/` under the working directory.
    pub state: Option<PathBuf>,
    /// `--build DIR`: the build project; default `<state>/build/`.
    pub build: Option<PathBuf>,
    pub target: Target,
    pub release: bool,
    pub clock: ClockMode,
    /// `--epoch RFC3339`, manual clock only.
    pub epoch: Option<String>,
    /// `--settle DURATION`: the settle rule of 16.4 as a `Duration`
    /// literal; default `2 s` (O29).
    pub settle: Option<String>,
    /// `--port RESOURCE=PORT`: `0` binds a free port.
    pub ports: Vec<(String, u16)>,
    /// `--json`: the journal records go to stdout.
    pub json: bool,
    /// `--fresh`: rename the old journal (O31).
    pub fresh: bool,
    /// Root of the standard library. Default: [`default_std_path`].
    pub std: Option<PathBuf>,
    /// `--fuel N`: the fuel limit per property expression and per reducer
    /// step (15.3, 16.6); default `ignite_eval::Fuel::DEFAULT_LIMIT`.
    pub fuel: Option<u64>,
    /// `--runtime PATH`: the runtime archive of the LLVM target. Default:
    /// [`default_runtime_path`].
    pub runtime: Option<PathBuf>,
    /// `--target-dir DIR`: cargo's target directory of the Rust target.
    /// Default: `<build>/target`. Tests share one across programs.
    pub target_dir: Option<PathBuf>,
}

/// Options of `ignite replay` (`docs/slice.md` 16.3, 16.16).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ReplayOptions {
    pub assembly: Option<String>,
    pub state: Option<PathBuf>,
    pub std: Option<PathBuf>,
    pub fuel: Option<u64>,
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
    /// Module path, e.g. `std.control`.
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

/// Imports and literal `module("p")` calls are dependencies before typing:
/// module records resolve their functions without an accompanying import.
/// Walk expression chains iteratively, including defaults and test bodies.
fn module_dependencies(program: &Program) -> BTreeSet<String> {
    use ignite_syntax::ast::{Block, Decl, Expr, ExprKind, StmtKind};

    enum Node<'a> {
        Expr(&'a Expr),
        Block(&'a Block),
    }
    let mut dependencies: BTreeSet<String> = program.imports.iter().map(imported_module).collect();
    let mut pending = Vec::new();
    for decl in &program.decls {
        match decl {
            Decl::Record(record) => pending.extend(
                record
                    .fields
                    .iter()
                    .filter_map(|f| f.default.as_ref())
                    .map(Node::Expr),
            ),
            Decl::Fn(function) => pending.push(Node::Block(&function.body)),
            Decl::Protocol(protocol) => {
                pending.extend(protocol.fns.iter().map(|f| Node::Block(&f.body)))
            }
            Decl::Const(constant) => pending.push(Node::Expr(&constant.value)),
            Decl::Test(test) => pending.push(Node::Block(&test.body)),
            Decl::Assembly(assembly) => {
                pending.extend(assembly.properties.iter().map(|p| Node::Expr(&p.value)))
            }
            Decl::System(system) => {
                pending.extend(
                    system
                        .resources
                        .iter()
                        .flat_map(|r| &r.properties)
                        .map(|p| Node::Expr(&p.value)),
                );
                pending.extend(
                    system
                        .components
                        .iter()
                        .chain(&system.contracts)
                        .chain(&system.policies)
                        .flat_map(|c| &c.properties)
                        .map(|p| Node::Expr(&p.value)),
                );
            }
            Decl::Enum(_) | Decl::Alias(_) => {}
        }
    }
    let own = program.package.as_ref().map(|p| p.dotted());
    while let Some(node) = pending.pop() {
        match node {
            Node::Block(block) => {
                for stmt in &block.stmts {
                    match &stmt.kind {
                        StmtKind::Let { value, .. }
                        | StmtKind::LetPattern { value, .. }
                        | StmtKind::Assign { value, .. }
                        | StmtKind::Expr(value) => pending.push(Node::Expr(value)),
                        StmtKind::Return(value) => pending.extend(value.iter().map(Node::Expr)),
                        StmtKind::While { cond, body } => {
                            pending.push(Node::Expr(cond));
                            pending.push(Node::Block(body));
                        }
                        StmtKind::Loop { body } => pending.push(Node::Block(body)),
                        StmtKind::For { iter, body, .. }
                        | StmtKind::ForPattern { iter, body, .. } => {
                            pending.push(Node::Expr(iter));
                            pending.push(Node::Block(body));
                        }
                        StmtKind::Assert { cond, message } => {
                            pending.push(Node::Expr(cond));
                            pending.extend(message.iter().map(Node::Expr));
                        }
                        StmtKind::Break | StmtKind::Continue => {}
                    }
                }
            }
            Node::Expr(expr) => match &expr.kind {
                ExprKind::Call { callee, args } => {
                    if matches!(&callee.kind, ExprKind::Name(name) if name.name == "module")
                        && let [arg] = args.as_slice()
                        && let ExprKind::Str(path) = &arg.kind
                        && own.as_ref() != Some(path)
                    {
                        dependencies.insert(path.clone());
                    }
                    pending.push(Node::Expr(callee));
                    pending.extend(args.iter().map(Node::Expr));
                }
                ExprKind::Method { receiver, args, .. } => {
                    pending.push(Node::Expr(receiver));
                    pending.extend(args.iter().map(Node::Expr));
                }
                ExprKind::Field { receiver, .. } | ExprKind::TupleField { receiver, .. } => {
                    pending.push(Node::Expr(receiver))
                }
                ExprKind::Index { receiver, index } => {
                    pending.push(Node::Expr(receiver));
                    pending.push(Node::Expr(index));
                }
                ExprKind::Try(value)
                | ExprKind::Await(value)
                | ExprKind::Spawn(value)
                | ExprKind::Unary { operand: value, .. }
                | ExprKind::NamedArg { value, .. } => pending.push(Node::Expr(value)),
                ExprKind::Binary { left, right, .. } => {
                    pending.push(Node::Expr(left));
                    pending.push(Node::Expr(right));
                }
                ExprKind::Return(value) => pending.extend(value.iter().map(|v| Node::Expr(v))),
                ExprKind::List(values) | ExprKind::Tuple(values) => {
                    pending.extend(values.iter().map(Node::Expr))
                }
                ExprKind::Record { fields, .. } => {
                    pending.extend(fields.iter().map(|f| Node::Expr(&f.value)))
                }
                ExprKind::RecordUpdate { fields, base, .. } => {
                    pending.extend(fields.iter().map(|f| Node::Expr(&f.value)));
                    pending.push(Node::Expr(base));
                }
                ExprKind::Match { scrutinee, arms } => {
                    pending.push(Node::Expr(scrutinee));
                    pending.extend(arms.iter().map(|a| Node::Expr(&a.body)));
                }
                ExprKind::If {
                    cond,
                    then,
                    otherwise,
                } => {
                    pending.push(Node::Expr(cond));
                    pending.push(Node::Block(then));
                    pending.extend(otherwise.iter().map(|v| Node::Expr(v)));
                }
                ExprKind::Block(block) => pending.push(Node::Block(block)),
                ExprKind::Closure(closure) => pending.push(Node::Expr(&closure.body)),
                ExprKind::Brace { entries } => {
                    pending.extend(entries.iter().map(|entry| Node::Expr(&entry.value)))
                }
                ExprKind::Int(_)
                | ExprKind::Str(_)
                | ExprKind::Bool(_)
                | ExprKind::Name(_)
                | ExprKind::Break
                | ExprKind::Continue
                | ExprKind::Quantity { .. } => {}
            },
        }
    }
    dependencies
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

/// `std.a.b` is `<root>/a/b.ignite`; `None` for a path outside `std`.
fn std_file(root: &Path, module: &str) -> Option<PathBuf> {
    let rest = module.strip_prefix("std.")?;
    if rest.split('.').any(|segment| {
        let mut bytes = segment.bytes();
        !bytes
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
            || !bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
    }) {
        return None;
    }
    let mut file = root.to_path_buf();
    for segment in rest.split('.') {
        file.push(segment);
    }
    file.set_extension("ignite");
    Some(file)
}

/// The canonical standard-library root at `std`, or [`default_std_path`].
fn std_root(std: Option<&Path>) -> Result<PathBuf, Error> {
    let root = std.map_or_else(default_std_path, Path::to_path_buf);
    root.canonicalize().map_err(io_error(&root))
}

/// Load, parse and check every `.ignite` file under `root`. Files are
/// checked in import order with the modules checked before them.
pub fn load_std(root: &Path) -> Result<StdLibrary, Error> {
    let root = root.canonicalize().map_err(io_error(root))?;
    let mut files = Vec::new();
    ignite_files(&root, &mut files)?;
    let files: BTreeMap<String, PathBuf> = files
        .into_iter()
        .map(|file| (module_path(&root, &file), file))
        .collect();
    let roots = files.keys().cloned().collect();
    load_modules(root, &roots, |module| files.get(module).cloned())
}

/// Load, parse and check the standard-library modules under `root` that
/// `roots` reach: the modules named and, transitively, the modules their
/// imports and literal module records name (`docs/slice.md` 16.22). A named module without a file
/// (`std.io`, whose names are the substrate's builtins; a module a program
/// imports that does not exist, which the checker reports at the import)
/// is skipped. The modules loaded are checked in import order, as
/// [`load_std`] checks all of them.
pub fn load_std_reachable(root: &Path, roots: &BTreeSet<String>) -> Result<StdLibrary, Error> {
    let root = root.canonicalize().map_err(io_error(root))?;
    load_modules(root.clone(), roots, |module| {
        std_file(&root, module).filter(|file| file.is_file())
    })
}

/// Parse the modules `roots` reach under the canonical `root`, each found
/// by `locate`, then check them in import order.
fn load_modules(
    root: PathBuf,
    roots: &BTreeSet<String>,
    locate: impl Fn(&str) -> Option<PathBuf>,
) -> Result<StdLibrary, Error> {
    let mut pending = roots.clone();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut parsed: BTreeMap<String, Parsed> = BTreeMap::new();
    while let Some(path) = pending.pop_first() {
        if !seen.insert(path.clone()) {
            continue;
        }
        let Some(file) = locate(&path) else {
            continue;
        };
        let source = read_source(&file)?;
        let program = parse(&source)?;
        check_package(&path, &source, &program)?;
        pending.extend(
            module_dependencies(&program)
                .into_iter()
                .filter(|module| !seen.contains(module)),
        );
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
            let targets = module_dependencies(&module.program)
                .into_iter()
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

/// The standard-library modules the evaluator calls into for `check`,
/// `plan` and `explain` beyond a program's imports (`docs/slice.md`
/// 16.22): `std.deploy`, whose `admits`, `includes` and `entries` resolve
/// an assembly (15.6, 16.17; `ignite_eval::resolve` builds its `Member`).
const EVAL_ROOTS: &[&str] = &["std.deploy"];

/// The same for `run` and `replay`, plus the reducers' modules the run
/// driver names: `std.control` (`ignite_eval::CONTROL_MODULE`: `step`,
/// `standing_at`, `companions`, `Event`, `Effect`, `Segment`; E125 without
/// it) and `std.sli` (`ProbeAttempt`, the probe event's payload in
/// `run::live`). `std.time` is reached through `std.control`'s import.
const RUN_ROOTS: &[&str] = &["std.deploy", "std.control", "std.sli"];

/// The program at `file`, parsed with its siblings, and the standard
/// library at `std` (default [`default_std_path`]) reduced to the modules
/// they reach (`docs/slice.md` 16.22): the modules their imports name,
/// transitively, `extra`, and the file's own module when it lies under the
/// standard library's root (so `ignite check std/x.ignite` checks it as a
/// module too, as it did when every module was loaded).
fn load_program_std(
    file: &Path,
    std: Option<&Path>,
    extra: &[&str],
) -> Result<(ParsedFile, StdLibrary), Error> {
    let root = std_root(std)?;
    let parsed = parse_program(file)?;
    let mut roots: BTreeSet<String> = parsed.std_modules().collect();
    let has_assembly = parsed
        .program
        .decls
        .iter()
        .any(|decl| matches!(decl, ignite_syntax::ast::Decl::Assembly(_)));
    roots.extend(
        extra
            .iter()
            .filter(|&&module| module != "std.deploy" || has_assembly)
            .map(|module| module.to_string()),
    );
    if let Ok(own) = file.canonicalize()
        && own.starts_with(&root)
    {
        roots.insert(module_path(&root, &own));
    }
    let library = load_std_reachable(&root, &roots)?;
    Ok((parsed, library))
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

/// A program file and its sibling modules (`docs/slice.md` section 14.4),
/// read and parsed, before the standard library is loaded: their imports
/// name the standard-library modules a command loads (16.22).
pub(crate) struct ParsedFile {
    source: SourceFile,
    program: Program,
    /// By module path, not yet ordered.
    siblings: BTreeMap<String, Parsed>,
}

impl ParsedFile {
    /// The program's module path (`program_module`).
    pub(crate) fn module(&self) -> String {
        program_module(&self.program)
    }

    /// Every module the program and its siblings import, standard-library
    /// or not; the loader skips the ones that are no standard-library file.
    fn std_modules(&self) -> impl Iterator<Item = String> + '_ {
        std::iter::once(&self.program)
            .chain(self.siblings.values().map(|sibling| &sibling.program))
            .flat_map(module_dependencies)
    }
}

/// Read and parse the program at `file` and its sibling modules: the
/// files `<name>.ignite` beside it whose `package` is `<program
/// package>.<name>`; a program without a `package` has none.
pub(crate) fn parse_program(file: &Path) -> Result<ParsedFile, Error> {
    let source = read_source(file)?;
    let program = parse(&source)?;
    let siblings = parse_siblings(file, &program)?;
    Ok(ParsedFile {
        source,
        program,
        siblings,
    })
}

/// Parse the sibling modules of the program at `file`, by module path.
fn parse_siblings(file: &Path, program: &Program) -> Result<BTreeMap<String, Parsed>, Error> {
    let mut parsed: BTreeMap<String, Parsed> = BTreeMap::new();
    let Some(package) = program.package.as_ref().map(|p| p.dotted()) else {
        return Ok(parsed);
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
    Ok(parsed)
}

/// Check and type the parsed sibling modules in import order, each seeing
/// the standard library and the siblings before it. Returns the siblings
/// with the symbols and types the program is checked against.
fn check_siblings(
    mut parsed: BTreeMap<String, Parsed>,
    library: &StdLibrary,
    std_types: &TypedModules,
) -> Result<(Vec<Sibling>, Modules, TypedModules), Error> {
    let mut modules = library.symbols();
    let mut typed = std_types.clone();
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

/// Check and type a parsed program and its sibling modules against the
/// typed standard library `std_types`.
pub(crate) fn type_parsed_with(
    parsed: ParsedFile,
    library: &StdLibrary,
    std_types: &TypedModules,
) -> Result<TypedFile, Error> {
    let ParsedFile {
        source,
        program,
        siblings,
    } = parsed;
    let (siblings, modules, typed) = check_siblings(siblings, library, std_types)?;
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

fn type_parsed(parsed: ParsedFile, library: &StdLibrary) -> Result<Typed, Error> {
    let (std_types, modules) = type_std(library)?;
    let file = type_parsed_with(parsed, library, &modules)?;
    Ok(Typed { file, std_types })
}

/// Generate both projects; diagnostics of a standard-library module or a
/// sibling are rendered with its file.
fn generate(
    parsed: ParsedFile,
    library: &StdLibrary,
    substrate: PathBuf,
) -> Result<Emitted, Error> {
    let typed = type_parsed(parsed, library)?;
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
    let std =
        ignite_rustgen::generate_std_for(&std_modules, &options, &program).map_err(|errors| {
            let (path, found) = errors.into_iter().next().expect("an error has a module");
            let module = library
                .modules
                .iter()
                .find(|m| m.path == path)
                .expect("errors name loaded modules");
            diagnostics(&module.source, found)
        })?;
    Ok(Emitted { program, std })
}

/// `ignite check`: load the standard library, then parse, check and type
/// the program and its sibling modules against it, then evaluate every
/// property expression of every system and assembly and resolve every
/// assembly (`docs/slice.md` O20, E110 to E118); `Ok` means no
/// diagnostics. The type checker owns the system rules (`E020`, `E104` to
/// `E109`), so nothing is generated.
pub fn check(file: &Path) -> Result<(), Error> {
    check_with_std(file, None)
}

/// [`check`] with the standard library at `std` (default
/// [`default_std_path`]).
pub fn check_with_std(file: &Path, std: Option<&Path>) -> Result<(), Error> {
    load(file, std)?.check()
}

/// A program read, parsed, checked and typed with the standard library and
/// its sibling modules: the input of the evaluator (`docs/slice.md`
/// section 15.9), as [`load`] returns it.
pub struct Loaded {
    library: StdLibrary,
    typed: Typed,
}

impl Loaded {
    /// Every module the evaluator may reach: the standard library, the
    /// siblings and the program, by module path.
    pub fn modules(&self) -> ignite_eval::Modules<'_> {
        let std = self
            .library
            .modules
            .iter()
            .zip(&self.typed.std_types)
            .map(|(module, types)| ignite_eval::Module {
                path: &module.path,
                source: &module.source,
                program: &module.program,
                types,
            });
        let file = &self.typed.file;
        let siblings = file.siblings.iter().map(|sibling| ignite_eval::Module {
            path: &sibling.path,
            source: &sibling.source,
            program: &sibling.program,
            types: &sibling.types,
        });
        let program = ignite_eval::Module {
            path: &file.types.module,
            source: &file.source,
            program: &file.program,
            types: &file.types,
        };
        std.chain(siblings)
            .chain(std::iter::once(program))
            .collect()
    }

    /// The program's types.
    pub fn types(&self) -> &TypedProgram {
        &self.typed.file.types
    }

    /// The program's AST, the object its types were computed from.
    pub fn program(&self) -> &Program {
        &self.typed.file.program
    }

    /// The program's source.
    pub fn source(&self) -> &SourceFile {
        &self.typed.file.source
    }

    /// The evaluation and resolution rules of `ignite check` (`docs/slice.md`
    /// O20) over every system and assembly, with the default fuel.
    pub fn check(&self) -> Result<(), Error> {
        let modules = self.modules();
        let mut evaluator = ignite_eval::Evaluator::new(&modules, ignite_eval::Fuel::default());
        ignite_eval::Model::check(&mut evaluator, self.types())
            .map_err(|found| diagnostics(self.source(), found))
    }

    /// The model of `options.assembly` (`docs/slice.md` section 15.5),
    /// diagnostics rendered with the program's file.
    pub fn model(&self, options: &PlanOptions) -> Result<ignite_eval::Model, Error> {
        let modules = self.modules();
        let mut evaluator = ignite_eval::Evaluator::new(&modules, fuel(options));
        ignite_eval::model(&mut evaluator, self.types(), options.assembly.as_deref())
            .map_err(|found| diagnostics(self.source(), found))
    }
}

fn fuel(options: &PlanOptions) -> ignite_eval::Fuel {
    options
        .fuel
        .map_or_else(ignite_eval::Fuel::default, ignite_eval::Fuel::with_units)
}

/// Read, parse, check and type the program at `file` and its siblings
/// against the standard library at `std` (default [`default_std_path`]),
/// for evaluation (`docs/slice.md` section 15.3).
pub fn load(file: &Path, std: Option<&Path>) -> Result<Loaded, Error> {
    load_for(file, std, EVAL_ROOTS)
}

/// [`load`] with the standard library reduced to the modules the program
/// reaches plus `extra` (16.22).
fn load_for(file: &Path, std: Option<&Path>, extra: &[&str]) -> Result<Loaded, Error> {
    let (parsed, library) = load_program_std(file, std, extra)?;
    let typed = type_parsed(parsed, &library)?;
    Ok(Loaded { library, typed })
}

/// `ignite plan`: the system model of the program at `file` as canonical
/// JSON (`docs/slice.md` sections 15.5 to 15.7), after the same pipeline
/// as [`check`]. Diagnostics of the evaluation and resolution (E110 to
/// E118) are rendered with the program's file.
pub fn plan(file: &Path, options: &PlanOptions) -> Result<String, Error> {
    let loaded = load(file, options.std.as_deref())?;
    Ok(loaded.model(options)?.to_json())
}

/// `ignite explain`: the explanation of `path` (a system, member, property
/// path or assembly) in the model of the program at `file`
/// (`docs/slice.md` section 15.8); E119 for a path that names nothing.
pub fn explain(file: &Path, path: &str, options: &PlanOptions) -> Result<String, Error> {
    let loaded = load(file, options.std.as_deref())?;
    let model = loaded.model(options)?;
    ignite_eval::explain(&model, path).map_err(|found| diagnostics(loaded.source(), vec![found]))
}

/// The default settle limit of 16.4 (O29).
const DEFAULT_SETTLE: std::time::Duration = std::time::Duration::from_secs(2);

/// The `--state` of 16.3: the option, else
/// `target/ignite-run/<assembly>/` under the working directory.
fn state_dir(state: Option<&Path>, assembly: &str) -> PathBuf {
    state
        .map(Path::to_path_buf)
        .unwrap_or_else(|| Path::new("target").join("ignite-run").join(assembly))
}

/// A diagnostic of the run phase (E125 to E127) at the program's file.
fn run_diagnostic(loaded: &Loaded, code: &'static str, message: String) -> Error {
    diagnostics(
        loaded.source(),
        vec![Diagnostic::new(code, Span::new(0, 0), message)],
    )
}

/// The model of a run or replay, with E125 (16.17): the program must
/// declare an assembly, and the selected one must run at least one
/// machine. `fuel` is `None` for a run or replay: their `--fuel` bounds
/// each reducer step (16.6, E128), and the plan keeps the default, so
/// `--fuel 1` reaches the reducers (`ignite plan --fuel` bounds the
/// properties).
fn run_model(
    loaded: &Loaded,
    assembly: Option<&str>,
    fuel: Option<u64>,
    std: Option<&Path>,
) -> Result<ignite_eval::Model, Error> {
    let options = PlanOptions {
        assembly: assembly.map(str::to_string),
        fuel,
        std: std.map(Path::to_path_buf),
    };
    let model = loaded.model(&options)?;
    let Some(selected) = model.assembly.as_ref() else {
        return Err(run_diagnostic(
            loaded,
            "E125",
            "`ignite run` needs an assembly, and the program declares none".to_string(),
        ));
    };
    if model.machines.is_empty() {
        return Err(run_diagnostic(
            loaded,
            "E125",
            format!(
                "assembly `{}` runs nothing: no member it hosts has a machine",
                selected.id
            ),
        ));
    }
    Ok(model)
}

/// The members of a run (16.3, A10): the buildable ones (member id to
/// executable name: every member of the resolution whose kind has `main`)
/// and the declared, not buildable ones (a component bound on its own
/// whose kind has neither `main` nor a machine).
fn run_members(model: &ignite_eval::Model) -> (BTreeMap<String, String>, Vec<String>) {
    let mut buildable = BTreeMap::new();
    let mut declared = Vec::new();
    let Some(entries) = model
        .assembly
        .as_ref()
        .and_then(|assembly| assembly.resolution.as_ref())
    else {
        return (buildable, declared);
    };
    for entry in entries {
        let Some(member) = model.member(&entry.member) else {
            continue;
        };
        if let Some(executable) = &member.executable {
            buildable.insert(member.id.clone(), executable.clone());
            continue;
        }
        let has_machine = model.machines.iter().any(|m| m.member == member.id);
        if entry.via == entry.member
            && member.declaration == ignite_types::MemberDeclaration::Component
            && !has_machine
        {
            declared.push(member.id.clone());
        }
    }
    (buildable, declared)
}

/// A hash of everything a build of `file` reads: the target and profile,
/// every `.ignite` file of the program's directory and the standard
/// library, the driver's own executable and the runtime archive. Equal
/// stamps mean an unchanged build (16.3).
fn build_stamp(file: &Path, options: &RunOptions) -> Result<u64, Error> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    format!("{:?} {}", options.target, options.release).hash(&mut hasher);
    let mut files = Vec::new();
    if let Some(dir) = file.parent() {
        ignite_files(dir, &mut files)?;
    }
    let std_root = options.std.clone().unwrap_or_else(default_std_path);
    ignite_files(&std_root, &mut files)?;
    for path in &files {
        path.hash(&mut hasher);
        std::fs::read(path)
            .map_err(io_error(path))?
            .hash(&mut hasher);
    }
    let mut stat = |path: &Path| {
        if let Ok(metadata) = std::fs::metadata(path) {
            metadata.len().hash(&mut hasher);
            if let Ok(modified) = metadata.modified()
                && let Ok(since) = modified.duration_since(std::time::UNIX_EPOCH)
            {
                since.as_nanos().hash(&mut hasher);
            }
        }
    };
    if let Ok(exe) = std::env::current_exe() {
        stat(&exe);
    }
    if options.target == Target::Llvm {
        stat(&options.runtime.clone().unwrap_or_else(default_runtime_path));
    }
    Ok(hasher.finish())
}

/// Build every buildable member of `file` into `build_dir` (16.3): a
/// no-op when the stamp of the last build equals the current one and its
/// executables exist. One build at a time per directory (a lock file), so
/// concurrent runs sharing a `--build` wait for the first. Returns member
/// id to the built executable.
fn build_members(
    file: &Path,
    options: &RunOptions,
    build_dir: &Path,
    executables: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, PathBuf>, Error> {
    std::fs::create_dir_all(build_dir).map_err(io_error(build_dir))?;
    let lock_path = build_dir.join("lock");
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(io_error(&lock_path))?;
    let _lock = nix::fcntl::Flock::lock(lock_file, nix::fcntl::FlockArg::LockExclusive).map_err(
        |(_, errno)| Error::Io {
            path: lock_path.clone(),
            error: errno.into(),
        },
    )?;
    let stamp = build_stamp(file, options)?;
    let stamp_path = build_dir.join("stamp");
    let by_name = |paths: &[PathBuf]| -> BTreeMap<String, PathBuf> {
        executables
            .iter()
            .filter_map(|(member, name)| {
                let path = paths.iter().find(|path| {
                    path.file_stem().and_then(|stem| stem.to_str()) == Some(name.as_str())
                })?;
                Some((member.clone(), path.clone()))
            })
            .collect()
    };
    if let Ok(previous) = std::fs::read_to_string(&stamp_path) {
        let mut lines = previous.lines();
        if lines.next() == Some(stamp.to_string().as_str()) {
            let paths: Vec<PathBuf> = lines.map(PathBuf::from).collect();
            let built = by_name(&paths);
            if built.len() == executables.len() && built.values().all(|path| path.is_file()) {
                return Ok(built);
            }
        }
    }
    let build_options = BuildOptions {
        target: options.target,
        out: Some(build_dir.to_path_buf()),
        target_dir: Some(
            options
                .target_dir
                .clone()
                .unwrap_or_else(|| build_dir.join("target")),
        ),
        release: options.release,
        std: options.std.clone(),
        runtime: options.runtime.clone(),
        ..BuildOptions::default()
    };
    let output = build(file, &build_options)?;
    let built = by_name(&output.executables);
    if built.len() != executables.len() {
        let missing: Vec<&str> = executables
            .iter()
            .filter(|(member, _)| !built.contains_key(*member))
            .map(|(_, name)| name.as_str())
            .collect();
        return Err(Error::Io {
            path: build_dir.to_path_buf(),
            error: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("the build produced no executable {}", missing.join(", ")),
            ),
        });
    }
    let mut record = stamp.to_string();
    for path in &output.executables {
        record.push('\n');
        record.push_str(&path.to_string_lossy());
    }
    std::fs::write(&stamp_path, record).map_err(io_error(&stamp_path))?;
    Ok(built)
}

/// The journal's header (16.5) for `model` at `epoch`.
fn journal_header(model_json: &ignite_eval::Json, epoch: i64) -> String {
    let mut object = BTreeMap::new();
    object.insert("epoch".to_string(), run::engine::duration_json(epoch));
    object.insert(
        "formatVersion".to_string(),
        ignite_eval::Json::Str(run::engine::JOURNAL_VERSION.to_string()),
    );
    object.insert(
        "kind".to_string(),
        ignite_eval::Json::Str("header".to_string()),
    );
    object.insert("model".to_string(), model_json.clone());
    object.insert("seq".to_string(), ignite_eval::Json::Int(0));
    ignite_eval::Json::Object(object).compact()
}

/// Check a journal's header against the current model (E126) and return
/// its epoch.
fn check_header(
    loaded: &Loaded,
    header: &str,
    model_json: &ignite_eval::Json,
) -> Result<i64, Error> {
    let mismatch = |what: String| run_diagnostic(loaded, "E126", what);
    let parsed = ignite_eval::Json::parse(header)
        .map_err(|reason| mismatch(format!("the journal's header does not parse: {reason}")))?;
    if parsed.get("kind").and_then(ignite_eval::Json::as_str) != Some("header") {
        return Err(mismatch(
            "the journal's first record is no header".to_string(),
        ));
    }
    match parsed
        .get("formatVersion")
        .and_then(ignite_eval::Json::as_str)
    {
        Some(version) if version == run::engine::JOURNAL_VERSION => {}
        Some(version) => {
            return Err(mismatch(format!(
                "the journal has formatVersion `{version}`, this driver writes `{}`",
                run::engine::JOURNAL_VERSION
            )));
        }
        None => {
            return Err(mismatch(
                "the journal's header has no formatVersion".to_string(),
            ));
        }
    }
    if parsed.get("model") != Some(model_json) {
        return Err(mismatch(
            "the journal was written for another model: use another --state or --fresh".to_string(),
        ));
    }
    parsed
        .get("epoch")
        .and_then(|epoch| epoch.get("ns"))
        .and_then(ignite_eval::Json::as_i64)
        .ok_or_else(|| mismatch("the journal's header has no epoch".to_string()))
}

/// The wall clock as nanoseconds since 1970-01-01T00:00:00Z.
fn wall_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|since| i64::try_from(since.as_nanos()).ok())
        .unwrap_or(0)
}

/// `ignite run` (`docs/slice.md` 16.3): plan as [`plan`] does, check E120
/// to E125 and E129, open the journal (E126, E127; before the build, so a
/// locked or foreign state directory costs no build), build every
/// buildable member, construct the machines and start them; the exit
/// status of 16.3 (0 a requested stop with no failed machine, 1 a machine
/// gave up or failed). `Err` is a diagnostic before the start.
pub fn run(file: &Path, options: &RunOptions) -> Result<i32, Error> {
    let loaded = load_for(file, options.std.as_deref(), RUN_ROOTS)?;
    let model = run_model(
        &loaded,
        options.assembly.as_deref(),
        None,
        options.std.as_deref(),
    )?;
    let assembly_id = model
        .assembly
        .as_ref()
        .map(|a| a.id.clone())
        .unwrap_or_default();
    let state = state_dir(options.state.as_deref(), &assembly_id);
    let build_dir = options.build.clone().unwrap_or_else(|| state.join("build"));
    let manual = options.clock == ClockMode::Manual;
    let epoch_option = match &options.epoch {
        Some(text) => Some(run::parse_rfc3339(text).ok_or_else(|| Error::Io {
            path: PathBuf::from(text),
            error: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "--epoch is no RFC 3339 instant",
            ),
        })?),
        None => None,
    };
    let settle = match &options.settle {
        Some(text) => run::clock::parse_duration(text)
            .and_then(|ns| u64::try_from(ns).ok())
            .map(std::time::Duration::from_nanos)
            .ok_or_else(|| Error::Io {
                path: PathBuf::from(text),
                error: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "--settle is no Duration literal",
                ),
            })?,
        None => DEFAULT_SETTLE,
    };
    let (executables, declared) = run_members(&model);
    if state.join("host-journal.jsonl").exists() || state.join("identity").exists() {
        return Err(run_diagnostic(
            &loaded,
            "E126",
            format!(
                "{} is the state of an `ignite host`; apply a plan to it with `ignite apply`, or run with another --state",
                state.display()
            ),
        ));
    }
    let mut journal =
        run::journal::Journal::open(&state, options.fresh).map_err(|error| match error {
            run::journal::JournalError::Locked(path) => run_diagnostic(
                &loaded,
                "E126",
                format!(
                    "another `ignite run` or `ignite host` holds {}",
                    path.display()
                ),
            ),
            run::journal::JournalError::Io(error) => Error::Io {
                path: state.clone(),
                error,
            },
        })?;
    if journal.dropped_torn_line {
        eprintln!(
            "ignite run: dropped a torn last line of {}",
            journal.path.display()
        );
    }
    let model_json = ignite_eval::Json::parse(&model.to_json()).map_err(|reason| Error::Io {
        path: file.to_path_buf(),
        error: std::io::Error::other(format!("the model is no JSON: {reason}")),
    })?;
    let modules = loaded.modules();
    let control_types = modules
        .get(ignite_eval::CONTROL_MODULE)
        .map(|module| module.types)
        .ok_or_else(|| {
            run_diagnostic(
                &loaded,
                "E125",
                format!(
                    "the standard library has no module `{}`",
                    ignite_eval::CONTROL_MODULE
                ),
            )
        })?;
    let mut evaluator = ignite_eval::Evaluator::new(
        &modules,
        options
            .fuel
            .map_or_else(ignite_eval::Fuel::default, ignite_eval::Fuel::with_units),
    );
    let mut core = run::engine::Core::new(&mut evaluator, &model);
    let (epoch, generation, start_at) = if journal.records.is_empty() {
        let epoch = if manual {
            epoch_option
                .unwrap_or_else(|| run::parse_rfc3339(run::DEFAULT_EPOCH).unwrap_or_default())
        } else {
            wall_now()
        };
        journal
            .append(&journal_header(&model_json, epoch), true)
            .map_err(io_error(&journal.path))?;
        (epoch, 1, 0)
    } else {
        let epoch = check_header(&loaded, &journal.records[0], &model_json)?;
        let replayed = run::replay::replay(&mut core, control_types, &journal.records[1..])
            .map_err(|error| run_diagnostic(&loaded, "E127", error.to_string()))?;
        let start_at = if manual {
            replayed.last_at
        } else {
            wall_now().saturating_sub(epoch)
        };
        (epoch, replayed.generation, start_at)
    };
    core.set_next_seq(journal.next_seq);
    let built = build_members(file, options, &build_dir, &executables)?;
    let bin = state.join("bin").join(generation.to_string());
    run::live::copy_into(&bin, &built).map_err(io_error(&bin))?;
    for (member, name) in &executables {
        if !model.machines.iter().any(|m| m.member == *member) {
            eprintln!("ignite run: built {member}: {}", bin.join(name).display());
        }
    }
    for member in &declared {
        eprintln!("ignite run: {member}: declared, not buildable in slice 12");
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let adapters = run::control::Adapters::start(tx.clone()).map_err(io_error(&state))?;
    run::control::read_control(tx.clone());
    run::control::watch_signals(tx.clone()).map_err(io_error(&state))?;
    let file = file.to_path_buf();
    let build_options = options.clone();
    let build_root = build_dir.clone();
    let names = executables.clone();
    let builder: run::live::Builder = std::sync::Arc::new(move || {
        build_members(&file, &build_options, &build_root, &names).map_err(|error| error.to_string())
    });
    let config = run::live::Config {
        state: state.clone(),
        manual,
        epoch,
        settle,
        json: options.json,
        ports: options.ports.iter().cloned().collect(),
        declared,
        executables,
        generation,
        start_at,
        builder,
    };
    let live = run::live::Live::new(
        core,
        &model,
        control_types,
        config,
        journal,
        adapters,
        tx,
        rx,
    );
    live.run().map_err(io_error(&state))
}

/// `ignite replay` (`docs/slice.md` 16.3, 16.15): read the journal of the
/// state directory, check its header against the current plan (E126),
/// re-step the reducers with the journaled events, compare every derived
/// record (E127) and return the final standings.
pub fn replay(file: &Path, options: &ReplayOptions) -> Result<Vec<Standing>, Error> {
    let loaded = load_for(file, options.std.as_deref(), RUN_ROOTS)?;
    let model = run_model(
        &loaded,
        options.assembly.as_deref(),
        None,
        options.std.as_deref(),
    )?;
    let assembly_id = model
        .assembly
        .as_ref()
        .map(|a| a.id.clone())
        .unwrap_or_default();
    let state = state_dir(options.state.as_deref(), &assembly_id);
    let path = state.join(run::journal::FILE);
    let content = std::fs::read_to_string(&path).map_err(io_error(&path))?;
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    if !content.is_empty() && !content.ends_with('\n') {
        lines.pop();
        eprintln!(
            "ignite replay: dropped a torn last line of {}",
            path.display()
        );
    }
    if lines.is_empty() {
        return Err(run_diagnostic(
            &loaded,
            "E126",
            format!("{} has no header", path.display()),
        ));
    }
    let model_json = ignite_eval::Json::parse(&model.to_json()).map_err(|reason| Error::Io {
        path: file.to_path_buf(),
        error: std::io::Error::other(format!("the model is no JSON: {reason}")),
    })?;
    check_header(&loaded, &lines[0], &model_json)?;
    let modules = loaded.modules();
    let control_types = modules
        .get(ignite_eval::CONTROL_MODULE)
        .map(|module| module.types)
        .ok_or_else(|| {
            run_diagnostic(
                &loaded,
                "E125",
                format!(
                    "the standard library has no module `{}`",
                    ignite_eval::CONTROL_MODULE
                ),
            )
        })?;
    let mut evaluator = ignite_eval::Evaluator::new(
        &modules,
        options
            .fuel
            .map_or_else(ignite_eval::Fuel::default, ignite_eval::Fuel::with_units),
    );
    let mut core = run::engine::Core::new(&mut evaluator, &model);
    run::replay::replay(&mut core, control_types, &lines[1..])
        .map_err(|error| run_diagnostic(&loaded, "E127", error.to_string()))?;
    Ok(core.standings())
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
    let (parsed, library) = load_program_std(file, std, &[])?;
    let substrate = substrate
        .map(Path::to_path_buf)
        .unwrap_or_else(default_substrate_path);
    generate(parsed, &library, substrate)
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
    let (parsed, library) = load_program_std(file, options.std.as_deref(), &[])?;
    let substrate = options
        .substrate
        .clone()
        .unwrap_or_else(default_substrate_path);
    let emitted = generate(parsed, &library, substrate)?;
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
    test_build::claim_target_dir(&out, &target_dir)?;
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
    let (parsed, library) = load_program_std(file, std, &[])?;
    let typed = type_parsed(parsed, &library)?;
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
