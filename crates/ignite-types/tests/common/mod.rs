//! Load a standard-library root the way the driver does: every `.ignite`
//! file under `root` is module `std.<relative path with dots>`, its
//! `package` must name that module, and modules are checked with
//! `check_module` and typed with `type_module` in import order, each seeing
//! the modules before it. Also an expression walker for the tests.
//!
//! Each test binary uses a different part of this module.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ignite_check::{Modules, Symbols, check_module};
use ignite_syntax::ast::{Block, Decl, Expr, ExprKind, Program, StmtKind};
use ignite_syntax::{SourceFile, parse};
use ignite_types::{TypedModules, TypedProgram, type_module};

/// One parsed, checked and typed standard-library file.
pub struct StdFile {
    pub module: String,
    pub source: SourceFile,
    pub program: Program,
    pub symbols: Symbols,
}

/// The loaded standard library: files in import order, their symbols and
/// their typed exports.
pub struct Std {
    pub files: Vec<StdFile>,
    pub modules: Modules,
    pub typed: TypedModules,
}

fn ignite_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            ignite_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "ignite") {
            out.push(path);
        }
    }
}

/// The `std.*` modules a program imports.
pub fn std_imports(program: &Program) -> Vec<String> {
    program
        .imports
        .iter()
        .filter_map(|import| {
            let segments = &import.path.segments;
            let module_len = if import.names.is_empty() {
                segments.len().saturating_sub(1)
            } else {
                segments.len()
            };
            let module = segments[..module_len]
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(".");
            module.starts_with("std.").then_some(module)
        })
        .collect()
}

/// Parse every file under `root`, check and type them in import order.
/// Any diagnostic, package mismatch or import cycle is an `Err` with the
/// rendered diagnostics.
pub fn load_std(root: &Path) -> Result<Std, String> {
    let mut paths = Vec::new();
    ignite_files(root, &mut paths);
    let mut parsed: BTreeMap<String, (SourceFile, Program)> = BTreeMap::new();
    for path in paths {
        let relative = path.strip_prefix(root).expect("file under root");
        let module = std::iter::once("std".to_string())
            .chain(
                relative
                    .with_extension("")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned()),
            )
            .collect::<Vec<_>>()
            .join(".");
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let source = SourceFile::new(path.display().to_string(), text);
        let program = parse(&source).map_err(|d| source.render_all(&d))?;
        let package = program.package.as_ref().map(|p| p.dotted());
        if package.as_deref() != Some(module.as_str()) {
            return Err(format!(
                "{}: package {package:?} does not name module `{module}`",
                path.display()
            ));
        }
        parsed.insert(module, (source, program));
    }

    fn visit(
        module: &str,
        parsed: &BTreeMap<String, (SourceFile, Program)>,
        order: &mut Vec<String>,
        visiting: &mut Vec<String>,
    ) -> Result<(), String> {
        if order.iter().any(|m| m == module) {
            return Ok(());
        }
        if visiting.iter().any(|m| m == module) {
            return Err(format!("import cycle through `{module}`: {visiting:?}"));
        }
        let Some((_, program)) = parsed.get(module) else {
            return Ok(());
        };
        visiting.push(module.to_string());
        for dependency in std_imports(program) {
            visit(&dependency, parsed, order, visiting)?;
        }
        visiting.pop();
        order.push(module.to_string());
        Ok(())
    }
    let mut order: Vec<String> = Vec::new();
    for module in parsed.keys() {
        visit(module, &parsed, &mut order, &mut Vec::new())?;
    }

    let mut std = Std {
        files: Vec::new(),
        modules: Modules::new(),
        typed: TypedModules::new(),
    };
    for module in order {
        let (source, program) = parsed.remove(&module).expect("ordered module was parsed");
        let symbols = check_module(&program, &std.modules).map_err(|d| source.render_all(&d))?;
        let typed =
            type_module(&program, &symbols, &std.typed).map_err(|d| source.render_all(&d))?;
        std.modules.insert(module.clone(), symbols.clone());
        std.typed.insert(module.clone(), typed.exports());
        std.files.push(StdFile {
            module,
            source,
            program,
            symbols,
        });
    }
    Ok(std)
}

