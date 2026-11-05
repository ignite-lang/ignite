//! Rust target (`docs/slice.md` section 6). Lowers checked Ignite modules to
//! two Cargo projects: the standard library as the `ignite-std` crate and the
//! program as a crate that depends on it and on the substrate `ignite-io`.
//!
//! Contract:
//!
//! * [`generate`] and [`generate_std`] are pure: they return source text and
//!   write nothing. [`write_project`] writes one project.
//! * [`generate_std_reachable`] (the driver's path, through
//!   [`generate_std_for`]) holds only the standard-library items the
//!   program crate's text reaches; [`generate_std_typed`] holds every item.
//! * The lowering table, the ownership rule (`.clone()` on places in value
//!   positions), the builtin method table and the project layout (6.2) in
//!   section 6 are the specification; the generated text for each row is
//!   the Rust shown there. The `main` of an executable component follows
//!   section 14.5; the type checker owns the component rules and lists the
//!   executables (`TypedProgram::executables`).
//! * Diagnostics it owns: `E020` unsupported or malformed resource (also: a
//!   `system` in a standard-library module, a component lowered without the
//!   type checker).
//! * Generated code is formatted by indentation only; it is not run through
//!   rustfmt. Tests compare against golden files after normalizing
//!   whitespace runs to one space.
//!
//! Positions. An expression is emitted either as a *value* (a place gets
//! `.clone()`) or *bare* (a place is emitted as written); the positions of
//! each kind are the lists of section 6. Independently, every `if`, `match`
//! and block that is part of an expression is in value position; only an
//! expression statement that is not the last statement of a value-position
//! block is not. `while`, `loop` and `for` are statements and are emitted
//! without a trailing `;`, so a trailing `loop` keeps its `!` type.
//!
//! Names. An uppercase name that is not a local, followed by `.name(..)`,
//! is a substrate static (`TcpListener`, `TcpStream`, `Stdin`, `Stdout`,
//! `Process`, `IoError`: `ignite_io::Owner::name(..)`, except
//! `Process.exit(c)`, which is the diverging `ignite_io::exit`), a builtin static, a
//! protocol role (`P::name(..)`) or a variant constructor. A protocol is
//! known from `Symbols::protocols` or from the module's own `protocol`
//! declarations; a constant from `Symbols::constants` or the module's own
//! `const` declarations. A constant is a zero-argument fn, so a non-local
//! name that is a constant is emitted as `N()` and is never a place.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ignite_check::{BUILTIN_ENUMS, BUILTIN_STATICS, Symbols};
use ignite_syntax::ast::{
    AliasDecl, BinaryOp, Block, ComponentDecl, ConstDecl, Decl, EnumDecl, Expr, ExprKind,
    FieldInit, FnDecl, Ident, Import, Pattern, PatternKind, Program, ProtocolDecl, RecordDecl,
    ResourceDecl, Span, Stmt, StmtKind, SystemDecl, TestDecl, Type, UnaryOp,
};
use ignite_syntax::diagnostic::{Diagnostic, SourceFile};
use ignite_types::{
    BraceKind, Builtin, Callee, ExecutableInfo, FailureSite, QualName, QuantityKind, Show,
    TestInfo, TestTry, Ty, TypedProgram,
};

/// Header of every generated Rust file. `non_snake_case` because Ignite fn
/// names follow the design (`localSocket` in `std.deploy`), not Rust's
/// convention. `bindings_with_variant_name` (deny by default, E0170)
/// because a lowercase name in an Ignite pattern binds the value even when
/// the matched enum has a variant of that name, and the generated code
/// never brings a variant into scope bare, so rustc binds it too.
pub const FILE_HEADER: &str = "#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]\n";

/// Header added before [`FILE_HEADER`] in a test build: compiler warnings
/// about generated code are noise in a test report.
pub const TEST_HEADER: &str = "#![allow(warnings)]\n";

/// Cargo package name of the generated standard library.
pub const STD_PACKAGE: &str = "ignite-std";

/// Substrate types whose statics lower to `ignite_io::<Owner>::<name>(..)`.
const SUBSTRATE_OWNERS: &[&str] = &[
    "TcpListener",
    "TcpStream",
    "Stdin",
    "Stdout",
    "Process",
    "IoError",
];

/// Modules that are the substrate: importing their substrate names emits
/// nothing. A Ignite declaration in one of them (`std.net.Endpoint`) is an
/// item of `ignite-std` like any other.
const SUBSTRATE_MODULES: &[&str] = &["std.net", "std.io", "std.fs", "std.process"];

/// The names of [`SUBSTRATE_MODULES`] that are the substrate's own types
/// and variants, lowered to `ignite_io::` paths rather than imported.
const SUBSTRATE_NAMES: &[&str] = &[
    "TcpListener",
    "TcpStream",
    "Stdin",
    "Stdout",
    "Process",
    "IoError",
    "Input",
    "Data",
    "Eof",
    "Exposure",
    "ByteStream",
    "Pipe",
    "PipeStream",
    "Directory",
    "Launcher",
    "ChildProcess",
    "Entropy",
    "Setting",
];

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Options {
    /// Path written into the generated `Cargo.toml` as the `ignite-io` dependency.
    pub substrate_path: PathBuf,
    /// A test build (`docs/slice.md` section 12.6) when not empty: the
    /// module paths whose tests and test-only items are lowered. Every
    /// other module drops its test-only declarations.
    pub test_modules: Vec<String>,
    /// The tests of the `ignite-test` executable, in run order. Its entry
    /// for a test is `<module's Rust path>::__ignite_test_<decl>`.
    pub tests: Vec<TestInfo>,
    /// Each module's source by module path: a failed `assert` or `?`
    /// prints the condition's text and a position from it.
    pub sources: BTreeMap<String, SourceFile>,
}

/// Name of the test executable of a test build.
pub const TEST_EXECUTABLE: &str = "ignite-test";

/// The Rust fn of test `decl` (the index of its `Decl::Test`).
pub fn test_fn_name(decl: usize) -> String {
    format!("__ignite_test_{decl}")
}

/// One executable of the generated project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Executable {
    /// Binary name; also the file name `src/bin/<name>.rs`.
    pub name: String,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedProject {
    /// Cargo package name: the last segment of the Ignite package, or `program`.
    pub package_name: String,
    pub cargo_toml: String,
    pub lib_rs: String,
    pub executables: Vec<Executable>,
}

/// Lower a checked program. `symbols` is what `check_module` returned for
/// it, including the names it imports. The project depends on `ignite-std`
/// at `std/` when the program imports a standard-library module other than
/// `std.net` and `std.io`.
pub fn generate(
    program: &Program,
    symbols: &Symbols,
    options: &Options,
) -> Result<GeneratedProject, Vec<Diagnostic>> {
    let module = program_module(program, None);
    let layout = Layout::program(&module);
    generate_program(program, symbols, None, options, &[], &layout)
        .map_err(|errors| errors.into_iter().flat_map(|(_, d)| d).collect())
}

/// [`generate`] with the program's types (`ignite_types::type_module` of
/// the same `program` object). The constructs of `docs/slice.md` section
/// 10 that depend on types (UFCS, fn-field calls, `to_text`, `+` on
/// `Text`, constant patterns, variants resolved by expected type) are
/// lowered correctly only with types.
pub fn generate_typed(
    program: &Program,
    symbols: &Symbols,
    types: &TypedProgram,
    options: &Options,
) -> Result<GeneratedProject, Vec<Diagnostic>> {
    let module = program_module(program, Some(types));
    let layout = Layout::program(&module);
    generate_program(program, symbols, Some(types), options, &[], &layout)
        .map_err(|errors| errors.into_iter().flat_map(|(_, d)| d).collect())
}

/// A module of the program's package other than the program itself
/// (`docs/slice.md` section 14.4): its module path (`examples.terminal.
/// commands`), AST, what `check_module` returned for it and its types.
pub type SiblingModule<'a> = (String, &'a Program, &'a Symbols, &'a TypedProgram);

