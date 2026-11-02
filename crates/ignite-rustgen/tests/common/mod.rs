//! Lower a program the way the driver does, for golden and build tests:
//! every `.ignite` file under a standard-library root is module
//! `std.<relative path with dots>`, checked with `check_module` in import
//! order; the program is checked against all of them. Any diagnostic
//! panics with the rendered text.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ignite_check::{Modules, Symbols, check_module};
use ignite_rustgen::{
    GeneratedProject, Options, SiblingModule, StdModule, TypedStdModule, generate,
    generate_package, generate_std, generate_std_for, generate_tests, generate_typed,
};
use ignite_syntax::SourceFile;
use ignite_syntax::ast::Program;
use ignite_types::{TypedModules, TypedProgram, type_module};

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

fn ignite_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            ignite_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "ignite") {
            out.push(path);
        }
    }
}

/// The name a source file carries in diagnostics and failure reports:
/// its path relative to the workspace root when it lies under it (so
/// goldens hold `crates/ignite-rustgen/tests/golden/x.ignite` on every
/// machine), else the path as given.
pub fn source_name(path: &Path) -> String {
    match (path.canonicalize(), workspace().canonicalize()) {
        (Ok(file), Ok(root)) => file
            .strip_prefix(&root)
            .map(|relative| relative.display().to_string())
            .unwrap_or_else(|_| path.display().to_string()),
        _ => path.display().to_string(),
    }
}

fn parse(path: &Path) -> (SourceFile, Program) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let source = SourceFile::new(source_name(path), text);
    let program = ignite_syntax::parse(&source)
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

/// The source files of the standard library at `root` by module path, for
/// `Options.sources`: failure reports inside std name the std file.
pub fn std_sources(root: &Path) -> BTreeMap<String, SourceFile> {
    let mut files = Vec::new();
    ignite_files(root, &mut files);
    files
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
            let text =
                std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            (module, SourceFile::new(source_name(file), text))
        })
        .collect()
}

/// `(module path, program, symbols)` in import order.
pub fn load_std(root: &Path) -> Vec<(String, Program, Symbols)> {
    let mut files = Vec::new();
    ignite_files(root, &mut files);
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
pub fn type_std(library: &[(String, Program, Symbols)]) -> Option<Vec<TypedProgram>> {
    let mut typed_modules = TypedModules::new();
    let mut std_types = Vec::new();
    for (module, program, symbols) in library {
        let types = type_module(program, symbols, &typed_modules).ok()?;
        typed_modules.insert(module.clone(), types.exports());
        std_types.push(types);
    }
    Some(std_types)
}

/// The program project and the `ignite-std` project for `file` with the
/// standard library at `std_root`. Like the driver, the typed generator
/// runs when the library and the program type; otherwise the untyped one
/// (older fixtures such as `pure.ignite` leave `List.empty()` and `None`
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
) -> Result<GeneratedProject, Vec<ignite_syntax::diagnostic::Diagnostic>> {
    let library = load_std(std_root);
    let modules: Modules = library
        .iter()
        .map(|(m, _, s)| (m.clone(), s.clone()))
        .collect();
    let source = SourceFile::new("test.ignite", text);
    let program = ignite_syntax::parse(&source)
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
    let mut sources = std_sources(std_root);
    sources.insert(types.module.clone(), source);
    let options = Options {
        substrate_path: Path::new("../ignite-io").to_path_buf(),
        sources,
        ..Options::default()
    };
    generate_typed(&program, &symbols, &types, &options)
}

/// A test build (`docs/slice.md` section 12.6) of program `file` against
/// the standard library at `std_root`: the program's tests and the
/// `ignite-test` executable.
pub fn lower_tests(
    file: &Path,
    std_root: &Path,
    substrate: &Path,
) -> (GeneratedProject, GeneratedProject) {
    lower_tests_with(file, std_root, substrate, false)
}

/// [`lower_tests`] through `generate_tests`, the driver's path: the
/// program sits at its module path inside the `ignite-tests` crate.
pub fn lower_tests_grouped(
    file: &Path,
    std_root: &Path,
    substrate: &Path,
) -> (GeneratedProject, GeneratedProject) {
    lower_tests_with(file, std_root, substrate, true)
}

fn lower_tests_with(
    file: &Path,
    std_root: &Path,
    substrate: &Path,
    grouped: bool,
) -> (GeneratedProject, GeneratedProject) {
    let library = load_std(std_root);
    let modules: Modules = library
        .iter()
        .map(|(m, _, s)| (m.clone(), s.clone()))
        .collect();
    let (source, program) = parse(file);
    let symbols = check_module(&program, &modules)
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let std_types = type_std(&library).expect("the standard library types");
    let mut typed_modules = TypedModules::new();
    for ((module, _, _), types) in library.iter().zip(&std_types) {
        typed_modules.insert(module.clone(), types.exports());
    }
    let types = type_module(&program, &symbols, &typed_modules)
        .unwrap_or_else(|d| panic!("typing failed:\n{}", source.render_all(&d)));
    let mut sources = std_sources(std_root);
    sources.insert(types.module.clone(), source.clone());
    let options = Options {
        substrate_path: substrate.to_path_buf(),
        test_modules: vec![types.module.clone()],
        tests: types.tests().to_vec(),
        sources,
    };
    let project = if grouped {
        generate_tests(&[(&program, &symbols, &types)], &options).unwrap_or_else(|errors| {
            let diagnostics: Vec<_> = errors.into_iter().flat_map(|(_, d)| d).collect();
            panic!("generate failed:\n{}", source.render_all(&diagnostics))
        })
    } else {
        generate_typed(&program, &symbols, &types, &options)
            .unwrap_or_else(|d| panic!("generate failed:\n{}", source.render_all(&d)))
    };
    let inputs: Vec<TypedStdModule<'_>> = library
        .iter()
        .zip(&std_types)
        .map(|((m, p, s), t)| (m.clone(), p, s, t))
        .collect();
    let std = generate_std_for(&inputs, &options, &project).unwrap_or_else(|e| panic!("{e:?}"));
    (project, std)
}

