//! Rust target (`docs/slice.md` section 6). Lowers checked Telos modules to
//! two Cargo projects: the standard library as the `telos-std` crate and the
//! program as a crate that depends on it and on the substrate `telos-io`.
//!
//! Contract:
//!
//! * [`generate`] and [`generate_std`] are pure: they return source text and
//!   write nothing. [`write_project`] writes one project.
//! * The lowering table, the ownership rule (`.clone()` on places in value
//!   positions), the builtin method table, the system lowering (6.1) and the
//!   project layout (6.2) in section 6 are the specification; the generated
//!   text for each row is the Rust shown there.
//! * Diagnostics it owns: `E020` unsupported or malformed resource or
//!   component (also: a `system` in a standard-library module, two
//!   components with one executable name), `E030` a component's protocol
//!   has no role for the component kind (`serve` for `Service`, `connect`
//!   for `Client`).
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
//! `Process`, `IoError`: `telos_io::Owner::name(..)`, except
//! `Process.exit(c)`, which is the diverging `telos_io::exit`), a builtin static, a
//! protocol role (`P::name(..)`) or a variant constructor. A protocol is
//! known from `Symbols::protocols` or from the module's own `protocol`
//! declarations; a constant from `Symbols::constants` or the module's own
//! `const` declarations. A constant is a zero-argument fn, so a non-local
//! name that is a constant is emitted as `N()` and is never a place.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use telos_check::{BUILTIN_ENUMS, BUILTIN_STATICS, Symbols};
use telos_syntax::ast::{
    AliasDecl, BinaryOp, Block, ComponentDecl, ConstDecl, Decl, EnumDecl, Expr, ExprKind,
    FieldInit, FnDecl, Ident, Import, Pattern, PatternKind, Program, ProtocolDecl, RecordDecl,
    ResourceDecl, Span, Stmt, StmtKind, SystemDecl, TestDecl, Type, UnaryOp,
};
use telos_syntax::diagnostic::{Diagnostic, SourceFile};
use telos_types::{Builtin, Callee, QualName, Show, TestInfo, TestTry, Ty, TypedProgram};

/// Header of every generated Rust file.
pub const FILE_HEADER: &str = "#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]\n";

/// Header added before [`FILE_HEADER`] in a test build: compiler warnings
/// about generated code are noise in a test report.
pub const TEST_HEADER: &str = "#![allow(warnings)]\n";

/// Cargo package name of the generated standard library.
pub const STD_PACKAGE: &str = "telos-std";

/// Substrate types whose statics lower to `telos_io::<Owner>::<name>(..)`.
const SUBSTRATE_OWNERS: &[&str] = &[
    "TcpListener",
    "TcpStream",
    "Stdin",
    "Stdout",
    "Process",
    "IoError",
];

/// Modules that are the substrate: importing from them emits nothing.
const SUBSTRATE_MODULES: &[&str] = &["std.net", "std.io"];

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Options {
    /// Path written into the generated `Cargo.toml` as the `telos-io` dependency.
    pub substrate_path: PathBuf,
    /// A test build (`docs/slice.md` section 12.6) when not empty: the
    /// module paths whose tests and test-only items are lowered. Every
    /// other module drops its test-only declarations.
    pub test_modules: Vec<String>,
    /// The tests of the `telos-test` executable, in run order. Its entry
    /// for a test is `<module's Rust path>::__telos_test_<decl>`.
    pub tests: Vec<TestInfo>,
    /// Each module's source by module path: a failed `assert` or `?`
    /// prints the condition's text and a position from it.
    pub sources: BTreeMap<String, SourceFile>,
}

/// Name of the test executable of a test build.
pub const TEST_EXECUTABLE: &str = "telos-test";

/// The Rust fn of test `decl` (the index of its `Decl::Test`).
pub fn test_fn_name(decl: usize) -> String {
    format!("__telos_test_{decl}")
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
    /// Cargo package name: the last segment of the Telos package, or `program`.
    pub package_name: String,
    pub cargo_toml: String,
    pub lib_rs: String,
    pub executables: Vec<Executable>,
}

/// Lower a checked program. `symbols` is what `check_module` returned for
/// it, including the names it imports. The project depends on `telos-std`
/// at `std/` when the program imports a standard-library module other than
/// `std.net` and `std.io`.
pub fn generate(
    program: &Program,
    symbols: &Symbols,
    options: &Options,
) -> Result<GeneratedProject, Vec<Diagnostic>> {
    generate_program(program, symbols, None, options)
}

/// [`generate`] with the program's types (`telos_types::type_module` of
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
    generate_program(program, symbols, Some(types), options)
}