/// [`generate_typed`] of a program whose package has sibling modules:
/// each sibling `<program module>.a.b` becomes `pub mod a { pub mod b
/// { .. } }` of the program crate, so `module("p")` records and imports
/// from siblings resolve inside the crate. Errors are grouped by module
/// path.
pub fn generate_package(
    program: &Program,
    symbols: &Symbols,
    types: &TypedProgram,
    siblings: &[SiblingModule<'_>],
    options: &Options,
) -> Result<GeneratedProject, Vec<(String, Vec<Diagnostic>)>> {
    let module = program_module(program, Some(types));
    let layout = Layout::program(&module);
    generate_program(program, symbols, Some(types), options, siblings, &layout)
}

/// Where the program's items live in the generated crate: the package
/// paths from `lib.rs` and from an executable's file (`Package`), with
/// the program's module path.
struct Layout {
    module: String,
    /// The package root inside `lib.rs`: `crate`, or `crate::a::b` in a
    /// test build of several programs.
    lib_root: String,
}

impl Layout {
    fn program(module: &str) -> Layout {
        Layout {
            module: module.to_string(),
            lib_root: "crate".to_string(),
        }
    }

    fn in_lib(&self) -> Package {
        Package::new(&self.module, &self.lib_root)
    }

    fn in_bin(&self, crate_name: &str) -> Package {
        Package::new(&self.module, crate_name)
    }
}

fn generate_program(
    program: &Program,
    symbols: &Symbols,
    types: Option<&TypedProgram>,
    options: &Options,
    siblings: &[SiblingModule<'_>],
    layout: &Layout,
) -> Result<GeneratedProject, Vec<(String, Vec<Diagnostic>)>> {
    let package_name = package_name(program);
    let crate_name = crate_name(&package_name);
    let module = program_module(program, types);
    let in_lib = layout.in_lib();
    let in_bin = layout.in_bin(&crate_name);
    let own = |diagnostics: Vec<Diagnostic>| vec![(module.clone(), diagnostics)];

    let mut diagnostics = Vec::new();
    let mut resources: BTreeMap<String, Resources> = BTreeMap::new();
    for decl in &program.decls {
        if let Decl::System(system) = decl {
            let system_resources = lower_resources(system, &mut diagnostics);
            resources.insert(system.name.name.clone(), system_resources);
            if types.is_none() {
                for component in &system.components {
                    diagnostics.push(e020(
                        component.kind.span,
                        format!(
                            "unsupported component kind `{}`: a component is an executable only through the type checker (`docs/slice.md` section 14.5)",
                            component.kind.name
                        ),
                    ));
                }
            }
        }
    }
    let executable_infos: &[ExecutableInfo] = types.map_or(&[], TypedProgram::executables);
    if !diagnostics.is_empty() {
        diagnostics.sort_by_key(|d| (d.span.start, d.code));
        return Err(own(diagnostics));
    }

    let uses = use_targets(&program.imports, "ignite_std", &in_lib);
    let mut lib = Writer::new();
    if !options.test_modules.is_empty() {
        lib.raw(TEST_HEADER);
    }
    lib.raw(FILE_HEADER);
    lib.raw(&module_items(
        program,
        symbols,
        types,
        &uses,
        "ignite_std",
        &ModuleOptions::of(options, &module, &in_lib),
    ));
    let mut errors = Vec::new();
    let mut tree = ModuleTree::default();
    for (path, sibling, sibling_symbols, sibling_types) in siblings {
        let mut sibling_diagnostics: Vec<Diagnostic> = sibling
            .decls
            .iter()
            .filter_map(|decl| match decl {
                Decl::System(system) => Some(e020(
                    system.name.span,
                    "only the program's own module can declare a system",
                )),
                _ => None,
            })
            .collect();
        let Some(relative) = path.strip_prefix(&format!("{module}.")) else {
            sibling_diagnostics.push(e020(
                Span::default(),
                format!("module `{path}` is not in the package of `{module}`"),
            ));
            errors.push((path.clone(), sibling_diagnostics));
            continue;
        };
        if !sibling_diagnostics.is_empty() {
            errors.push((path.clone(), sibling_diagnostics));
            continue;
        }
        let segments: Vec<&str> = relative.split('.').collect();
        let sibling_uses = use_targets(&sibling.imports, "ignite_std", &in_lib);
        tree.insert(
            &segments,
            module_items(
                sibling,
                sibling_symbols,
                Some(sibling_types),
                &sibling_uses,
                "ignite_std",
                &ModuleOptions::of(options, path, &in_lib),
            ),
        );
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut siblings_text = String::new();
    tree.render(&mut siblings_text, 0);
    lib.raw(&siblings_text);

    let testing = !options.test_modules.is_empty();
    let bin_uses = use_targets(&program.imports, "ignite_std", &in_bin);
    let mut executables = Vec::new();
    if let Some(types) = types {
        for info in executable_infos {
            let Some(system) = program.decls.iter().find_map(|decl| match decl {
                Decl::System(system) if system.name.name == info.system => Some(system),
                _ => None,
            }) else {
                continue;
            };
            let system_resources = &resources[&info.system];
            match executable_main(
                &crate_name,
                &bin_uses,
                program,
                symbols,
                types,
                options.sources.get(&module),
                info,
                system,
                system_resources,
                &in_bin,
            ) {
                Ok(_) if testing => {}
                Ok(source) => executables.push(Executable {
                    name: info.executable.clone(),
                    source,
                }),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(own(diagnostics));
    }
    if testing {
        executables.push(Executable {
            name: TEST_EXECUTABLE.to_string(),
            source: test_main(&crate_name, &module, &options.tests),
        });
    }

    let lib_rs = lib.finish();
    let feature = testing.then_some("runner");
    let uses_std = testing
        || imports_std(&program.imports)
        || siblings
            .iter()
            .any(|(_, sibling, _, _)| imports_std(&sibling.imports))
        || !executable_infos.is_empty();
    let cargo_toml = cargo_toml(
        &package_name,
        &executables,
        &options.substrate_path,
        uses_std,
        feature,
    );
    Ok(GeneratedProject {
        package_name,
        cargo_toml,
        lib_rs,
        executables,
    })
}

/// One checked standard-library module: its module path (`std.console`),
/// its AST and what `check_module` returned for it.
pub type StdModule<'a> = (String, &'a Program, &'a Symbols);

/// Lower the standard library to the `ignite-std` crate: one nested
/// `pub mod` per module (`std.a.b` is
/// `pub mod a { pub mod b { .. } }`); imports become
/// `use crate::..`. Errors are grouped by module path.
pub fn generate_std(
    modules: &[StdModule<'_>],
    options: &Options,
) -> Result<GeneratedProject, Vec<(String, Vec<Diagnostic>)>> {
    let modules: Vec<_> = modules
        .iter()
        .map(|(path, program, symbols)| (path.clone(), *program, *symbols, None))
        .collect();
    prepare_std(&modules, options).map(|prepared| prepared.generate_all(&options.substrate_path))
}

/// A [`StdModule`] with its types.
pub type TypedStdModule<'a> = (String, &'a Program, &'a Symbols, &'a TypedProgram);

/// [`generate_std`] with each module's types (see [`generate_typed`]).
pub fn generate_std_typed(
    modules: &[TypedStdModule<'_>],
    options: &Options,
) -> Result<GeneratedProject, Vec<(String, Vec<Diagnostic>)>> {
    prepare_std_typed(modules, options)
        .map(|prepared| prepared.generate_all(&options.substrate_path))
}

/// Lower typed standard-library items once, before choosing program roots.
/// The returned template owns its lowered text and keeps no AST references.
/// Test-module selection and each std module's source positions are frozen
/// from `options`; the substrate path is supplied to each rendering instead.
pub fn prepare_std_typed(
    modules: &[TypedStdModule<'_>],
    options: &Options,
) -> Result<PreparedStd, Vec<(String, Vec<Diagnostic>)>> {
    let modules: Vec<_> = modules
        .iter()
        .map(|(path, program, symbols, types)| (path.clone(), *program, *symbols, Some(*types)))
        .collect();
    prepare_std(&modules, options)
}

/// [`generate_std_typed`] holding only the items `roots` reach
/// (`docs/slice.md` 16.22 item 2; the LLVM target's `std.ll` is emitted
/// the same way, `docs/llvm.md` section 1). `roots` are Rust source texts:
/// the program crate's `lib.rs` and its executables, or the test crate's.
/// An item (a fn, type, protocol, constant, test or glue fn) is kept when
/// a root or a kept item names it; a `use` line keeps the names that are
/// kept (a glob stays); a module whose items are all dropped stays as an
/// empty `pub mod`. A name is any identifier of the text that is not a
/// method or field (`.name`), so the walk is over names, not resolved
/// paths: two modules' items of one name are kept together, and nothing
/// the text can reach is dropped.
pub fn generate_std_reachable(
    modules: &[TypedStdModule<'_>],
    options: &Options,
    roots: &[&str],
) -> Result<GeneratedProject, Vec<(String, Vec<Diagnostic>)>> {
    prepare_std_typed(modules, options)
        .map(|prepared| prepared.generate_reachable(&options.substrate_path, roots))
}

/// [`generate_std_reachable`] with the roots of `program`: its `lib.rs`
/// and the source of every executable.
pub fn generate_std_for(
    modules: &[TypedStdModule<'_>],
    options: &Options,
    program: &GeneratedProject,
) -> Result<GeneratedProject, Vec<(String, Vec<Diagnostic>)>> {
    prepare_std_typed(modules, options)
        .map(|prepared| prepared.generate_for(&options.substrate_path, program))
}

/// Immutable lowered standard-library items. Each rendering independently
/// computes reachability; rendering one program cannot prune a later one.
#[derive(Debug)]
pub struct PreparedStd {
    modules: Vec<LoweredModule>,
    testing: bool,
    dependencies: OnceLock<StdDependencies>,
}

impl PreparedStd {
    /// Render every prepared item without pruning.
    pub fn generate_all(&self, substrate: &Path) -> GeneratedProject {
        self.render(substrate, None)
    }

    /// Render only the prepared items reached by Rust source `roots`.
    pub fn generate_reachable(&self, substrate: &Path, roots: &[&str]) -> GeneratedProject {
        let dependencies = self
            .dependencies
            .get_or_init(|| StdDependencies::new(&self.modules));
        let retained = dependencies.reachable(&self.modules, roots);
        self.render(substrate, Some(&retained))
    }

    /// Render for the program's library and all its executable sources.
    pub fn generate_for(&self, substrate: &Path, program: &GeneratedProject) -> GeneratedProject {
        let mut roots = vec![program.lib_rs.as_str()];
        roots.extend(
            program
                .executables
                .iter()
                .map(|executable| executable.source.as_str()),
        );
        self.generate_reachable(substrate, &roots)
    }

    fn render(&self, substrate: &Path, retained: Option<&RetainedStd>) -> GeneratedProject {
        let mut tree = ModuleTree::default();
        for (index, module) in self.modules.iter().enumerate() {
            let text = module.render(
                retained.map(|retained| retained.items[index].as_slice()),
                retained.map(|retained| &retained.names),
            );
            tree.insert(&std_segments(&module.path), text);
        }
        let mut lib = String::new();
        if self.testing {
            lib.push_str(TEST_HEADER);
        }
        lib.push_str(FILE_HEADER);
        tree.render(&mut lib, 0);
        GeneratedProject {
            package_name: STD_PACKAGE.to_string(),
            cargo_toml: std_cargo_toml(substrate, &lib),
            lib_rs: lib,
            executables: Vec::new(),
        }
    }
}

type StdInput<'a> = (String, &'a Program, &'a Symbols, Option<&'a TypedProgram>);

/// One lowered standard-library module before rendering: its imports
/// and items stay intact when a [`PreparedStd`] renders different roots.
#[derive(Debug)]
struct LoweredModule {
    path: String,
    uses: Vec<UseTarget>,
    items: Vec<Item>,
}

impl LoweredModule {
    fn render(&self, keep: Option<&[bool]>, referenced: Option<&BTreeSet<String>>) -> String {
        let mut out = String::new();
        for target in &self.uses {
            let line = match referenced {
                Some(referenced) => target.render_referenced(referenced),
                None => Some(target.render()),
            };
            if let Some(line) = line {
                out.push_str(&line);
                out.push('\n');
            }
        }
        for (index, item) in self.items.iter().enumerate() {
            if keep.is_none_or(|keep| keep[index]) {
                out.push_str(&item.text);
            }
        }
        out
    }
}

/// One item of a module: the Rust name it defines, without a raw
/// identifier's `r#`, and its text as [`Writer`] lines at indentation 0.
#[derive(Debug)]
struct Item {
    name: String,
    text: String,
}

/// The target of one `use` line: the Rust path of the module and the
/// names taken from it, as emitted (`*` for a glob).
#[derive(Debug)]
struct UseTarget {
    path: String,
    names: Vec<String>,
}

impl UseTarget {
    fn render(&self) -> String {
        match self.names.as_slice() {
            [single] => format!("use {}::{single};", self.path),
            _ => format!("use {}::{{{}}};", self.path, self.names.join(", ")),
        }
    }

    fn render_referenced(&self, referenced: &BTreeSet<String>) -> Option<String> {
        let names: Vec<&str> = self
            .names
            .iter()
            .filter(|name| name.as_str() == "*" || referenced.contains(plain_ident(name)))
            .map(String::as_str)
            .collect();
        match names.as_slice() {
            [] => None,
            [single] => Some(format!("use {}::{single};", self.path)),
            _ => Some(format!("use {}::{{{}}};", self.path, names.join(", "))),
        }
    }
}

/// The path segments of the std module `path` under the crate root:
/// `std.a.b` is `a`, `b`.
fn std_segments(path: &str) -> Vec<&str> {
    match path.strip_prefix("std.") {
        Some(rest) => rest.split('.').collect(),
        None if path == "std" => Vec::new(),
        None => path.split('.').collect(),
    }
}

/// The identifiers of the Rust source `text` that can name an item: every
/// identifier token except one after a `.` (a method or a field; `..` is
/// a range and ends no such position), with the `r#` of a raw identifier
/// dropped. Strings and comments are not skipped: a name in them keeps an
/// item and never drops one.
fn rust_identifiers<'a>(text: &'a str, mut visit: impl FnMut(&'a str)) {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut after_dot = false;
    while i < bytes.len() {
        let b = bytes[i];
        if matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'_') {
            let start = i;
            while i < bytes.len()
                && matches!(bytes[i], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
            {
                i += 1;
            }
            if i - start == 1 && b == b'r' && bytes.get(i) == Some(&b'#') {
                i += 1;
                continue;
            }
            if !after_dot {
                visit(&text[start..i]);
            }
            after_dot = false;
        } else if b.is_ascii_digit() {
            while i < bytes.len()
                && matches!(bytes[i], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
            {
                i += 1;
            }
            after_dot = false;
        } else if b == b'.' {
            if bytes.get(i + 1) == Some(&b'.') {
                i += 2;
                after_dot = false;
            } else {
                i += 1;
                after_dot = true;
            }
        } else if b.is_ascii_whitespace() {
            i += 1;
        } else {
            i += 1;
            after_dot = false;
        }
    }
}

/// A Rust identifier as [`rust_identifiers`] reports it.
fn plain_ident(rust: &str) -> &str {
    rust.strip_prefix("r#").unwrap_or(rust)
}

/// Dependencies are source identifiers under the same conservative rule
/// as the original pruning walk. Compute them once per immutable template.
#[derive(Debug)]
struct StdDependencies {
    definitions: BTreeMap<String, Vec<(usize, usize)>>,
    references: Vec<Vec<Vec<String>>>,
}

struct RetainedStd {
    items: Vec<Vec<bool>>,
    names: BTreeSet<String>,
}

impl StdDependencies {
    fn new(modules: &[LoweredModule]) -> Self {
        let mut definitions: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
        let mut references = Vec::new();
        for (module_index, module) in modules.iter().enumerate() {
            let mut items = Vec::new();
            for (item_index, item) in module.items.iter().enumerate() {
                definitions
                    .entry(item.name.clone())
                    .or_default()
                    .push((module_index, item_index));
                let mut names = Vec::new();
                rust_identifiers(&item.text, |name| names.push(name));
                names.sort_unstable();
                names.dedup();
                items.push(names.into_iter().map(str::to_string).collect());
            }
            references.push(items);
        }
        Self {
            definitions,
            references,
        }
    }

    fn reachable(&self, modules: &[LoweredModule], roots: &[&str]) -> RetainedStd {
        // Generated operator chains can repeat a few names millions of times.
        // Borrow every token and queue each distinct name once; only the final
        // use-filter set needs owned strings. Roots and item bodies stay alive
        // throughout this walk, and rendering order comes from the modules.
        let mut names = HashSet::new();
        let mut pending = Vec::new();
        for root in roots {
            rust_identifiers(root, |name| {
                if names.insert(name) {
                    pending.push(name);
                }
            });
        }
        let mut items: Vec<Vec<bool>> = modules
            .iter()
            .map(|module| vec![false; module.items.len()])
            .collect();
        while let Some(name) = pending.pop() {
            if let Some(defined) = self.definitions.get(name) {
                for &(module, item) in defined {
                    if !items[module][item] {
                        items[module][item] = true;
                        for reference in &self.references[module][item] {
                            if names.insert(reference.as_str()) {
                                pending.push(reference.as_str());
                            }
                        }
                    }
                }
            }
        }
        RetainedStd {
            items,
            names: names.into_iter().map(str::to_string).collect(),
        }
    }
}

fn prepare_std(
    modules: &[StdInput<'_>],
    options: &Options,
) -> Result<PreparedStd, Vec<(String, Vec<Diagnostic>)>> {
    let mut errors = Vec::new();
    let mut lowered = Vec::new();
    for (path, program, symbols, types) in modules {
        let systems: Vec<Diagnostic> = program
            .decls
            .iter()
            .filter_map(|decl| match decl {
                Decl::System(system) => Some(e020(
                    system.name.span,
                    "a standard-library module cannot declare a system",
                )),
                _ => None,
            })
            .collect();
        if !systems.is_empty() {
            errors.push((path.clone(), systems));
            continue;
        }
        let package = Package::new(path, "crate");
        lowered.push(LoweredModule {
            path: path.clone(),
            uses: use_targets(&program.imports, "crate", &package),
            items: module_item_list(
                program,
                symbols,
                *types,
                "crate",
                &ModuleOptions::of(options, path, &package),
            ),
        });
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(PreparedStd {
        modules: lowered,
        testing: !options.test_modules.is_empty(),
        dependencies: OnceLock::new(),
    })
}

/// Module items keyed by path segment, for nested `pub mod` output.
#[derive(Default)]
struct ModuleTree {
    items: String,
    children: BTreeMap<String, ModuleTree>,
}

impl ModuleTree {
    fn insert(&mut self, segments: &[&str], items: String) {
        match segments.split_first() {
            None => self.items.push_str(&items),
            Some((first, rest)) => self
                .children
                .entry(first.to_string())
                .or_default()
                .insert(rest, items),
        }
    }

    fn render(&self, out: &mut String, level: usize) {
        for line in self.items.lines() {
            if !line.is_empty() {
                out.push_str(&indentation(level));
            }
            out.push_str(line);
            out.push('\n');
        }
        for (name, child) in &self.children {
            out.push('\n');
            let _ = writeln!(out, "{}pub mod {} {{", indentation(level), ident(name));
            child.render(out, level + 1);
            let _ = writeln!(out, "{}}}", indentation(level));
        }
    }
}

/// The module path and imported names of one import, or `None` when the
/// import names no item (`import a`).
fn import_target(import: &Import) -> Option<(Vec<&str>, Vec<&str>)> {
    let segments: Vec<&str> = import
        .path
        .segments
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    if import.names.is_empty() {
        let (last, module) = segments.split_last()?;
        if module.is_empty() {
            return None;
        }
        Some((module.to_vec(), vec![*last]))
    } else {
        let names = import.names.iter().map(|n| n.name.as_str()).collect();
        Some((segments, names))
    }
}

/// Whether an import refers to a standard-library module that is a file
/// (not `std.net` or `std.io`).
fn is_std_file_import(module: &[&str]) -> bool {
    module.first() == Some(&"std")
        && module.len() >= 2
        && !SUBSTRATE_MODULES.contains(&module.join(".").as_str())
}

/// The names an import takes from `ignite-std`: every name of a
/// standard-library file module, and the names of a substrate module that
/// are not the substrate's own (`std.net.Endpoint`). `None` when nothing
/// comes from `ignite-std`.
fn std_import<'i>(module: &[&'i str], names: &[&'i str]) -> Option<Vec<&'i str>> {
    if is_std_file_import(module) {
        return Some(names.to_vec());
    }
    if module.first() == Some(&"std") && SUBSTRATE_MODULES.contains(&module.join(".").as_str()) {
        let names: Vec<&str> = names
            .iter()
            .copied()
            .filter(|name| !SUBSTRATE_NAMES.contains(name))
            .collect();
        return (!names.is_empty()).then_some(names);
    }
    None
}

fn imports_std(imports: &[Import]) -> bool {
    imports
        .iter()
        .filter_map(import_target)
        .any(|(module, names)| std_import(&module, &names).is_some())
}

/// The modules of the program's package (`docs/slice.md` section 14.4):
/// the program's own module path and the Rust path its items have from the
/// file being generated (`crate` in the program's `lib.rs`, the crate name
/// in an executable's file, `crate::a::b` in a test build). A sibling
/// module `<program>.c` is `<root>::c`; with an empty module path every
/// non-std module `a.b` is `<root>::a::b`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Package {
    module: String,
    root: String,
}

impl Package {
    fn new(module: &str, root: &str) -> Package {
        Package {
            module: module.to_string(),
            root: root.to_string(),
        }
    }

    /// The Rust path of the non-std module `module`, `None` when it is not
    /// in the package.
    fn path(&self, module: &str) -> Option<String> {
        if module == self.module {
            return Some(self.root.clone());
        }
        let rest = if self.module.is_empty() {
            module
        } else {
            module.strip_prefix(&format!("{}.", self.module))?
        };
        let mut path = self.root.clone();
        for segment in rest.split('.') {
            path.push_str("::");
            path.push_str(&ident(segment));
        }
        Some(path)
    }
}

/// The `use` lines of a program module or executable whose generated text
/// is `body`: [`UseTarget::render`] of each target, without the imported
/// constants `body` does not name. A clause (`conforms [RFC855]`) is not
/// generated, so a catalogue imported only for a clause leaves no `use`
/// line, and no `use` line roots its constants in the reachable
/// `ignite-std` (`docs/slice.md` D2 and 19.13; LLVM's `prune` drops them
/// the same way). Every other imported name keeps its `use`.
fn kept_use_lines(targets: &[UseTarget], symbols: &Symbols, body: &str) -> Vec<String> {
    let constants: BTreeSet<String> = symbols
        .constants
        .iter()
        .filter(|name| symbols.imported.contains_key(*name))
        .map(|name| ident(name))
        .collect();
    let mut referenced = BTreeSet::new();
    rust_identifiers(body, |name| {
        referenced.insert(name);
    });
    targets
        .iter()
        .filter_map(|target| {
            let names: Vec<String> = target
                .names
                .iter()
                .filter(|name| {
                    !constants.contains(name.as_str()) || referenced.contains(plain_ident(name))
                })
                .cloned()
                .collect();
            (!names.is_empty()).then(|| {
                UseTarget {
                    path: target.path.clone(),
                    names,
                }
                .render()
            })
        })
        .collect()
}

/// `use <root>::a::b::{X, Y};` per standard-library import and
/// `use <package root>::c::{f};` per import from a module of the
/// program's package; nothing for the substrate's own names or for imports
/// of a module that is neither.
fn use_targets(imports: &[Import], root: &str, package: &Package) -> Vec<UseTarget> {
    imports
        .iter()
        .filter_map(import_target)
        .filter_map(|(module, names)| {
            let (path, names) = if let Some(names) = std_import(&module, &names) {
                let mut path = root.to_string();
                for segment in &module[1..] {
                    path.push_str("::");
                    path.push_str(&ident(segment));
                }
                (path, names)
            } else if module.first() != Some(&"std") {
                (package.path(&module.join("."))?, names)
            } else {
                return None;
            };
            Some(UseTarget {
                path,
                names: names.iter().map(|n| ident(n)).collect(),
            })
        })
        .collect()
}

/// The `use` lines ([`kept_use_lines`]) and every record, enum, protocol
/// and fn of one program module, without the file header. `root` is the
/// Rust path of the `ignite-std` crate from this module: `ignite_std` in a
/// program.
fn module_items(
    program: &Program,
    symbols: &Symbols,
    types: Option<&TypedProgram>,
    uses: &[UseTarget],
    root: &'static str,
    module: &ModuleOptions<'_>,
) -> String {
    let mut items = String::new();
    for item in module_item_list(program, symbols, types, root, module) {
        items.push_str(&item.text);
    }
    let mut w = Writer::new();
    for line in kept_use_lines(uses, symbols, &items) {
        w.line(&line);
    }
    w.raw(&items);
    w.finish()
}

/// The items of [`module_items`], each with the name it defines: every
/// record, enum, alias, protocol, fn, constant and test of the module in
/// declaration order, then the glue fns they use.
fn module_item_list(
    program: &Program,
    symbols: &Symbols,
    types: Option<&TypedProgram>,
    root: &'static str,
    module: &ModuleOptions<'_>,
) -> Vec<Item> {
    let mut generator = Generator::new(symbols, program, types, module.path, module.source);
    generator.root = root;
    generator.package = module.package.clone();
    let mut items = Vec::new();
    for (index, decl) in program.decls.iter().enumerate() {
        if decl.is_test_only() && !module.tests {
            continue;
        }
        let mut w = Writer::new();
        let name = match decl {
            Decl::Assembly(_) | Decl::System(_) => continue,
            Decl::Alias(alias) => {
                generator.alias(&mut w, alias);
                ident(&alias.name.name)
            }
            Decl::Record(record) => {
                generator.record(&mut w, record);
                ident(&record.name.name)
            }
            Decl::Enum(decl) => {
                generator.enumeration(&mut w, decl);
                ident(&decl.name.name)
            }
            Decl::Fn(decl) => {
                generator.function(&mut w, decl);
                fn_symbol(decl)
            }
            Decl::Protocol(decl) => {
                generator.protocol(&mut w, decl);
                ident(&decl.name.name)
            }
            Decl::Const(decl) => {
                generator.constant(&mut w, decl);
                ident(&decl.name.name)
            }
            Decl::Test(test) => {
                generator.test_decl(&mut w, test, index);
                test_fn_name(index)
            }
        };
        items.push(Item {
            name: plain_ident(&name).to_string(),
            text: w.finish(),
        });
    }
    for (name, text) in generator.glue_item_list() {
        items.push(Item { name, text });
    }
    items
}

/// What [`module_items`] needs to know about one module beyond its AST.
struct ModuleOptions<'a> {
    path: &'a str,
    /// Lower its tests and test-only items (a module selected for a test
    /// build).
    tests: bool,
    source: Option<&'a SourceFile>,
    /// The program's package, for paths to its sibling modules.
    package: Package,
}

impl<'a> ModuleOptions<'a> {
    fn of(options: &'a Options, path: &'a str, package: &Package) -> ModuleOptions<'a> {
        ModuleOptions {
            path,
            tests: options.test_modules.iter().any(|m| m == path),
            source: options.sources.get(path),
            package: package.clone(),
        }
    }
}

/// The module path of a program: its typed module, else its package, else
/// `main`.
fn program_module(program: &Program, types: Option<&TypedProgram>) -> String {
    if let Some(types) = types {
        return types.module.clone();
    }
    program
        .package
        .as_ref()
        .map_or_else(|| "main".to_string(), |package| package.dotted())
}

/// The Rust path of the module `module` from the program crate
/// `crate_name` whose own module path is `program`.
fn module_rust_path(crate_name: &str, program: &str, module: &str) -> String {
    if module == program {
        return crate_name.to_string();
    }
    let mut path = String::from("ignite_std");
    for segment in module.split('.').skip(1) {
        path.push_str("::");
        path.push_str(&ident(segment));
    }
    path
}

/// `src/bin/ignite-test.rs`: [`ignite_io::test_main`] over `tests`.
fn test_main(crate_name: &str, program: &str, tests: &[TestInfo]) -> String {
    test_main_with(tests, |module| {
        module_rust_path(crate_name, program, module)
    })
}

/// [`test_main`] with `path` giving the Rust path of a test's module.
fn test_main_with(tests: &[TestInfo], path: impl Fn(&str) -> String) -> String {
    let mut w = Writer::new();
    w.raw(TEST_HEADER);
    w.raw(FILE_HEADER);
    w.line("");
    w.open("fn main() {");
    w.open("ignite_io::test_main(&[");
    for test in tests {
        w.line(&format!(
            "ignite_io::Test {{ name: \"{}\", run: &{}::{} }},",
            escape_str(&test.full_name),
            path(&test.module),
            test_fn_name(test.decl)
        ));
    }
    w.close("])");
    w.close("}");
    w.finish()
}

/// Cargo package name of the project of [`generate_tests`].
pub const TEST_PACKAGE: &str = "ignite-tests";

/// One program of [`generate_tests`]: its AST, what `check_module`
/// returned for it and its types.
pub type TestProgram<'a> = (&'a Program, &'a Symbols, &'a TypedProgram);

/// A test build of several programs in one crate, [`TEST_PACKAGE`]
/// (`docs/slice.md` section 12.7). Each program is the module of its
/// module path (`examples.todo` is `pub mod examples { pub mod todo { .. } }`),
/// so the programs' names never meet; the only executable is
/// [`TEST_EXECUTABLE`] over `options.tests`. The project depends on
/// `ignite-std` at `std/`. The programs' module paths must differ. Errors
/// are grouped by module path.
pub fn generate_tests(
    programs: &[TestProgram<'_>],
    options: &Options,
) -> Result<GeneratedProject, Vec<(String, Vec<Diagnostic>)>> {
    let mut errors = Vec::new();
    let mut tree = ModuleTree::default();
    let mut modules = BTreeSet::new();
    // Every program sits at its full module path under `crate`, so the
    // package of each is rooted at `crate` with an empty module path.
    let layout = Layout {
        module: String::new(),
        lib_root: "crate".to_string(),
    };
    for (program, symbols, types) in programs {
        match generate_program(program, symbols, Some(types), options, &[], &layout) {
            Ok(project) => {
                let items = project
                    .lib_rs
                    .strip_prefix(TEST_HEADER)
                    .unwrap_or(&project.lib_rs);
                let items = items.strip_prefix(FILE_HEADER).unwrap_or(items);
                let segments: Vec<&str> = types.module.split('.').collect();
                tree.insert(&segments, items.to_string());
                modules.insert(types.module.as_str());
            }
            Err(grouped) => {
                let diagnostics: Vec<Diagnostic> =
                    grouped.into_iter().flat_map(|(_, d)| d).collect();
                errors.push((types.module.clone(), diagnostics));
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let crate_name = TEST_PACKAGE.replace('-', "_");
    let mut lib = String::from(TEST_HEADER);
    lib.push_str(FILE_HEADER);
    tree.render(&mut lib, 0);
    let main = test_main_with(&options.tests, |module| {
        if !modules.contains(module) {
            return module_rust_path(&crate_name, "", module);
        }
        let mut path = crate_name.clone();
        for segment in module.split('.') {
            path.push_str("::");
            path.push_str(&ident(segment));
        }
        path
    });
    let executables = vec![Executable {
        name: TEST_EXECUTABLE.to_string(),
        source: main,
    }];
    let cargo_toml = cargo_toml(
        TEST_PACKAGE,
        &executables,
        &options.substrate_path,
        true,
        Some("runner"),
    );
    Ok(GeneratedProject {
        package_name: TEST_PACKAGE.to_string(),
        cargo_toml,
        lib_rs: lib,
        executables,
    })
}

/// Write `Cargo.toml`, `src/lib.rs` and `src/bin/*.rs` under `out`, creating
/// directories as needed and overwriting existing files. Also used for the
/// standard library, written to `<out>/std`. A file that already holds the
/// text is left alone, so cargo does not rebuild an unchanged project. The
/// directory is owned by the generator: a `src/bin/*.rs` from an earlier
/// build whose executable is gone is removed, and `src/bin` with it when no
/// executable is left, so cargo does not find a stale binary.
pub fn write_project(project: &GeneratedProject, out: &Path) -> io::Result<()> {
    fs::create_dir_all(out.join("src"))?;
    write_if_changed(out.join("Cargo.toml"), &project.cargo_toml)?;
    write_if_changed(out.join("src").join("lib.rs"), &project.lib_rs)?;
    let bin = out.join("src").join("bin");
    let names: Vec<String> = project
        .executables
        .iter()
        .map(|executable| format!("{}.rs", executable.name))
        .collect();
    if bin.is_dir() {
        for entry in fs::read_dir(&bin)? {
            let path = entry?.path();
            let stale = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_none_or(|name| !names.iter().any(|kept| kept == name));
            if stale && path.is_file() {
                fs::remove_file(&path)?;
            }
        }
        if project.executables.is_empty() {
            fs::remove_dir(&bin)?;
        }
    }
    if !project.executables.is_empty() {
        fs::create_dir_all(&bin)?;
        for executable in &project.executables {
            write_if_changed(
                bin.join(format!("{}.rs", executable.name)),
                &executable.source,
            )?;
        }
    }
    Ok(())
}

/// Write `contents` to `path` unless the file already holds it.
fn write_if_changed(path: impl AsRef<Path>, contents: &str) -> io::Result<()> {
    let path = path.as_ref();
    if fs::read(path).is_ok_and(|current| current == contents.as_bytes()) {
        return Ok(());
    }
    fs::write(path, contents)
}

/// The Cargo package name of a program: the last segment of its `package`
/// path with `.` replaced by `-`, or `program` without a package.
pub fn package_name(program: &Program) -> String {
    program
        .package
        .as_ref()
        .map(|path| path.last().name.replace('.', "-"))
        .unwrap_or_else(|| "program".to_string())
}

/// The Rust name of the program crate of the Cargo package `package_name`:
/// `-` as `_`, with a `_` after a name a crate cannot have in a path
/// (`async`, `self`, `super_`; [`ident`]'s rule, since a crate name cannot
/// be raw). [`cargo_toml`] names the lib so when it differs from the
/// package name.
fn crate_name(package_name: &str) -> String {
    let name = package_name.replace('-', "_");
    if escapes_to(&name, |base| {
        matches!(base, "self" | "Self" | "crate" | "super") || RUST_KEYWORDS.contains(&base)
    }) {
        format!("{name}_")
    } else {
        name
    }
}

/// A Rust string literal body for `text`: `\\ \" \n \r \t \0` escaped,
/// other control bytes as `\x..`, everything else as is.
pub fn escape_str(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// Lines with indentation, for top-level items and generated `main`s.
struct Writer {
    out: String,
    indent: usize,
}

impl Writer {
    fn new() -> Writer {
        Writer {
            out: String::new(),
            indent: 0,
        }
    }

    fn raw(&mut self, text: &str) {
        self.out.push_str(text);
    }

    fn line(&mut self, text: &str) {
        if !text.is_empty() {
            self.out.push_str(&indentation(self.indent));
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn open(&mut self, text: &str) {
        self.line(text);
        self.indent += 1;
    }

    fn close(&mut self, text: &str) {
        self.indent -= 1;
        self.line(text);
    }

    fn finish(self) -> String {
        self.out
    }
}

fn indentation(level: usize) -> String {
    "    ".repeat(level)
}

/// Every Rust keyword (strict, reserved and edition 2024) that a raw
/// identifier can spell: [`ident`] writes it as `r#type`. The Ignite
/// keywords among them reach `ident` too, because a field name may be a
/// keyword (`type Token { type: Int }`, `t.type`, `Token { type = 1 }`).
/// `self`, `Self`, `crate` and `super` cannot be raw; `ident` mangles them.
const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "dyn", "else", "enum", "extern", "false",
    "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "static", "struct", "trait", "true", "type", "unsafe", "use", "where", "while",
    "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof", "unsized",
    "virtual", "yield", "try", "gen",
];

/// Names of the Rust prelude the generated code uses bare (`Box::new`,
/// `Vec<_>`, `String::from`, `Some(..)`, the bounds `Clone + Send + Sync`,
/// `dyn Fn`): a user declaration of one of these names (`type Box`) is
/// emitted as `Box_`, as `self` is `self_`, so it does not shadow the
/// prelude item (`docs/friction.md` row 35). `Option` and `Result` are not
/// listed: they are Ignite's own builtins (E004 for a user declaration) and
/// lower to the Rust types of the same names.
const RUST_PRELUDE_NAMES: &[&str] = &[
    "Box",
    "Vec",
    "String",
    "Some",
    "None",
    "Ok",
    "Err",
    "Clone",
    "Copy",
    "Send",
    "Sync",
    "Sized",
    "Unpin",
    "Drop",
    "Fn",
    "FnMut",
    "FnOnce",
    "Default",
    "Iterator",
    "IntoIterator",
    "DoubleEndedIterator",
    "ExactSizeIterator",
    "Extend",
    "FromIterator",
    "From",
    "Into",
    "TryFrom",
    "TryInto",
    "AsRef",
    "AsMut",
    "ToOwned",
    "ToString",
    "PartialEq",
    "Eq",
    "PartialOrd",
    "Ord",
    "Future",
    "IntoFuture",
];

/// `name` is one of `mangled` followed by zero or more `_`.
fn escapes_to(name: &str, mangled: impl Fn(&str) -> bool) -> bool {
    mangled(name.trim_end_matches('_'))
}

/// A Ignite identifier as a Rust identifier. A name Rust cannot spell raw
/// (`self`, `Self`, `crate`, `super`) or a prelude name gets a `_`, and so
/// does every name that is one of them followed by `_`s: `self` is
/// `self_`, `self_` is `self__`. The mapping is one to one, so two Ignite
/// names never become one Rust name.
fn ident(name: &str) -> String {
    if escapes_to(name, |base| {
        matches!(base, "self" | "Self" | "crate" | "super") || RUST_PRELUDE_NAMES.contains(&base)
    }) {
        format!("{name}_")
    } else if RUST_KEYWORDS.contains(&name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

/// The associated fn of a record holding the default of `field`.
fn default_fn(field: &str) -> String {
    format!("default_{field}")
}

/// The value in base units of the quantity literal `literal unit`
/// (`docs/slice.md` section 14.1), as the typer computes it; `None` for an
/// unknown unit, an inexact decimal part or a value out of range. The
/// generator reads the typer's value and falls back to this one.
fn quantity_value(literal: &str, unit: &str) -> Option<i64> {
    let (_, factor) = ignite_syntax::ast::quantity_unit(unit)?;
    let (whole, fraction) = literal.split_once('.').unwrap_or((literal, ""));
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let mut value = whole.checked_mul(factor)?;
    if !fraction.is_empty() {
        let digits: i64 = fraction.parse().ok()?;
        let scale = 10i64.checked_pow(u32::try_from(fraction.len()).ok()?)?;
        let part = factor.checked_mul(digits)?;
        if part % scale != 0 {
            return None;
        }
        value = value.checked_add(part / scale)?;
    }
    Some(value)
}

/// The Rust path of a variant of `owner`, which is already Rust (an
/// [`ident`] or a path; `ident` is not idempotent).
fn variant_path(owner: &str, variant: &str) -> String {
    match owner {
        "Option" | "Result" => variant.to_string(),
        "Input" => format!("ignite_io::Input::{variant}"),
        "Exposure" => format!("ignite_io::Exposure::{variant}"),
        _ => format!("{owner}::{}", ident(variant)),
    }
}

/// A Ignite type as Rust.
pub fn rust_type(ty: &Type) -> String {
    rust_type_in(ty, &BTreeSet::new())
}

/// [`rust_type`] of an annotation inside a declaration whose type
/// parameters are `params`: a bare name among them is the parameter, even
/// when a builtin type has the same name (`fn keep<Text>(value: Text)`), and
/// a builtin enum they shadow is spelled by its path
/// ([`builtin_enum_path`]).
fn rust_type_in(ty: &Type, params: &BTreeSet<String>) -> String {
    let recurse = |t: &Type| rust_type_in(t, params);
    if ty.is_fn() {
        let ps: Vec<String> = ty.fn_params().iter().map(recurse).collect();
        let ret = ty.fn_ret().map_or_else(|| "()".to_string(), recurse);
        return fn_value_type(&ps, &ret);
    }
    if ty.args.is_empty() && params.contains(&ty.name.name) {
        return ident(&ty.name.name);
    }
    let args: Vec<String> = ty.args.iter().map(recurse).collect();
    let generic = |base: &str| {
        if args.is_empty() {
            base.to_string()
        } else {
            format!("{}<{}>", base, args.join(", "))
        }
    };
    match ty.name.name.as_str() {
        "Text" => "String".to_string(),
        "Bytes" => "Vec<u8>".to_string(),
        "Byte" => "u8".to_string(),
        "Int" => "i64".to_string(),
        "Bool" => "bool".to_string(),
        "Unit" => "()".to_string(),
        "Tuple" => format!("({})", args.join(", ")),
        "List" => generic("Vec"),
        "TcpListener" | "TcpStream" | "PipeStream" | "Stdin" | "Stdout" | "Input" | "IoError"
        | "Exposure" | "Directory" | "Launcher" | "ChildProcess" | "Entropy" | "Setting" => {
            format!("ignite_io::{}", ty.name.name)
        }
        "Store" => generic("ignite_io::Store"),
        "Map" => generic("std::collections::BTreeMap"),
        "Duration" | "ByteSize" | "Ratio" => "i64".to_string(),
        "Ref" | "Signal" | "SystemRef" | "Protocol" => "String".to_string(),
        "Module" if args.len() == 1 => args[0].clone(),
        name @ ("Option" | "Result") => generic(&builtin_enum_path(name, params)),
        other => generic(&ident(other)),
    }
}

/// The Rust name of the builtin enum `Option` or `Result`: the bare name,
/// or its `std` path when a type parameter in `params` has the same name
/// (Rust would read the bare name as the parameter, E0109). `ident` leaves
/// both names as they are, so no other spelling of a parameter reaches
/// them.
fn builtin_enum_path(name: &str, params: &BTreeSet<String>) -> String {
    if !params.contains(name) {
        return name.to_string();
    }
    match name {
        "Option" => "std::option::Option".to_string(),
        "Result" => "std::result::Result".to_string(),
        _ => name.to_string(),
    }
}

/// The Rust type of a fn value, closure or named fn (`docs/slice.md`
/// section 13.3): a shared, thread-safe `dyn Fn`.
fn fn_value_type(params: &[String], ret: &str) -> String {
    format!(
        "std::sync::Arc<dyn Fn({}) -> {ret} + Send + Sync>",
        params.join(", ")
    )
}

/// The bounds every type parameter carries in Rust (`docs/slice.md`
/// section 11.1).
const PARAM_BOUNDS: &str = "Clone + Send + Sync + 'static";

/// `<A: bounds, B: bounds>` for a declaration's type parameters, or
/// nothing.
fn bounded_params(params: &[Ident]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let params: Vec<String> = params
        .iter()
        .map(|p| format!("{}: {PARAM_BOUNDS}", ident(&p.name)))
        .collect();
    format!("<{}>", params.join(", "))
}

/// [`bounded_params`] for a fn, associated fn or role: a parameter bounded
/// by `ByteStream` also carries the substrate's trait
/// (`S: ignite_io::ByteStream + Clone + ..`, `docs/slice.md` 18.10).
fn fn_type_params(decl: &FnDecl) -> String {
    if decl.bounds.is_empty() {
        return bounded_params(&decl.type_params);
    }
    let params: Vec<String> = decl
        .type_params
        .iter()
        .map(|p| {
            let interfaces: String = decl
                .bounds
                .iter()
                .filter(|b| b.param.name == p.name)
                .flat_map(|b| &b.interfaces)
                .map(|i| format!("ignite_io::{} + ", ident(&i.name)))
                .collect();
            format!("{}: {interfaces}{PARAM_BOUNDS}", ident(&p.name))
        })
        .collect();
    format!("<{}>", params.join(", "))
}

/// The names of a declaration's type parameters.
fn param_names(params: &[Ident]) -> BTreeSet<String> {
    params.iter().map(|p| p.name.clone()).collect()
}

/// `<A, B>` for a declaration's type parameters, or nothing.
fn param_args(params: &[Ident]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let params: Vec<String> = params.iter().map(|p| ident(&p.name)).collect();
    format!("<{}>", params.join(", "))
}

/// Expressions that need no parentheses as a method receiver or operand of
/// a prefix operator.
fn is_atomic(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Quantity { .. } => true,
        ExprKind::NamedArg { value, .. } => is_atomic(value),
        ExprKind::Brace { .. } => false,
        ExprKind::TupleField { .. } => true,
        ExprKind::Closure(_)
        | ExprKind::RecordUpdate { .. }
        | ExprKind::Break
        | ExprKind::Continue => false,
        ExprKind::Tuple(_) => true,
        ExprKind::Return(_) => false,
        ExprKind::Int(value) => *value >= 0,
        ExprKind::Str(_)
        | ExprKind::Bool(_)
        | ExprKind::Name(_)
        | ExprKind::List(_)
        | ExprKind::Call { .. }
        | ExprKind::Method { .. }
        | ExprKind::Field { .. }
        | ExprKind::Index { .. }
        | ExprKind::Try(_)
        | ExprKind::Await(_) => true,
        ExprKind::Spawn(_) => false,
        ExprKind::Record { .. }
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. }
        | ExprKind::Match { .. }
        | ExprKind::If { .. }
        | ExprKind::Block(_) => false,
    }
}

fn wrap(expr: &Expr, text: String) -> String {
    if is_atomic(expr) {
        text
    } else {
        format!("({text})")
    }
}

/// `&text`, parenthesized when `expr` is not atomic.
fn borrow(expr: &Expr, text: String) -> String {
    format!("&{}", wrap(expr, text))
}

struct Generator<'a> {
    symbols: &'a Symbols,
    /// Protocol names in scope: from `symbols` and the module's own decls.
    protocols: BTreeSet<String>,
    /// Constant names in scope: from `symbols` and the module's own decls.
    constants: BTreeSet<String>,
    scopes: Vec<Vec<String>>,
    indent: usize,
    /// The module's types, when the caller has them.
    types: Option<&'a TypedProgram>,
    /// Counter for generated names (`__p1`) in pattern tests.
    fresh: usize,
    /// Rust path of the `ignite-std` crate (`ignite_std` or `crate`), for
    /// variants of enums the module does not import.
    root: &'static str,
    /// Locals emitted under another Rust name: the resources a `main`
    /// binds (`resource_variable`).
    renames: BTreeMap<String, String>,
    /// Names of [`Generator::renames`] the generated text read.
    renamed_uses: BTreeSet<String>,
    /// Parameters of the current fn that are references (`&T`) in its
    /// body: read only and not `Copy`.
    borrowed: BTreeSet<String>,
    /// `for` elements that are references (`&T`) in their body, with the
    /// index of the scope that binds them ([`Generator::borrowed_for`]).
    borrowed_elements: Vec<(usize, String)>,
    /// The source the module was read from, for `assert` and `?` reports.
    source: Option<&'a SourceFile>,
    /// Fixed filename escaped once for every failure site in this module.
    escaped_site_file: String,
    /// The module path, the position of a report without a source.
    module: String,
    /// Equality and debug glue fns the generated text calls, in order of
    /// first use; [`Generator::glue_items`] emits them.
    glue: Vec<(Glue, Ty)>,
    /// Inside a closure body: the number of scopes outside it. A local
    /// bound in one of those is a capture.
    closure_base: Option<usize>,
    /// The program's package: paths of its sibling modules.
    package: Package,
    /// The type parameters in scope of the item being generated. A record
    /// or enum of the same name is spelled by its module path
    /// ([`Generator::unshadowed`]), since Rust would read the bare name as
    /// the parameter.
    type_params: BTreeSet<String>,
}

/// An enum variant of a concrete type: its name and its payload types,
/// each with whether it is boxed.
type Variant = (String, Vec<(Ty, bool)>);

/// The kind of a glue fn (`docs/slice.md` sections 13.5 and 13.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Glue {
    /// `fn __ignite_eq_N(a: &T, b: &T) -> bool`.
    Eq,
    /// `fn __ignite_debug_N(v: &T, out: &mut String)`.
    Debug,
}

/// The most terms one Rust operator chain holds (`docs/slice.md` section
/// 13.1, row 14). A longer left spine lowers to chunked lets of at most
/// this many terms, one per line: rustc's cost per panic location grows
/// with the length of its source line.
const CHAIN_LIMIT: usize = 64;

/// The class of an operator whose left-deep chains are chunked (1
/// arithmetic, 2 logic; the result has the left operand's type), or 0. A
/// chunked spine holds operators of one class.
fn fold_class(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => 1,
        BinaryOp::And | BinaryOp::Or => 2,
        _ => 0,
    }
}

/// The Rust symbol of the associated fn `Owner.name` (`docs/slice.md`
/// section 13.8).
fn assoc_symbol(owner: &str, name: &str) -> String {
    format!("{owner}__{name}")
}

/// The Rust name of a fn: [`assoc_symbol`] for `fn Owner.name(..)`, else
/// its name as a Rust identifier.
fn fn_symbol(decl: &FnDecl) -> String {
    match &decl.owner {
        Some(owner) => assoc_symbol(&owner.name, &decl.name.name),
        None => ident(&decl.name.name),
    }
}

impl<'a> Generator<'a> {
    fn new(
        symbols: &'a Symbols,
        program: &Program,
        types: Option<&'a TypedProgram>,
        module: &str,
        source: Option<&'a SourceFile>,
    ) -> Generator<'a> {
        let mut protocols: BTreeSet<String> = symbols.protocols.keys().cloned().collect();
        let mut constants = symbols.constants.clone();
        for decl in &program.decls {
            match decl {
                Decl::Protocol(protocol) => {
                    protocols.insert(protocol.name.name.clone());
                }
                Decl::Const(constant) => {
                    constants.insert(constant.name.name.clone());
                }
                _ => {}
            }
        }
        Generator {
            symbols,
            protocols,
            constants,
            scopes: Vec::new(),
            indent: 0,
            types,
            fresh: 0,
            root: "ignite_std",
            renames: BTreeMap::new(),
            renamed_uses: BTreeSet::new(),
            borrowed: BTreeSet::new(),
            borrowed_elements: Vec::new(),
            source,
            escaped_site_file: escape_str(source.map_or(module, |source| source.path.as_str())),
            module: module.to_string(),
            glue: Vec::new(),
            closure_base: None,
            package: Package::new("", "crate"),
            type_params: BTreeSet::new(),
        }
    }

    /// Whether `module` is the module being generated.
    fn is_own_module(&self, module: &str) -> bool {
        module == self.module || self.types.is_some_and(|t| t.module == module)
    }

    /// The Rust path of module `module` from the item being generated:
    /// `None` for the module itself, the `ignite-std` path of a std module
    /// (`ignite_std::console`, `crate::console` inside `ignite-std`), the
    /// package path of a sibling module (`crate::commands`).
    fn module_path(&self, module: &str) -> Option<String> {
        if self.is_own_module(module) {
            return None;
        }
        if module == "std" || module.starts_with("std.") {
            let mut path = self.root.to_string();
            for segment in module.split('.').skip(1) {
                path.push_str("::");
                path.push_str(&ident(segment));
            }
            return Some(path);
        }
        self.package.path(module)
    }

    /// [`Generator::module_path`] that also names the module being
    /// generated (`crate`, `crate::commands`, `crate::console` inside
    /// `ignite-std`): the path of a record or enum shadowed by a type
    /// parameter.
    fn item_module_path(&self, module: &str) -> Option<String> {
        if !self.is_own_module(module) {
            return self.module_path(module);
        }
        if module == "std" || module.starts_with("std.") {
            let mut path = self.root.to_string();
            for segment in module.split('.').skip(1) {
                path.push_str("::");
                path.push_str(&ident(segment));
            }
            return Some(path);
        }
        self.package.path(module)
    }

    /// The Rust name of the user record, enum or protocol `name` as written
    /// in the source: its [`ident`], or its module path when a type
    /// parameter in scope has the same name (`crate::P::serve` inside
    /// `fn call<P>`). Already Rust: callers do not `ident` it again, since
    /// `ident` of `Self_` is `Self__`.
    fn unshadowed(&self, name: &str) -> String {
        let user_type = self.symbols.records.contains(name)
            || self.protocols.contains(name)
            || (self.symbols.enums.contains_key(name)
                && !BUILTIN_ENUMS.iter().any(|(e, _)| *e == name));
        if !user_type || !self.type_params.contains(name) {
            return ident(name);
        }
        let module = match self.symbols.imported.get(name) {
            Some(module) => module.clone(),
            None => self
                .types
                .map_or_else(|| self.module.clone(), |t| t.module.clone()),
        };
        match self.item_module_path(&module) {
            Some(path) => format!("{path}::{}", ident(name)),
            None => ident(name),
        }
    }

    /// [`rust_type`] of an annotation in the item being generated, with its
    /// type parameters in scope ([`rust_type_in`]).
    fn annotation(&self, ty: &Type) -> String {
        rust_type_in(ty, &self.type_params)
    }

    /// `"<file>", <line>, <col>` of a failure site (`docs/slice.md`
    /// section 14.8), the arguments every `ignite_io::checked` call and
    /// `ignite_io::fail` take after their operands.
    fn site_args(&self, site: FailureSite) -> String {
        match self.source {
            Some(source) => {
                let (line, col) = source.line_col(site.offset);
                format!("\"{}\", {line}, {col}", self.escaped_site_file)
            }
            None => format!("\"{}\", 0, 0", self.escaped_site_file),
        }
    }

    /// The `ignite_io::checked` fn of the arithmetic `op` at `expr`, with
    /// its site arguments, when the typer recorded a failure site for it:
    /// `add` on `Int` and quantities, `add_byte` on `Byte`.
    fn checked_op(&self, expr: &Expr, op: BinaryOp) -> Option<(String, String)> {
        let site = self.types?.failure_site(expr)?;
        let name = match op {
            BinaryOp::Add => "add",
            BinaryOp::Sub => "sub",
            BinaryOp::Mul => "mul",
            BinaryOp::Div => "div",
            BinaryOp::Rem => "rem",
            _ => return None,
        };
        let suffix = match self.type_of(expr) {
            Some(Ty::Byte) => "_byte",
            _ => "",
        };
        Some((
            format!("ignite_io::checked::{name}{suffix}"),
            self.site_args(site),
        ))
    }

    /// `ignite_io::fail(..)` for the failure site of `expr` with `detail`,
    /// when the typer recorded one.
    fn fail_call(&self, expr: &Expr, detail: &str) -> Option<String> {
        let site = self.types?.failure_site(expr)?;
        Some(format!(
            "ignite_io::fail(\"{}\", {}, \"{}\")",
            site.op.as_str(),
            self.site_args(site),
            escape_str(detail)
        ))
    }

    /// Whether a declared parameter type is `Copy` in Rust (`Int`, `Byte`,
    /// `Bool`, `Unit`, fn types), read through aliases when types are
    /// known.
    fn is_copy(&self, ty: &Type) -> bool {
        if let Some(resolved) = self.types.and_then(|t| t.resolved_type(ty)) {
            return matches!(
                resolved,
                Ty::Int | Ty::Byte | Ty::Bool | Ty::Unit | Ty::Fn(..) | Ty::EffectFn(..)
            );
        }
        ty.is_fn() || matches!(ty.name.name.as_str(), "Int" | "Byte" | "Bool" | "Unit")
    }

    /// Whether `expr`, a place in a move position, takes its value without
    /// a copy: the moves analysis marks it (`docs/slice.md` 20.3). Rust
    /// cannot move out of a reference or a capture; the analysis marks no
    /// place rooted at a borrowed parameter, a `for` element or a capture,
    /// and this keeps the copy should it ever mark one.
    fn moves(&self, expr: &Expr) -> bool {
        self.types.is_some_and(|t| t.moves(expr))
            && place_root(expr)
                .is_some_and(|root| !self.is_borrowed(root) && !self.is_captured(root))
    }

    /// Whether `callee` consumes its parameter `index` (`docs/slice.md`
    /// 20.3.3): such a parameter is declared by value and takes its
    /// argument as a value. False without types.
    fn consumes(&self, callee: Option<&Callee>, index: usize) -> bool {
        match (self.types, callee) {
            (Some(types), Some(callee)) => types.consumed(callee, index),
            _ => false,
        }
    }

    /// Whether the call `expr` of the non-local name `id` calls a fn of the
    /// module or an imported one (not a variant, `log` or a constant).
    fn calls_fn(&self, expr: &Expr, id: &Ident) -> bool {
        match self.types {
            Some(_) => matches!(self.callee(expr), Some(Callee::Fn(_))),
            None => {
                id.name != "log"
                    && self.symbols.functions.contains_key(&id.name)
                    && !self.constants.contains(&id.name)
                    && self.enum_of_variant(&id.name).is_none()
            }
        }
    }

    /// Whether local `name` is a parameter passed by reference or a `for`
    /// element bound by reference, not shadowed.
    fn is_borrowed(&self, name: &str) -> bool {
        if self.borrowed.contains(name) && self.is_outermost(name) {
            return true;
        }
        let owned = name.to_string();
        self.scopes
            .iter()
            .rposition(|scope| scope.contains(&owned))
            .is_some_and(|index| {
                self.borrowed_elements
                    .iter()
                    .any(|(at, element)| *at == index && *element == owned)
            })
    }

    /// Whether `for name in iter { body }` can iterate `iter` by reference
    /// instead of over a copy: `iter` is a place holding a list whose
    /// elements are not `Copy` in Rust, and `body` changes neither `name`
    /// nor the variable `iter` is rooted at. The element is then a
    /// reference in `body`, read like a borrowed parameter.
    fn borrowed_for(&self, name: &str, iter: &Expr, body: &Block) -> bool {
        if !self.is_place(iter) {
            return false;
        }
        let Some(Ty::List(element)) = self.type_of(iter) else {
            return false;
        };
        if !matches!(
            **element,
            Ty::Text
                | Ty::List(_)
                | Ty::Option(_)
                | Ty::Result(..)
                | Ty::Record(_)
                | Ty::Enum(_)
                | Ty::Tuple(_)
                | Ty::GenericRecord(..)
                | Ty::GenericEnum(..)
                | Ty::Map(..)
        ) {
            return false;
        }
        let mut mutated = BTreeSet::new();
        mutated_names(body, self.types, &mut mutated);
        !mutated.contains(name) && place_root(iter).is_some_and(|root| !mutated.contains(root))
    }

    /// `name` as a Rust operand that must not be a reference: `(*name)`
    /// for a borrowed parameter.
    fn deref_if_borrowed(&self, expr: &Expr, text: String) -> String {
        match &expr.kind {
            ExprKind::Name(id) if self.is_borrowed(&id.name) => format!("(*{text})"),
            _ => text,
        }
    }

    /// An argument for a parameter passed by reference: a borrowed
    /// parameter as it is, a place borrowed (`&x.f`, no copy), any other
    /// value borrowed after it is computed.
    /// Arguments by reference, left to right. A place whose variable a
    /// later argument modifies (`x.pick(if c { x = ..; 1 } else { 2 })`)
    /// is copied before the later argument runs, so the call sees the
    /// value the place had when its argument was evaluated. The argument
    /// of a parameter that `callee` consumes is a value instead
    /// ([`Generator::value`]: moved when the analysis says so, else a copy).
    fn ref_args<'e>(
        &mut self,
        callee: Option<&Callee>,
        args: impl IntoIterator<Item = &'e Expr>,
    ) -> Vec<String> {
        let args: Vec<&Expr> = args.into_iter().collect();
        let mut later = BTreeSet::new();
        let mut modified = vec![BTreeSet::new(); args.len()];
        for (index, arg) in args.iter().enumerate().rev() {
            modified[index] = later.clone();
            mutated_in(arg, self.types, &mut later);
        }
        args.iter()
            .zip(modified)
            .enumerate()
            .map(|(index, (arg, modified))| {
                if self.consumes(callee, index) {
                    return self.value(arg);
                }
                let root = if let Some((list, _, _)) = self.borrowed_at(arg) {
                    place_root(list)
                } else if self.is_place(arg) {
                    place_root(arg)
                } else if self.field_of_element(arg) {
                    self.element_root(arg)
                } else {
                    None
                };
                if root.is_some_and(|r| modified.contains(r)) {
                    format!("&{}", self.value(arg))
                } else {
                    self.ref_arg(arg)
                }
            })
            .collect()
    }

    fn ref_arg(&mut self, arg: &Expr) -> String {
        if let ExprKind::Name(id) = &arg.kind
            && self.is_borrowed(&id.name)
        {
            return self.name(id);
        }
        if self.is_place(arg) {
            return format!("&{}", self.bare(arg));
        }
        if let Some((list, index, site)) = self.borrowed_at(arg) {
            return self.checked_at_call(list, index, site);
        }
        if self.field_of_element(arg) {
            return format!("&{}", self.element_field(arg));
        }
        let text = self.value(arg);
        borrow(arg, text)
    }

    /// `place.at(i)` with a failure site, whose element an argument by
    /// reference can borrow from the list (`ignite_io::checked::at`
    /// returns `&T`) instead of copying: the list, the index, the site.
    fn borrowed_at<'e>(&self, expr: &'e Expr) -> Option<(&'e Expr, &'e Expr, FailureSite)> {
        let ExprKind::Method { receiver, args, .. } = &expr.kind else {
            return None;
        };
        let [index] = args.as_slice() else {
            return None;
        };
        if !matches!(self.callee(expr), Some(Callee::Builtin(Builtin::ListAt)))
            || !self.is_place(receiver)
        {
            return None;
        }
        let site = self.types.and_then(|t| t.failure_site(expr))?;
        Some((&**receiver, index, site))
    }

    /// A value in a result position (the tail of a value block, an arm, a
    /// branch): a move position of `docs/slice.md` 20.3.1. A place moves
    /// only where the moves analysis marks it ([`Generator::value`]); else
    /// it is copied, as a tail that is read later (in the block, after the
    /// `if` or `match`, in the next loop iteration) must be. A place of a
    /// Rust `Copy` type, or of an unknown type, is written as it is: Rust
    /// copies it. A place reached through a borrowed parameter or a capture
    /// is always copied, since it cannot be moved out. Anything else as
    /// written.
    fn result(&mut self, expr: &Expr) -> String {
        let copies = matches!(
            self.type_of(expr),
            None | Some(Ty::Int | Ty::Byte | Ty::Bool | Ty::Unit | Ty::Quantity(_))
        );
        let reached =
            place_root(expr).is_some_and(|root| self.is_borrowed(root) || self.is_captured(root));
        if self.is_place(expr) && (reached || !copies) {
            return self.value(expr);
        }
        self.bare(expr)
    }

    /// The arguments a fn-value wrapper passes on to the fn `callee`: its
    /// owned `names`, each by reference, or by value to a parameter the
    /// callee consumes (`docs/slice.md` 20.3.4).
    fn wrapper_args(&self, callee: Option<&Callee>, names: &[String]) -> Vec<String> {
        names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                if self.consumes(callee, index) {
                    name.clone()
                } else {
                    format!("&{name}")
                }
            })
            .collect()
    }

    /// A fn of the module or an imported one used as a value: a closure
    /// that takes its arguments for the fn (by reference, or by value to
    /// a consumed parameter), in an `Arc` cast to the fn value type
    /// (`docs/slice.md` section 13.3:
    /// `(Arc::new(|__a0: i64| f(&__a0)) as Arc<dyn Fn(i64) -> i64 + ..>)`).
    /// Without types the closure is left to inference.
    fn fn_value(&mut self, expr: &Expr, id: &Ident) -> String {
        let target = format!("{}{}", self.name(id), self.turbofish(expr));
        let arity = self.symbols.functions.get(&id.name).map(|info| info.arity);
        self.fn_value_of(expr, target, arity)
    }

    /// [`Generator::fn_value`] of the Rust fn path `target`; `arity` is
    /// used when the value's type is not known.
    fn fn_value_of(&mut self, expr: &Expr, target: String, arity: Option<usize>) -> String {
        let fn_ty = match self.type_of(expr) {
            Some(Ty::Fn(params, ret) | Ty::EffectFn(params, ret, _)) => {
                Some((params.clone(), (**ret).clone()))
            }
            _ => None,
        };
        let arity = match (&fn_ty, arity) {
            (Some((params, _)), _) => params.len(),
            (None, Some(arity)) => arity,
            (None, None) => return target,
        };
        let names: Vec<String> = (0..arity).map(|i| format!("__a{i}")).collect();
        let refs = self.wrapper_args(self.callee(expr), &names);
        let call = format!("{target}({})", refs.join(", "));
        match fn_ty {
            Some((params, ret)) => {
                let typed: Vec<String> = names
                    .iter()
                    .zip(&params)
                    .map(|(n, t)| format!("{n}: {}", self.rust_ty(t)))
                    .collect();
                let types: Vec<String> = params.iter().map(|t| self.rust_ty(t)).collect();
                format!(
                    "(std::sync::Arc::new(|{}| {call}) as {})",
                    typed.join(", "),
                    fn_value_type(&types, &self.rust_ty(&ret))
                )
            }
            None => format!("std::sync::Arc::new(|{}| {call})", names.join(", ")),
        }
    }

    /// A type as Rust, from the typer's [`Ty`]: the instantiation of a
    /// generic use. A type parameter is its own name.
    fn rust_ty(&self, ty: &Ty) -> String {
        let list = |items: &[Ty]| -> String {
            items
                .iter()
                .map(|t| self.rust_ty(t))
                .collect::<Vec<_>>()
                .join(", ")
        };
        match ty {
            Ty::Map(key, value) => format!(
                "std::collections::BTreeMap<{}, {}>",
                self.rust_ty(key),
                self.rust_ty(value)
            ),
            Ty::Quantity(_) => "i64".to_string(),
            Ty::Ref(_) | Ty::Signal(_) | Ty::SystemRef | Ty::Protocol => "String".to_string(),
            Ty::Int => "i64".to_string(),
            Ty::Bool => "bool".to_string(),
            Ty::Byte => "u8".to_string(),
            Ty::Unit | Ty::Never => "()".to_string(),
            Ty::Text => "String".to_string(),
            Ty::List(item) => format!("Vec<{}>", self.rust_ty(item)),
            Ty::Option(item) => format!(
                "{}<{}>",
                builtin_enum_path("Option", &self.type_params),
                self.rust_ty(item)
            ),
            Ty::Result(ok, err) => format!(
                "{}<{}, {}>",
                builtin_enum_path("Result", &self.type_params),
                self.rust_ty(ok),
                self.rust_ty(err)
            ),
            Ty::Record(q) | Ty::Enum(q) => self.enum_name(q),
            Ty::GenericRecord(q, args) | Ty::GenericEnum(q, args) => {
                format!("{}<{}>", self.enum_name(q), list(args))
            }
            Ty::Fn(params, ret) | Ty::EffectFn(params, ret, _) => {
                let params: Vec<String> = params.iter().map(|t| self.rust_ty(t)).collect();
                fn_value_type(&params, &self.rust_ty(ret))
            }
            Ty::Tuple(items) => format!("({})", list(items)),
            Ty::TcpListener => "ignite_io::TcpListener".to_string(),
            Ty::Capability(kind) => format!("ignite_io::{}", kind.name()),
            Ty::TcpStream => "ignite_io::TcpStream".to_string(),
            Ty::PipeStream => "ignite_io::PipeStream".to_string(),
            Ty::IoError => "ignite_io::IoError".to_string(),
            Ty::Input => "ignite_io::Input".to_string(),
            Ty::Param(name) => ident(name),
            Ty::Store(item) => format!("ignite_io::Store<{}>", self.rust_ty(item)),
        }
    }

    /// `::<A, B>` for the type arguments the typer recorded for a generic
    /// use, or nothing.
    fn turbofish(&self, expr: &Expr) -> String {
        match self.types.and_then(|t| t.instance(expr)) {
            Some(args) if !args.is_empty() => {
                let args: Vec<String> = args.iter().map(|t| self.rust_ty(t)).collect();
                format!("::<{}>", args.join(", "))
            }
            _ => String::new(),
        }
    }

    /// [`Generator::turbofish`] for a call, role call or `spawn` of a fn or
    /// role with at least one bounded type parameter, nothing for any other
    /// (`docs/slice.md` 18.10, D8): the calls of unbounded generic fns stay
    /// as they were.
    fn bounded_turbofish(&self, expr: &Expr) -> String {
        let Some(types) = self.types else {
            return String::new();
        };
        let (name, params) = match self.callee(expr) {
            Some(Callee::Fn(q)) => (q.clone(), types.type_params(q)),
            Some(Callee::Role { protocol, role }) => (
                QualName::new(protocol.module.clone(), format!("{}.{role}", protocol.name)),
                types.role_type_params(protocol, role),
            ),
            _ => return String::new(),
        };
        let bounded = params
            .unwrap_or(&[])
            .iter()
            .any(|p| types.bound(&name, p).is_some());
        if bounded {
            self.turbofish(expr)
        } else {
            String::new()
        }
    }

    /// The Rust name of record or enum `q` in this module: its name when
    /// it is builtin, own or imported, else its path in `ignite-std`
    /// (`ignite_std::console::Action`). The builtin `Exposure` is the
    /// substrate's `ignite_io::Exposure`.
    fn enum_name(&self, q: &QualName) -> String {
        if q.is_builtin() {
            return match q.name.as_str() {
                "Exposure" => "ignite_io::Exposure".to_string(),
                _ => q.name.clone(),
            };
        }
        if self.type_params.contains(&q.name)
            && let Some(path) = self.item_module_path(&q.module)
        {
            return format!("{path}::{}", ident(&q.name));
        }
        if self.symbols.imported.get(&q.name) == Some(&q.module) {
            return ident(&q.name);
        }
        match self.module_path(&q.module) {
            Some(path) => format!("{path}::{}", ident(&q.name)),
            None => ident(&q.name),
        }
    }

    /// The Rust path of record type `ty` as a struct literal or associated
    /// fn owner: `Name`, or `Name::<A, B>` for a generic instance.
    fn type_path(&self, ty: &Ty) -> String {
        match ty {
            Ty::GenericRecord(q, args) | Ty::GenericEnum(q, args) => {
                let args: Vec<String> = args.iter().map(|t| self.rust_ty(t)).collect();
                format!("{}::<{}>", self.enum_name(q), args.join(", "))
            }
            Ty::Record(q) | Ty::Enum(q) => self.enum_name(q),
            other => self.rust_ty(other),
        }
    }

    /// `type A<T> = B` as `pub type A<T> = B;`: aliases stay names in the
    /// generated Rust, so importing one needs no expansion.
    fn alias(&mut self, w: &mut Writer, alias: &AliasDecl) {
        let params = if alias.params.is_empty() {
            String::new()
        } else {
            let names: Vec<String> = alias.params.iter().map(|p| ident(&p.name)).collect();
            format!("<{}>", names.join(", "))
        };
        w.line("");
        w.line(&format!(
            "pub type {}{params} = {};",
            ident(&alias.name.name),
            rust_type_in(&alias.ty, &param_names(&alias.params))
        ));
    }

    /// The defaults of a record's fields: `field: T = e` as
    /// `impl R { pub fn default_field() -> T { e } }`; a literal that omits
    /// the field calls `R::default_field()`.
    fn defaults(&mut self, w: &mut Writer, record: &RecordDecl) {
        if record.fields.iter().all(|f| f.default.is_none()) {
            return;
        }
        w.line("");
        w.open(&format!(
            "impl{} {}{} {{",
            bounded_params(&record.type_params),
            ident(&record.name.name),
            param_args(&record.type_params)
        ));
        for field in &record.fields {
            let Some(default) = &field.default else {
                continue;
            };
            self.scopes = vec![Vec::new()];
            self.indent = w.indent + 1;
            self.type_params = param_names(&record.type_params);
            let value = self.value(default);
            self.type_params.clear();
            self.scopes.clear();
            w.line("#[allow(non_snake_case)]");
            w.open(&format!(
                "pub fn {}() -> {} {{",
                default_fn(&field.name.name),
                rust_type_in(&field.ty, &param_names(&record.type_params))
            ));
            w.line(&value);
            w.close("}");
        }
        w.close("}");
    }

    fn callee(&self, e: &Expr) -> Option<&'a Callee> {
        self.types?.callee(e)
    }

    fn type_of(&self, e: &Expr) -> Option<&'a Ty> {
        self.types?.try_type_of(e)
    }

    fn fresh(&mut self) -> String {
        self.fresh += 1;
        format!("__p{}", self.fresh)
    }

    /// `const N: T = e` as `pub fn N() -> T { e }`; every use is `N()`.
    fn constant(&mut self, w: &mut Writer, decl: &ConstDecl) {
        self.scopes = vec![Vec::new()];
        self.indent = w.indent + 1;
        let value = self.value(&decl.value);
        self.scopes.clear();
        w.line("");
        w.line("#[allow(non_snake_case)]");
        w.open(&format!(
            "pub fn {}() -> {} {{",
            ident(&decl.name.name),
            rust_type(&decl.ty)
        ));
        w.line(&value);
        w.close("}");
    }

    fn protocol(&mut self, w: &mut Writer, decl: &ProtocolDecl) {
        let name = ident(&decl.name.name);
        w.line("");
        w.line(&format!("pub struct {name};"));
        w.line("");
        w.open(&format!("impl {name} {{"));
        for (index, role) in decl.fns.iter().enumerate() {
            self.function_line(w, role, index > 0, true);
        }
        w.close("}");
    }

    /// The `QualName` of a record or enum this module declares.
    fn own(&self, name: &Ident) -> Option<QualName> {
        self.types
            .map(|t| QualName::new(t.module.clone(), name.name.clone()))
    }

    fn record(&mut self, w: &mut Writer, record: &RecordDecl) {
        let q = self.own(&record.name);
        w.line("");
        w.line("#[derive(Clone)]");
        w.open(&format!(
            "pub struct {}{} {{",
            ident(&record.name.name),
            bounded_params(&record.type_params)
        ));
        for field in &record.fields {
            let boxed = match (self.types, &q) {
                (Some(types), Some(q)) => types.boxed_field(q, &field.name.name),
                _ => false,
            };
            let ty = rust_type_in(&field.ty, &param_names(&record.type_params));
            let ty = if boxed { format!("Box<{ty}>") } else { ty };
            w.line(&format!("pub {}: {ty},", ident(&field.name.name)));
        }
        w.close("}");
        self.defaults(w, record);
    }

    /// An enum without payloads also derives the traits of a `BTreeMap`
    /// key (`docs/slice.md` 14.3: enums without payloads are `Map` keys);
    /// the derived order is the declaration order of the variants, as in
    /// the LLVM target and the evaluator.
    fn enumeration(&mut self, w: &mut Writer, decl: &EnumDecl) {
        let q = self.own(&decl.name);
        w.line("");
        if decl.variants.iter().all(|v| v.payload.is_empty()) {
            w.line("#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]");
        } else {
            w.line("#[derive(Clone)]");
        }
        w.open(&format!(
            "pub enum {}{} {{",
            ident(&decl.name.name),
            bounded_params(&decl.type_params)
        ));
        let params = param_names(&decl.type_params);
        for (v, variant) in decl.variants.iter().enumerate() {
            if variant.payload.is_empty() {
                w.line(&format!("{},", ident(&variant.name.name)));
            } else {
                let payload: Vec<String> = variant
                    .payload
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| {
                        let boxed = match (self.types, &q) {
                            (Some(types), Some(q)) => types.boxed_payload(q, v, i),
                            _ => false,
                        };
                        if boxed {
                            format!("Box<{}>", rust_type_in(ty, &params))
                        } else {
                            rust_type_in(ty, &params)
                        }
                    })
                    .collect();
                w.line(&format!(
                    "{}({}),",
                    ident(&variant.name.name),
                    payload.join(", ")
                ));
            }
        }
        w.close("}");
    }

    fn function(&mut self, w: &mut Writer, decl: &FnDecl) {
        self.function_line(w, decl, true, false);
    }

    /// `test "name" { body }` as `pub fn __ignite_test_<decl>() { body }`
    /// (`docs/slice.md` section 12.3: typed like `fn() -> Unit`).
    fn test_decl(&mut self, w: &mut Writer, test: &TestDecl, decl: usize) {
        self.borrowed.clear();
        self.scopes = vec![Vec::new()];
        self.indent = w.indent;
        let body = self.block(&test.body, false);
        self.scopes.clear();
        w.line("");
        w.line(&format!("pub fn {}() {body}", test_fn_name(decl)));
    }

    /// `<path>:<line>:<col>` of byte `offset` of this module's source.
    fn position(&self, offset: usize) -> String {
        match self.source {
            Some(source) => {
                let (line, col) = source.line_col(offset);
                format!("{}:{line}:{col}", source.path)
            }
            None => format!("{}:?:?", self.module),
        }
    }

    /// The source text of `span`, as written.
    fn source_text(&self, span: Span) -> String {
        self.source
            .and_then(|source| source.text.get(span.start..span.end))
            .map_or_else(|| "<condition>".to_string(), str::to_string)
    }

    /// A Rust `String` expression showing the value of the Rust place
    /// `value` of type `ty` (`docs/slice.md` section 12.3).
    fn show(&mut self, show: Option<&Show>, ty: &Ty, value: &str) -> String {
        match show {
            Some(Show::Debug) => self.debug_text(ty, &format!("&{value}")),
            Some(Show::Builtin(Builtin::TextToText)) => format!("{value}.clone()"),
            Some(Show::Builtin(Builtin::BytesToText)) => {
                format!("String::from_utf8_lossy(&{value}).into_owned()")
            }
            Some(Show::Builtin(_)) => format!("{value}.to_string()"),
            Some(Show::Fn(q)) if self.consumes(Some(&Callee::Fn(q.clone())), 0) => {
                format!("{}({value}.clone())", self.fn_path(q))
            }
            Some(Show::Fn(q)) => format!("{}(&{value})", self.fn_path(q)),
            Some(Show::IoErrorMessage) => format!("{value}.message.clone()"),
            None => format!("String::from(\"{}\")", escape_str(&format!("<{ty}>"))),
        }
    }

    /// `assert cond` and `assert cond, message` (`docs/slice.md` section
    /// 12.5). For `a == b` and `a != b` each side is evaluated once into a
    /// local; scalars and `Text` compare with Rust `==`, every other type
    /// with its equality glue (section 13.5).
    fn assert(&mut self, stmt: &Stmt, cond: &Expr, message: Option<&Expr>) -> String {
        let text = escape_str(&self.source_text(cond.span));
        let at = escape_str(&self.position(stmt.span.start));
        let message = message.map(|m| self.value(m));
        let report = |operands: &str| match &message {
            Some(m) => format!(
                "{{ let __ignite_message: String = {m}; ignite_io::assert_failed(\"{text}\", Some(&__ignite_message), {operands}, \"{at}\") }}"
            ),
            None => format!("ignite_io::assert_failed(\"{text}\", None, {operands}, \"{at}\")"),
        };
        let compare = self.types.and_then(|t| t.assert_compare(cond));
        if let (
            Some(compare),
            ExprKind::Binary {
                op: op @ (BinaryOp::Eq | BinaryOp::Ne),
                left,
                right,
            },
        ) = (compare, &cond.kind)
        {
            let left = self.value(left);
            let right = self.value(right);
            let equal = match compare.operand {
                Ty::Int
                | Ty::Byte
                | Ty::Bool
                | Ty::Text
                | Ty::Unit
                | Ty::Quantity(_)
                | Ty::Ref(_)
                | Ty::Signal(_)
                | Ty::SystemRef
                | Ty::Protocol => "__ignite_left == __ignite_right".to_string(),
                _ => self.eq_test(&compare.operand, "&__ignite_left", "&__ignite_right"),
            };
            let holds = match op {
                BinaryOp::Eq => format!("({equal})"),
                _ => format!("!({equal})"),
            };
            let shown_left = self.show(compare.show.as_ref(), &compare.operand, "__ignite_left");
            let shown_right = self.show(compare.show.as_ref(), &compare.operand, "__ignite_right");
            let operands = "Some((&__ignite_shown_left, &__ignite_shown_right))";
            let ty = self.rust_ty(&compare.operand);
            return format!(
                "{{ let __ignite_left: {ty} = {left}; let __ignite_right: {ty} = {right}; if !{holds} {{ let __ignite_shown_left: String = {shown_left}; let __ignite_shown_right: String = {shown_right}; {} }} }}",
                report(operands)
            );
        }
        let cond = self.condition(cond);
        format!("if !({cond}) {{ {} }}", report("None"))
    }

    /// `e?` directly in a test body (`docs/slice.md` section 12.5): `Err`
    /// or `None` fails the test.
    fn test_try(&mut self, expr: &Expr, inner: &Expr, test_try: &TestTry) -> String {
        let value = self.value(inner);
        let at = escape_str(&self.position(expr.span.end.saturating_sub(1)));
        match &test_try.error {
            Some(error) => {
                let shown = self.show(test_try.show.as_ref(), error, "__ignite_error");
                format!(
                    "(match {value} {{ Ok(__ignite_ok) => __ignite_ok, Err(__ignite_error) => {{ let __ignite_shown: String = {shown}; ignite_io::try_failed(Some(&__ignite_shown), \"{at}\") }} }})"
                )
            }
            None => format!(
                "(match {value} {{ Some(__ignite_ok) => __ignite_ok, None => ignite_io::try_failed(None, \"{at}\") }})"
            ),
        }
    }

    /// One `pub fn`, preceded by an empty line when `spaced`. A protocol
    /// `role` takes its parameters by value (a generated `main` calls it);
    /// every other fn takes them by reference (section 6, parameters).
    fn function_line(&mut self, w: &mut Writer, decl: &FnDecl, spaced: bool, role: bool) {
        let mut mutated = BTreeSet::new();
        mutated_names(&decl.body, self.types, &mut mutated);
        let mut prologue = Vec::new();
        self.borrowed.clear();
        self.type_params = decl.type_params.iter().map(|p| p.name.clone()).collect();
        let callee = self
            .types
            .map(|t| Callee::Fn(QualName::new(t.module.clone(), decl.qualified_name())));
        let params: Vec<String> = decl
            .params
            .iter()
            .enumerate()
            .map(|(index, p)| {
                let name = ident(&p.name.name);
                let ty = self.annotation(&p.ty);
                if role {
                    return format!("mut {name}: {ty}");
                }
                if self.consumes(callee.as_ref(), index) {
                    let mutability = if mutated.contains(&p.name.name) {
                        "mut "
                    } else {
                        ""
                    };
                    return format!("{mutability}{name}: {ty}");
                }
                if mutated.contains(&p.name.name) || self.is_copy(&p.ty) {
                    prologue.push(format!("let mut {name} = {name}.clone();"));
                } else {
                    self.borrowed.insert(p.name.name.clone());
                }
                format!("{name}: &{ty}")
            })
            .collect();
        let ret = match &decl.ret {
            Some(ty) => format!(" -> {}", self.annotation(ty)),
            None => String::new(),
        };
        let value = decl
            .ret
            .as_ref()
            .is_some_and(|ty| !(ty.name.name == "Unit" && ty.args.is_empty()));
        self.scopes = vec![decl.params.iter().map(|p| p.name.name.clone()).collect()];
        self.indent = w.indent;
        let mut body = self.block(&decl.body, value);
        self.type_params.clear();
        self.scopes.clear();
        self.borrowed.clear();
        if !prologue.is_empty() {
            let inner = indentation(w.indent + 1);
            let lines: String = prologue
                .iter()
                .map(|line| format!("{inner}{line}\n"))
                .collect();
            body.insert_str("{\n".len(), &lines);
        }
        if spaced {
            w.line("");
        }
        if decl.owner.is_some() {
            w.line("#[allow(non_snake_case)]");
        }
        let name = fn_symbol(decl);
        w.line(&format!(
            "pub fn {name}{}({}){ret} {body}",
            fn_type_params(decl),
            params.join(", ")
        ));
    }

    fn is_local(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|scope| scope.iter().any(|n| n == name))
    }

    /// Whether local `name` is bound in the outermost scope (a fn's
    /// parameters, a `main`'s resources) and not shadowed.
    fn is_outermost(&self, name: &str) -> bool {
        let name = name.to_string();
        self.scopes.iter().rposition(|scope| scope.contains(&name)) == Some(0)
    }

    fn bind(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(name.to_string());
        }
    }

    fn is_enum(&self, name: &str) -> bool {
        self.symbols.enums.contains_key(name) || BUILTIN_ENUMS.iter().any(|(e, _)| *e == name)
    }

    /// The enum owning a bare variant name, if exactly one does.
    fn enum_of_variant(&self, variant: &str) -> Option<String> {
        if self.symbols.variants.contains_key(variant) {
            return self.symbols.enum_of_variant(variant).map(str::to_string);
        }
        let owners: Vec<&str> = BUILTIN_ENUMS
            .iter()
            .filter(|(_, variants)| variants.contains(&variant))
            .map(|(e, _)| *e)
            .collect();
        match owners.as_slice() {
            [owner] => Some(owner.to_string()),
            _ => None,
        }
    }

    /// A non-local uppercase name used as a type qualifier (`Enum.V`,
    /// `Bytes.empty`).
    fn qualifier<'e>(&self, expr: &'e Expr) -> Option<&'e str> {
        match &expr.kind {
            ExprKind::Name(id) if id.is_uppercase() && !self.is_local(&id.name) => {
                Some(id.name.as_str())
            }
            _ => None,
        }
    }

    /// Whether `expr` has type `Never` (`Process.exit(c)`, whose Rust form
    /// `ignite_io::exit` returns `!`), so `expr?` lowers to `expr` alone.
    /// Without types only a `Process.exit` call is recognised.
    fn diverges(&self, expr: &Expr) -> bool {
        if let Some(ty) = self.type_of(expr) {
            return matches!(ty, Ty::Never);
        }
        match &expr.kind {
            ExprKind::Method { receiver, name, .. } => {
                self.qualifier(receiver) == Some("Process") && name.name == "exit"
            }
            _ => false,
        }
    }

    /// Whether `expr` is a value whose type only its context decides: `[]`,
    /// `None`, `List.empty()`, `Map.empty()`, `Store.new()`, or a block or
    /// `if` ending in one. The typer decides such a `let` from later uses
    /// (`docs/slice.md` section 13, row 3); rustc does not when the first
    /// use is a generic fn such as `ignite_io::checked::at`, so the `let`
    /// carries the type the typer recorded.
    fn undecided(&self, expr: &Expr) -> bool {
        let tail = |block: &Block| matches!(block.stmts.last(), Some(Stmt { kind: StmtKind::Expr(e), .. }) if self.undecided(e));
        let empty = |receiver: &Expr, name: &Ident| {
            matches!(
                (self.qualifier(receiver), name.name.as_str()),
                (Some("List" | "Map"), "empty") | (Some("Store"), "new")
            )
        };
        match &expr.kind {
            ExprKind::List(items) => items.is_empty(),
            ExprKind::Name(id) => id.name == "None" && !self.is_local(&id.name),
            ExprKind::Method {
                receiver,
                name,
                args,
            } => args.is_empty() && empty(receiver, name),
            ExprKind::Call { callee, args } => {
                args.is_empty()
                    && matches!(&callee.kind, ExprKind::Field { receiver, name } if empty(receiver, name))
            }
            ExprKind::Block(block) => tail(block),
            ExprKind::If { then, .. } => tail(then),
            _ => false,
        }
    }

    fn is_place(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Name(id) => self.is_local(&id.name),
            ExprKind::Field { receiver, .. } | ExprKind::TupleField { receiver, .. } => {
                self.assoc_callee(expr).is_none() && self.is_place(receiver)
            }
            _ => false,
        }
    }

    /// Whether the Rust value of the `Text` expression `expr` is a `String`
    /// of its own that nothing else reads: a literal, a concatenation or
    /// interpolation, or a builtin that builds a new text. `to_bytes` then
    /// consumes it instead of copying it.
    fn fresh_text(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Str(_) => true,
            ExprKind::Binary { op, .. } => self.is_text_add(expr, *op),
            _ => matches!(
                self.callee(expr),
                Some(Callee::Builtin(
                    Builtin::IntToText
                        | Builtin::IntToTextMethod
                        | Builtin::ByteToText
                        | Builtin::BoolToText
                        | Builtin::TextConcat
                        | Builtin::TextTrim
                ))
            ),
        }
    }

    fn block(&mut self, block: &Block, value: bool) -> String {
        self.scopes.push(Vec::new());
        self.indent += 1;
        let mut out = String::from("{\n");
        let count = block.stmts.len();
        for (index, stmt) in block.stmts.iter().enumerate() {
            let tail = value && index + 1 == count;
            let text = self.stmt(stmt, tail);
            out.push_str(&indentation(self.indent));
            out.push_str(&text);
            out.push('\n');
        }
        self.indent -= 1;
        out.push_str(&indentation(self.indent));
        out.push('}');
        self.scopes.pop();
        out
    }

    /// One statement; `tail` is set for the last statement of a
    /// value-position block.
    fn stmt(&mut self, stmt: &Stmt, tail: bool) -> String {
        match &stmt.kind {
            StmtKind::ForPattern {
                pattern,
                iter,
                body,
            } => {
                let iter = self.value(iter);
                self.scopes.push(Vec::new());
                let pattern = self.let_pattern(pattern, false);
                let body = self.block(body, false);
                self.scopes.pop();
                format!("for {pattern} in ({iter}) {body}")
            }
            StmtKind::LetPattern {
                pattern,
                value,
                mutable,
            } => {
                let value = self.value(value);
                let pattern = self.let_pattern(pattern, *mutable);
                format!("let {pattern} = {value};")
            }
            StmtKind::Let {
                name,
                ty,
                value,
                mutable,
            } => {
                let decided = match ty {
                    None if self.undecided(value) => self.type_of(value).cloned(),
                    _ => None,
                };
                let value = self.value(value);
                self.bind(&name.name);
                let ty = match (ty, decided) {
                    (Some(ty), _) => format!(": {}", self.annotation(ty)),
                    (None, Some(decided)) => format!(": {}", self.rust_ty(&decided)),
                    (None, None) => String::new(),
                };
                let mutability = if *mutable { "mut " } else { "" };
                format!("let {mutability}{}{ty} = {value};", ident(&name.name))
            }
            StmtKind::Assign { target, value } => {
                let mut place = ident(&target.base.name);
                for field in &target.fields {
                    place.push('.');
                    place.push_str(&ident(&field.name));
                }
                let value = self.value(value);
                let owner = self.types.and_then(|types| types.place_owner(target));
                match target.fields.last() {
                    Some(last) if self.is_boxed_field(owner, &last.name) => {
                        format!("{place} = Box::new({value});")
                    }
                    _ => format!("{place} = {value};"),
                }
            }
            StmtKind::Return(Some(value)) => format!("return {};", self.value(value)),
            StmtKind::Return(None) => "return;".to_string(),
            StmtKind::While { cond, body } => {
                let cond = self.condition(cond);
                format!("while {cond} {}", self.block(body, false))
            }
            StmtKind::Loop { body } => format!("loop {}", self.block(body, false)),
            StmtKind::For { name, iter, body } if self.borrowed_for(&name.name, iter, body) => {
                let iter = self.bare(iter);
                let scope = self.scopes.len();
                self.scopes.push(vec![name.name.clone()]);
                self.borrowed_elements.push((scope, name.name.clone()));
                let body = self.block(body, false);
                self.borrowed_elements.pop();
                self.scopes.pop();
                format!("for {} in ({iter}).iter() {body}", ident(&name.name))
            }
            StmtKind::For { name, iter, body } => {
                let iter = self.value(iter);
                self.scopes.push(vec![name.name.clone()]);
                let body = self.block(body, false);
                self.scopes.pop();
                format!("for {} in ({iter}) {body}", ident(&name.name))
            }
            StmtKind::Break => "break;".to_string(),
            StmtKind::Continue => "continue;".to_string(),
            StmtKind::Assert { cond, message } => self.assert(stmt, cond, message.as_ref()),
            StmtKind::Expr(expr) if tail => self.result(expr),
            StmtKind::Expr(expr) => match &expr.kind {
                ExprKind::If { .. } => format!("{};", self.if_expr(expr, false)),
                ExprKind::Block(block) => format!("{};", self.block(block, false)),
                _ => format!("{};", self.bare(expr)),
            },
        }
    }

    /// An `if` or `while` condition: bare, parenthesized when it is a record
    /// literal so Rust does not read its braces as the body.
    fn condition(&mut self, expr: &Expr) -> String {
        let text = self.bare(expr);
        if matches!(expr.kind, ExprKind::Record { .. }) {
            format!("({text})")
        } else {
            self.deref_if_borrowed(expr, text)
        }
    }

    /// Value position: a place is cloned.
    fn value(&mut self, expr: &Expr) -> String {
        self.expr(expr, true)
    }

    /// Non-value position: a place is emitted as written.
    fn bare(&mut self, expr: &Expr) -> String {
        self.expr(expr, false)
    }

    /// A method receiver: bare, parenthesized unless atomic.
    fn receiver(&mut self, expr: &Expr) -> String {
        let text = self.bare(expr);
        wrap(expr, text)
    }

    /// Whether `expr` is a place (`x[i]`, `x.f[i].g`, `x[i][j]`) rooted in
    /// a local and reached through at least one index.
    fn indexed_place(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Index { receiver, .. } => {
                self.is_place(receiver) || self.indexed_place(receiver)
            }
            ExprKind::Field { receiver, .. } | ExprKind::TupleField { receiver, .. } => {
                self.assoc_callee(expr).is_none() && self.indexed_place(receiver)
            }
            _ => false,
        }
    }

    /// An [`indexed_place`](Self::indexed_place) as the receiver of a
    /// method that changes it (`push`, `append`, `clear`, `put`, `remove`;
    /// `docs/slice.md` 13.11 row d): the element behind every index through
    /// `ignite_io::checked::at_mut` on a `&mut` borrow of its list, so the
    /// call writes the element and not a copy of it. Each index is bound
    /// by a `let` pushed to `lets`, in source order, before the borrow it
    /// feeds: an index that reads the list could not run while the list is
    /// borrowed.
    fn mutable_place(&mut self, expr: &Expr, lets: &mut Vec<String>) -> String {
        match &expr.kind {
            ExprKind::Index { receiver, index } => {
                let list = self.mutable_place(receiver, lets);
                let index_text = self.bare(index);
                let index_text = self.deref_if_borrowed(index, index_text);
                let temp = self.fresh();
                lets.push(format!("let {temp} = {index_text};"));
                match self.types.and_then(|t| t.failure_site(expr)) {
                    Some(site) => format!(
                        "(*ignite_io::checked::at_mut(&mut {list}, {temp}, {}))",
                        self.site_args(site)
                    ),
                    None => format!("{list}[({temp}) as usize]"),
                }
            }
            ExprKind::Field { receiver, name } => {
                let receiver_ty = self.type_of(receiver);
                let text = format!(
                    "{}.{}",
                    self.mutable_place(receiver, lets),
                    ident(&name.name)
                );
                if self.is_boxed_field(receiver_ty, &name.name) {
                    format!("(*{text})")
                } else {
                    text
                }
            }
            ExprKind::TupleField { receiver, index } => {
                format!("{}.{index}", self.mutable_place(receiver, lets))
            }
            _ => self.receiver(expr),
        }
    }

    fn args(&mut self, args: &[Expr]) -> Vec<String> {
        args.iter().map(|a| self.value(a)).collect()
    }

    fn expr(&mut self, expr: &Expr, clone: bool) -> String {
        if let Some(reference) = self.types.and_then(|t| t.reference(expr)) {
            return format!("String::from(\"{}\")", escape_str(&reference.text()));
        }
        if clone && self.is_place(expr) {
            if self.moves(expr) {
                return self.bare(expr);
            }
            return format!("{}.clone()", self.bare(expr));
        }
        if self.field_of_element(expr) {
            let place = self.element_field(expr);
            return format!("{place}{}", self.element_clone(expr));
        }
        match &expr.kind {
            ExprKind::Quantity { literal, unit } => self.quantity(expr, literal, unit),
            ExprKind::NamedArg { value, .. } => self.expr(value, clone),
            ExprKind::Brace { entries } => self.brace(expr, entries),
            ExprKind::Closure(closure) => self.closure(expr, closure),
            ExprKind::RecordUpdate { name, fields, base } => {
                self.record_update(expr, name, fields, base)
            }
            ExprKind::TupleField { receiver, index } => {
                format!("{}.{index}", self.receiver(receiver))
            }
            ExprKind::Break => "break".to_string(),
            ExprKind::Continue => "continue".to_string(),
            ExprKind::Tuple(items) => format!("({})", self.args(items).join(", ")),
            ExprKind::Return(Some(value)) => format!("return {}", self.value(value)),
            ExprKind::Return(None) => "return".to_string(),
            ExprKind::Int(value) if *value < 0 => format!("({value})"),
            ExprKind::Int(value) => value.to_string(),
            ExprKind::Str(text) => format!("String::from(\"{}\")", escape_str(text)),
            ExprKind::Bool(value) => value.to_string(),
            ExprKind::Name(id) => match self.typed_variant(expr, &id.name) {
                Some(path) => path,
                None if !self.is_local(&id.name)
                    && match self.types {
                        Some(_) => matches!(self.callee(expr), Some(Callee::Fn(_))),
                        None => {
                            self.symbols.functions.contains_key(&id.name)
                                && !self.constants.contains(&id.name)
                        }
                    } =>
                {
                    self.fn_value(expr, id)
                }
                None => self.name(id),
            },
            ExprKind::Record { name, fields } => self.record_literal(expr, name, fields),
            ExprKind::List(items) => format!("vec![{}]", self.args(items).join(", ")),
            ExprKind::Call { .. }
                if matches!(
                    self.callee(expr),
                    Some(Callee::Builtin(Builtin::ModuleRecord))
                ) =>
            {
                self.module_record(expr)
            }
            ExprKind::Call { callee, args } => match &callee.kind {
                ExprKind::Name(id) if !self.is_local(&id.name) => {
                    match self.typed_variant(expr, &id.name) {
                        Some(path) => {
                            format!("{path}({})", self.variant_args(expr, args).join(", "))
                        }
                        None if self.calls_fn(expr, id) => {
                            let target = match self.callee(expr) {
                                Some(Callee::Fn(q)) if !q.name.contains('.') => self.fn_path(q),
                                _ => self.name(id),
                            };
                            let target = format!("{target}{}", self.bounded_turbofish(expr));
                            self.fn_call(expr, target, None, args)
                        }
                        None => self.call(expr, callee, args),
                    }
                }
                _ => match self.assoc_callee(expr) {
                    Some(q) => self.assoc_call(expr, q, args),
                    None => self.call(expr, callee, args),
                },
            },
            ExprKind::Method {
                receiver,
                name,
                args,
            } => self.method(expr, receiver, name, args),
            ExprKind::Field { receiver, name } => {
                if let Some(q) = self.assoc_callee(expr) {
                    let target = format!("{}{}", self.fn_path(q), self.turbofish(expr));
                    return self.fn_value_of(expr, target, None);
                }
                if let Some(owner) = self.qualifier(receiver)
                    && self.is_enum(owner)
                {
                    return variant_path(&self.unshadowed(owner), &name.name);
                }
                let receiver_ty = self.type_of(receiver);
                let text = format!("{}.{}", self.receiver(receiver), ident(&name.name));
                if self.is_boxed_field(receiver_ty, &name.name) {
                    format!("(*{text})")
                } else {
                    text
                }
            }
            ExprKind::Index { receiver, index } => {
                if let Some(site) = self.types.and_then(|t| t.failure_site(expr)) {
                    return self.checked_at(expr, receiver, index, site);
                }
                let receiver = self.receiver(receiver);
                let index_text = self.bare(index);
                let index_text = self.deref_if_borrowed(index, index_text);
                format!(
                    "{receiver}[({index_text}) as usize]{}",
                    self.element_clone(expr)
                )
            }
            ExprKind::Try(inner) if self.diverges(inner) => self.value(inner),
            ExprKind::Try(inner) if self.types.and_then(|t| t.test_try(expr)).is_some() => {
                let test_try = self.types.and_then(|t| t.test_try(expr)).expect("checked");
                self.test_try(expr, inner, test_try)
            }
            ExprKind::Try(inner) => format!("({})?", self.value(inner)),
            ExprKind::Await(inner) => format!("({})", self.value(inner)),
            ExprKind::Spawn(operand) => self.spawn(operand),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            } if self.types.and_then(|t| t.failure_site(expr)).is_some() => {
                let site = self
                    .types
                    .and_then(|t| t.failure_site(expr))
                    .expect("a site");
                let text = self.value(operand);
                let text = self.deref_if_borrowed(operand, text);
                format!("ignite_io::checked::neg({text}, {})", self.site_args(site))
            }
            ExprKind::Unary { op, operand } => {
                let symbol = match op {
                    UnaryOp::Not => "!",
                    UnaryOp::Neg => "-",
                };
                let text = self.value(operand);
                format!("{symbol}{}", wrap(operand, text))
            }
            ExprKind::Binary { op, left, right } if self.is_text_add(expr, *op) => {
                let mut parts = vec![&**right];
                let mut head = &**left;
                while let ExprKind::Binary {
                    op: BinaryOp::Add,
                    left,
                    right,
                } = &head.kind
                    && self.is_text_add(head, BinaryOp::Add)
                {
                    parts.push(right);
                    head = left;
                }
                parts.push(head);
                parts.reverse();
                if parts.len() > CHAIN_LIMIT {
                    let rights: Vec<(BinaryOp, &Expr, &Expr)> =
                        parts[1..].iter().map(|p| (BinaryOp::Add, *p, *p)).collect();
                    return self.chunked(parts[0], &rights, true);
                }
                let mut later = BTreeSet::new();
                let mut modified = vec![BTreeSet::new(); parts.len()];
                for (index, part) in parts.iter().enumerate().rev() {
                    modified[index] = later.clone();
                    mutated_in(part, self.types, &mut later);
                }
                let mut template = String::new();
                let mut texts = Vec::new();
                for (part, modified) in parts.into_iter().zip(&modified) {
                    self.format_part(part, modified, &mut template, &mut texts);
                }
                if texts.is_empty() {
                    return format!("format!(\"{template}\")");
                }
                format!("format!(\"{template}\", {})", texts.join(", "))
            }
            ExprKind::Binary {
                op: op @ (BinaryOp::Eq | BinaryOp::Ne),
                left,
                right,
            } if self.structural_operand(left, right).is_some() => {
                self.structural_eq(*op, left, right)
            }
            ExprKind::Binary { op, left, right } => {
                if let Some(chunked) = self.chunked_spine(expr) {
                    return chunked;
                }
                let (left_text, right_text) = if is_comparison(*op) {
                    (self.compared(left, Some(right)), self.compared(right, None))
                } else {
                    (self.operand(left), self.operand(right))
                };
                match self.checked_op(expr, *op) {
                    Some((checked, site)) => {
                        format!("{checked}({left_text}, {right_text}, {site})")
                    }
                    None => format!("{left_text} {} {right_text}", binary_symbol(*op)),
                }
            }
            ExprKind::Match { scrutinee, arms } => self.match_expr(expr, scrutinee, arms),
            ExprKind::If { .. } => self.if_expr(expr, true),
            ExprKind::Block(block) => self.block(block, true),
        }
    }

    /// `.clone()` after an element read (`l[i]`, `l.at(i)`) whose type is
    /// not `Copy` in Rust: Rust cannot move out of a `Vec`. Nothing for
    /// `Int`, `Byte`, `Bool`, `Unit` and when types are not known.
    fn element_clone(&self, expr: &Expr) -> &'static str {
        match self.type_of(expr) {
            None | Some(Ty::Int | Ty::Byte | Ty::Bool | Ty::Unit) => "",
            Some(_) => ".clone()",
        }
    }

    /// Whether `expr` (with operator `op`) is `+` on `Text`.
    fn is_text_add(&self, expr: &Expr, op: BinaryOp) -> bool {
        op == BinaryOp::Add && self.type_of(expr) == Some(&Ty::Text)
    }

    /// One part of a Text `+` chain in its `format!`: a literal inlined
    /// into `template` with its braces doubled, anything else as `{}` and
    /// an argument. A `to_text` of Text, Int, Byte or Bool (each `{x}` of
    /// an interpolation) passes its receiver, whose `Display` is that
    /// text, so the part is not copied or converted first. `format!`
    /// borrows every argument until the last part is evaluated, so a part
    /// that is a place (or the `to_text` of one) rooted at a variable a
    /// later part assigns or changes (`modified`) is copied, as it is
    /// written: `x.clone()`, `n.to_string()`.
    fn format_part(
        &mut self,
        part: &Expr,
        modified: &BTreeSet<String>,
        template: &mut String,
        args: &mut Vec<String>,
    ) {
        if self.place_in(part, modified) {
            template.push_str("{}");
            args.push(self.value(part));
            return;
        }
        if self.types.and_then(|t| t.reference(part)).is_none() {
            if let ExprKind::Str(text) = &part.kind {
                template.push_str(&escape_str(text).replace('{', "{{").replace('}', "}}"));
                return;
            }
            if let ExprKind::Method {
                receiver,
                args: none,
                ..
            } = &part.kind
                && none.is_empty()
                && matches!(
                    self.callee(part),
                    Some(Callee::Builtin(
                        Builtin::TextToText
                            | Builtin::IntToTextMethod
                            | Builtin::ByteToText
                            | Builtin::BoolToText
                    ))
                )
                && !self.place_in(receiver, modified)
            {
                template.push_str("{}");
                args.push(self.bare(receiver));
                return;
            }
        }
        template.push_str("{}");
        args.push(self.bare(part));
    }

    /// `expr` is a place rooted at one of `names`.
    fn place_in(&self, expr: &Expr, names: &BTreeSet<String>) -> bool {
        self.is_place(expr) && place_root(expr).is_some_and(|root| names.contains(root))
    }

    /// The Rust path of the variant a bare name resolves to by expected
    /// type (`docs/slice.md` section 10.1), when types are known.
    fn typed_variant(&self, expr: &Expr, name: &str) -> Option<String> {
        if self.is_local(name) {
            return None;
        }
        match self.callee(expr)? {
            Callee::Variant { enum_, .. } => Some(variant_path(&self.enum_name(enum_), name)),
            _ => None,
        }
    }

    fn operand(&mut self, expr: &Expr) -> String {
        let text = self.bare(expr);
        match expr.kind {
            ExprKind::Binary { .. }
            | ExprKind::If { .. }
            | ExprKind::Match { .. }
            | ExprKind::Block(_)
            | ExprKind::Record { .. } => format!("({text})"),
            _ => self.deref_if_borrowed(expr, text),
        }
    }

    /// The Rust name of local `name`.
    fn local_name(&mut self, name: &str) -> String {
        if self.is_outermost(name)
            && let Some(rename) = self.renames.get(name)
        {
            self.renamed_uses.insert(name.to_string());
            return rename.clone();
        }
        ident(name)
    }

    fn name(&mut self, id: &Ident) -> String {
        if self.is_local(&id.name) {
            return self.local_name(&id.name);
        }
        if id.name == "Unit" {
            return "()".to_string();
        }
        if self.constants.contains(&id.name) {
            return format!("{}()", ident(&id.name));
        }
        if let Some(owner) = self.enum_of_variant(&id.name) {
            return variant_path(&self.unshadowed(&owner), &id.name);
        }
        ident(&id.name)
    }

    /// A record literal; an omitted field calls the record's default fn,
    /// with the literal's type arguments for a generic record (Rust cannot
    /// infer them from a default that does not mention them).
    fn record_literal(&mut self, expr: &Expr, name: &Ident, fields: &[FieldInit]) -> String {
        let record = self.unshadowed(&name.name);
        let ty = self.type_of(expr);
        let boxed = |this: &Self, field: &str, value: String| {
            if this.is_boxed_field(ty, field) {
                format!("Box::new({value})")
            } else {
                value
            }
        };
        let mut texts: Vec<String> = fields
            .iter()
            .map(|f| {
                let value = self.value(&f.value);
                format!(
                    "{}: {}",
                    ident(&f.name.name),
                    boxed(self, &f.name.name, value)
                )
            })
            .collect();
        if let Some(declared) = self.symbols.record_fields.get(&name.name) {
            let instance = self.turbofish(expr);
            for field in declared {
                if !fields.iter().any(|f| f.name.name == *field) {
                    let value = format!("{record}{instance}::{}()", default_fn(field));
                    texts.push(format!("{}: {}", ident(field), boxed(self, field, value)));
                }
            }
        }
        if texts.is_empty() {
            return format!("{record} {{}}");
        }
        format!("{record} {{ {} }}", texts.join(", "))
    }

    /// A quantity literal (`docs/slice.md` section 14.1): its value in base
    /// units as an `i64`, from the typer, else from the literal and the
    /// unit table.
    fn quantity(&self, expr: &Expr, literal: &str, unit: &Ident) -> String {
        let value = match self.types.and_then(|t| t.quantity(expr)) {
            Some((_, value)) => value,
            None => quantity_value(literal, &unit.name).unwrap_or(0),
        };
        if value < 0 {
            format!("({value}i64)")
        } else {
            format!("{value}i64")
        }
    }

    /// A brace literal (`docs/slice.md` section 14.3): a record literal of
    /// the expected record type, or a `BTreeMap` of its entries with
    /// `Text` keys for identifier keys and reference texts for qualified
    /// keys.
    fn brace(&mut self, expr: &Expr, entries: &[ignite_syntax::ast::BraceEntry]) -> String {
        let kind = self.types.and_then(|t| t.brace_kind(expr)).cloned();
        match kind {
            Some(BraceKind::Record(ty)) => {
                let fields: Vec<(String, &Expr)> = entries
                    .iter()
                    .map(|entry| (entry.key.last().name.clone(), &entry.value))
                    .collect();
                self.typed_record_literal(&ty, &fields, &BTreeMap::new())
            }
            Some(BraceKind::Map(..)) => {
                if entries.is_empty() {
                    return "std::collections::BTreeMap::new()".to_string();
                }
                let pairs: Vec<String> = entries
                    .iter()
                    .map(|entry| {
                        let key = match self.types.and_then(|t| t.brace_key(&entry.key)) {
                            Some(reference) => reference.text(),
                            None => entry.key.last().name.clone(),
                        };
                        let value = self.value(&entry.value);
                        format!("(String::from(\"{}\"), {value})", escape_str(&key))
                    })
                    .collect();
                format!("std::collections::BTreeMap::from([{}])", pairs.join(", "))
            }
            None => {
                let fields: Vec<String> = entries
                    .iter()
                    .map(|entry| {
                        let value = self.value(&entry.value);
                        format!("{}: {value}", ident(&entry.key.last().name))
                    })
                    .collect();
                format!("{{ {} }}", fields.join(", "))
            }
        }
    }

    /// A literal of record type `ty` with the given fields (name and
    /// value expression) in source order, then the record's defaults for
    /// the omitted fields; `fixed` holds Rust texts that replace a field's
    /// value or default (a component's `executable`, a resource handle).
    fn typed_record_literal(
        &mut self,
        ty: &Ty,
        fields: &[(String, &Expr)],
        fixed: &BTreeMap<String, String>,
    ) -> String {
        let path = self.type_path(ty);
        let declared = self.fields_of(ty).unwrap_or_default();
        let mut texts: Vec<String> = Vec::new();
        for (name, value) in fields {
            let text = match fixed.get(name) {
                Some(text) => text.clone(),
                None => self.value(value),
            };
            let boxed = declared
                .iter()
                .any(|(field, _, boxed)| field == name && *boxed);
            let text = if boxed {
                format!("Box::new({text})")
            } else {
                text
            };
            texts.push(format!("{}: {text}", ident(name)));
        }
        for (name, _, boxed) in &declared {
            if fields.iter().any(|(field, _)| field == name) {
                continue;
            }
            let text = match fixed.get(name) {
                Some(text) => text.clone(),
                None => format!("{path}::{}()", default_fn(name)),
            };
            let text = if *boxed {
                format!("Box::new({text})")
            } else {
                text
            };
            texts.push(format!("{}: {text}", ident(name)));
        }
        if texts.is_empty() {
            return format!("{path} {{}}");
        }
        format!("{path} {{ {} }}", texts.join(", "))
    }

    /// `module("p")` (`docs/slice.md` section 14.4): the record of the
    /// module's fns as fn values, in `Some(..)` when an `Option` of the
    /// record is expected.
    fn module_record(&mut self, expr: &Expr) -> String {
        let Some(record) = self.types.and_then(|t| t.module_record(expr)).cloned() else {
            return format!("{}()", ident("module"));
        };
        let declared = self.fields_of(&record.record).unwrap_or_default();
        let path = self.type_path(&record.record);
        let fields: Vec<String> = record
            .fns
            .iter()
            .map(|(field, q)| {
                let target = self.module_fn_path(q);
                let value = match declared.iter().find(|(name, _, _)| name == field) {
                    Some((_, ty, _)) => {
                        self.fn_value_typed(target, ty, Some(&Callee::Fn(q.clone())))
                    }
                    None => target,
                };
                format!("{}: {value}", ident(field))
            })
            .collect();
        let literal = if fields.is_empty() {
            format!("{path} {{}}")
        } else {
            format!("{path} {{ {} }}", fields.join(", "))
        };
        match self.type_of(expr) {
            Some(Ty::Option(_)) => format!("Some({literal})"),
            _ => literal,
        }
    }

    /// The Rust fn `target` as a fn value of fn type `ty`
    /// ([`Generator::fn_value_of`] with the type given).
    fn fn_value_typed(&mut self, target: String, ty: &Ty, callee: Option<&Callee>) -> String {
        let (params, ret) = match ty {
            Ty::Fn(params, ret) | Ty::EffectFn(params, ret, _) => (params, &**ret),
            _ => return target,
        };
        let names: Vec<String> = (0..params.len()).map(|i| format!("__a{i}")).collect();
        let refs = self.wrapper_args(callee, &names);
        let typed: Vec<String> = names
            .iter()
            .zip(params)
            .map(|(n, t)| format!("{n}: {}", self.rust_ty(t)))
            .collect();
        let types: Vec<String> = params.iter().map(|t| self.rust_ty(t)).collect();
        format!(
            "(std::sync::Arc::new(|{}| {target}({})) as {})",
            typed.join(", "),
            refs.join(", "),
            fn_value_type(&types, &self.rust_ty(ret))
        )
    }

    /// A call of the Ignite fn `target` with `args` by reference, by value
    /// to a parameter the callee consumes, the receiver of a UFCS call
    /// first. Named arguments (`docs/slice.md`
    /// section 14.2) are reordered to the parameters; when the order
    /// changes, every argument is evaluated into a temporary first, in
    /// source order.
    fn fn_call(
        &mut self,
        call: &Expr,
        target: String,
        receiver: Option<&Expr>,
        args: &[Expr],
    ) -> String {
        let order: Option<Vec<usize>> = self
            .types
            .and_then(|t| t.named_args(call))
            .filter(|order| order.iter().enumerate().any(|(i, j)| i != *j))
            .map(<[usize]>::to_vec);
        let callee = self.callee(call);
        let Some(order) = order else {
            let texts = self.ref_args(callee, receiver.into_iter().chain(args));
            return format!("{target}({})", texts.join(", "));
        };
        let borrow = |this: &Self, param: usize| {
            if this.consumes(callee, param) {
                ""
            } else {
                "&"
            }
        };
        let mut lets = String::new();
        let mut texts = Vec::new();
        if let Some(receiver) = receiver {
            let value = self.value(receiver);
            let _ = write!(lets, "let __ignite_r = {value}; ");
            texts.push(format!("{}__ignite_r", borrow(self, 0)));
        }
        for (index, arg) in args.iter().enumerate() {
            let value = self.value(arg);
            let _ = write!(lets, "let __ignite_a{index} = {value}; ");
        }
        let first = usize::from(receiver.is_some());
        for (param, index) in order.iter().enumerate() {
            texts.push(format!("{}__ignite_a{index}", borrow(self, first + param)));
        }
        format!("{{ {lets}{target}({}) }}", texts.join(", "))
    }

    /// `list[i]` and `list.at(i)` with a failure site: the element through
    /// `ignite_io::checked::at`, copied or cloned.
    fn checked_at(
        &mut self,
        expr: &Expr,
        receiver: &Expr,
        index: &Expr,
        site: FailureSite,
    ) -> String {
        let call = self.checked_at_call(receiver, index, site);
        match self.element_clone(expr) {
            "" => format!("(*{call})"),
            clone => format!("{call}{clone}"),
        }
    }

    /// `ignite_io::checked::at(list, index, site)`: a `&T` into the list.
    fn checked_at_call(&mut self, receiver: &Expr, index: &Expr, site: FailureSite) -> String {
        let list = self.ref_arg(receiver);
        let index_text = self.value(index);
        let index_text = self.deref_if_borrowed(index, index_text);
        format!(
            "ignite_io::checked::at({list}, {index_text}, {})",
            self.site_args(site)
        )
    }

    /// The list and the index of `expr` when it reads an element through
    /// `ignite_io::checked::at` (`l[i]`, `l.at(i)` with a failure site)
    /// from a list that can be borrowed: a place, an element read or a
    /// [`field_of_element`](Self::field_of_element).
    fn element_read<'e>(&self, expr: &'e Expr) -> Option<(&'e Expr, &'e Expr)> {
        let (list, index) = match &expr.kind {
            ExprKind::Index { receiver, index } => (&**receiver, &**index),
            ExprKind::Method { receiver, args, .. }
                if args.len() == 1
                    && matches!(self.callee(expr), Some(Callee::Builtin(Builtin::ListAt))) =>
            {
                (&**receiver, &args[0])
            }
            _ => return None,
        };
        self.types.and_then(|t| t.failure_site(expr))?;
        (self.is_place(list) || self.element_read(list).is_some() || self.field_of_element(list))
            .then_some((list, index))
    }

    /// Whether `expr` is a field (or tuple field) of an
    /// [`element_read`](Self::element_read), directly or through further
    /// fields (`l[i].f`, `l.at(i).f.g`, `l[i].f[j].g`): a place inside the
    /// list, read through the `&T` that `checked::at` returns instead of a
    /// copy of the whole element.
    fn field_of_element(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Field { receiver, .. } | ExprKind::TupleField { receiver, .. } => {
                self.assoc_callee(expr).is_none()
                    && (self.element_read(receiver).is_some() || self.field_of_element(receiver))
            }
            _ => false,
        }
    }

    /// The local an [`element_read`](Self::element_read) or a
    /// [`field_of_element`](Self::field_of_element) borrows from.
    fn element_root<'e>(&self, expr: &'e Expr) -> Option<&'e str> {
        if self.is_place(expr) {
            return place_root(expr);
        }
        match &expr.kind {
            ExprKind::Field { receiver, .. }
            | ExprKind::TupleField { receiver, .. }
            | ExprKind::Index { receiver, .. }
            | ExprKind::Method { receiver, .. } => self.element_root(receiver),
            _ => None,
        }
    }

    /// An [`element_read`](Self::element_read) as a `&T` into its list:
    /// `ignite_io::checked::at(list, index, site)`, the list borrowed, not
    /// copied.
    fn element_at(&mut self, expr: &Expr) -> String {
        let (list, index) = self.element_read(expr).expect("an element read");
        let site = self
            .types
            .and_then(|t| t.failure_site(expr))
            .expect("a site");
        let list_text = if self.is_place(list) {
            self.ref_arg(list)
        } else if self.element_read(list).is_some() {
            self.element_at(list)
        } else {
            format!("&{}", self.element_field(list))
        };
        let index_text = self.value(index);
        let index_text = self.deref_if_borrowed(index, index_text);
        format!(
            "ignite_io::checked::at({list_text}, {index_text}, {})",
            self.site_args(site)
        )
    }

    /// A [`field_of_element`](Self::field_of_element) as a Rust place
    /// (`ignite_io::checked::at(&l, 0, ..).key`): reading it, borrowing
    /// it or cloning it copies the field, never the element.
    fn element_field(&mut self, expr: &Expr) -> String {
        let (receiver, member) = match &expr.kind {
            ExprKind::Field { receiver, name } => (&**receiver, ident(&name.name)),
            ExprKind::TupleField { receiver, index } => (&**receiver, index.to_string()),
            _ => unreachable!("a field of an element"),
        };
        let base = if self.element_read(receiver).is_some() {
            self.element_at(receiver)
        } else {
            self.element_field(receiver)
        };
        let text = format!("{base}.{member}");
        match &expr.kind {
            ExprKind::Field { name, .. }
                if self.is_boxed_field(self.type_of(receiver), &name.name) =>
            {
                format!("(*{text})")
            }
            _ => text,
        }
    }

    /// An operand of a comparison: a
    /// [`field_of_element`](Self::field_of_element) whose type is not
    /// `Copy` compared in place (`==` and `<` take their operands by
    /// reference) unless `later`, the operand evaluated after it while it
    /// is borrowed, changes the list; anything else as
    /// [`operand`](Self::operand) gives it.
    fn compared(&mut self, expr: &Expr, later: Option<&Expr>) -> String {
        if self.field_of_element(expr) && !self.element_clone(expr).is_empty() {
            let mut changed = BTreeSet::new();
            if let Some(later) = later {
                mutated_in(later, self.types, &mut changed);
            }
            if self
                .element_root(expr)
                .is_none_or(|root| !changed.contains(root))
            {
                return self.element_field(expr);
            }
        }
        self.operand(expr)
    }

    fn call(&mut self, expr: &Expr, callee: &Expr, args: &[Expr]) -> String {
        match &callee.kind {
            ExprKind::Name(id) if !self.is_local(&id.name) => {
                if id.name == "log" && args.len() == 1 {
                    let arg = self.value(&args[0]);
                    return format!("ignite_io::log(&({arg}))");
                }
                let target = format!("{}{}", self.name(id), self.bounded_turbofish(expr));
                format!("{target}({})", self.args(args).join(", "))
            }
            ExprKind::Field { receiver, name } => {
                if let Some(owner) = self.qualifier(receiver)
                    && let Some(text) = self.qualified_call(expr, owner, name, args)
                {
                    return text;
                }
                let target = self.bare(callee);
                format!("({target})({})", self.args(args).join(", "))
            }
            _ => {
                let target = self.bare(callee);
                let target = wrap(callee, target);
                format!("{target}({})", self.args(args).join(", "))
            }
        }
    }

    /// `spawn f(a, b)`: the arguments are bound to `__spawn_N` (value
    /// position, so places are cloned) before the task starts; the task
    /// logs an `Err` result.
    fn spawn(&mut self, operand: &Expr) -> String {
        let instance = self.bounded_turbofish(operand);
        let (target, args, by_ref): (Option<String>, Vec<&Expr>, bool) = match &operand.kind {
            ExprKind::Call { callee, args } => match &callee.kind {
                ExprKind::Name(id) if !self.is_local(&id.name) => {
                    (Some(self.name(id)), args.iter().collect(), true)
                }
                ExprKind::Field { receiver, name } => {
                    (self.role_path(receiver, name), args.iter().collect(), false)
                }
                _ => (None, Vec::new(), false),
            },
            ExprKind::Method {
                receiver,
                name,
                args,
            } => match self.callee(operand) {
                Some(Callee::Fn(q)) if q.name.contains('.') => {
                    (Some(self.fn_path(q)), args.iter().collect(), true)
                }
                Some(Callee::Fn(q)) => (
                    Some(self.fn_path(q)),
                    std::iter::once(&**receiver).chain(args).collect(),
                    true,
                ),
                _ => (self.role_path(receiver, name), args.iter().collect(), false),
            },
            _ => (None, Vec::new(), false),
        };
        const LOG: &str = "ignite_io::log(&format!(\"task ended with error: {}\", error));";
        let Some(target) = target.map(|target| format!("{target}{instance}")) else {
            let task = self.value(operand);
            return format!(
                "{{ ignite_io::spawn(move || {{ if let Err(error) = ({task}) {{ {LOG} }} }}); }}"
            );
        };
        let callee = self.callee(operand);
        let mut out = String::from("{ ");
        let mut names = Vec::new();
        for (index, arg) in args.iter().enumerate() {
            let value = self.value(arg);
            let _ = write!(out, "let __spawn_{index} = {value}; ");
            let borrow = if by_ref && !self.consumes(callee, index) {
                "&"
            } else {
                ""
            };
            names.push(format!("{borrow}__spawn_{index}"));
        }
        let _ = write!(
            out,
            "ignite_io::spawn(move || {{ if let Err(error) = {target}({}) {{ {LOG} }} }}); }}",
            names.join(", ")
        );
        out
    }

    /// `P::name` for `P.name` where `P` is a protocol or another non-local
    /// uppercase qualifier (a user fn cannot be qualified).
    fn role_path(&self, receiver: &Expr, name: &Ident) -> Option<String> {
        let owner = self.qualifier(receiver)?;
        if SUBSTRATE_OWNERS.contains(&owner) {
            return Some(format!("ignite_io::{owner}::{}", ident(&name.name)));
        }
        Some(format!("{}::{}", self.unshadowed(owner), ident(&name.name)))
    }

    /// `Owner.name(args)` where `Owner` is a substrate type, a builtin
    /// static owner, a protocol or an enum.
    fn qualified_call(
        &mut self,
        expr: &Expr,
        owner: &str,
        name: &Ident,
        args: &[Expr],
    ) -> Option<String> {
        if SUBSTRATE_OWNERS.contains(&owner)
            || BUILTIN_STATICS
                .iter()
                .any(|(o, n)| *o == owner && *n == name.name)
        {
            return Some(self.static_call(owner, &name.name, args));
        }
        if self.protocols.contains(owner) {
            let texts = self.args(args);
            return Some(format!(
                "{}::{}{}({})",
                self.unshadowed(owner),
                ident(&name.name),
                self.bounded_turbofish(expr),
                texts.join(", ")
            ));
        }
        if self.is_enum(owner) {
            let path = variant_path(&self.unshadowed(owner), &name.name);
            return Some(format!(
                "{path}({})",
                self.variant_args(expr, args).join(", ")
            ));
        }
        None
    }

    fn static_call(&mut self, owner: &str, name: &str, args: &[Expr]) -> String {
        if let ("Int", "parse", [text]) = (owner, name, args) {
            return format!("ignite_io::parse_int({})", self.text_bytes(text));
        }
        let texts = self.args(args);
        match (owner, name, texts.as_slice()) {
            ("Bytes", "empty", []) => "Vec::<u8>::new()".to_string(),
            ("Bytes", "of", [_]) if matches!(&args[0].kind, ExprKind::List(items) if items.is_empty()) => {
                "Vec::<u8>::new()".to_string()
            }
            ("Bytes", "of", [list]) => {
                format!("({list}).iter().map(|x| *x as u8).collect::<Vec<u8>>()")
            }
            ("List", "empty", []) => "Vec::new()".to_string(),
            ("Map", "empty", []) => "std::collections::BTreeMap::new()".to_string(),
            ("Store", "new", []) => "ignite_io::Store::new()".to_string(),
            ("Pipe", "pair", []) => "ignite_io::Pipe::pair()".to_string(),
            ("Directory", "temporary", []) => "ignite_io::Directory::temporary()".to_string(),
            ("Launcher", "new", []) => "ignite_io::Launcher::new()".to_string(),
            ("Entropy", "new", []) => "ignite_io::Entropy::new()".to_string(),
            ("Setting", "of", [flag, value]) => format!("ignite_io::Setting::of({flag}, {value})"),
            ("Int", "to_text", [value]) => {
                format!("{}.to_string()", wrap(&args[0], value.clone()))
            }
            ("Process", "exit", [code]) => format!("ignite_io::exit(({code}) as i32)"),
            ("Clock", "monotonic", []) => "ignite_io::clock::monotonic()".to_string(),
            ("Endpoint", "of", [reference]) => {
                let endpoint = self.enum_name(&QualName::new("std.net", "Endpoint"));
                format!(
                    "ignite_io::endpoint_of({}).map(|(address, port)| {endpoint} {{ address, port }})",
                    borrow(&args[0], reference.clone())
                )
            }
            (owner, _, _) if SUBSTRATE_OWNERS.contains(&owner) => {
                format!("ignite_io::{owner}::{name}({})", texts.join(", "))
            }
            _ => format!("{owner}::{name}({})", texts.join(", ")),
        }
    }

    /// The bytes of a `Text` argument that is only read, for
    /// `ignite_io::parse_int`: `bytes.to_text()` reads `bytes` itself, with
    /// no `Text` built, and a checked `slice` borrows
    /// ([`checked_slice_ref`](Self::checked_slice_ref)).
    fn text_bytes(&mut self, text: &Expr) -> String {
        if let ExprKind::Method { receiver, .. } = &text.kind
            && matches!(
                self.callee(text),
                Some(Callee::Builtin(Builtin::BytesToText))
            )
        {
            return self
                .checked_slice_ref(receiver)
                .unwrap_or_else(|| self.ref_arg(receiver));
        }
        self.ref_arg(text)
    }

    /// `list.slice(from, to)` with a failure site, as a borrow of `list`
    /// (`ignite_io::checked::slice_ref`) where the caller only reads it, a
    /// `slice` receiver borrowed the same way; `None` for any other
    /// expression and for bounds that modify a variable, which the borrow
    /// of `list` would span.
    fn checked_slice_ref(&mut self, expr: &Expr) -> Option<String> {
        let ExprKind::Method { receiver, args, .. } = &expr.kind else {
            return None;
        };
        if !matches!(self.callee(expr), Some(Callee::Builtin(Builtin::ListSlice))) {
            return None;
        }
        let mut modified = BTreeSet::new();
        for arg in args {
            mutated_in(arg, self.types, &mut modified);
        }
        if !modified.is_empty() {
            return None;
        }
        let site = self.types.and_then(|t| t.failure_site(expr))?;
        let list = self
            .checked_slice_ref(receiver)
            .unwrap_or_else(|| self.ref_arg(receiver));
        let a = self.args(args);
        Some(format!(
            "ignite_io::checked::slice_ref({list}, {}, {}, {})",
            a[0],
            a[1],
            self.site_args(site)
        ))
    }

    /// A call that changes an indexed receiver in place
    /// (`lists[i].push(v)`, `holders[i].items.append(vs)`,
    /// `maps[i].put(k, v)`): a block binding the indexes and the arguments
    /// first, then the call on the [`mutable_place`](Self::mutable_place).
    fn indexed_mutation(
        &mut self,
        expr: &Expr,
        receiver: &Expr,
        name: &Ident,
        args: &[Expr],
    ) -> String {
        let mut lets = Vec::new();
        let place = self.mutable_place(receiver, &mut lets);
        let values = self.args(args);
        let temps: Vec<String> = values
            .iter()
            .map(|value| {
                let temp = self.fresh();
                lets.push(format!("let {temp} = {value};"));
                temp
            })
            .collect();
        let call = match self.callee(expr) {
            Some(Callee::Builtin(Builtin::MapPut)) => {
                format!("{place}.insert({}, {});", temps[0], temps[1])
            }
            Some(Callee::Builtin(Builtin::MapRemove)) => {
                let call = format!("{place}.remove(&{})", temps[0]);
                match self.type_of(expr) {
                    Some(Ty::Unit) => format!("{call};"),
                    _ => call,
                }
            }
            _ => match name.name.as_str() {
                "push" => format!("{place}.push({})", temps[0]),
                "append" => format!("{place}.extend_from_slice(&{})", temps[0]),
                _ => format!("{place}.clear()"),
            },
        };
        format!("{{ {} {call} }}", lets.join(" "))
    }

    fn method(&mut self, expr: &Expr, receiver: &Expr, name: &Ident, args: &[Expr]) -> String {
        match self.callee(expr) {
            Some(Callee::Fn(q)) if q.name.contains('.') => {
                return self.assoc_call(expr, q, args);
            }
            Some(Callee::Fn(q)) => {
                let target = format!("{}{}", self.fn_path(q), self.bounded_turbofish(expr));
                return self.fn_call(expr, target, Some(receiver), args);
            }
            Some(Callee::Builtin(
                Builtin::PipeStreamRead
                | Builtin::PipeStreamWriteAll
                | Builtin::PipeStreamClose
                | Builtin::PipeStreamSetIdleTimeout
                | Builtin::PipeStreamReceivedBatches,
            )) => {
                let r = self.receiver(receiver);
                let a = self.args(args);
                return format!("{r}.{}({})", ident(&name.name), a.join(", "));
            }
            Some(Callee::Builtin(
                b @ (Builtin::LauncherExecute | Builtin::LauncherStart | Builtin::ChildProcessWait),
            )) => {
                let r = self.receiver(receiver);
                let a = self.args(args);
                let call = match b {
                    Builtin::ChildProcessWait => format!("{r}.wait()"),
                    _ => format!("{r}.{}({})", ident(&name.name), substrate_invocation(&a[0])),
                };
                if *b == Builtin::LauncherStart {
                    return call;
                }
                let exit = self.enum_name(&QualName::new("std.process", "ChildExit"));
                return format!("{call}.map(|e| {})", record_from_substrate(&exit, "e"));
            }
            Some(Callee::Builtin(
                Builtin::StreamRead
                | Builtin::StreamWriteAll
                | Builtin::StreamClose
                | Builtin::StreamSetIdleTimeout,
            )) => {
                let stream = self
                    .type_of(receiver)
                    .map(|ty| self.rust_ty(ty))
                    .expect("a bounded receiver has a type");
                let r = self.receiver(receiver);
                let a = self.args(args);
                let a: Vec<String> = std::iter::once(format!("&{r}")).chain(a).collect();
                return format!(
                    "<{stream} as ignite_io::ByteStream>::{}({})",
                    ident(&name.name),
                    a.join(", ")
                );
            }
            _ if mutates_receiver(expr, &name.name, self.types) && self.indexed_place(receiver) => {
                return self.indexed_mutation(expr, receiver, name, args);
            }
            Some(Callee::Builtin(builtin @ (Builtin::ListAt | Builtin::ListSlice)))
                if self.types.and_then(|t| t.failure_site(expr)).is_some() =>
            {
                let site = self
                    .types
                    .and_then(|t| t.failure_site(expr))
                    .expect("a site");
                if *builtin == Builtin::ListAt {
                    return self.checked_at(expr, receiver, &args[0], site);
                }
                let mut modified = BTreeSet::new();
                for arg in args {
                    mutated_in(arg, self.types, &mut modified);
                }
                let list = if self.is_place(receiver)
                    && place_root(receiver).is_some_and(|root| modified.contains(root))
                {
                    format!("&{}", self.value(receiver))
                } else {
                    self.ref_arg(receiver)
                };
                let a = self.args(args);
                return format!(
                    "ignite_io::checked::slice({list}, {}, {}, {})",
                    a[0],
                    a[1],
                    self.site_args(site)
                );
            }
            Some(Callee::Builtin(Builtin::MapGet)) => {
                let r = self.receiver(receiver);
                let a = self.args(args);
                return format!("{r}.get({}).cloned()", borrow(&args[0], a[0].clone()));
            }
            Some(Callee::Builtin(Builtin::MapPut)) => {
                let r = self.receiver(receiver);
                let a = self.args(args);
                return format!("{{ {r}.insert({}, {}); }}", a[0], a[1]);
            }
            Some(Callee::Builtin(Builtin::MapRemove)) => {
                let r = self.receiver(receiver);
                let a = self.args(args);
                let call = format!("{r}.remove({})", borrow(&args[0], a[0].clone()));
                return match self.type_of(expr) {
                    Some(Ty::Unit) => format!("{{ {call}; }}"),
                    _ => call,
                };
            }
            Some(Callee::Builtin(Builtin::MapContains)) => {
                let r = self.receiver(receiver);
                let a = self.args(args);
                return format!("{r}.contains_key({})", borrow(&args[0], a[0].clone()));
            }
            Some(Callee::Builtin(Builtin::MapKeys)) => {
                let r = self.receiver(receiver);
                return format!("{r}.keys().cloned().collect::<Vec<_>>()");
            }
            Some(Callee::Builtin(Builtin::MapLen)) => {
                let r = self.receiver(receiver);
                return format!("({r}.len() as i64)");
            }
            Some(Callee::Builtin(Builtin::DurationToMs)) => {
                let r = self.bare(receiver);
                let r = self.deref_if_borrowed(receiver, r);
                return format!("({} / 1000000)", wrap(receiver, r));
            }
            Some(Callee::Builtin(
                Builtin::DurationToNs | Builtin::ByteSizeToInt | Builtin::RatioPpm,
            )) => {
                let r = self.bare(receiver);
                let r = self.deref_if_borrowed(receiver, r);
                return wrap(receiver, r);
            }
            Some(Callee::Builtin(Builtin::IoErrorWithExitCode)) => {
                let r = self.value(receiver);
                let a = self.args(args);
                return format!("{}.with_exit_code(({}) as i32)", wrap(receiver, r), a[0]);
            }
            Some(Callee::Builtin(Builtin::Debug)) => {
                let ty = self.type_of(receiver).cloned().unwrap_or(Ty::Unit);
                let value = self.ref_arg(receiver);
                return self.debug_text(&ty, &value);
            }
            Some(Callee::Builtin(Builtin::StoreUpdate)) => {
                return self.store_with(receiver, "update", args);
            }
            Some(Callee::Builtin(Builtin::StoreUpsert)) => {
                return self.store_with(receiver, "upsert", args);
            }
            Some(Callee::FieldFn { field, .. }) => {
                let r = self.receiver(receiver);
                let a = self.args(args);
                return format!("({r}.{})({})", ident(field), a.join(", "));
            }
            Some(Callee::Builtin(
                Builtin::IntToTextMethod | Builtin::ByteToText | Builtin::BoolToText,
            )) => {
                let r = self.receiver(receiver);
                return format!("{r}.to_string()");
            }
            Some(Callee::Builtin(Builtin::TextToText)) => {
                let r = self.receiver(receiver);
                return format!("{r}.clone()");
            }
            Some(Callee::Builtin(Builtin::BytesToText)) => {
                if let Some(bytes) = self.checked_slice_ref(receiver) {
                    return format!("String::from_utf8_lossy({bytes}).into_owned()");
                }
            }
            _ => {}
        }
        if let Some(owner) = self.qualifier(receiver)
            && let Some(text) = self.qualified_call(expr, owner, name, args)
        {
            return text;
        }
        if let ("append", [value]) = (name.name.as_str(), args)
            && place_root(value).is_none_or(|root| place_root(receiver) != Some(root))
        {
            let r = self.receiver(receiver);
            return format!("{r}.extend_from_slice({})", self.ref_arg(value));
        }
        let r = self.receiver(receiver);
        if matches!(self.callee(expr), Some(Callee::Builtin(Builtin::ListMap)))
            && let [arg] = args
            && let Some(f) = self.passed_closure(arg)
        {
            return format!("{r}.iter().cloned().map({f}).collect::<Vec<_>>()");
        }
        let a = self.args(args);
        let arg = |i: usize, a: &[String]| borrow(&args[i], a[i].clone());
        match (name.name.as_str(), a.len()) {
            ("map", 1) => format!(
                "{r}.iter().cloned().map(&*{}).collect::<Vec<_>>()",
                wrap(&args[0], a[0].clone())
            ),
            ("to_int", 0) => format!("({r} as i64)"),
            ("to_byte", 0) => format!("u8::try_from({r}).ok()"),
            ("len", 0) => format!("({r}.len() as i64)"),
            ("is_empty", 0) => format!("{r}.is_empty()"),
            ("trim", 0) => format!("{r}.trim().to_string()"),
            ("starts_with", 1) => format!("{r}.starts_with({})", arg(0, &a)),
            ("strip_prefix", 1) => {
                format!("{r}.strip_prefix({}).map(|s| s.to_string())", arg(0, &a))
            }
            ("concat", 1) => format!("format!(\"{{}}{{}}\", {r}, {})", a[0]),
            ("to_bytes", 0) if self.fresh_text(receiver) => format!("{r}.into_bytes()"),
            ("to_bytes", 0) => format!("{r}.clone().into_bytes()"),
            ("to_text", 0) => format!(
                "String::from_utf8_lossy({}).into_owned()",
                borrow(receiver, r.clone())
            ),
            ("at", 1) => format!("{r}[({}) as usize]{}", a[0], self.element_clone(expr)),
            ("slice", 2) => format!("{r}[({}) as usize..({}) as usize].to_vec()", a[0], a[1]),
            ("push", 1) => format!("{r}.push({})", a[0]),
            ("append", 1) => format!("{r}.extend_from_slice({})", arg(0, &a)),
            ("clear", 0) => format!("{r}.clear()"),
            ("join", 1) => format!("{r}.join({})", arg(0, &a)),
            ("copy", 0) => format!("{r}.clone()"),
            _ => format!("{r}.{}({})", ident(&name.name), a.join(", ")),
        }
    }

    fn match_expr(
        &mut self,
        expr: &Expr,
        scrutinee: &Expr,
        arms: &[ignite_syntax::ast::Arm],
    ) -> String {
        let no_arm = self
            .fail_call(expr, "no arm matched")
            .unwrap_or_else(|| "unreachable!()".to_string());
        let ty = self.type_of(scrutinee).cloned();
        let lent = self.match_by_ref(scrutinee, ty.as_ref(), arms);
        let mut scrutinee_text = if lent {
            self.ref_arg(scrutinee)
        } else {
            self.value(scrutinee)
        };
        if matches!(scrutinee.kind, ExprKind::Record { .. }) {
            scrutinee_text = format!("({scrutinee_text})");
        }
        let mut out = format!("match {scrutinee_text} {{\n");
        self.indent += 1;
        let mut guarded = false;
        let mut catch_all = false;
        let mut ranged = false;
        for arm in arms {
            self.scopes.push(Vec::new());
            let mut bindings = Vec::new();
            let native = self.is_native(&arm.pattern, ty.as_ref());
            catch_all = match &arm.pattern.kind {
                PatternKind::Wildcard => true,
                PatternKind::Binding(id) => !id.is_uppercase(),
                _ => false,
            };
            guarded |= !native && !catch_all;
            ranged |= has_range(&arm.pattern);
            let lent_before = self.borrowed_elements.len();
            let pattern = if native {
                let text = self.pattern(&arm.pattern, ty.as_ref());
                if lent {
                    self.lend_bindings(&arm.pattern, &mut bindings);
                }
                text
            } else {
                let subject = if lent { "__m" } else { "(&__m)" }.to_string();
                let test = self.test(&arm.pattern, &subject, ty.as_ref());
                self.extract(&arm.pattern, subject, ty.as_ref(), &mut bindings);
                match test {
                    Some(test) => format!("__m if {test}"),
                    None => "__m".to_string(),
                }
            };
            let mut body = match &arm.body.kind {
                ExprKind::Block(block) => self.block(block, true),
                _ => self.result(&arm.body),
            };
            self.borrowed_elements.truncate(lent_before);
            self.scopes.pop();
            if !bindings.is_empty() {
                let lets: String = bindings
                    .iter()
                    .map(|(name, access)| {
                        format!("let {} = {}; ", ident(name), binding_value(access))
                    })
                    .collect();
                body = format!("{{ {lets}{body} }}");
            }
            out.push_str(&indentation(self.indent));
            let _ = writeln!(out, "{pattern} => {body},");
        }
        if (guarded || ranged) && !catch_all {
            out.push_str(&indentation(self.indent));
            out.push_str("#[allow(unreachable_patterns)]\n");
            out.push_str(&indentation(self.indent));
            let _ = writeln!(out, "_ => {no_arm},");
        }
        self.indent -= 1;
        out.push_str(&indentation(self.indent));
        out.push('}');
        out
    }

    /// Whether `match scrutinee { arms }` can match a reference to its
    /// scrutinee instead of a copy: the scrutinee is a place or a
    /// [`field_of_element`](Self::field_of_element) whose type is known,
    /// lendable ([`lendable`]) and not `Copy` in Rust, and no arm changes
    /// the variable the place is rooted at.
    fn match_by_ref(
        &self,
        scrutinee: &Expr,
        ty: Option<&Ty>,
        arms: &[ignite_syntax::ast::Arm],
    ) -> bool {
        let root = if self.is_place(scrutinee) {
            place_root(scrutinee)
        } else if self.field_of_element(scrutinee) {
            self.element_root(scrutinee)
        } else {
            None
        };
        let Some(root) = root else {
            return false;
        };
        if !ty.is_some_and(|ty| lendable(ty) && !rust_copy(ty)) {
            return false;
        }
        let mut mutated = BTreeSet::new();
        for arm in arms {
            mutated_in(&arm.body, self.types, &mut mutated);
        }
        !mutated.contains(root)
    }

    /// The bindings of a native `pattern` matched against a reference,
    /// where each is a reference: a lendable binding that is not `Copy`
    /// stays one in the arm, read like a borrowed parameter; any other is
    /// copied out at the start of the arm (pushed to `out`).
    fn lend_bindings(&mut self, pattern: &Pattern, out: &mut Vec<(String, String)>) {
        let mut names = Vec::new();
        pattern_bindings(pattern, &mut names);
        let scope = self.scopes.len() - 1;
        for id in names {
            if !self.scopes[scope].contains(&id.name) {
                continue;
            }
            let ty = self.types.and_then(|t| t.binding_type(id.span));
            if ty.is_some_and(|ty| lendable(ty) && !rust_copy(ty)) {
                self.borrowed_elements.push((scope, id.name.clone()));
            } else {
                out.push((id.name.clone(), ident(&id.name)));
            }
        }
    }

    /// An `if`; `value` says whether it is in value position. Its branches
    /// are value-position blocks only when it is and it has an `else`.
    fn if_expr(&mut self, expr: &Expr, value: bool) -> String {
        let ExprKind::If {
            cond,
            then,
            otherwise,
        } = &expr.kind
        else {
            return self.bare(expr);
        };
        let branches = value && otherwise.is_some();
        let cond = self.condition(cond);
        let then = self.block(then, branches);
        let mut out = format!("if {cond} {then}");
        if let Some(otherwise) = otherwise {
            let text = match &otherwise.kind {
                ExprKind::Block(block) => self.block(block, branches),
                ExprKind::If { .. } => self.if_expr(otherwise, value),
                _ => {
                    let inner = self.result(otherwise);
                    format!("{{ {inner} }}")
                }
            };
            out.push_str(" else ");
            out.push_str(&text);
        }
        out
    }

    /// The constant a pattern compares with, as the Rust call `N()`.
    fn pattern_const(&self, pattern: &Pattern) -> Option<String> {
        if let Some(types) = self.types {
            return types
                .pattern_const(pattern)
                .map(|q| format!("{}()", ident(&q.name)));
        }
        let name = match &pattern.kind {
            PatternKind::Variant { path, args } if path.len() == 1 && args.is_empty() => {
                &path[0].name
            }
            PatternKind::Binding(id) if id.is_uppercase() => &id.name,
            _ => return None,
        };
        (self.constants.contains(name) && self.enum_of_variant(name).is_none())
            .then(|| format!("{}()", ident(name)))
    }

    /// Whether a pattern is a Rust pattern as written: no text, list or
    /// constant patterns anywhere in it. Other patterns become a guard
    /// ([`Generator::test`]) and bindings read out of the scrutinee
    /// ([`Generator::extract`]). A sub-pattern other than `_` at a boxed
    /// payload position (`docs/slice.md` section 13.4) is not native
    /// either: Rust cannot match through a `Box`.
    fn is_native(&self, pattern: &Pattern, ty: Option<&Ty>) -> bool {
        if self.pattern_const(pattern).is_some() {
            return false;
        }
        match &pattern.kind {
            PatternKind::Range { .. } => self.range(pattern).is_some(),
            PatternKind::Str(_) | PatternKind::List(_) | PatternKind::Rest(_) => false,
            PatternKind::Or(items) => items.iter().all(|p| self.is_native(p, ty)),
            PatternKind::Tuple(items) => {
                let elements = tuple_elements(ty, items.len());
                items
                    .iter()
                    .zip(&elements)
                    .all(|(p, t)| self.is_native(p, t.as_ref()))
            }
            PatternKind::Variant { path, args } => {
                let variant = path.last().map_or("_", |i| i.name.as_str());
                let (_, payload) = self.variant_owner(path, variant, ty);
                args.iter().enumerate().all(|(i, p)| {
                    let boxed = !matches!(p.kind, PatternKind::Wildcard)
                        && self.is_boxed_payload(ty, variant, i);
                    !boxed && self.is_native(p, payload.get(i))
                })
            }
            PatternKind::Wildcard
            | PatternKind::Int(_)
            | PatternKind::Bool(_)
            | PatternKind::Binding(_) => true,
        }
    }

    /// Both bounds of a range pattern: from the typer, else from integer
    /// literal bounds.
    fn range(&self, pattern: &Pattern) -> Option<(i64, i64)> {
        if let Some(types) = self.types {
            return types.range_bounds(pattern);
        }
        match &pattern.kind {
            PatternKind::Range { start, end } => match (&start.kind, &end.kind) {
                (PatternKind::Int(lo), PatternKind::Int(hi)) => Some((*lo, *hi)),
                _ => None,
            },
            _ => None,
        }
    }

    /// The enum owning variant `name` of a pattern on a value of type `ty`,
    /// and the variant's payload types when types are known.
    fn variant_owner(
        &self,
        path: &[Ident],
        name: &str,
        ty: Option<&Ty>,
    ) -> (Option<String>, Vec<Ty>) {
        let typed = match (self.types, ty) {
            (Some(types), Some(ty)) => types.variant(ty, name).map(|(_, payload)| {
                let owner = match ty {
                    Ty::Enum(q) | Ty::GenericEnum(q, _) => self.enum_name(q),
                    Ty::Option(_) => "Option".to_string(),
                    Ty::Result(..) => "Result".to_string(),
                    _ => "Input".to_string(),
                };
                (owner, payload)
            }),
            _ => None,
        };
        let payload = typed.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();
        let owner = if path.len() >= 2 {
            Some(self.unshadowed(&path[path.len() - 2].name))
        } else if let Some((owner, _)) = typed {
            Some(owner)
        } else {
            self.enum_of_variant(name).map(|o| self.unshadowed(&o))
        };
        (owner, payload)
    }

    fn variant_head(&self, path: &[Ident], name: &str, ty: Option<&Ty>) -> (String, Vec<Ty>) {
        let (owner, payload) = self.variant_owner(path, name, ty);
        let head = match owner {
            Some(owner) => variant_path(&owner, name),
            None => ident(name),
        };
        (head, payload)
    }

    /// A native pattern ([`Generator::is_native`]) on a value of type `ty`.
    fn pattern(&mut self, pattern: &Pattern, ty: Option<&Ty>) -> String {
        match &pattern.kind {
            PatternKind::Range { .. } => match self.range(pattern) {
                Some((lo, hi)) => format!("{lo}..={hi}"),
                None => "_".to_string(),
            },
            PatternKind::Wildcard => "_".to_string(),
            PatternKind::Int(value) => value.to_string(),
            PatternKind::Str(text) => format!("\"{}\"", escape_str(text)),
            PatternKind::Bool(value) => value.to_string(),
            PatternKind::Or(items) => {
                let items: Vec<String> = items.iter().map(|p| self.pattern(p, ty)).collect();
                items.join(" | ")
            }
            PatternKind::Tuple(items) => {
                let elements = tuple_elements(ty, items.len());
                let items: Vec<String> = items
                    .iter()
                    .zip(&elements)
                    .map(|(p, t)| self.pattern(p, t.as_ref()))
                    .collect();
                format!("({})", items.join(", "))
            }
            PatternKind::List(items) => {
                let items: Vec<String> = items.iter().map(|p| self.pattern(p, None)).collect();
                format!("[{}]", items.join(", "))
            }
            PatternKind::Rest(None) => "..".to_string(),
            PatternKind::Rest(Some(id)) => {
                self.bind(&id.name);
                format!("{} @ ..", ident(&id.name))
            }
            PatternKind::Binding(id) => {
                if id.is_uppercase() {
                    let (owner, _) = self.variant_owner(&[], &id.name, ty);
                    if let Some(owner) = owner {
                        return variant_path(&owner, &id.name);
                    }
                }
                self.bind(&id.name);
                ident(&id.name)
            }
            PatternKind::Variant { path, args } => {
                let variant = match path.last() {
                    Some(last) => last.name.as_str(),
                    None => return "_".to_string(),
                };
                let (head, payload) = self.variant_head(path, variant, ty);
                if args.is_empty() {
                    head
                } else {
                    let args: Vec<String> = args
                        .iter()
                        .enumerate()
                        .map(|(i, p)| self.pattern(p, payload.get(i)))
                        .collect();
                    format!("{head}({})", args.join(", "))
                }
            }
        }
    }

    /// `let` with an irrefutable pattern: bindings, `_` and tuples of those.
    fn let_pattern(&mut self, pattern: &Pattern, mutable: bool) -> String {
        match &pattern.kind {
            PatternKind::Wildcard => "_".to_string(),
            PatternKind::Binding(id) => {
                self.bind(&id.name);
                let mutability = if mutable { "mut " } else { "" };
                format!("{mutability}{}", ident(&id.name))
            }
            PatternKind::Tuple(items) => {
                let items: Vec<String> =
                    items.iter().map(|p| self.let_pattern(p, mutable)).collect();
                format!("({})", items.join(", "))
            }
            _ => self.pattern(pattern, None),
        }
    }

    /// The condition under which `pattern` matches the value `subject` (a
    /// Rust expression of type `&T`); `None` when it always does. Binds
    /// nothing in Rust: names it introduces are fresh (`__pN`).
    fn test(&mut self, pattern: &Pattern, subject: &str, ty: Option<&Ty>) -> Option<String> {
        if let Some(constant) = self.pattern_const(pattern) {
            return Some(format!("(*{subject} == {constant})"));
        }
        match &pattern.kind {
            PatternKind::Range { .. } => {
                let (lo, hi) = self.range(pattern)?;
                Some(format!("({lo}..={hi}).contains({subject})"))
            }
            PatternKind::Wildcard | PatternKind::Rest(_) => None,
            PatternKind::Int(value) => Some(format!("(*{subject} == {value})")),
            PatternKind::Str(text) => Some(format!("(*{subject} == \"{}\")", escape_str(text))),
            PatternKind::Bool(value) => Some(format!("(*{subject} == {value})")),
            PatternKind::Binding(id) => {
                if id.is_uppercase() {
                    let (owner, _) = self.variant_owner(&[], &id.name, ty);
                    if let Some(owner) = owner {
                        let path = variant_path(&owner, &id.name);
                        return Some(format!("matches!({subject}, {path})"));
                    }
                }
                None
            }
            PatternKind::Or(items) => {
                let mut tests = Vec::new();
                for item in items {
                    tests.push(self.test(item, subject, ty)?);
                }
                Some(format!("({})", tests.join(" || ")))
            }
            PatternKind::Tuple(items) => {
                let elements = tuple_elements(ty, items.len());
                let mut names = Vec::new();
                let mut tests = Vec::new();
                for (item, element) in items.iter().zip(&elements) {
                    let name = self.fresh();
                    match self.test(item, &name, element.as_ref()) {
                        Some(test) => {
                            names.push(name);
                            tests.push(test);
                        }
                        None => names.push("_".to_string()),
                    }
                }
                if tests.is_empty() {
                    return None;
                }
                Some(format!(
                    "{{ let ({}) = {subject}; {} }}",
                    names.join(", "),
                    tests.join(" && ")
                ))
            }
            PatternKind::List(items) => {
                let element = match ty {
                    Some(Ty::List(t)) => Some((**t).clone()),
                    _ => None,
                };
                let rest = items
                    .iter()
                    .position(|item| matches!(item.kind, PatternKind::Rest(_)));
                let mut lets = String::new();
                let mut tests = match rest {
                    None => vec![format!("{subject}.len() == {}", items.len())],
                    Some(_) if items.len() == 1 => {
                        return None;
                    }
                    Some(_) => vec![format!("{subject}.len() >= {}", items.len() - 1)],
                };
                for (i, item) in items.iter().enumerate() {
                    let index = list_index(subject, i, rest, items.len());
                    let name = self.fresh();
                    if let Some(test) = self.test(item, &name, element.as_ref()) {
                        let _ = write!(lets, "let {name} = &{subject}[{index}]; ");
                        tests.push(test);
                    }
                }
                if tests.len() == 1 {
                    return Some(format!("({})", tests[0]));
                }
                let first = tests.remove(0);
                Some(format!("({first} && {{ {lets}{} }})", tests.join(" && ")))
            }
            PatternKind::Variant { path, args } => {
                let variant = path.last().map(|i| i.name.as_str()).unwrap_or("_");
                let (head, payload) = self.variant_head(path, variant, ty);
                if args.is_empty() {
                    return Some(format!("matches!({subject}, {head})"));
                }
                let mut names = Vec::new();
                let mut tests = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    let name = self.fresh();
                    let inner = if self.is_boxed_payload(ty, variant, i) {
                        format!("(&**{name})")
                    } else {
                        name.clone()
                    };
                    match self.test(arg, &inner, payload.get(i)) {
                        Some(test) => {
                            names.push(name);
                            tests.push(test);
                        }
                        None => names.push("_".to_string()),
                    }
                }
                if tests.is_empty() {
                    return Some(format!("matches!({subject}, {head}(..))"));
                }
                Some(format!(
                    "(match {subject} {{ {head}({}) => {}, _ => false }})",
                    names.join(", "),
                    tests.join(" && ")
                ))
            }
        }
    }

    /// Each binding of `pattern` with a Rust expression of type `&T` that
    /// reads it out of `subject` (also `&T`); binds the names in scope.
    fn extract(
        &mut self,
        pattern: &Pattern,
        subject: String,
        ty: Option<&Ty>,
        out: &mut Vec<(String, String)>,
    ) {
        if self.pattern_const(pattern).is_some() {
            return;
        }
        match &pattern.kind {
            PatternKind::Range { .. } => {}
            PatternKind::Binding(id) => {
                if id.is_uppercase() && self.variant_owner(&[], &id.name, ty).0.is_some() {
                    return;
                }
                self.bind(&id.name);
                out.push((id.name.clone(), subject));
            }
            PatternKind::Tuple(items) => {
                let elements = tuple_elements(ty, items.len());
                for (i, (item, element)) in items.iter().zip(&elements).enumerate() {
                    self.extract(item, format!("(&{subject}.{i})"), element.as_ref(), out);
                }
            }
            PatternKind::List(items) => {
                let element = match ty {
                    Some(Ty::List(t)) => Some((**t).clone()),
                    _ => None,
                };
                let rest = items
                    .iter()
                    .position(|item| matches!(item.kind, PatternKind::Rest(_)));
                for (i, item) in items.iter().enumerate() {
                    if let PatternKind::Rest(Some(id)) = &item.kind {
                        let end = match items.len() - i - 1 {
                            0 => String::new(),
                            n => format!("{subject}.len() - {n}"),
                        };
                        self.bind(&id.name);
                        out.push((id.name.clone(), format!("{subject}[{i}..{end}].to_vec()")));
                        continue;
                    }
                    let index = list_index(&subject, i, rest, items.len());
                    self.extract(
                        item,
                        format!("(&{subject}[{index}])"),
                        element.as_ref(),
                        out,
                    );
                }
            }
            PatternKind::Variant { path, args } => {
                let variant = path.last().map(|i| i.name.as_str()).unwrap_or("_");
                let (head, payload) = self.variant_head(path, variant, ty);
                for (i, arg) in args.iter().enumerate() {
                    let fields: Vec<&str> = (0..args.len())
                        .map(|j| if i == j { "__x" } else { "_" })
                        .collect();
                    let deref = if self.is_boxed_payload(ty, variant, i) {
                        "&**"
                    } else {
                        ""
                    };
                    let access = format!(
                        "(match {subject} {{ {head}({}) => {deref}__x, _ => unreachable!() }})",
                        fields.join(", ")
                    );
                    self.extract(arg, access, payload.get(i), out);
                }
            }
            PatternKind::Wildcard
            | PatternKind::Int(_)
            | PatternKind::Str(_)
            | PatternKind::Bool(_)
            | PatternKind::Or(_)
            | PatternKind::Rest(_) => {}
        }
    }
}