/// The sibling modules of the program at `file` (`docs/slice.md` section
/// 14.14 item 13): the `.ignite` files beside it whose package is
/// `<program package>.<stem>`, in an order where each follows the siblings
/// it imports.
fn sibling_programs(file: &Path, program: &Program) -> Vec<(String, SourceFile, Program)> {
    let Some(package) = program.package.as_ref().map(|p| p.dotted()) else {
        return Vec::new();
    };
    let dir = file
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let own = file.canonicalize().ok();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "ignite")
                && p.is_file()
                && p.canonicalize().ok() != own
        })
        .collect();
    entries.sort();
    let mut pending: Vec<(String, SourceFile, Program)> = entries
        .into_iter()
        .filter_map(|entry| {
            let stem = entry.file_stem()?.to_string_lossy().into_owned();
            let path = format!("{package}.{stem}");
            let (source, sibling) = parse(&entry);
            (sibling.package.as_ref().map(|p| p.dotted()) == Some(path.clone()))
                .then_some((path, source, sibling))
        })
        .collect();
    let names: BTreeSet<String> = pending.iter().map(|(m, _, _)| m.clone()).collect();
    let mut done: BTreeSet<String> = BTreeSet::new();
    let mut ordered = Vec::new();
    while !pending.is_empty() {
        let ready = pending
            .iter()
            .position(|(_, _, p)| {
                imported_modules(p)
                    .iter()
                    .all(|m| !names.contains(m) || done.contains(m))
            })
            .expect("sibling imports form a cycle");
        let next = pending.remove(ready);
        done.insert(next.0.clone());
        ordered.push(next);
    }
    ordered
}

fn lower_with(
    file: &Path,
    std_root: &Path,
    substrate: &Path,
    require_types: bool,
) -> (GeneratedProject, GeneratedProject) {
    let library = load_std(std_root);
    let mut modules: Modules = library
        .iter()
        .map(|(m, _, s)| (m.clone(), s.clone()))
        .collect();
    let (source, program) = parse(file);
    let mut options = Options {
        substrate_path: substrate.to_path_buf(),
        ..Options::default()
    };
    let std_types = type_std(&library);
    let mut typed_modules = TypedModules::new();
    if let Some(std_types) = &std_types {
        for ((module, _, _), types) in library.iter().zip(std_types) {
            typed_modules.insert(module.clone(), types.exports());
        }
    }
    let mut typed_siblings: Vec<(String, Program, Symbols, TypedProgram)> = Vec::new();
    let mut sibling_sources: Vec<(String, SourceFile)> = Vec::new();
    for (path, sibling_source, sibling) in sibling_programs(file, &program) {
        let symbols = check_module(&sibling, &modules)
            .unwrap_or_else(|d| panic!("check failed:\n{}", sibling_source.render_all(&d)));
        if std_types.is_some() {
            let types = type_module(&sibling, &symbols, &typed_modules)
                .unwrap_or_else(|d| panic!("typing failed:\n{}", sibling_source.render_all(&d)));
            typed_modules.insert(path.clone(), types.exports());
            typed_siblings.push((path.clone(), sibling, symbols.clone(), types));
        }
        modules.insert(path.clone(), symbols);
        sibling_sources.push((path, sibling_source));
    }
    let symbols = check_module(&program, &modules)
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let types = std_types
        .as_ref()
        .map(|_| type_module(&program, &symbols, &typed_modules));
    match (&std_types, types) {
        (Some(std_types), Some(Ok(types))) => {
            options.sources = std_sources(std_root);
            options.sources.insert(types.module.clone(), source.clone());
            for (path, sibling_source) in sibling_sources {
                options.sources.insert(path, sibling_source);
            }
            let siblings: Vec<SiblingModule<'_>> = typed_siblings
                .iter()
                .map(|(path, sibling, symbols, types)| (path.clone(), sibling, symbols, types))
                .collect();
            let project = generate_package(&program, &symbols, &types, &siblings, &options)
                .unwrap_or_else(|errors| {
                    let diagnostics: Vec<_> = errors.into_iter().flat_map(|(_, d)| d).collect();
                    panic!("generate failed:\n{}", source.render_all(&diagnostics))
                });
            let inputs: Vec<TypedStdModule<'_>> = library
                .iter()
                .zip(std_types)
                .map(|((m, p, s), t)| (m.clone(), p, s, t))
                .collect();
            let std =
                generate_std_for(&inputs, &options, &project).unwrap_or_else(|e| panic!("{e:?}"));
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
