//! Lower a program the way the driver does, for golden and build tests:
//! every `.telos` file under a standard-library root is module
//! `std.<relative path with dots>`, checked with `check_module` in import
//! order; the program is checked against all of them. Any diagnostic
//! panics with the rendered text.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use telos_check::{Modules, Symbols, check_module};
use telos_rustgen::{
    GeneratedProject, Options, StdModule, TypedStdModule, generate, generate_std,
    generate_std_typed, generate_typed,
};
use telos_syntax::SourceFile;
use telos_syntax::ast::Program;
use telos_types::{TypedModules, TypedProgram, type_module};

pub fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The repository's standard library.
pub fn repository_std() -> PathBuf {
    workspace().join("std")
}

/// The standard-library fixture of the golden tests.
pub fn golden_std() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/std")
}

fn telos_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            telos_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "telos") {
            out.push(path);
        }
    }
}

fn parse(path: &Path) -> (SourceFile, Program) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let source = SourceFile::new(path.display().to_string(), text);
    let program = telos_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    (source, program)
}

fn imported_modules(program: &Program) -> Vec<String> {
    program
        .imports
        .iter()
        .map(|import| {
            let segments = &import.path.segments;
            let count = if import.names.is_empty() {
                segments.len() - 1
            } else {
                segments.len()
            };
            segments[..count]
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(".")
        })
        .collect()
}

/// `(module path, program, symbols)` in import order.
pub fn load_std(root: &Path) -> Vec<(String, Program, Symbols)> {
    let mut files = Vec::new();
    telos_files(root, &mut files);
    let mut pending: Vec<(String, SourceFile, Program)> = files
        .iter()
        .map(|file| {
            let relative = file.strip_prefix(root).unwrap().with_extension("");
            let module = std::iter::once("std".to_string())
                .chain(
                    relative
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned()),
                )
                .collect::<Vec<_>>()
                .join(".");
            let (source, program) = parse(file);
            (module, source, program)
        })
        .collect();
    let names: BTreeSet<String> = pending.iter().map(|(m, _, _)| m.clone()).collect();
    let mut modules = Modules::new();
    let mut ordered = Vec::new();
    while !pending.is_empty() {
        let ready = pending
            .iter()
            .position(|(_, _, program)| {
                imported_modules(program)
                    .iter()
                    .all(|m| !names.contains(m) || modules.contains_key(m))
            })
            .expect("standard-library imports form a cycle");
        let (module, source, program) = pending.remove(ready);
        let symbols = check_module(&program, &modules)
            .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
        modules.insert(module.clone(), symbols.clone());
        ordered.push((module, program, symbols));
    }
    ordered
}

/// The standard library typed in import order, or `None` when a module
/// does not type.
fn type_std(library: &[(String, Program, Symbols)]) -> Option<Vec<TypedProgram>> {
    let mut typed_modules = TypedModules::new();
    let mut std_types = Vec::new();
    for (module, program, symbols) in library {
        let types = type_module(program, symbols, &typed_modules).ok()?;
        typed_modules.insert(module.clone(), types.exports());
        std_types.push(types);
    }
    Some(std_types)
}

/// The program project and the `telos-std` project for `file` with the
/// standard library at `std_root`. Like the driver, the typed generator
/// runs when the library and the program type; otherwise the untyped one
/// (older fixtures such as `pure.telos` leave `List.empty()` and `None`
/// without annotations).
pub fn lower(
    file: &Path,
    std_root: &Path,
    substrate: &Path,
) -> (GeneratedProject, GeneratedProject) {
    lower_with(file, std_root, substrate, false)
}

/// [`lower`], panicking with the typer's diagnostics instead of falling
/// back to the untyped generator.
pub fn lower_typed(
    file: &Path,
    std_root: &Path,
    substrate: &Path,
) -> (GeneratedProject, GeneratedProject) {
    lower_with(file, std_root, substrate, true)
}

/// The typed generator's result for program `text`, checked and typed
/// against the standard library at `std_root`; panics when checking or
/// typing fails, so only the generator's own diagnostics come back.
pub fn generate_source(
    text: &str,
    std_root: &Path,
) -> Result<GeneratedProject, Vec<telos_syntax::diagnostic::Diagnostic>> {
    let library = load_std(std_root);
    let modules: Modules = library
        .iter()
        .map(|(m, _, s)| (m.clone(), s.clone()))
        .collect();
    let source = SourceFile::new("test.telos", text);
    let program = telos_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let symbols = check_module(&program, &modules)
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let std_types = type_std(&library).expect("the standard library types");
    let mut typed_modules = TypedModules::new();
    for ((module, _, _), types) in library.iter().zip(&std_types) {
        typed_modules.insert(module.clone(), types.exports());
    }
    let types = type_module(&program, &symbols, &typed_modules)
        .unwrap_or_else(|d| panic!("typing failed:\n{}", source.render_all(&d)));
    let options = Options {
        substrate_path: Path::new("../telos-io").to_path_buf(),
        ..Options::default()
    };
    generate_typed(&program, &symbols, &types, &options)
}

fn lower_with(
    file: &Path,
    std_root: &Path,
    substrate: &Path,
    require_types: bool,
) -> (GeneratedProject, GeneratedProject) {
    let library = load_std(std_root);
    let modules: Modules = library
        .iter()
        .map(|(m, _, s)| (m.clone(), s.clone()))
        .collect();
    let (source, program) = parse(file);
    let symbols = check_module(&program, &modules)
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let options = Options {
        substrate_path: substrate.to_path_buf(),
        ..Options::default()
    };
    let std_types = type_std(&library);
    let types = std_types.as_ref().map(|std_types| {
        let mut typed_modules = TypedModules::new();
        for ((module, _, _), types) in library.iter().zip(std_types) {
            typed_modules.insert(module.clone(), types.exports());
        }
        type_module(&program, &symbols, &typed_modules)
    });
    match (&std_types, types) {
        (Some(std_types), Some(Ok(types))) => {
            let project = generate_typed(&program, &symbols, &types, &options)
                .unwrap_or_else(|d| panic!("generate failed:\n{}", source.render_all(&d)));
            let inputs: Vec<TypedStdModule<'_>> = library
                .iter()
                .zip(std_types)
                .map(|((m, p, s), t)| (m.clone(), p, s, t))
                .collect();
            let std = generate_std_typed(&inputs, &options).unwrap_or_else(|e| panic!("{e:?}"));
            (project, std)
        }
        (_, types) => {
            if require_types {
                match types {
                    Some(Err(d)) => panic!("typing failed:\n{}", source.render_all(&d)),
                    _ => panic!("typing the standard library failed"),
                }
            }
            let project = generate(&program, &symbols, &options)
                .unwrap_or_else(|d| panic!("generate failed:\n{}", source.render_all(&d)));
            let inputs: Vec<StdModule<'_>> =
                library.iter().map(|(m, p, s)| (m.clone(), p, s)).collect();
            let std = generate_std(&inputs, &options).unwrap_or_else(|e| panic!("{e:?}"));
            (project, std)
        }
    }
}