/// Slice 9 (`docs/slice.md` section 13): equality and debug glue, boxing,
/// closures, associated fns.
impl<'a> Generator<'a> {
    /// The record or enum a type names, with its type arguments.
    fn nominal(ty: &Ty) -> Option<(&QualName, &[Ty])> {
        match ty {
            Ty::Record(q) | Ty::Enum(q) => Some((q, [].as_slice())),
            Ty::GenericRecord(q, args) | Ty::GenericEnum(q, args) => Some((q, args)),
            _ => None,
        }
    }

    /// The fields of record type `ty`, type arguments substituted, each
    /// with whether it is boxed.
    fn fields_of(&self, ty: &Ty) -> Option<Vec<(String, Ty, bool)>> {
        let types = self.types?;
        let (q, args) = Self::nominal(ty)?;
        let fields = types.record_fields(q)?;
        let params = types.type_params(q).unwrap_or(&[]);
        Some(
            fields
                .iter()
                .map(|(name, t)| {
                    (
                        name.clone(),
                        t.substitute(params, args),
                        types.boxed_field(q, name),
                    )
                })
                .collect(),
        )
    }

    /// The variants of user enum type `ty`, type arguments substituted,
    /// each payload with whether it is boxed.
    fn variants_of(&self, ty: &Ty) -> Option<Vec<Variant>> {
        let types = self.types?;
        let (q, args) = Self::nominal(ty)?;
        if q.is_builtin() {
            let (_, variants) = BUILTIN_ENUMS.iter().find(|(name, _)| *name == q.name)?;
            return Some(
                variants
                    .iter()
                    .map(|name| (name.to_string(), Vec::new()))
                    .collect(),
            );
        }
        let variants = types.enum_variants(q)?;
        let params = types.type_params(q).unwrap_or(&[]);
        Some(
            variants
                .iter()
                .enumerate()
                .map(|(v, (name, payload))| {
                    let payload = payload
                        .iter()
                        .enumerate()
                        .map(|(i, t)| (t.substitute(params, args), types.boxed_payload(q, v, i)))
                        .collect();
                    (name.clone(), payload)
                })
                .collect(),
        )
    }