fn generate_program(
    program: &Program,
    symbols: &Symbols,
    types: Option<&TypedProgram>,
    options: &Options,
) -> Result<GeneratedProject, Vec<Diagnostic>> {
    let package_name = package_name(program);
    let crate_name = package_name.replace('-', "_");

    let mut diagnostics = Vec::new();
    let mut components = Vec::new();
    for decl in &program.decls {
        if let Decl::System(system) = decl {
            components.extend(lower_system(program, symbols, system, &mut diagnostics));
        }
    }
    let mut executables_seen: BTreeSet<&str> = BTreeSet::new();
    for component in &components {
        if !executables_seen.insert(&component.executable) {
            diagnostics.push(e020(
                component.span,
                format!(
                    "executable `{}` is declared by more than one component",
                    component.executable
                ),
            ));
        }
    }
    if !diagnostics.is_empty() {
        diagnostics.sort_by_key(|d| (d.span.start, d.code));
        return Err(diagnostics);
    }

    let uses = use_lines(&program.imports, "telos_std");
    let mut lib = Writer::new();
    if !options.test_modules.is_empty() {
        lib.raw(TEST_HEADER);
    }
    lib.raw(FILE_HEADER);
    let module = program_module(program, types);
    lib.raw(&module_items(
        program,
        symbols,
        types,
        &uses,
        "telos_std",
        &ModuleOptions::of(options, &module),
    ));

    let testing = !options.test_modules.is_empty();
    let mut executables = Vec::new();
    for component in &components {
        match component_main(&crate_name, &uses, program, symbols, types, component) {
            Ok(_) if testing => {}
            Ok(source) => executables.push(Executable {
                name: component.executable.clone(),
                source,
            }),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    if testing {
        executables.push(Executable {
            name: TEST_EXECUTABLE.to_string(),
            source: test_main(&crate_name, &module, &options.tests),
        });
    }

    let lib_rs = lib.finish();
    let feature = testing.then_some("runner");
    let cargo_toml = cargo_toml(
        &package_name,
        &executables,
        &options.substrate_path,
        testing || imports_std(&program.imports),
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

/// Lower the standard library to the `telos-std` crate: one nested
/// `pub mod` per module (`std.protocol.telnet` is
/// `pub mod protocol { pub mod telnet { .. } }`); imports become
/// `use crate::..`. Errors are grouped by module path.
pub fn generate_std(
    modules: &[StdModule<'_>],
    options: &Options,
) -> Result<GeneratedProject, Vec<(String, Vec<Diagnostic>)>> {
    let modules: Vec<_> = modules
        .iter()
        .map(|(path, program, symbols)| (path.clone(), *program, *symbols, None))
        .collect();
    std_project(&modules, options)
}

/// A [`StdModule`] with its types.
pub type TypedStdModule<'a> = (String, &'a Program, &'a Symbols, &'a TypedProgram);

/// [`generate_std`] with each module's types (see [`generate_typed`]).
pub fn generate_std_typed(
    modules: &[TypedStdModule<'_>],
    options: &Options,
) -> Result<GeneratedProject, Vec<(String, Vec<Diagnostic>)>> {
    let modules: Vec<_> = modules
        .iter()
        .map(|(path, program, symbols, types)| (path.clone(), *program, *symbols, Some(*types)))
        .collect();
    std_project(&modules, options)
}

type StdInput<'a> = (String, &'a Program, &'a Symbols, Option<&'a TypedProgram>);

fn std_project(
    modules: &[StdInput<'_>],
    options: &Options,
) -> Result<GeneratedProject, Vec<(String, Vec<Diagnostic>)>> {
    let mut errors = Vec::new();
    let mut tree = ModuleTree::default();
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
        let uses = use_lines(&program.imports, "crate");
        let segments: Vec<&str> = match path.strip_prefix("std.") {
            Some(rest) => rest.split('.').collect(),
            None if path == "std" => Vec::new(),
            None => path.split('.').collect(),
        };
        tree.insert(
            &segments,
            module_items(
                program,
                symbols,
                *types,
                &uses,
                "crate",
                &ModuleOptions::of(options, path),
            ),
        );
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut lib = String::new();
    if !options.test_modules.is_empty() {
        lib.push_str(TEST_HEADER);
    }
    lib.push_str(FILE_HEADER);
    tree.render(&mut lib, 0);
    Ok(GeneratedProject {
        package_name: STD_PACKAGE.to_string(),
        cargo_toml: std_cargo_toml(&options.substrate_path, &lib),
        lib_rs: lib,
        executables: Vec::new(),
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

fn imports_std(imports: &[Import]) -> bool {
    imports
        .iter()
        .filter_map(import_target)
        .any(|(module, _)| is_std_file_import(&module))
}

/// `use <root>::a::b::{X, Y};` per standard-library import; nothing for the
/// substrate modules or for imports outside `std`.
fn use_lines(imports: &[Import], root: &str) -> Vec<String> {
    imports
        .iter()
        .filter_map(import_target)
        .filter(|(module, _)| is_std_file_import(module))
        .map(|(module, names)| {
            let mut path = root.to_string();
            for segment in &module[1..] {
                path.push_str("::");
                path.push_str(&ident(segment));
            }
            let names: Vec<String> = names.iter().map(|n| ident(n)).collect();
            match names.as_slice() {
                [single] => format!("use {path}::{single};"),
                _ => format!("use {path}::{{{}}};", names.join(", ")),
            }
        })
        .collect()
}

/// The `use` lines and every record, enum, protocol and fn of one module,
/// without the file header. `root` is the Rust path of the `telos-std`
/// crate from this module: `telos_std` in a program, `crate` inside it.
fn module_items(
    program: &Program,
    symbols: &Symbols,
    types: Option<&TypedProgram>,
    uses: &[String],
    root: &'static str,
    module: &ModuleOptions<'_>,
) -> String {
    let mut generator = Generator::new(symbols, program, types);
    generator.root = root;
    generator.source = module.source;
    generator.module = module.path.to_string();
    let mut w = Writer::new();
    for line in uses {
        w.line(line);
    }
    for (index, decl) in program.decls.iter().enumerate() {
        if decl.is_test_only() && !module.tests {
            continue;
        }
        match decl {
            Decl::Alias(alias) => generator.alias(&mut w, alias),
            Decl::Record(record) => generator.record(&mut w, record),
            Decl::Enum(decl) => generator.enumeration(&mut w, decl),
            Decl::Fn(decl) => generator.function(&mut w, decl),
            Decl::Protocol(decl) => generator.protocol(&mut w, decl),
            Decl::Const(decl) => generator.constant(&mut w, decl),
            Decl::Test(test) => generator.test_decl(&mut w, test, index),
            Decl::System(_) => {}
        }
    }
    w.finish()
}

/// What [`module_items`] needs to know about one module beyond its AST.
struct ModuleOptions<'a> {
    path: &'a str,
    /// Lower its tests and test-only items (a module selected for a test
    /// build).
    tests: bool,
    source: Option<&'a SourceFile>,
}

impl<'a> ModuleOptions<'a> {
    fn of(options: &'a Options, path: &'a str) -> ModuleOptions<'a> {
        ModuleOptions {
            path,
            tests: options.test_modules.iter().any(|m| m == path),
            source: options.sources.get(path),
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
    let mut path = String::from("telos_std");
    for segment in module.split('.').skip(1) {
        path.push_str("::");
        path.push_str(&ident(segment));
    }
    path
}

/// `src/bin/telos-test.rs`: [`telos_io::test_main`] over `tests`.
fn test_main(crate_name: &str, program: &str, tests: &[TestInfo]) -> String {
    let mut w = Writer::new();
    w.raw(TEST_HEADER);
    w.raw(FILE_HEADER);
    w.line("");
    w.open("fn main() {");
    w.open("telos_io::test_main(&[");
    for test in tests {
        w.line(&format!(
            "telos_io::Test {{ name: \"{}\", run: &{}::{} }},",
            escape_str(&test.full_name),
            module_rust_path(crate_name, program, &test.module),
            test_fn_name(test.decl)
        ));
    }
    w.close("])");
    w.close("}");
    w.finish()
}

/// Write `Cargo.toml`, `src/lib.rs` and `src/bin/*.rs` under `out`, creating
/// directories as needed and overwriting existing files. Also used for the
/// standard library, written to `<out>/std`. A file that already holds the
/// text is left alone, so cargo does not rebuild an unchanged project.
pub fn write_project(project: &GeneratedProject, out: &Path) -> io::Result<()> {
    fs::create_dir_all(out.join("src"))?;
    write_if_changed(out.join("Cargo.toml"), &project.cargo_toml)?;
    write_if_changed(out.join("src").join("lib.rs"), &project.lib_rs)?;
    if !project.executables.is_empty() {
        let bin = out.join("src").join("bin");
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

const RUST_ONLY_KEYWORDS: &[&str] = &[
    "as", "async", "const", "dyn", "extern", "impl", "mod", "move", "mut", "pub", "ref", "static",
    "struct", "trait", "unsafe", "use", "where", "abstract", "become", "box", "do", "final",
    "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen",
];

/// A Telos identifier as a Rust identifier.
fn ident(name: &str) -> String {
    if matches!(name, "self" | "Self" | "crate" | "super") {
        format!("{name}_")
    } else if RUST_ONLY_KEYWORDS.contains(&name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

/// The associated fn of a record holding the default of `field`.
fn default_fn(field: &str) -> String {
    format!("default_{field}")
}

/// The Rust path of a variant of `owner`.
fn variant_path(owner: &str, variant: &str) -> String {
    match owner {
        "Option" | "Result" => variant.to_string(),
        "Input" => format!("telos_io::Input::{variant}"),
        _ => format!("{}::{}", ident(owner), ident(variant)),
    }
}

/// A Telos type as Rust.
pub fn rust_type(ty: &Type) -> String {
    if ty.is_fn() {
        let params: Vec<String> = ty.fn_params().iter().map(rust_type).collect();
        let ret = ty.fn_ret().map_or_else(|| "()".to_string(), rust_type);
        return format!("fn({}) -> {ret}", params.join(", "));
    }
    let args: Vec<String> = ty.args.iter().map(rust_type).collect();
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
        "TcpListener" | "TcpStream" | "Stdin" | "Stdout" | "Input" | "IoError" => {
            format!("telos_io::{}", ty.name.name)
        }
        "Store" => generic("telos_io::Store"),
        other => generic(&ident(other)),
    }
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
        telos_syntax::ast::ExprKind::Closure(_) | telos_syntax::ast::ExprKind::RecordUpdate { .. } | telos_syntax::ast::ExprKind::TupleField { .. } | telos_syntax::ast::ExprKind::Break | telos_syntax::ast::ExprKind::Continue => unimplemented!("slice 9: closures, record update, tuple fields, break and continue expressions"),
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
    /// Rust path of the `telos-std` crate (`telos_std` or `crate`), for
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
    /// The source the module was read from, for `assert` and `?` reports.
    source: Option<&'a SourceFile>,
    /// The module path, the position of a report without a source.
    module: String,
}

impl<'a> Generator<'a> {
    fn new(
        symbols: &'a Symbols,
        program: &Program,
        types: Option<&'a TypedProgram>,
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
            root: "telos_std",
            renames: BTreeMap::new(),
            renamed_uses: BTreeSet::new(),
            borrowed: BTreeSet::new(),
            source: None,
            module: String::new(),
        }
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

    /// Whether local `name` is a parameter passed by reference, not
    /// shadowed.
    fn is_borrowed(&self, name: &str) -> bool {
        self.borrowed.contains(name) && self.is_outermost(name)
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
    fn ref_arg(&mut self, arg: &Expr) -> String {
        if let ExprKind::Name(id) = &arg.kind
            && self.is_borrowed(&id.name)
        {
            return self.name(id);
        }
        if self.is_place(arg) {
            return format!("&{}", self.bare(arg));
        }
        let text = self.value(arg);
        borrow(arg, text)
    }

    /// A value in a result position (the tail of a value block, an arm, a
    /// branch): a place reached through a borrowed parameter is copied,
    /// since it cannot be moved out; anything else as written.
    fn result(&mut self, expr: &Expr) -> String {
        if self.is_place(expr) && place_root(expr).is_some_and(|root| self.is_borrowed(root)) {
            return self.value(expr);
        }
        self.bare(expr)
    }

    /// A fn of the module or an imported one used as a value: a closure
    /// that borrows its arguments for the fn, cast to the fn pointer type
    /// (`(|__a0: i64| f(&__a0)) as fn(i64) -> i64`). Without types the
    /// closure is left to inference.
    fn fn_value(&mut self, expr: &Expr, id: &Ident) -> String {
        let target = format!("{}{}", self.name(id), self.turbofish(expr));
        let fn_ty = match self.type_of(expr) {
            Some(Ty::Fn(params, ret) | Ty::EffectFn(params, ret, _)) => {
                Some((params.clone(), (**ret).clone()))
            }
            _ => None,
        };
        let arity = match &fn_ty {
            Some((params, _)) => params.len(),
            None => match self.symbols.functions.get(&id.name) {
                Some(info) => info.arity,
                None => return target,
            },
        };
        let names: Vec<String> = (0..arity).map(|i| format!("__a{i}")).collect();
        let refs: Vec<String> = names.iter().map(|n| format!("&{n}")).collect();
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
                    "((|{}| {call}) as fn({}) -> {})",
                    typed.join(", "),
                    types.join(", "),
                    self.rust_ty(&ret)
                )
            }
            None => format!("(|{}| {call})", names.join(", ")),
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
            Ty::Int => "i64".to_string(),
            Ty::Bool => "bool".to_string(),
            Ty::Byte => "u8".to_string(),
            Ty::Unit | Ty::Never => "()".to_string(),
            Ty::Text => "String".to_string(),
            Ty::List(item) => format!("Vec<{}>", self.rust_ty(item)),
            Ty::Option(item) => format!("Option<{}>", self.rust_ty(item)),
            Ty::Result(ok, err) => format!("Result<{}, {}>", self.rust_ty(ok), self.rust_ty(err)),
            Ty::Record(q) | Ty::Enum(q) => self.enum_name(q),
            Ty::GenericRecord(q, args) | Ty::GenericEnum(q, args) => {
                format!("{}<{}>", self.enum_name(q), list(args))
            }
            Ty::Fn(params, ret) | Ty::EffectFn(params, ret, _) => {
                format!("fn({}) -> {}", list(params), self.rust_ty(ret))
            }
            Ty::Tuple(items) => format!("({})", list(items)),
            Ty::TcpListener => "telos_io::TcpListener".to_string(),
            Ty::TcpStream => "telos_io::TcpStream".to_string(),
            Ty::IoError => "telos_io::IoError".to_string(),
            Ty::Input => "telos_io::Input".to_string(),
            Ty::Param(name) => ident(name),
            Ty::Store(item) => format!("telos_io::Store<{}>", self.rust_ty(item)),
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

    /// The Rust name of record or enum `q` in this module: its name when
    /// it is builtin, own or imported, else its path in `telos-std`
    /// (`telos_std::console::Action`).
    fn enum_name(&self, q: &QualName) -> String {
        let own = self.types.is_some_and(|t| t.module == q.module);
        if q.is_builtin() || own || self.symbols.imported.get(&q.name) == Some(&q.module) {
            return q.name.clone();
        }
        let mut path = self.root.to_string();
        for segment in q.module.split('.').skip(1) {
            path.push_str("::");
            path.push_str(&ident(segment));
        }
        format!("{path}::{}", ident(&q.name))
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
            rust_type(&alias.ty)
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
            let value = self.value(default);
            self.scopes.clear();
            w.line("#[allow(non_snake_case)]");
            w.open(&format!(
                "pub fn {}() -> {} {{",
                default_fn(&field.name.name),
                rust_type(&field.ty)
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

    fn record(&mut self, w: &mut Writer, record: &RecordDecl) {
        w.line("");
        w.line("#[derive(Clone, Debug)]");
        w.open(&format!(
            "pub struct {}{} {{",
            ident(&record.name.name),
            bounded_params(&record.type_params)
        ));
        for field in &record.fields {
            w.line(&format!(
                "pub {}: {},",
                ident(&field.name.name),
                rust_type(&field.ty)
            ));
        }
        w.close("}");
        self.defaults(w, record);
    }

    fn enumeration(&mut self, w: &mut Writer, decl: &EnumDecl) {
        w.line("");
        w.line("#[derive(Clone, Debug)]");
        w.open(&format!(
            "pub enum {}{} {{",
            ident(&decl.name.name),
            bounded_params(&decl.type_params)
        ));
        for variant in &decl.variants {
            if variant.payload.is_empty() {
                w.line(&format!("{},", ident(&variant.name.name)));
            } else {
                let payload: Vec<String> = variant.payload.iter().map(rust_type).collect();
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

    /// `test "name" { body }` as `pub fn __telos_test_<decl>() { body }`
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
    fn show(&self, show: Option<&Show>, ty: &Ty, value: &str) -> String {
        match show {
            Some(telos_types::Show::Debug) => unimplemented!("slice 9: debug text"),
            Some(Show::Builtin(Builtin::TextToText)) => format!("{value}.clone()"),
            Some(Show::Builtin(Builtin::BytesToText)) => {
                format!("String::from_utf8_lossy(&{value}).into_owned()")
            }
            Some(Show::Builtin(_)) => format!("{value}.to_string()"),
            Some(Show::Fn(q)) => format!("{}(&{value})", ident(&q.name)),
            Some(Show::IoErrorMessage) => format!("{value}.message.clone()"),
            None => format!("String::from(\"{}\")", escape_str(&format!("<{ty}>"))),
        }
    }

    /// `assert cond` and `assert cond, message` (`docs/slice.md` section
    /// 12.5). For `a == b` and `a != b` each side is evaluated once into a
    /// local; types with Rust `==` compare with it, every other type
    /// structurally through its derived `Debug` text, which spells out every
    /// field, variant and element.
    fn assert(&mut self, stmt: &Stmt, cond: &Expr, message: Option<&Expr>) -> String {
        let text = escape_str(&self.source_text(cond.span));
        let at = escape_str(&self.position(stmt.span.start));
        let message = message.map(|m| self.value(m));
        let report = |operands: &str| match &message {
            Some(m) => format!(
                "{{ let __telos_message: String = {m}; telos_io::assert_failed(\"{text}\", Some(&__telos_message), {operands}, \"{at}\") }}"
            ),
            None => format!("telos_io::assert_failed(\"{text}\", None, {operands}, \"{at}\")"),
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
                Ty::Int | Ty::Byte | Ty::Bool | Ty::Text | Ty::Unit => {
                    "__telos_left == __telos_right".to_string()
                }
                _ => "format!(\"{:?}\", __telos_left) == format!(\"{:?}\", __telos_right)"
                    .to_string(),
            };
            let holds = match op {
                BinaryOp::Eq => format!("({equal})"),
                _ => format!("!({equal})"),
            };
            let shown_left = self.show(compare.show.as_ref(), &compare.operand, "__telos_left");
            let shown_right = self.show(compare.show.as_ref(), &compare.operand, "__telos_right");
            let operands = "Some((&__telos_shown_left, &__telos_shown_right))";
            let ty = self.rust_ty(&compare.operand);
            return format!(
                "{{ let __telos_left: {ty} = {left}; let __telos_right: {ty} = {right}; if !{holds} {{ let __telos_shown_left: String = {shown_left}; let __telos_shown_right: String = {shown_right}; {} }} }}",
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
                let shown = self.show(test_try.show.as_ref(), error, "__telos_error");
                format!(
                    "(match {value} {{ Ok(__telos_ok) => __telos_ok, Err(__telos_error) => {{ let __telos_shown: String = {shown}; telos_io::try_failed(Some(&__telos_shown), \"{at}\") }} }})"
                )
            }
            None => format!(
                "(match {value} {{ Some(__telos_ok) => __telos_ok, None => telos_io::try_failed(None, \"{at}\") }})"
            ),
        }
    }

    /// One `pub fn`, preceded by an empty line when `spaced`. A protocol
    /// `role` takes its parameters by value (a generated `main` calls it);
    /// every other fn takes them by reference (section 6, parameters).
    fn function_line(&mut self, w: &mut Writer, decl: &FnDecl, spaced: bool, role: bool) {
        let mut mutated = BTreeSet::new();
        mutated_names(&decl.body, &mut mutated);
        let mut prologue = Vec::new();
        self.borrowed.clear();
        let params: Vec<String> = decl
            .params
            .iter()
            .map(|p| {
                let name = ident(&p.name.name);
                let ty = rust_type(&p.ty);
                if role {
                    return format!("mut {name}: {ty}");
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
            Some(ty) => format!(" -> {}", rust_type(ty)),
            None => String::new(),
        };
        let value = decl
            .ret
            .as_ref()
            .is_some_and(|ty| !(ty.name.name == "Unit" && ty.args.is_empty()));
        self.scopes = vec![decl.params.iter().map(|p| p.name.name.clone()).collect()];
        self.indent = w.indent;
        let mut body = self.block(&decl.body, value);
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
        w.line(&format!(
            "pub fn {}{}({}){ret} {body}",
            ident(&decl.name.name),
            bounded_params(&decl.type_params),
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
    /// `telos_io::exit` returns `!`), so `expr?` lowers to `expr` alone.
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

    fn is_place(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Name(id) => self.is_local(&id.name),
            ExprKind::Field { receiver, .. } => self.is_place(receiver),
            _ => false,
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
                let value = self.value(value);
                self.bind(&name.name);
                let ty = match ty {
                    Some(ty) => format!(": {}", rust_type(ty)),
                    None => String::new(),
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
                format!("{place} = {};", self.value(value))
            }
            StmtKind::Return(Some(value)) => format!("return {};", self.value(value)),
            StmtKind::Return(None) => "return;".to_string(),
            StmtKind::While { cond, body } => {
                let cond = self.condition(cond);
                format!("while {cond} {}", self.block(body, false))
            }
            StmtKind::Loop { body } => format!("loop {}", self.block(body, false)),
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

    fn args(&mut self, args: &[Expr]) -> Vec<String> {
        args.iter().map(|a| self.value(a)).collect()
    }

    fn expr(&mut self, expr: &Expr, clone: bool) -> String {
        if clone && self.is_place(expr) {
            return format!("{}.clone()", self.bare(expr));
        }
        match &expr.kind {
            telos_syntax::ast::ExprKind::Closure(_) | telos_syntax::ast::ExprKind::RecordUpdate { .. } | telos_syntax::ast::ExprKind::TupleField { .. } | telos_syntax::ast::ExprKind::Break | telos_syntax::ast::ExprKind::Continue => unimplemented!("slice 9: closures, record update, tuple fields, break and continue expressions"),
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
            ExprKind::Call { callee, args } => match &callee.kind {
                ExprKind::Name(id) if !self.is_local(&id.name) => {
                    match self.typed_variant(expr, &id.name) {
                        Some(path) => format!("{path}({})", self.args(args).join(", ")),
                        None if self.calls_fn(expr, id) => {
                            let target = self.name(id);
                            let args: Vec<String> = args.iter().map(|a| self.ref_arg(a)).collect();
                            format!("{target}({})", args.join(", "))
                        }
                        None => self.call(callee, args),
                    }
                }
                _ => self.call(callee, args),
            },
            ExprKind::Method {
                receiver,
                name,
                args,
            } => self.method(expr, receiver, name, args),
            ExprKind::Field { receiver, name } => {
                if let Some(owner) = self.qualifier(receiver)
                    && self.is_enum(owner)
                {
                    return variant_path(owner, &name.name);
                }
                format!("{}.{}", self.receiver(receiver), ident(&name.name))
            }
            ExprKind::Index { receiver, index } => {
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
                let texts: Vec<String> = parts.iter().map(|p| self.bare(p)).collect();
                format!(
                    "format!(\"{}\", {})",
                    "{}".repeat(texts.len()),
                    texts.join(", ")
                )
            }
            ExprKind::Binary { op, left, right } => {
                let left_text = self.operand(left);
                let right_text = self.operand(right);
                format!("{left_text} {} {right_text}", binary_symbol(*op))
            }
            ExprKind::Match { scrutinee, arms } => self.match_expr(scrutinee, arms),
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

    fn name(&mut self, id: &Ident) -> String {
        if self.is_local(&id.name) {
            if self.is_outermost(&id.name)
                && let Some(rename) = self.renames.get(&id.name)
            {
                self.renamed_uses.insert(id.name.clone());
                return rename.clone();
            }
            return ident(&id.name);
        }
        if id.name == "Unit" {
            return "()".to_string();
        }
        if self.constants.contains(&id.name) {
            return format!("{}()", ident(&id.name));
        }
        if let Some(owner) = self.enum_of_variant(&id.name) {
            return variant_path(&owner, &id.name);
        }
        ident(&id.name)
    }

    /// A record literal; an omitted field calls the record's default fn,
    /// with the literal's type arguments for a generic record (Rust cannot
    /// infer them from a default that does not mention them).
    fn record_literal(&mut self, expr: &Expr, name: &Ident, fields: &[FieldInit]) -> String {
        let record = ident(&name.name);
        let mut texts: Vec<String> = fields
            .iter()
            .map(|f| format!("{}: {}", ident(&f.name.name), self.value(&f.value)))
            .collect();
        if let Some(declared) = self.symbols.record_fields.get(&name.name) {
            let instance = self.turbofish(expr);
            for field in declared {
                if !fields.iter().any(|f| f.name.name == *field) {
                    texts.push(format!(
                        "{}: {record}{instance}::{}()",
                        ident(field),
                        default_fn(field)
                    ));
                }
            }
        }
        if texts.is_empty() {
            return format!("{record} {{}}");
        }
        format!("{record} {{ {} }}", texts.join(", "))
    }

    fn call(&mut self, callee: &Expr, args: &[Expr]) -> String {
        match &callee.kind {
            ExprKind::Name(id) if !self.is_local(&id.name) => {
                if id.name == "log" && args.len() == 1 {
                    let arg = self.value(&args[0]);
                    return format!("telos_io::log(&({arg}))");
                }
                let target = self.name(id);
                format!("{target}({})", self.args(args).join(", "))
            }
            ExprKind::Field { receiver, name } => {
                if let Some(owner) = self.qualifier(receiver)
                    && let Some(text) = self.qualified_call(owner, name, args)
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
                Some(Callee::Fn(q)) => (
                    Some(ident(&q.name)),
                    std::iter::once(&**receiver).chain(args).collect(),
                    true,
                ),
                _ => (self.role_path(receiver, name), args.iter().collect(), false),
            },
            _ => (None, Vec::new(), false),
        };
        const LOG: &str = "telos_io::log(&format!(\"task ended with error: {}\", error));";
        let Some(target) = target else {
            let task = self.value(operand);
            return format!(
                "{{ telos_io::spawn(move || {{ if let Err(error) = ({task}) {{ {LOG} }} }}); }}"
            );
        };
        let mut out = String::from("{ ");
        let mut names = Vec::new();
        for (index, arg) in args.iter().enumerate() {
            let value = self.value(arg);
            let _ = write!(out, "let __spawn_{index} = {value}; ");
            let borrow = if by_ref { "&" } else { "" };
            names.push(format!("{borrow}__spawn_{index}"));
        }
        let _ = write!(
            out,
            "telos_io::spawn(move || {{ if let Err(error) = {target}({}) {{ {LOG} }} }}); }}",
            names.join(", ")
        );
        out
    }

    /// `P::name` for `P.name` where `P` is a protocol or another non-local
    /// uppercase qualifier (a user fn cannot be qualified).
    fn role_path(&self, receiver: &Expr, name: &Ident) -> Option<String> {
        let owner = self.qualifier(receiver)?;
        if SUBSTRATE_OWNERS.contains(&owner) {
            return Some(format!("telos_io::{owner}::{}", ident(&name.name)));
        }
        Some(format!("{}::{}", ident(owner), ident(&name.name)))
    }

    /// `Owner.name(args)` where `Owner` is a substrate type, a builtin
    /// static owner, a protocol or an enum.
    fn qualified_call(&mut self, owner: &str, name: &Ident, args: &[Expr]) -> Option<String> {
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
                "{}::{}({})",
                ident(owner),
                ident(&name.name),
                texts.join(", ")
            ));
        }
        if self.is_enum(owner) {
            let path = variant_path(owner, &name.name);
            return Some(format!("{path}({})", self.args(args).join(", ")));
        }
        None
    }

    fn static_call(&mut self, owner: &str, name: &str, args: &[Expr]) -> String {
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
            ("Store", "new", []) => "telos_io::Store::new()".to_string(),
            ("Int", "to_text", [value]) => {
                format!("{}.to_string()", wrap(&args[0], value.clone()))
            }
            ("Process", "exit", [code]) => format!("telos_io::exit(({code}) as i32)"),
            ("Int", "parse", [text]) => format!(
                "Some({text}).filter(|t: &String| !t.starts_with('+')).and_then(|t| t.parse::<i64>().ok())"
            ),
            (owner, _, _) if SUBSTRATE_OWNERS.contains(&owner) => {
                format!("telos_io::{owner}::{name}({})", texts.join(", "))
            }
            _ => format!("{owner}::{name}({})", texts.join(", ")),
        }
    }

    fn method(&mut self, expr: &Expr, receiver: &Expr, name: &Ident, args: &[Expr]) -> String {
        match self.callee(expr) {
            Some(Callee::Fn(q)) => {
                let mut texts = vec![self.ref_arg(receiver)];
                for arg in args {
                    texts.push(self.ref_arg(arg));
                }
                return format!("{}({})", ident(&q.name), texts.join(", "));
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
            _ => {}
        }
        if let Some(owner) = self.qualifier(receiver)
            && let Some(text) = self.qualified_call(owner, name, args)
        {
            return text;
        }
        let r = self.receiver(receiver);
        let a = self.args(args);
        let arg = |i: usize, a: &[String]| borrow(&args[i], a[i].clone());
        match (name.name.as_str(), a.len()) {
            ("map", 1) => format!("{r}.iter().cloned().map({}).collect::<Vec<_>>()", a[0]),
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

    fn match_expr(&mut self, scrutinee: &Expr, arms: &[telos_syntax::ast::Arm]) -> String {
        let mut scrutinee_text = self.value(scrutinee);
        if matches!(scrutinee.kind, ExprKind::Record { .. }) {
            scrutinee_text = format!("({scrutinee_text})");
        }
        let ty = self.type_of(scrutinee).cloned();
        let mut out = format!("match {scrutinee_text} {{\n");
        self.indent += 1;
        for arm in arms {
            self.scopes.push(Vec::new());
            let mut bindings = Vec::new();
            let pattern = if self.is_native(&arm.pattern) {
                self.pattern(&arm.pattern, ty.as_ref())
            } else {
                let subject = "(&__m)".to_string();
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
            self.scopes.pop();
            if !bindings.is_empty() {
                let lets: String = bindings
                    .iter()
                    .map(|(name, access)| format!("let {} = ({access}).clone(); ", ident(name)))
                    .collect();
                body = format!("{{ {lets}{body} }}");
            }
            out.push_str(&indentation(self.indent));
            let _ = writeln!(out, "{pattern} => {body},");
        }
        self.indent -= 1;
        out.push_str(&indentation(self.indent));
        out.push('}');
        out
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
    /// ([`Generator::extract`]).
    fn is_native(&self, pattern: &Pattern) -> bool {
        if self.pattern_const(pattern).is_some() {
            return false;
        }
        match &pattern.kind {
            telos_syntax::ast::PatternKind::Range { .. } => unimplemented!("slice 9: range patterns"),
            PatternKind::Str(_) | PatternKind::List(_) => false,
            PatternKind::Or(items) | PatternKind::Tuple(items) => {
                items.iter().all(|p| self.is_native(p))
            }
            PatternKind::Variant { args, .. } => args.iter().all(|p| self.is_native(p)),
            PatternKind::Wildcard
            | PatternKind::Int(_)
            | PatternKind::Bool(_)
            | PatternKind::Binding(_) => true,
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
            Some(path[path.len() - 2].name.clone())
        } else if let Some((owner, _)) = typed {
            Some(owner)
        } else {
            self.enum_of_variant(name)
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
            telos_syntax::ast::PatternKind::Range { .. } => unimplemented!("slice 9: range patterns"),
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
            telos_syntax::ast::PatternKind::Range { .. } => unimplemented!("slice 9: range patterns"),
            PatternKind::Wildcard => None,
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
                let mut lets = String::new();
                let mut tests = vec![format!("{subject}.len() == {}", items.len())];
                for (i, item) in items.iter().enumerate() {
                    let name = self.fresh();
                    if let Some(test) = self.test(item, &name, element.as_ref()) {
                        let _ = write!(lets, "let {name} = &{subject}[{i}]; ");
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
                    match self.test(arg, &name, payload.get(i)) {
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
            telos_syntax::ast::PatternKind::Range { .. } => unimplemented!("slice 9: range patterns"),
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
                for (i, item) in items.iter().enumerate() {
                    self.extract(item, format!("(&{subject}[{i}])"), element.as_ref(), out);
                }
            }
            PatternKind::Variant { path, args } => {
                let variant = path.last().map(|i| i.name.as_str()).unwrap_or("_");
                let (head, payload) = self.variant_head(path, variant, ty);
                for (i, arg) in args.iter().enumerate() {
                    let fields: Vec<&str> = (0..args.len())
                        .map(|j| if i == j { "__x" } else { "_" })
                        .collect();
                    let access = format!(
                        "(match {subject} {{ {head}({}) => __x, _ => unreachable!() }})",
                        fields.join(", ")
                    );
                    self.extract(arg, access, payload.get(i), out);
                }
            }
            PatternKind::Wildcard
            | PatternKind::Int(_)
            | PatternKind::Str(_)
            | PatternKind::Bool(_)
            | PatternKind::Or(_) => {}
        }
    }
}

/// Builtin methods that change their receiver in place.
const MUTATING_METHODS: &[&str] = &["push", "append", "clear"];

/// The names a block may change in place: the base of every assignment
/// and the root of every receiver of a [`MUTATING_METHODS`] call. A name
/// shadowed by a binding still counts, so the answer can only be too
/// large; a parameter outside the set is never changed by the body.
fn mutated_names(block: &Block, out: &mut BTreeSet<String>) {
    for stmt in &block.stmts {
        match &stmt.kind {
            StmtKind::LetPattern { value, .. } | StmtKind::Let { value, .. } => {
                mutated_in(value, out);
            }
            StmtKind::Assign { target, value } => {
                out.insert(target.base.name.clone());
                mutated_in(value, out);
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    mutated_in(value, out);
                }
            }
            StmtKind::While { cond, body } => {
                mutated_in(cond, out);
                mutated_names(body, out);
            }
            StmtKind::Loop { body } => mutated_names(body, out),
            StmtKind::For { iter, body, .. } => {
                mutated_in(iter, out);
                mutated_names(body, out);
            }
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Expr(expr) => mutated_in(expr, out),
            StmtKind::Assert { cond, message } => {
                mutated_in(cond, out);
                if let Some(message) = message {
                    mutated_in(message, out);
                }
            }
        }
    }
}

fn mutated_in(expr: &Expr, out: &mut BTreeSet<String>) {
    match &expr.kind {
        telos_syntax::ast::ExprKind::Closure(_) | telos_syntax::ast::ExprKind::RecordUpdate { .. } | telos_syntax::ast::ExprKind::TupleField { .. } | telos_syntax::ast::ExprKind::Break | telos_syntax::ast::ExprKind::Continue => unimplemented!("slice 9: closures, record update, tuple fields, break and continue expressions"),
        ExprKind::Int(_) | ExprKind::Str(_) | ExprKind::Bool(_) | ExprKind::Name(_) => {}
        ExprKind::Tuple(items) | ExprKind::List(items) => {
            items.iter().for_each(|item| mutated_in(item, out));
        }
        ExprKind::Return(value) => {
            if let Some(value) = value {
                mutated_in(value, out);
            }
        }
        ExprKind::Record { fields, .. } => {
            fields.iter().for_each(|f| mutated_in(&f.value, out));
        }
        ExprKind::Call { callee, args } => {
            mutated_in(callee, out);
            args.iter().for_each(|arg| mutated_in(arg, out));
        }
        ExprKind::Method {
            receiver,
            name,
            args,
        } => {
            if MUTATING_METHODS.contains(&name.name.as_str())
                && let Some(root) = place_root(receiver)
            {
                out.insert(root.to_string());
            }
            mutated_in(receiver, out);
            args.iter().for_each(|arg| mutated_in(arg, out));
        }
        ExprKind::Field { receiver, .. } => mutated_in(receiver, out),
        ExprKind::Index { receiver, index } => {
            mutated_in(receiver, out);
            mutated_in(index, out);
        }
        ExprKind::Try(inner)
        | ExprKind::Await(inner)
        | ExprKind::Spawn(inner)
        | ExprKind::Unary { operand: inner, .. } => mutated_in(inner, out),
        ExprKind::Binary { left, right, .. } => {
            mutated_in(left, out);
            mutated_in(right, out);
        }
        ExprKind::Match { scrutinee, arms } => {
            mutated_in(scrutinee, out);
            arms.iter().for_each(|arm| mutated_in(&arm.body, out));
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            mutated_in(cond, out);
            mutated_names(then, out);
            if let Some(otherwise) = otherwise {
                mutated_in(otherwise, out);
            }
        }
        ExprKind::Block(block) => mutated_names(block, out),
    }
}

/// The name a place expression (`x`, `x.a.b`, `x[i]`) starts from.
fn place_root(expr: &Expr) -> Option<&str> {
    match &expr.kind {
        ExprKind::Name(id) => Some(&id.name),
        ExprKind::Field { receiver, .. } | ExprKind::Index { receiver, .. } => place_root(receiver),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Service,
    Client,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Service => "Service",
            Kind::Client => "Client",
        }
    }

    /// The property naming the component's resource.
    fn resource_property(self) -> &'static str {
        match self {
            Kind::Service => "listener",
            Kind::Client => "target",
        }
    }

    /// The protocol role the component runs.
    fn role(self) -> &'static str {
        match self {
            Kind::Service => "serve",
            Kind::Client => "connect",
        }
    }
}

/// A validated component: what its `main` needs.
struct Component<'p> {
    kind: Kind,
    executable: String,
    span: Span,
    resource: String,
    address: String,
    port: i64,
    protocol: String,
    /// The `application` expression of a `Service`.
    application: Option<&'p Expr>,
    /// The system's `Store` resources: name and Rust type, in declaration
    /// order. Every `main` creates them first (`docs/slice.md` 11.4).
    stores: Vec<(String, String)>,
    /// The system's `TcpListener` resources other than the component's
    /// own, which its `main` does not bind.
    unbound: Vec<String>,
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
}

fn e020(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("E020", span, message)
}

fn e030(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("E030", span, message)
}

fn lower_system<'p>(
    program: &Program,
    symbols: &Symbols,
    system: &'p SystemDecl,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Component<'p>> {
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
            other => diagnostics.push(e020(
                resource.kind.span,
                format!("unsupported resource kind `{other}`; expected `TcpListener` or `Store`"),
            )),
        }
    }
    system
        .components
        .iter()
        .filter_map(|component| {
            lower_component(program, symbols, component, &resources, diagnostics)
        })
        .collect()
}

/// `resource name: Store<T>`: the Rust type `telos_io::Store<T>`. A store
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
            ("address" | "port", _) if is_duplicate(&resource.properties, property) => {
                diagnostics.push(e020(
                    property.name.span,
                    format!("duplicate property `{}`", property.name.name),
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

/// The role names of protocol `name`, from `symbols` (own and imported) or
/// from the program's own declarations; `None` when it is not a protocol.
fn protocol_roles(program: &Program, symbols: &Symbols, name: &str) -> Option<Vec<String>> {
    if let Some(roles) = symbols.protocols.get(name) {
        return Some(roles.keys().cloned().collect());
    }
    program.decls.iter().find_map(|decl| match decl {
        Decl::Protocol(protocol) if protocol.name.name == name => Some(
            protocol
                .fns
                .iter()
                .map(|role| role.name.name.clone())
                .collect(),
        ),
        _ => None,
    })
}

fn lower_component<'p>(
    program: &Program,
    symbols: &Symbols,
    component: &'p ComponentDecl,
    resources: &Resources,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Component<'p>> {
    let kind = match component.kind.name.as_str() {
        "Service" => Kind::Service,
        "Client" => Kind::Client,
        other => {
            diagnostics.push(e020(
                component.kind.span,
                format!("unsupported component kind `{other}`; expected `Service` or `Client`"),
            ));
            return None;
        }
    };
    let resource_property = kind.resource_property();
    let before = diagnostics.len();
    let mut resource: Option<(String, Option<Listener>)> = None;
    let mut protocol: Option<String> = None;
    let mut application: Option<&Expr> = None;
    let mut executable: Option<String> = None;
    let mut seen: Vec<&str> = Vec::new();
    for property in &component.properties {
        let key = property.name.name.as_str();
        if seen.contains(&key) {
            diagnostics.push(e020(
                property.name.span,
                format!("duplicate property `{key}`"),
            ));
            continue;
        }
        seen.push(key);
        match key {
            _ if key == resource_property => match &property.value.kind {
                ExprKind::Name(id) if resources.listeners.contains_key(&id.name) => {
                    resource = Some((id.name.clone(), resources.listeners[&id.name].clone()));
                }
                _ => diagnostics.push(e020(
                    property.value.span,
                    format!("property `{key}` must name a `TcpListener` resource of this system"),
                )),
            },
            "protocol" => match &property.value.kind {
                ExprKind::Name(id) => match protocol_roles(program, symbols, &id.name) {
                    Some(roles) if roles.iter().any(|r| r == kind.role()) => {
                        protocol = Some(id.name.clone());
                    }
                    Some(_) => diagnostics.push(e030(
                        property.value.span,
                        format!(
                            "protocol `{}` has no `{}` role, which a `{}` component runs",
                            id.name,
                            kind.role(),
                            kind.name()
                        ),
                    )),
                    None => diagnostics.push(e020(
                        property.value.span,
                        format!("`{}` is not a protocol", id.name),
                    )),
                },
                _ => diagnostics.push(e020(
                    property.value.span,
                    "property `protocol` must name a protocol",
                )),
            },
            "application" if kind == Kind::Service => application = Some(&property.value),
            "executable" => match &property.value.kind {
                ExprKind::Str(text) if is_executable_name(text) => {
                    executable = Some(text.clone());
                }
                _ => diagnostics.push(e020(
                    property.value.span,
                    "property `executable` must be a string literal naming a binary",
                )),
            },
            other => diagnostics.push(e020(
                property.name.span,
                format!(
                    "unsupported property `{other}` on component kind `{}`",
                    kind.name()
                ),
            )),
        }
    }
    let mut required = vec![resource_property, "protocol"];
    if kind == Kind::Service {
        required.push("application");
    }
    for name in required {
        if !seen.contains(&name) {
            diagnostics.push(e020(
                component.name.span,
                format!("component `{}` has no `{name}`", component.name.name),
            ));
        }
    }
    if diagnostics.len() != before {
        return None;
    }
    let (resource, Some(bound)) = resource? else {
        return None;
    };
    let unbound = resources
        .listeners
        .keys()
        .filter(|name| kind == Kind::Client || **name != resource)
        .cloned()
        .collect();
    Some(Component {
        kind,
        executable: executable.unwrap_or_else(|| component.name.name.clone()),
        span: component.span,
        resource,
        address: bound.address,
        port: bound.port,
        protocol: protocol?,
        application,
        stores: resources.stores.clone(),
        unbound,
    })
}

fn is_executable_name(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A Rust variable for a resource, kept apart from the names `main` binds.
fn resource_variable(name: &str) -> String {
    match name {
        "application" | "port" | "stream" | "error" => format!("{name}_"),
        _ => ident(name),
    }
}

/// `src/bin/<executable>.rs`: `use <crate>::*;`, the program's `use` lines
/// and the `main` of section 6.1.
fn component_main(
    crate_name: &str,
    uses: &[String],
    program: &Program,
    symbols: &Symbols,
    types: Option<&TypedProgram>,
    component: &Component<'_>,
) -> Result<String, Diagnostic> {
    let address = escape_str(&component.address);
    let protocol = ident(&component.protocol);
    let mut w = Writer::new();
    w.raw(FILE_HEADER);
    w.line(&format!("use {crate_name}::*;"));
    for line in uses {
        w.line(line);
    }
    w.line("");
    w.open("fn main() {");
    w.open("std::process::exit(telos_io::run(|| {");
    for (name, ty) in &component.stores {
        w.line(&format!(
            "let {}: {ty} = telos_io::Store::new();",
            resource_variable(name)
        ));
    }
    match component.kind {
        Kind::Service => {
            let resource = resource_variable(&component.resource);
            let address_format =
                escape_str(&component.address.replace('{', "{{").replace('}', "}}"));
            let mut application_line = None;
            let mut after_bind = false;
            if let Some(application) = component.application {
                let mut generator = Generator::new(symbols, program, types);
                generator.indent = w.indent;
                let names = component
                    .stores
                    .iter()
                    .map(|(name, _)| name)
                    .chain([&component.resource])
                    .chain(&component.unbound);
                let mut scope = Vec::new();
                for name in names {
                    scope.push(name.clone());
                    generator
                        .renames
                        .insert(name.clone(), resource_variable(name));
                }
                generator.scopes = vec![scope];
                let value = generator.value(application);
                if let Some(name) = component
                    .unbound
                    .iter()
                    .find(|name| generator.renamed_uses.contains(*name))
                {
                    return Err(e020(
                        application.span,
                        format!(
                            "resource `{name}` is a `TcpListener` that component `{}` does not bind; \
                             only its own listener and `Store` resources are values here",
                            component.executable
                        ),
                    ));
                }
                after_bind = generator.renamed_uses.contains(&component.resource);
                application_line = Some(format!("let application = {value};"));
            }
            if !after_bind && let Some(line) = &application_line {
                w.line(line);
            }
            w.line(&format!(
                "let port = telos_io::args::port()?.unwrap_or({});",
                component.port
            ));
            w.line(&format!(
                "let {resource} = telos_io::TcpListener::bind(String::from(\"{address}\"), port)?;"
            ));
            w.line(&format!(
                "telos_io::log(&format!(\"listening on {address_format}:{{}}\", {resource}.local_port()));"
            ));
            if after_bind && let Some(line) = &application_line {
                w.line(line);
            }
            w.open("loop {");
            w.line(&format!("let stream = {resource}.accept()?;"));
            w.line("let application = application.clone();");
            w.open("telos_io::spawn(move || {");
            w.open(&format!(
                "if let Err(error) = {protocol}::serve(stream, application) {{"
            ));
            w.line("telos_io::log(&format!(\"session ended with error: {}\", error));");
            w.close("}");
            w.close("});");
            w.close("}");
            w.line("#[allow(unreachable_code)]");
            w.line("Ok::<(), telos_io::IoError>(())");
        }
        Kind::Client => {
            w.line(&format!(
                "let port = telos_io::args::port()?.unwrap_or({});",
                component.port
            ));
            w.line(&format!(
                "let stream = telos_io::TcpStream::connect(String::from(\"{address}\"), port).map_err(|error| error.with_exit_code(3))?;"
            ));
            w.line(&format!(
                "{protocol}::connect(stream).map_err(|error| error.with_exit_code(4))"
            ));
        }
    }
    w.close("}))");
    w.close("}");
    Ok(w.finish())
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
    out.push_str("\n[lib]\n");
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
    out.push_str(&telos_io_dependency(&substrate, feature));
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
/// built into one `--target-dir` share one `telos-std` artifact, and the
/// one written earlier counts as up to date.
fn std_cargo_toml(substrate: &Path, lib_rs: &str) -> String {
    let substrate = substrate.to_string_lossy().replace('\\', "/");
    let mut out = String::new();
    out.push_str("[package]\n");
    let _ = writeln!(out, "name = {}", toml_string(STD_PACKAGE));
    let _ = writeln!(out, "version = \"0.0.0-{:016x}\"", content_hash(lib_rs));
    out.push_str("edition = \"2024\"\n");
    out.push_str("\n[lib]\n");
    out.push_str("path = \"src/lib.rs\"\n");
    out.push_str("\n[dependencies]\n");
    out.push_str(&telos_io_dependency(&substrate, None));
    out
}

/// The `telos-io` dependency line, with `feature` enabled when given.
fn telos_io_dependency(substrate: &str, feature: Option<&str>) -> String {
    match feature {
        Some(feature) => format!(
            "telos-io = {{ path = {}, features = [{}] }}\n",
            toml_string(substrate),
            toml_string(feature)
        ),
        None => format!("telos-io = {{ path = {} }}\n", toml_string(substrate)),
    }
}