/// Type `program` again (keys are the node ids of `program`, so the result
/// also answers for a clone of it).
pub fn retype(program: &Program, symbols: &Symbols, typed: &TypedModules) -> TypedProgram {
    type_module(program, symbols, typed).expect("module typed before")
}

/// Every expression of every function, protocol role, constant and field
/// default of `program`, sub-expressions included, in source order.
pub fn expressions(program: &Program) -> Vec<&Expr> {
    let mut out = Vec::new();
    for decl in &program.decls {
        match decl {
            Decl::Fn(f) => block(&f.body, &mut out),
            Decl::Protocol(p) => {
                for f in &p.fns {
                    block(&f.body, &mut out);
                }
            }
            Decl::Const(c) => expr(&c.value, &mut out),
            Decl::Record(r) => {
                for field in &r.fields {
                    if let Some(default) = &field.default {
                        expr(default, &mut out);
                    }
                }
            }
            Decl::Test(t) => block(&t.body, &mut out),
            Decl::Alias(_) | Decl::Enum(_) | Decl::System(_) | Decl::Assembly(_) => {}
        }
    }
    out
}

fn block<'p>(b: &'p Block, out: &mut Vec<&'p Expr>) {
    for stmt in &b.stmts {
        match &stmt.kind {
            StmtKind::Let { value, .. }
            | StmtKind::LetPattern { value, .. }
            | StmtKind::Assign { value, .. } => expr(value, out),
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    expr(value, out);
                }
            }
            StmtKind::While { cond, body } => {
                expr(cond, out);
                block(body, out);
            }
            StmtKind::Loop { body } => block(body, out),
            StmtKind::For { iter, body, .. } | StmtKind::ForPattern { iter, body, .. } => {
                expr(iter, out);
                block(body, out);
            }
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Assert { cond, message } => {
                expr(cond, out);
                if let Some(message) = message {
                    expr(message, out);
                }
            }
            StmtKind::Expr(e) => expr(e, out),
        }
    }
}

fn expr<'p>(e: &'p Expr, out: &mut Vec<&'p Expr>) {
    out.push(e);
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Str(_) | ExprKind::Bool(_) | ExprKind::Name(_) => {}
        ExprKind::Record { fields, .. } => {
            for field in fields {
                expr(&field.value, out);
            }
        }
        ExprKind::List(items) | ExprKind::Tuple(items) => {
            for item in items {
                expr(item, out);
            }
        }
        ExprKind::Call { callee, args } => {
            expr(callee, out);
            for arg in args {
                expr(arg, out);
            }
        }
        ExprKind::Method { receiver, args, .. } => {
            expr(receiver, out);
            for arg in args {
                expr(arg, out);
            }
        }
        ExprKind::Field { receiver, .. } => expr(receiver, out),
        ExprKind::Return(value) => {
            if let Some(value) = value {
                expr(value, out);
            }
        }
        ExprKind::Index { receiver, index } => {
            expr(receiver, out);
            expr(index, out);
        }
        ExprKind::Try(inner) | ExprKind::Await(inner) | ExprKind::Spawn(inner) => expr(inner, out),
        ExprKind::Unary { operand, .. } => expr(operand, out),
        ExprKind::Binary { left, right, .. } => {
            expr(left, out);
            expr(right, out);
        }
        ExprKind::Match { scrutinee, arms } => {
            expr(scrutinee, out);
            for arm in arms {
                expr(&arm.body, out);
            }
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            expr(cond, out);
            block(then, out);
            if let Some(otherwise) = otherwise {
                expr(otherwise, out);
            }
        }
        ExprKind::Block(b) => block(b, out),
        ExprKind::Closure(c) => expr(&c.body, out),
        ExprKind::RecordUpdate { fields, base, .. } => {
            for field in fields {
                expr(&field.value, out);
            }
            expr(base, out);
        }
        ExprKind::TupleField { receiver, .. } => expr(receiver, out),
        ExprKind::Break | ExprKind::Continue | ExprKind::Quantity { .. } => {}
        ExprKind::NamedArg { value, .. } => expr(value, out),
        ExprKind::Brace { entries } => {
            for entry in entries {
                expr(&entry.value, out);
            }
        }
    }
}