    /// Whether field `field` of the record of type `ty` is boxed.
    fn is_boxed_field(&self, ty: Option<&Ty>, field: &str) -> bool {
        match (self.types, ty.and_then(Self::nominal)) {
            (Some(types), Some((q, _))) => types.boxed_field(q, field),
            _ => false,
        }
    }

    /// Whether payload `index` of variant `variant` of a value of type
    /// `ty` is boxed.
    fn is_boxed_payload(&self, ty: Option<&Ty>, variant: &str, index: usize) -> bool {
        let (Some(types), Some(ty)) = (self.types, ty) else {
            return false;
        };
        let Some((q, _)) = Self::nominal(ty) else {
            return false;
        };
        types
            .variant(ty, variant)
            .is_some_and(|(v, _)| types.boxed_payload(q, v, index))
    }

    /// The name of the glue fn of `kind` for `ty`, registering it.
    fn glue_fn(&mut self, kind: Glue, ty: &Ty) -> String {
        let index = match self.glue.iter().position(|(k, t)| *k == kind && t == ty) {
            Some(index) => index,
            None => {
                self.glue.push((kind, ty.clone()));
                self.glue.len() - 1
            }
        };
        match kind {
            Glue::Eq => format!("__ignite_eq_{index}"),
            Glue::Debug => format!("__ignite_debug_{index}"),
        }
    }

    /// A Rust `bool` expression: whether the values behind the references
    /// `a` and `b` of type `ty` are equal (`docs/slice.md` section 13.5).
    fn eq_test(&mut self, ty: &Ty, a: &str, b: &str) -> String {
        match ty {
            Ty::Int | Ty::Byte | Ty::Bool | Ty::Quantity(_) => format!("(*{a} == *{b})"),
            Ty::Unit | Ty::Never => "true".to_string(),
            Ty::Text | Ty::Ref(_) | Ty::Signal(_) | Ty::SystemRef | Ty::Protocol => {
                format!("({a} == {b})")
            }
            _ => format!("{}({a}, {b})", self.glue_fn(Glue::Eq, ty)),
        }
    }

    /// Rust statements appending the debug text of the value behind the
    /// reference `v` of type `ty` to the `&mut String` `out`
    /// (`docs/slice.md` section 13.6).
    fn debug_write(&mut self, ty: &Ty, v: &str, out: &str) -> String {
        let fixed = |text: &str| format!("{out}.push_str(\"{}\");", escape_str(text));
        match ty {
            Ty::Int | Ty::Byte | Ty::Bool => format!("{out}.push_str(&{v}.to_string());"),
            Ty::Quantity(kind) => {
                let kind = match kind {
                    QuantityKind::Duration => "Duration",
                    QuantityKind::ByteSize => "ByteSize",
                    QuantityKind::Ratio => "Ratio",
                };
                format!(
                    "{out}.push_str(&ignite_io::quantity_debug(ignite_io::Quantity::{kind}, *{v}));"
                )
            }
            Ty::Ref(_) | Ty::Signal(_) | Ty::SystemRef | Ty::Protocol => {
                self.debug_write(&Ty::Text, v, out)
            }
            Ty::Unit => fixed("()"),
            Ty::Never => String::new(),
            Ty::Fn(..) | Ty::EffectFn(..) => fixed("<fn>"),
            Ty::TcpStream => fixed("<TcpStream>"),
            Ty::PipeStream => fixed("<PipeStream>"),
            Ty::TcpListener => fixed("<TcpListener>"),
            Ty::Store(_) => fixed("<Store>"),
            Ty::Capability(kind) => fixed(&format!("<{}>", kind.name())),
            Ty::Param(name) => fixed(&format!("<{name}>")),
            _ => format!("{}({v}, {out});", self.glue_fn(Glue::Debug, ty)),
        }
    }

    /// A Rust `String` expression: the debug text of the value behind the
    /// reference `v` of type `ty`.
    fn debug_text(&mut self, ty: &Ty, v: &str) -> String {
        let write = self.debug_write(ty, "__ignite_v", "(&mut __ignite_text)");
        format!(
            "{{ let __ignite_v = {v}; let mut __ignite_text = String::new(); {write} __ignite_text }}"
        )
    }

    /// The glue fns registered so far, and those they need, as items.
    fn glue_items(&mut self) -> String {
        self.glue_item_list()
            .into_iter()
            .map(|(_, text)| text)
            .collect()
    }

    /// [`Generator::glue_items`] one item at a time, each with its name.
    fn glue_item_list(&mut self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut index = 0;
        while index < self.glue.len() {
            let (kind, ty) = self.glue[index].clone();
            let rust = self.rust_ty(&ty);
            let item = match kind {
                Glue::Eq => {
                    let body = self.eq_body(&ty);
                    (
                        format!("__ignite_eq_{index}"),
                        format!(
                            "\nfn __ignite_eq_{index}(a: &{rust}, b: &{rust}) -> bool {{\n    {body}\n}}\n"
                        ),
                    )
                }
                Glue::Debug => {
                    let body = self.debug_body(&ty);
                    (
                        format!("__ignite_debug_{index}"),
                        format!(
                            "\nfn __ignite_debug_{index}(v: &{rust}, out: &mut String) {{\n    {body}\n}}\n"
                        ),
                    )
                }
            };
            out.push(item);
            index += 1;
        }
        out
    }

    /// The body of the equality glue of a structural type.
    fn eq_body(&mut self, ty: &Ty) -> String {
        match ty {
            Ty::List(item) => {
                let test = self.eq_test(item, "x", "y");
                format!("a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| {test})")
            }
            Ty::Map(key, value) => {
                let keys = self.eq_test(key, "kx", "ky");
                let values = self.eq_test(value, "x", "y");
                format!(
                    "a.len() == b.len() && a.iter().zip(b.iter()).all(|((kx, x), (ky, y))| {keys} && {values})"
                )
            }
            Ty::Option(item) => {
                let test = self.eq_test(item, "x", "y");
                format!(
                    "match (a, b) {{ (Some(x), Some(y)) => {test}, (None, None) => true, _ => false }}"
                )
            }
            Ty::Result(ok, err) => {
                let ok = self.eq_test(ok, "x", "y");
                let err = self.eq_test(err, "x", "y");
                format!(
                    "match (a, b) {{ (Ok(x), Ok(y)) => {ok}, (Err(x), Err(y)) => {err}, _ => false }}"
                )
            }
            Ty::Input => {
                let data = self.eq_test(&Ty::List(Box::new(Ty::Byte)), "x", "y");
                format!(
                    "match (a, b) {{ (ignite_io::Input::Data(x), ignite_io::Input::Data(y)) => {data}, (ignite_io::Input::Eof, ignite_io::Input::Eof) => true, _ => false }}"
                )
            }
            Ty::Tuple(items) => {
                let tests: Vec<String> = items
                    .iter()
                    .enumerate()
                    .map(|(i, t)| self.eq_test(t, &format!("&a.{i}"), &format!("&b.{i}")))
                    .collect();
                and_all(tests)
            }
            Ty::Record(_) | Ty::GenericRecord(..) => {
                let fields = self.fields_of(ty).unwrap_or_default();
                let tests: Vec<String> = fields
                    .iter()
                    .map(|(name, t, boxed)| {
                        let deref = if *boxed { "*" } else { "" };
                        let field = ident(name);
                        self.eq_test(
                            t,
                            &format!("&{deref}a.{field}"),
                            &format!("&{deref}b.{field}"),
                        )
                    })
                    .collect();
                and_all(tests)
            }
            Ty::Enum(q) | Ty::GenericEnum(q, _) => {
                let owner = self.enum_name(q);
                let variants = self.variants_of(ty).unwrap_or_default();
                let mut arms = Vec::new();
                for (name, payload) in &variants {
                    let path = variant_path(&owner, name);
                    if payload.is_empty() {
                        arms.push(format!("({path}, {path}) => true"));
                        continue;
                    }
                    let xs: Vec<String> = (0..payload.len()).map(|i| format!("x{i}")).collect();
                    let ys: Vec<String> = (0..payload.len()).map(|i| format!("y{i}")).collect();
                    let tests: Vec<String> = payload
                        .iter()
                        .enumerate()
                        .map(|(i, (t, boxed))| {
                            let deref = if *boxed { "&**" } else { "" };
                            self.eq_test(t, &format!("{deref}x{i}"), &format!("{deref}y{i}"))
                        })
                        .collect();
                    arms.push(format!(
                        "({path}({}), {path}({})) => {}",
                        xs.join(", "),
                        ys.join(", "),
                        and_all(tests)
                    ));
                }
                if variants.len() != 1 {
                    arms.push("_ => false".to_string());
                }
                format!("match (a, b) {{ {} }}", arms.join(", "))
            }
            _ => self.eq_test(ty, "a", "b"),
        }
    }

    /// The body of the debug glue of a type.
    fn debug_body(&mut self, ty: &Ty) -> String {
        let push = |text: &str| format!("out.push_str(\"{}\");", escape_str(text));
        match ty {
            Ty::Text => concat!(
                "out.push('\"'); ",
                "for c in v.chars() { match c { ",
                "'\"' => out.push_str(\"\\\\\\\"\"), ",
                "'\\\\' => out.push_str(\"\\\\\\\\\"), ",
                "'\\n' => out.push_str(\"\\\\n\"), ",
                "'\\r' => out.push_str(\"\\\\r\"), ",
                "'\\t' => out.push_str(\"\\\\t\"), ",
                "'\\0' => out.push_str(\"\\\\0\"), ",
                "c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!(\"\\\\u{{{:x}}}\", c as u32)), ",
                "c => out.push(c), ",
                "} } ",
                "out.push('\"');"
            )
            .to_string(),
            Ty::List(item) => {
                let write = self.debug_write(item, "x", "out");
                format!(
                    "out.push('['); for (i, x) in v.iter().enumerate() {{ if i > 0 {{ out.push_str(\", \"); }} {write} }} out.push(']');"
                )
            }
            Ty::Map(key, value) => {
                let write_key = self.debug_write(key, "k", "out");
                let write_value = self.debug_write(value, "x", "out");
                format!(
                    "out.push('{{'); for (i, (k, x)) in v.iter().enumerate() {{ if i > 0 {{ out.push_str(\", \"); }} {write_key} out.push_str(\": \"); {write_value} }} out.push('}}');"
                )
            }
            Ty::Option(item) => {
                let write = self.debug_write(item, "x", "out");
                format!(
                    "match v {{ Some(x) => {{ out.push_str(\"Some(\"); {write} out.push(')'); }} None => out.push_str(\"None\"), }}"
                )
            }
            Ty::Result(ok, err) => {
                let ok = self.debug_write(ok, "x", "out");
                let err = self.debug_write(err, "x", "out");
                format!(
                    "match v {{ Ok(x) => {{ out.push_str(\"Ok(\"); {ok} out.push(')'); }} Err(x) => {{ out.push_str(\"Err(\"); {err} out.push(')'); }} }}"
                )
            }
            Ty::Input => {
                let data = self.debug_write(&Ty::List(Box::new(Ty::Byte)), "x", "out");
                format!(
                    "match v {{ ignite_io::Input::Data(x) => {{ out.push_str(\"Data(\"); {data} out.push(')'); }} ignite_io::Input::Eof => out.push_str(\"Eof\"), }}"
                )
            }
            Ty::IoError => {
                let message = self.debug_write(&Ty::Text, "&v.message", "out");
                format!("out.push_str(\"IoError(\"); {message} out.push(')');")
            }
            Ty::Tuple(items) => {
                let mut body = String::from("out.push('(');");
                for (i, t) in items.iter().enumerate() {
                    if i > 0 {
                        body.push_str(" out.push_str(\", \");");
                    }
                    body.push(' ');
                    body.push_str(&self.debug_write(t, &format!("&v.{i}"), "out"));
                }
                body.push_str(" out.push(')');");
                body
            }
            Ty::Record(q) | Ty::GenericRecord(q, _) => {
                let Some(fields) = self.fields_of(ty) else {
                    return push(&format!("<{}>", q.name));
                };
                if fields.is_empty() {
                    return push(&format!("{} {{}}", q.name));
                }
                let mut body = push(&format!("{} {{ ", q.name));
                for (i, (name, t, boxed)) in fields.iter().enumerate() {
                    let separator = if i > 0 { ", " } else { "" };
                    body.push(' ');
                    body.push_str(&push(&format!("{separator}{name}: ")));
                    let deref = if *boxed { "*" } else { "" };
                    body.push(' ');
                    body.push_str(&self.debug_write(
                        t,
                        &format!("&{deref}v.{}", ident(name)),
                        "out",
                    ));
                }
                body.push(' ');
                body.push_str(&push(" }"));
                body
            }
            Ty::Enum(q) | Ty::GenericEnum(q, _) => {
                let owner = self.enum_name(q);
                let Some(variants) = self.variants_of(ty) else {
                    return push(&format!("<{}>", q.name));
                };
                let mut arms = Vec::new();
                for (name, payload) in &variants {
                    let path = variant_path(&owner, name);
                    let head = push(&format!("{}.{name}", q.name));
                    if payload.is_empty() {
                        arms.push(format!("{path} => {{ {head} }}"));
                        continue;
                    }
                    let xs: Vec<String> = (0..payload.len()).map(|i| format!("x{i}")).collect();
                    let mut body = format!("{head} out.push('(');");
                    for (i, (t, boxed)) in payload.iter().enumerate() {
                        if i > 0 {
                            body.push_str(" out.push_str(\", \");");
                        }
                        let deref = if *boxed { "&**" } else { "" };
                        body.push(' ');
                        body.push_str(&self.debug_write(t, &format!("{deref}x{i}"), "out"));
                    }
                    body.push_str(" out.push(')');");
                    arms.push(format!("{path}({}) => {{ {body} }}", xs.join(", ")));
                }
                format!("match v {{ {} }}", arms.join(" "))
            }
            _ => self.debug_write(ty, "v", "out"),
        }
    }

    /// The Rust path of a fn: an associated fn `Owner.name` as its mangled
    /// symbol, qualified by its module unless it is this one; any other fn
    /// by its name (own, imported or glob-imported), or qualified by its
    /// module when it was resolved through the expected type without an
    /// import (`docs/slice.md` section 14.14 item 17).
    fn fn_path(&self, q: &QualName) -> String {
        let Some((owner, name)) = q.name.split_once('.') else {
            if q.module.is_empty()
                || self.is_own_module(&q.module)
                || self.symbols.imported.contains_key(&q.name)
            {
                return ident(&q.name);
            }
            return self.module_fn_path(q);
        };
        let symbol = assoc_symbol(owner, name);
        match self.module_path(&q.module) {
            Some(path) => format!("{path}::{symbol}"),
            None => symbol,
        }
    }

    /// The Rust path of fn `q` from any module, imported or not: the
    /// fns of a `module("p")` record (`docs/slice.md` section 14.4) are
    /// named by module, not through a `use`.
    fn module_fn_path(&self, q: &QualName) -> String {
        let symbol = match q.name.split_once('.') {
            Some((owner, name)) => assoc_symbol(owner, name),
            None => ident(&q.name),
        };
        match self.module_path(&q.module) {
            Some(path) => format!("{path}::{symbol}"),
            None => symbol,
        }
    }

    /// The associated fn an expression calls or names, if any.
    fn assoc_callee(&self, expr: &Expr) -> Option<&'a QualName> {
        match self.callee(expr)? {
            Callee::Fn(q) if q.name.contains('.') => Some(q),
            _ => None,
        }
    }

    /// A call of associated fn `q` with `args` by reference.
    fn assoc_call(&mut self, expr: &Expr, q: &QualName, args: &[Expr]) -> String {
        let target = format!("{}{}", self.fn_path(q), self.turbofish(expr));
        self.fn_call(expr, target, None, args)
    }

    /// Variant constructor arguments: values, a boxed payload in
    /// `Box::new`.
    fn variant_args(&mut self, expr: &Expr, args: &[Expr]) -> Vec<String> {
        let boxed: Vec<bool> = match (self.types, self.callee(expr)) {
            (Some(types), Some(Callee::Variant { enum_, index })) => (0..args.len())
                .map(|i| types.boxed_payload(enum_, *index, i))
                .collect(),
            _ => vec![false; args.len()],
        };
        args.iter()
            .zip(boxed)
            .map(|(arg, boxed)| {
                let value = self.value(arg);
                if boxed {
                    format!("Box::new({value})")
                } else {
                    value
                }
            })
            .collect()
    }

    /// The operand type of `left == right` when it is structural
    /// (`docs/slice.md` section 13.5); `None` for a scalar or `Text`.
    fn structural_operand(&self, left: &Expr, right: &Expr) -> Option<&'a Ty> {
        let types = self.types?;
        let ty = match self.type_of(left)? {
            Ty::Never => self.type_of(right)?,
            ty => ty,
        };
        (types.equality(ty) == Some(ignite_types::Equality::Structural)).then_some(ty)
    }

    /// `==` or `!=` on a structural type: its equality glue on both
    /// operands, left evaluated first.
    fn structural_eq(&mut self, op: BinaryOp, left: &Expr, right: &Expr) -> String {
        let ty = self
            .structural_operand(left, right)
            .expect("a structural operand");
        let glue = self.glue_fn(Glue::Eq, ty);
        let [left, right]: [String; 2] = self
            .ref_args(None, [left, right])
            .try_into()
            .expect("two operands");
        let negation = if op == BinaryOp::Ne { "!" } else { "" };
        format!("{negation}{glue}({left}, {right})")
    }

    /// `|p1, ..| body` (`docs/slice.md` section 13.3): each capture cloned
    /// at creation, the closure in an `Arc` cast to the fn value type.
    fn closure(&mut self, expr: &Expr, closure: &ignite_syntax::ast::Closure) -> String {
        self.closure_with(expr, closure, true)
    }

    /// `s.update(id, f)` or `s.upsert(id, f)`: a closure literal `f` passed
    /// as a plain closure, any other fn value as `&*f`.
    fn store_with(&mut self, receiver: &Expr, method: &str, args: &[Expr]) -> String {
        let r = self.receiver(receiver);
        let id = self.value(&args[0]);
        let f = match self.passed_closure(&args[1]) {
            Some(f) => f,
            None => {
                let f = self.value(&args[1]);
                format!("&*{}", wrap(&args[1], f))
            }
        };
        format!("{r}.{method}({id}, {f})")
    }

    /// A closure literal passed as an argument the callee calls before it
    /// returns and takes as `impl FnOnce`/`FnMut` (`Store.update`,
    /// `Store.upsert`, `List.map`): a plain closure that borrows its
    /// captures, without the `Arc` and the clone of each capture. Captures
    /// stay by copy (13.3): nothing can assign the enclosing fn's bindings
    /// while the call runs.
    fn passed_closure(&mut self, arg: &Expr) -> Option<String> {
        match &arg.kind {
            ExprKind::Closure(closure) if self.types.is_some() => {
                Some(self.closure_with(arg, closure, false))
            }
            _ => None,
        }
    }

    /// [`Generator::closure`] when `stored`, else
    /// [`Generator::passed_closure`]: not `move`, captures read through the
    /// enclosing fn's bindings, borrowed ones still borrowed.
    fn closure_with(
        &mut self,
        expr: &Expr,
        closure: &ignite_syntax::ast::Closure,
        stored: bool,
    ) -> String {
        let info = self.types.and_then(|t| t.closure(expr));
        let mut lets = String::new();
        if stored && let Some(info) = info {
            for capture in &info.captures {
                if self.is_local(&capture.name) {
                    let name = self.local_name(&capture.name);
                    let _ = write!(lets, "let {name} = {name}.clone(); ");
                }
            }
        }
        let mut mutated = BTreeSet::new();
        mutated_in(&closure.body, self.types, &mut mutated);
        let params: Vec<String> = closure
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mutability = if mutated.contains(&p.name.name) {
                    "mut "
                } else {
                    ""
                };
                let ty = match (info, &p.ty) {
                    (Some(info), _) => format!(": {}", self.rust_ty(&info.params[i].1)),
                    (None, Some(ty)) => format!(": {}", self.annotation(ty)),
                    (None, None) => String::new(),
                };
                format!("{mutability}{}{ty}", ident(&p.name.name))
            })
            .collect();
        let ret = info.map(|info| self.rust_ty(&info.ret));
        let value = info.is_none_or(|info| !matches!(info.ret, Ty::Unit));

        let (borrowed, borrowed_elements) = if stored {
            (
                std::mem::take(&mut self.borrowed),
                std::mem::take(&mut self.borrowed_elements),
            )
        } else {
            (self.borrowed.clone(), self.borrowed_elements.clone())
        };
        let base = self.closure_base.replace(self.scopes.len());
        self.scopes
            .push(closure.params.iter().map(|p| p.name.name.clone()).collect());
        let body = match &closure.body.kind {
            ExprKind::Block(block) => self.block(block, value),
            _ => format!("{{ {} }}", self.result(&closure.body)),
        };
        self.scopes.pop();
        self.closure_base = base;
        self.borrowed = borrowed;
        self.borrowed_elements = borrowed_elements;

        let arrow = ret
            .as_ref()
            .map_or_else(String::new, |ret| format!(" -> {ret}"));
        if !stored {
            return format!("|{}|{arrow} {body}", params.join(", "));
        }
        let function = format!("move |{}|{arrow} {body}", params.join(", "));
        let arc = if lets.is_empty() {
            format!("std::sync::Arc::new({function})")
        } else {
            format!("std::sync::Arc::new({{ {lets}{function} }})")
        };
        match (info, ret) {
            (Some(info), Some(ret)) => {
                let params: Vec<String> =
                    info.params.iter().map(|(_, t)| self.rust_ty(t)).collect();
                format!("({arc} as {})", fn_value_type(&params, &ret))
            }
            _ => arc,
        }
    }

    /// Whether local `name` is bound outside the closure whose body is
    /// being generated.
    fn is_captured(&self, name: &str) -> bool {
        let Some(base) = self.closure_base else {
            return false;
        };
        let name = name.to_string();
        self.scopes
            .iter()
            .rposition(|scope| scope.contains(&name))
            .is_some_and(|index| index < base)
    }

    /// `Name { f = e, ..base }`: the fields in source order, then `base`.
    fn record_update(
        &mut self,
        expr: &Expr,
        name: &Ident,
        fields: &[FieldInit],
        base: &Expr,
    ) -> String {
        let ty = self.type_of(expr).cloned();
        let texts: Vec<String> = fields
            .iter()
            .map(|f| {
                let value = self.value(&f.value);
                let value = if self.is_boxed_field(ty.as_ref(), &f.name.name) {
                    format!("Box::new({value})")
                } else {
                    value
                };
                format!("{}: {value}", ident(&f.name.name))
            })
            .collect();
        let base = self.value(base);
        let mut parts = texts;
        parts.push(format!("..{base}"));
        format!("{} {{ {} }}", ident(&name.name), parts.join(", "))
    }

    /// A left spine of one operator class (`a + b + c + ..`) of more than
    /// [`CHAIN_LIMIT`] terms as [`Generator::chunked`]; `None` otherwise.
    fn chunked_spine(&mut self, expr: &Expr) -> Option<String> {
        let ExprKind::Binary { op, .. } = &expr.kind else {
            return None;
        };
        let class = fold_class(*op);
        if class == 0 {
            return None;
        }
        let mut rights: Vec<(BinaryOp, &Expr, &Expr)> = Vec::new();
        let mut head = expr;
        while let ExprKind::Binary { op, left, right } = &head.kind
            && fold_class(*op) == class
        {
            rights.push((*op, right, head));
            head = left;
        }
        if rights.len() < CHAIN_LIMIT {
            return None;
        }
        rights.reverse();
        Some(self.chunked(head, &rights, false))
    }

    /// The left-deep chain `head op1 t1 op2 t2 ..` as lets of at most
    /// [`CHAIN_LIMIT`] terms, one per line, each starting from the previous
    /// let's value: evaluation order and the short-circuit of `&&` and
    /// `||` are those of the nested form. Each right has its operator, its
    /// term and the `Binary` node it belongs to (for the node's failure
    /// site: arithmetic with one lowers to a `ignite_io::checked` call).
    /// `text` concatenates Text parts (every op is `+`) into one `String`.
    fn chunked(&mut self, head: &Expr, rights: &[(BinaryOp, &Expr, &Expr)], text: bool) -> String {
        let mut out = String::from("{\n");
        let terms: Vec<(Option<BinaryOp>, &Expr, &Expr)> = std::iter::once((None, head, head))
            .chain(rights.iter().map(|(op, t, node)| (Some(*op), *t, *node)))
            .collect();
        let mut previous: Option<String> = None;
        let mut start = 0;
        let mut index = 0;
        while start < terms.len() {
            let size = match previous {
                None => CHAIN_LIMIT,
                Some(_) => CHAIN_LIMIT - 1,
            };
            let chunk = &terms[start..(start + size).min(terms.len())];
            start += chunk.len();
            let value = if text {
                let start = previous
                    .clone()
                    .unwrap_or_else(|| "String::new()".to_string());
                let mut value = format!("{{ let mut __ignite_s = {start}; ");
                for (_, term, _) in chunk {
                    let part = match &term.kind {
                        ExprKind::Str(text) => format!("\"{}\"", escape_str(text)),
                        _ => format!("&{}", self.bare(term)),
                    };
                    let _ = write!(value, "__ignite_s.push_str({part}); ");
                }
                value.push_str("__ignite_s }");
                value
            } else {
                let uniform = chunk
                    .iter()
                    .filter_map(|(op, _, _)| *op)
                    .all(|op| Some(op) == chunk.iter().find_map(|(op, _, _)| *op));
                let mut value = previous.clone().unwrap_or_default();
                for (op, term, node) in chunk {
                    let term = self.operand(term);
                    value = match op {
                        None => term,
                        Some(op) => match self.checked_op(node, *op) {
                            Some((checked, site)) => format!("{checked}({value}, {term}, {site})"),
                            None if uniform => format!("{value} {} {term}", binary_symbol(*op)),
                            None => format!("({value} {} {term})", binary_symbol(*op)),
                        },
                    };
                }
                value
            };
            let name = format!("__ignite_c{index}");
            let _ = writeln!(out, "let {name} = {value};");
            previous = Some(name);
            index += 1;
        }
        out.push_str(&previous.unwrap_or_default());
        out.push_str("\n}");
        out
    }
}

/// Whether `pattern` holds a range pattern: the type checker splits Int
/// and Byte into intervals for exhaustiveness (`docs/slice.md` section 13),
/// so a match over ranges may have no catch-all arm.
fn has_range(pattern: &Pattern) -> bool {
    match &pattern.kind {
        PatternKind::Range { .. } => true,
        PatternKind::Or(items) | PatternKind::Tuple(items) | PatternKind::List(items) => {
            items.iter().any(has_range)
        }
        PatternKind::Variant { args, .. } => args.iter().any(has_range),
        _ => false,
    }
}

/// The Rust index of element `i` of a list pattern of `count` elements on
/// `subject`: `i` before the rest at `rest` (or without one), counted from
/// the end after it (`docs/slice.md` 20.5).
fn list_index(subject: &str, i: usize, rest: Option<usize>, count: usize) -> String {
    match rest {
        Some(at) if i > at => format!("{subject}.len() - {}", count - i),
        _ => i.to_string(),
    }
}

/// The Rust expression a pattern binding takes its value from: a place
/// read through a reference is cloned, a rest binding's `to_vec()` is
/// already an owned copy.
fn binding_value(access: &str) -> String {
    if access.ends_with(".to_vec()") {
        access.to_string()
    } else {
        format!("({access}).clone()")
    }
}

/// The names `pattern` binds, in order; an uppercase binding may name a
/// variant and is told apart by the caller.
fn pattern_bindings<'p>(pattern: &'p Pattern, out: &mut Vec<&'p Ident>) {
    match &pattern.kind {
        PatternKind::Binding(id) => out.push(id),
        PatternKind::Rest(Some(id)) => out.push(id),
        PatternKind::Or(items) | PatternKind::Tuple(items) | PatternKind::List(items) => {
            items.iter().for_each(|item| pattern_bindings(item, out));
        }
        PatternKind::Variant { args, .. } => {
            args.iter().for_each(|arg| pattern_bindings(arg, out));
        }
        _ => {}
    }
}

/// Whether a value of type `ty` can be read through a reference where
/// the generated code reads a borrowed parameter: the types a `for` over
/// a list place lends its elements as ([`Generator::borrowed_for`]).
fn lendable(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Text
            | Ty::List(_)
            | Ty::Option(_)
            | Ty::Result(..)
            | Ty::Record(_)
            | Ty::Enum(_)
            | Ty::Tuple(_)
            | Ty::GenericRecord(..)
            | Ty::GenericEnum(..)
            | Ty::Map(..)
    )
}

/// Whether the Rust type of `ty` is `Copy`: scalars, quantities, and
/// options, results and tuples of those.
fn rust_copy(ty: &Ty) -> bool {
    match ty {
        Ty::Int | Ty::Bool | Ty::Byte | Ty::Unit | Ty::Never | Ty::Quantity(_) => true,
        Ty::Option(inner) => rust_copy(inner),
        Ty::Result(ok, err) => rust_copy(ok) && rust_copy(err),
        Ty::Tuple(items) => items.iter().all(rust_copy),
        _ => false,
    }
}

/// `a && b && ..`, `true` for none.
fn and_all(tests: Vec<String>) -> String {
    if tests.is_empty() {
        "true".to_string()
    } else {
        tests.join(" && ")
    }
}

/// Builtin methods that change their receiver in place.
const MUTATING_METHODS: &[&str] = &["push", "append", "clear"];

/// The names a block may change in place: the base of every assignment
/// and the root of every receiver of a [`MUTATING_METHODS`] call. A name
/// shadowed by a binding still counts, so the answer can only be too
/// large; a parameter outside the set is never changed by the body.
fn mutated_names(block: &Block, types: Option<&TypedProgram>, out: &mut BTreeSet<String>) {
    for stmt in &block.stmts {
        match &stmt.kind {
            StmtKind::ForPattern { iter, body, .. } => {
                mutated_in(iter, types, out);
                mutated_names(body, types, out);
            }
            StmtKind::LetPattern { value, .. } | StmtKind::Let { value, .. } => {
                mutated_in(value, types, out);
            }
            StmtKind::Assign { target, value } => {
                out.insert(target.base.name.clone());
                mutated_in(value, types, out);
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    mutated_in(value, types, out);
                }
            }
            StmtKind::While { cond, body } => {
                mutated_in(cond, types, out);
                mutated_names(body, types, out);
            }
            StmtKind::Loop { body } => mutated_names(body, types, out),
            StmtKind::For { iter, body, .. } => {
                mutated_in(iter, types, out);
                mutated_names(body, types, out);
            }
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Expr(expr) => mutated_in(expr, types, out),
            StmtKind::Assert { cond, message } => {
                mutated_in(cond, types, out);
                if let Some(message) = message {
                    mutated_in(message, types, out);
                }
            }
        }
    }
}

/// Whether the method call `expr` named `name` changes its receiver in
/// place: a [`MUTATING_METHODS`] name, or `put` and `remove` on a `Map`
/// (known from the types; `Store` has the same names and is a handle).
/// The fields of `std.process.Invocation` and `std.process.ChildExit` in
/// declaration order, as `ignite_io::Invocation` and
/// `ignite_io::ChildExit` name them (`docs/slice.md` 21.5).
const INVOCATION_FIELDS: [&str; 8] = [
    "program",
    "arguments",
    "directory",
    "environment",
    "inherit_environment",
    "timeout",
    "output_limit",
    "input",
];
const CHILD_EXIT_FIELDS: [&str; 5] = ["ending", "code", "stdout", "stderr", "dropped"];

/// The `ignite_io::Invocation` of the generated `std.process.Invocation`
/// value `value`, field by field.
fn substrate_invocation(value: &str) -> String {
    let fields: Vec<String> = INVOCATION_FIELDS
        .iter()
        .map(|f| format!("{f}: i.{}", ident(f)))
        .collect();
    format!(
        "{{ let i = {value}; ignite_io::Invocation {{ {} }} }}",
        fields.join(", ")
    )
}

/// The generated record `record` (`std.process.ChildExit`) of the
/// `ignite_io::ChildExit` named `value`, field by field.
fn record_from_substrate(record: &str, value: &str) -> String {
    let fields: Vec<String> = CHILD_EXIT_FIELDS
        .iter()
        .map(|f| format!("{}: {value}.{f}", ident(f)))
        .collect();
    format!("{record} {{ {} }}", fields.join(", "))
}

fn mutates_receiver(expr: &Expr, name: &str, types: Option<&TypedProgram>) -> bool {
    if MUTATING_METHODS.contains(&name) {
        return true;
    }
    matches!(
        types.and_then(|t| t.callee(expr)),
        Some(Callee::Builtin(Builtin::MapPut | Builtin::MapRemove))
    )
}

fn mutated_in(expr: &Expr, types: Option<&TypedProgram>, out: &mut BTreeSet<String>) {
    match &expr.kind {
        ExprKind::Quantity { .. } => {}
        ExprKind::NamedArg { value, .. } => mutated_in(value, types, out),
        ExprKind::Brace { entries } => {
            entries
                .iter()
                .for_each(|entry| mutated_in(&entry.value, types, out));
        }
        ExprKind::Closure(closure) => mutated_in(&closure.body, types, out),
        ExprKind::RecordUpdate { fields, base, .. } => {
            fields.iter().for_each(|f| mutated_in(&f.value, types, out));
            mutated_in(base, types, out);
        }
        ExprKind::TupleField { receiver, .. } => mutated_in(receiver, types, out),
        ExprKind::Break | ExprKind::Continue => {}
        ExprKind::Int(_) | ExprKind::Str(_) | ExprKind::Bool(_) | ExprKind::Name(_) => {}
        ExprKind::Tuple(items) | ExprKind::List(items) => {
            items.iter().for_each(|item| mutated_in(item, types, out));
        }
        ExprKind::Return(value) => {
            if let Some(value) = value {
                mutated_in(value, types, out);
            }
        }
        ExprKind::Record { fields, .. } => {
            fields.iter().for_each(|f| mutated_in(&f.value, types, out));
        }
        ExprKind::Call { callee, args } => {
            mutated_in(callee, types, out);
            args.iter().for_each(|arg| mutated_in(arg, types, out));
        }
        ExprKind::Method {
            receiver,
            name,
            args,
        } => {
            if mutates_receiver(expr, &name.name, types)
                && let Some(root) = place_root(receiver)
            {
                out.insert(root.to_string());
            }
            mutated_in(receiver, types, out);
            args.iter().for_each(|arg| mutated_in(arg, types, out));
        }
        ExprKind::Field { receiver, .. } => mutated_in(receiver, types, out),
        ExprKind::Index { receiver, index } => {
            mutated_in(receiver, types, out);
            mutated_in(index, types, out);
        }
        ExprKind::Try(inner)
        | ExprKind::Await(inner)
        | ExprKind::Spawn(inner)
        | ExprKind::Unary { operand: inner, .. } => mutated_in(inner, types, out),
        ExprKind::Binary { .. } => {
            let mut head = expr;
            while let ExprKind::Binary { left, right, .. } = &head.kind {
                mutated_in(right, types, out);
                head = left;
            }
            mutated_in(head, types, out);
        }
        ExprKind::Match { scrutinee, arms } => {
            mutated_in(scrutinee, types, out);
            arms.iter()
                .for_each(|arm| mutated_in(&arm.body, types, out));
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            mutated_in(cond, types, out);
            mutated_names(then, types, out);
            if let Some(otherwise) = otherwise {
                mutated_in(otherwise, types, out);
            }
        }
        ExprKind::Block(block) => mutated_names(block, types, out),
    }
}

/// The name a place expression (`x`, `x.a.b`, `x[i]`) starts from.
fn place_root(expr: &Expr) -> Option<&str> {
    match &expr.kind {
        ExprKind::Name(id) => Some(&id.name),
        ExprKind::Field { receiver, .. }
        | ExprKind::TupleField { receiver, .. }
        | ExprKind::Index { receiver, .. } => place_root(receiver),
        _ => None,
    }
}

/// The element types of a tuple of `count` elements, `None` each when the
/// type is not known.
fn tuple_elements(ty: Option<&Ty>, count: usize) -> Vec<Option<Ty>> {
    match ty {
        Some(Ty::Tuple(elements)) if elements.len() == count => {
            elements.iter().cloned().map(Some).collect()
        }
        _ => vec![None; count],
    }
}

fn binary_symbol(op: BinaryOp) -> &'static str {
    op.symbol()
}

/// `==`, `!=`, `<`, `>`, `<=`, `>=`: Rust takes both operands by reference.
fn is_comparison(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge
    )
}

#[derive(Clone)]
struct Listener {
    address: String,
    port: i64,
}

/// The resources of one system.
#[derive(Default)]
struct Resources {
    /// `TcpListener` resources; `None` when the declaration is invalid.
    listeners: BTreeMap<String, Option<Listener>>,
    /// `Store` resources: name and Rust type, in declaration order.
    stores: Vec<(String, String)>,
    /// `Directory`, `Launcher`, `Entropy` and `Setting` resources
    /// (`docs/slice.md` 21.4 to 21.7) with their literal properties.
    capabilities: BTreeMap<String, CapabilityResource>,
}

/// A capability resource as the generated `main` creates it at launch.
#[derive(Clone, Debug)]
enum CapabilityResource {
    Directory {
        path: String,
        create: bool,
        flag: String,
    },
    Launcher {
        maximum_children: i64,
    },
    Entropy,
    Setting {
        flag: String,
        default: Option<String>,
    },
}

impl CapabilityResource {
    /// The creation order of the generated `main` (`docs/slice.md` 21.8):
    /// settings, directories, launchers, entropy.
    fn order(&self) -> u8 {
        match self {
            CapabilityResource::Setting { .. } => 0,
            CapabilityResource::Directory { .. } => 1,
            CapabilityResource::Launcher { .. } => 2,
            CapabilityResource::Entropy => 3,
        }
    }

    /// The Rust expression that creates the capability at launch; `?`
    /// ends the program through `ignite_io::run` on a failure.
    fn launch(&self) -> String {
        match self {
            CapabilityResource::Setting { flag, default } => {
                let default = match default {
                    Some(text) => format!("Some(\"{}\")", escape_str(text)),
                    None => "None".to_string(),
                };
                format!(
                    "ignite_io::Setting::launch(\"{}\", {default})?",
                    escape_str(flag)
                )
            }
            CapabilityResource::Directory { path, create, flag } => format!(
                "ignite_io::Directory::launch(\"{}\", {create}, \"{}\")?",
                escape_str(path),
                escape_str(flag)
            ),
            CapabilityResource::Launcher { maximum_children } => {
                format!("ignite_io::Launcher::with_maximum({maximum_children})")
            }
            CapabilityResource::Entropy => "ignite_io::Entropy::open()?".to_string(),
        }
    }
}

/// `resource name: Directory { .. }` and the other capability kinds: the
/// literal properties the type checker validated (E104), defaults filled.
fn lower_capability(resource: &ResourceDecl) -> Option<CapabilityResource> {
    let text = |name: &str| {
        resource
            .properties
            .iter()
            .find(|p| p.name.name == name)
            .and_then(|p| match &p.value.kind {
                ExprKind::Str(text) => Some(text.clone()),
                _ => None,
            })
    };
    let property = |name: &str| resource.properties.iter().find(|p| p.name.name == name);
    Some(match resource.kind.name.as_str() {
        "Directory" => CapabilityResource::Directory {
            path: text("path").unwrap_or_default(),
            create: property("create")
                .is_some_and(|p| matches!(p.value.kind, ExprKind::Bool(true))),
            flag: text("flag").unwrap_or_default(),
        },
        "Launcher" => CapabilityResource::Launcher {
            maximum_children: property("maximum_children")
                .and_then(|p| match p.value.kind {
                    ExprKind::Int(n) => Some(n),
                    _ => None,
                })
                .unwrap_or(ignite_types::LAUNCHER_DEFAULT_MAXIMUM_CHILDREN),
        },
        "Entropy" => CapabilityResource::Entropy,
        "Setting" => CapabilityResource::Setting {
            flag: text("flag").unwrap_or_default(),
            default: text("default"),
        },
        _ => return None,
    })
}

fn e020(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("E020", span, message)
}

/// The resources of one system, validated.
fn lower_resources(system: &SystemDecl, diagnostics: &mut Vec<Diagnostic>) -> Resources {
    let mut resources = Resources::default();
    for resource in &system.resources {
        let name = resource.name.name.clone();
        match resource.kind.name.as_str() {
            "TcpListener" => {
                let listener = lower_resource(resource, diagnostics);
                resources.listeners.insert(name, listener);
            }
            "Store" => {
                if let Some(ty) = lower_store(resource, diagnostics) {
                    resources.stores.push((name, ty));
                }
            }
            other => match lower_capability(resource) {
                Some(capability) => {
                    resources.capabilities.insert(name, capability);
                }
                None => diagnostics.push(e020(
                    resource.kind.span,
                    format!(
                        "unsupported resource kind `{other}`; expected `TcpListener`, `Store`, `Directory`, `Launcher`, `Entropy` or `Setting`"
                    ),
                )),
            },
        }
    }
    resources
}

/// `resource name: Store<T>`: the Rust type `ignite_io::Store<T>`. A store
/// takes exactly one type argument and no properties.
fn lower_store(resource: &ResourceDecl, diagnostics: &mut Vec<Diagnostic>) -> Option<String> {
    let before = diagnostics.len();
    if resource.args.len() != 1 {
        diagnostics.push(e020(
            resource.kind.span,
            format!(
                "resource kind `Store` takes one type argument, `{}` has {}",
                resource.name.name,
                resource.args.len()
            ),
        ));
    }
    for property in &resource.properties {
        diagnostics.push(e020(
            property.name.span,
            format!(
                "unsupported property `{}` on resource kind `Store`",
                property.name.name
            ),
        ));
    }
    (diagnostics.len() == before).then(|| {
        rust_type(&Type {
            name: resource.kind.clone(),
            args: resource.args.clone(),
            span: resource.kind.span,
            fn_names: Vec::new(),
            requires: None,
        })
    })
}

fn lower_resource(resource: &ResourceDecl, diagnostics: &mut Vec<Diagnostic>) -> Option<Listener> {
    if !resource.args.is_empty() {
        diagnostics.push(e020(
            resource.kind.span,
            "resource kind `TcpListener` takes no type arguments",
        ));
        return None;
    }
    let mut address = None;
    let mut port = None;
    let mut valid = true;
    for property in &resource.properties {
        match (property.name.name.as_str(), &property.value.kind) {
            ("address", ExprKind::Str(text)) if address.is_none() => address = Some(text.clone()),
            ("port", ExprKind::Int(value)) if port.is_none() => port = Some(*value),
            ("address" | "port" | "exposure" | "backlog", _)
                if is_duplicate(&resource.properties, property) =>
            {
                diagnostics.push(e020(
                    property.name.span,
                    format!("duplicate property `{}`", property.name.name),
                ));
                valid = false;
            }
            ("exposure", ExprKind::Name(_) | ExprKind::Field { .. }) => {}
            ("backlog", ExprKind::Int(_)) => {}
            ("exposure", _) => {
                diagnostics.push(e020(
                    property.value.span,
                    "property `exposure` must be an `Exposure` variant",
                ));
                valid = false;
            }
            ("backlog", _) => {
                diagnostics.push(e020(
                    property.value.span,
                    "property `backlog` must be an integer literal",
                ));
                valid = false;
            }
            ("address", _) => {
                diagnostics.push(e020(
                    property.value.span,
                    "property `address` must be a string literal",
                ));
                valid = false;
            }
            ("port", _) => {
                diagnostics.push(e020(
                    property.value.span,
                    "property `port` must be an integer literal",
                ));
                valid = false;
            }
            (other, _) => {
                diagnostics.push(e020(
                    property.name.span,
                    format!("unsupported property `{other}` on resource kind `TcpListener`"),
                ));
                valid = false;
            }
        }
    }
    for (name, missing) in [("address", address.is_none()), ("port", port.is_none())] {
        if missing && !resource.properties.iter().any(|p| p.name.name == name) {
            diagnostics.push(e020(
                resource.name.span,
                format!("resource `{}` has no `{name}`", resource.name.name),
            ));
            valid = false;
        }
    }
    match (valid, address, port) {
        (true, Some(address), Some(port)) => Some(Listener { address, port }),
        _ => None,
    }
}

fn is_duplicate(properties: &[FieldInit], property: &FieldInit) -> bool {
    properties
        .iter()
        .position(|p| p.name.name == property.name.name)
        .is_some_and(|first| !std::ptr::eq(&properties[first], property))
}

/// A Rust variable for a resource, kept apart from the names `main` binds,
/// escaped one to one as [`ident`] escapes `self`: `port` is `port_`,
/// `port_` is `port__`.
fn resource_variable(name: &str) -> String {
    if escapes_to(name, |base| {
        matches!(base, "application" | "port" | "stream" | "error")
    }) {
        format!("{name}_")
    } else {
        ident(name)
    }
}

/// The Rust variable holding the `Endpoint` value of a `TcpListener`
/// resource used as a target.
fn endpoint_variable(name: &str) -> String {
    format!("{}_endpoint", resource_variable(name))
}

/// `src/bin/<executable>.rs` of an executable component (`docs/slice.md`
/// section 14.5): under `ignite_io::run`, the `Store` resources the
/// component uses, its `TcpListener` resources (the first with the
/// `--port` override, each with the `listening on` log), the `Endpoint`
/// values of listeners it names as targets (`--port` applies to them too),
/// the embedded components' values in dependency order, the component's
/// own value, then `K.main(value)`, whose `Err` `run` maps to its exit
/// code.
#[allow(clippy::too_many_arguments)]
fn executable_main(
    crate_name: &str,
    uses: &[UseTarget],
    program: &Program,
    symbols: &Symbols,
    types: &TypedProgram,
    source: Option<&SourceFile>,
    info: &ExecutableInfo,
    system: &SystemDecl,
    resources: &Resources,
    package: &Package,
) -> Result<String, Diagnostic> {
    let Some(decl) = system
        .components
        .iter()
        .find(|c| c.name.name == info.component)
    else {
        return Err(e020(
            system.name.span,
            format!(
                "component `{}` is not declared in system `{}`",
                info.component, info.system
            ),
        ));
    };
    let mut head = Writer::new();
    head.raw(FILE_HEADER);
    head.line(&format!("use {crate_name}::*;"));
    let mut w = Writer::new();
    w.line("");
    w.open("fn main() {");
    w.open("std::process::exit(ignite_io::run(|| {");

    let mut generator = Generator::new(symbols, program, Some(types), &package.module, source);
    generator.indent = w.indent;
    generator.package = package.clone();
    let mut scope = Vec::new();
    for name in system
        .resources
        .iter()
        .map(|r| &r.name.name)
        .chain(system.components.iter().map(|c| &c.name.name))
    {
        scope.push(name.clone());
        generator
            .renames
            .insert(name.clone(), resource_variable(name));
    }
    generator.scopes = vec![scope];

    let mut defined: BTreeSet<String> = BTreeSet::new();
    for (name, ty) in &info.resources {
        if let Ty::Store(_) = ty
            && defined.insert(name.clone())
        {
            w.line(&format!(
                "let {}: {} = ignite_io::Store::new();",
                resource_variable(name),
                generator.rust_ty(ty)
            ));
        }
    }
    let mut capabilities: Vec<(&str, &CapabilityResource)> = info
        .resources
        .iter()
        .filter(|(_, ty)| matches!(ty, Ty::Capability(_)))
        .filter_map(|(name, _)| {
            resources
                .capabilities
                .get(name)
                .map(|capability| (name.as_str(), capability))
        })
        .collect();
    capabilities.sort_by_key(|(_, capability)| capability.order());
    for (name, capability) in capabilities {
        if defined.insert(name.to_string()) {
            w.line(&format!(
                "let {} = {};",
                resource_variable(name),
                capability.launch()
            ));
        }
    }
    let listener_of = |name: &str| -> Result<Listener, Diagnostic> {
        match resources.listeners.get(name) {
            Some(Some(listener)) => Ok(listener.clone()),
            _ => Err(e020(
                decl.span,
                format!(
                    "resource `{name}` is not a `TcpListener` of system `{}`",
                    info.system
                ),
            )),
        }
    };
    let mut listeners: Vec<&str> = Vec::new();
    let mut endpoints: Vec<(&str, &Ty)> = Vec::new();
    for (name, ty) in &info.resources {
        match ty {
            Ty::TcpListener if !listeners.contains(&name.as_str()) => listeners.push(name),
            Ty::Record(_) | Ty::GenericRecord(..)
                if resources.listeners.contains_key(name)
                    && !endpoints.iter().any(|(n, _)| *n == name) =>
            {
                endpoints.push((name, ty));
            }
            _ => {}
        }
    }
    if !listeners.is_empty() || !endpoints.is_empty() {
        w.line("let port = ignite_io::args::port()?;");
    }
    for (index, name) in listeners.iter().enumerate() {
        let listener = listener_of(name)?;
        let variable = resource_variable(name);
        let address = escape_str(&listener.address);
        let address_format = escape_str(&listener.address.replace('{', "{{").replace('}', "}}"));
        let port = if index == 0 {
            format!("port.unwrap_or({})", listener.port)
        } else {
            listener.port.to_string()
        };
        w.line(&format!(
            "let {variable} = ignite_io::TcpListener::bind(String::from(\"{address}\"), {port})?;"
        ));
        w.line(&format!(
            "ignite_io::log(&format!(\"listening on {address_format}:{{}}\", {variable}.local_port()));"
        ));
        defined.insert((*name).to_string());
    }
    for (name, ty) in &endpoints {
        let listener = listener_of(name)?;
        w.line(&format!(
            "let {} = {} {{ address: String::from(\"{}\"), port: port.unwrap_or({}) }};",
            endpoint_variable(name),
            generator.type_path(ty),
            escape_str(&listener.address),
            listener.port
        ));
    }
    for (name, ty) in info
        .embedded
        .iter()
        .chain([(info.component.clone(), info.kind.clone())].iter())
    {
        let Some(component) = system.components.iter().find(|c| c.name.name == *name) else {
            return Err(e020(
                decl.span,
                format!(
                    "component `{name}` is not declared in system `{}`",
                    info.system
                ),
            ));
        };
        let value = component_value(&mut generator, &info.system, component, ty, resources);
        w.line(&format!("let {} = {value};", resource_variable(name)));
        defined.insert(name.clone());
    }
    if let Some(name) = generator
        .renamed_uses
        .iter()
        .find(|name| !defined.contains(*name) && !endpoints.iter().any(|(n, _)| n == name))
    {
        return Err(e020(
            decl.span,
            format!(
                "`{name}` is not a resource or component that component `{}` uses",
                info.component
            ),
        ));
    }
    let borrow = if generator.consumes(Some(&Callee::Fn(info.main.clone())), 0) {
        ""
    } else {
        "&"
    };
    w.line(&format!(
        "{}({borrow}{})",
        generator.fn_path(&info.main),
        resource_variable(&info.component)
    ));
    w.close("}))");
    w.close("}");
    w.raw(&generator.glue_items());
    let body = w.finish();
    for line in kept_use_lines(uses, symbols, &body) {
        head.line(&line);
    }
    head.raw(&body);
    Ok(head.finish())
}

/// The record value of component `decl` of kind `kind` (`docs/slice.md`
/// section 14.4): its properties as fields, with a `TcpListener` resource
/// named where a record (`Endpoint`) is expected replaced by the
/// endpoint value, an omitted `executable` set to the component's name,
/// an omitted `Signal` output set to its reference text, and the kind's
/// defaults for every other omitted field.
fn component_value(
    generator: &mut Generator<'_>,
    system: &str,
    decl: &ComponentDecl,
    kind: &Ty,
    resources: &Resources,
) -> String {
    let declared = generator.fields_of(kind).unwrap_or_default();
    let mut fixed: BTreeMap<String, String> = BTreeMap::new();
    for (field, ty, _) in &declared {
        let given = decl.properties.iter().any(|p| p.name.name == *field);
        if given {
            continue;
        }
        if field == "executable" {
            fixed.insert(
                field.clone(),
                format!("String::from(\"{}\")", escape_str(&decl.name.name)),
            );
        } else if let Ty::Signal(_) = ty {
            fixed.insert(
                field.clone(),
                format!(
                    "String::from(\"{}\")",
                    escape_str(&format!("{system}.{}.{field}", decl.name.name))
                ),
            );
        }
    }
    for property in &decl.properties {
        if let ExprKind::Name(id) = &property.value.kind
            && resources.listeners.contains_key(&id.name)
            && declared.iter().any(|(field, ty, _)| {
                *field == property.name.name && matches!(ty, Ty::Record(_) | Ty::GenericRecord(..))
            })
        {
            fixed.insert(
                property.name.name.clone(),
                format!("{}.clone()", endpoint_variable(&id.name)),
            );
        }
    }
    let fields: Vec<(String, &Expr)> = decl
        .properties
        .iter()
        .map(|p| (p.name.name.clone(), &p.value))
        .collect();
    generator.typed_record_literal(kind, &fields, &fixed)
}

fn toml_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn cargo_toml(
    package_name: &str,
    executables: &[Executable],
    substrate: &Path,
    uses_std: bool,
    feature: Option<&str>,
) -> String {
    let substrate = substrate.to_string_lossy().replace('\\', "/");
    let mut out = String::new();
    out.push_str("[package]\n");
    let _ = writeln!(out, "name = {}", toml_string(package_name));
    out.push_str("version = \"0.0.0\"\n");
    out.push_str("edition = \"2024\"\n");
    out.push_str("\n[workspace]\n");
    out.push_str("\n[profile.release]\noverflow-checks = true\n");
    out.push_str("\n[lib]\n");
    let lib = crate_name(package_name);
    if lib != package_name.replace('-', "_") {
        let _ = writeln!(out, "name = {}", toml_string(&lib));
    }
    out.push_str("path = \"src/lib.rs\"\n");
    for executable in executables {
        out.push_str("\n[[bin]]\n");
        let _ = writeln!(out, "name = {}", toml_string(&executable.name));
        let _ = writeln!(
            out,
            "path = {}",
            toml_string(&format!("src/bin/{}.rs", executable.name))
        );
    }
    out.push_str("\n[dependencies]\n");
    out.push_str(&ignite_io_dependency(&substrate, feature));
    if uses_std {
        let _ = writeln!(out, "{STD_PACKAGE} = {{ path = \"std\" }}");
    }
    out
}

/// FNV-1a, 64 bit: stable across Rust versions, unlike `DefaultHasher`.
fn content_hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// `std/Cargo.toml`. No `[workspace]`: the crate lives inside the program's
/// project directory and is part of its workspace.
///
/// The version carries a hash of `lib.rs`. Cargo derives an artifact's
/// name from the package name, version and workspace-relative path, which
/// are the same for every generated `std/`; without the hash, two programs
/// built into one `--target-dir` share one `ignite-std` artifact, and the
/// one written earlier counts as up to date. The hash follows an `h`: a
/// pre-release identifier of digits only may not start with `0` (SemVer
/// 2.0.0 item 9), and a hash whose 16 hex digits are all decimal would be one.
fn std_cargo_toml(substrate: &Path, lib_rs: &str) -> String {
    let substrate = substrate.to_string_lossy().replace('\\', "/");
    let mut out = String::new();
    out.push_str("[package]\n");
    let _ = writeln!(out, "name = {}", toml_string(STD_PACKAGE));
    let _ = writeln!(out, "version = \"0.0.0-h{:016x}\"", content_hash(lib_rs));
    out.push_str("edition = \"2024\"\n");
    out.push_str("\n[lib]\n");
    out.push_str("path = \"src/lib.rs\"\n");
    out.push_str("\n[dependencies]\n");
    out.push_str(&ignite_io_dependency(&substrate, None));
    out
}

/// The `ignite-io` dependency line, with `feature` enabled when given.
fn ignite_io_dependency(substrate: &str, feature: Option<&str>) -> String {
    match feature {
        Some(feature) => format!(
            "ignite-io = {{ path = {}, features = [{}] }}\n",
            toml_string(substrate),
            toml_string(feature)
        ),
        None => format!("ignite-io = {{ path = {} }}\n", toml_string(substrate)),
    }
}
