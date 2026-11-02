//! Lower a program the way the driver does, for golden and build tests:
//! every `.ignite` file under a standard-library root is module
//! `std.<relative path with dots>`, checked with `check_module` in import
//! order; the program is checked against all of them. Any diagnostic
//! panics with the rendered text.
#![allow(dead_code)]

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ignite_check::{Modules, Symbols, check_module};
use ignite_rustgen::{
    GeneratedProject, Options, PreparedStd as LoweredStd, SiblingModule, StdModule, TypedStdModule,
    generate, generate_package, generate_std, generate_std_for, generate_tests, generate_typed,
    prepare_std_typed,
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

/// One immutable fixture's semantic input. Typed expression maps use AST
/// addresses, so the library and types are always borrowed together, never
/// cloned. Generated projects remain specific to each program and its options.
struct PreparedStd {
    library: Vec<(String, Program, Symbols)>,
    modules: Modules,
    types: Option<Vec<TypedProgram>>,
    typed_modules: TypedModules,
    sources: BTreeMap<String, SourceFile>,
    plain_std: OnceLock<LoweredStd>,
    test_std: OnceLock<LoweredStd>,
}

impl PreparedStd {
    fn load(root: &Path) -> Self {
        let library = load_std(root);
        let modules = library
            .iter()
            .map(|(module, _, symbols)| (module.clone(), symbols.clone()))
            .collect();
        let types = type_std(&library);
        let typed_modules = types
            .as_ref()
            .map(|types| {
                library
                    .iter()
                    .zip(types)
                    .map(|((module, _, _), types)| (module.clone(), types.exports()))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            library,
            modules,
            types,
            typed_modules,
            sources: std_sources(root),
            plain_std: OnceLock::new(),
            test_std: OnceLock::new(),
        }
    }

    fn inputs(&self) -> Vec<TypedStdModule<'_>> {
        self.library
            .iter()
            .zip(self.types.as_deref().expect("the standard library types"))
            .map(|((module, program, symbols), types)| (module.clone(), program, symbols, types))
            .collect()
    }
}

/// Only the repository and checked-in golden roots are immutable fixtures.
/// Custom roots may be rewritten by a test, so they are prepared on every call.
enum StdFixture {
    Shared(&'static PreparedStd),
    Fresh(Box<PreparedStd>),
}

impl Deref for StdFixture {
    type Target = PreparedStd;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Shared(fixture) => fixture,
            Self::Fresh(fixture) => fixture,
        }
    }
}

impl StdFixture {
    fn generate_std_for(&self, options: &Options, program: &GeneratedProject) -> GeneratedProject {
        // The immutable templates freeze only std-specific lowering inputs.
        // Program roots and the substrate path are supplied afresh below.
        let fixed_context = matches!(self, Self::Shared(_))
            && self.library.iter().all(|(module, _, _)| {
                !options.test_modules.contains(module)
                    && options.sources.get(module) == self.sources.get(module)
            });
        if !fixed_context {
            return generate_std_for(&self.inputs(), options, program)
                .unwrap_or_else(|errors| panic!("{errors:?}"));
        }
        let cell = if options.test_modules.is_empty() {
            &self.plain_std
        } else {
            &self.test_std
        };
        let template = cell.get_or_init(|| {
            prepare_std_typed(&self.inputs(), options).unwrap_or_else(|errors| panic!("{errors:?}"))
        });
        template.generate_for(&options.substrate_path, program)
    }
}

fn std_fixture(root: &Path) -> StdFixture {
    static REPOSITORY: OnceLock<PreparedStd> = OnceLock::new();
    static GOLDEN: OnceLock<PreparedStd> = OnceLock::new();
    static ROOTS: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();
    let (repository, golden) = ROOTS.get_or_init(|| {
        (
            repository_std()
                .canonicalize()
                .expect("repository std root"),
            golden_std().canonicalize().expect("golden std root"),
        )
    });
    if let Ok(canonical) = root.canonicalize() {
        if canonical == *repository {
            return StdFixture::Shared(REPOSITORY.get_or_init(|| PreparedStd::load(root)));
        }
        if canonical == *golden {
            return StdFixture::Shared(GOLDEN.get_or_init(|| PreparedStd::load(root)));
        }
    }
    StdFixture::Fresh(Box::new(PreparedStd::load(root)))
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
    let fixture = std_fixture(std_root);
    let source = SourceFile::new("test.ignite", text);
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let symbols = check_module(&program, &fixture.modules)
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    fixture.types.as_ref().expect("the standard library types");
    let types = type_module(&program, &symbols, &fixture.typed_modules)
        .unwrap_or_else(|d| panic!("typing failed:\n{}", source.render_all(&d)));
    let mut sources = fixture.sources.clone();
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
    let fixture = std_fixture(std_root);
    let (source, program) = parse(file);
    let symbols = check_module(&program, &fixture.modules)
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    fixture.types.as_ref().expect("the standard library types");
    let types = type_module(&program, &symbols, &fixture.typed_modules)
        .unwrap_or_else(|d| panic!("typing failed:\n{}", source.render_all(&d)));
    let mut sources = fixture.sources.clone();
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
    let std = fixture.generate_std_for(&options, &project);
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
    let fixture = std_fixture(std_root);
    let library = &fixture.library;
    let mut modules = Cow::Borrowed(&fixture.modules);
    let (source, program) = parse(file);
    let mut options = Options {
        substrate_path: substrate.to_path_buf(),
        ..Options::default()
    };
    let std_types = fixture.types.as_deref();
    let mut typed_modules = Cow::Borrowed(&fixture.typed_modules);
    let mut typed_siblings: Vec<(String, Program, Symbols, TypedProgram)> = Vec::new();
    let mut sibling_sources: Vec<(String, SourceFile)> = Vec::new();
    for (path, sibling_source, sibling) in sibling_programs(file, &program) {
        let symbols = check_module(&sibling, modules.as_ref())
            .unwrap_or_else(|d| panic!("check failed:\n{}", sibling_source.render_all(&d)));
        if std_types.is_some() {
            let types = type_module(&sibling, &symbols, typed_modules.as_ref())
                .unwrap_or_else(|d| panic!("typing failed:\n{}", sibling_source.render_all(&d)));
            typed_modules.to_mut().insert(path.clone(), types.exports());
            typed_siblings.push((path.clone(), sibling, symbols.clone(), types));
        }
        modules.to_mut().insert(path.clone(), symbols);
        sibling_sources.push((path, sibling_source));
    }
    let symbols = check_module(&program, modules.as_ref())
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let types = std_types.map(|_| type_module(&program, &symbols, typed_modules.as_ref()));
    match (std_types, types) {
        (Some(_), Some(Ok(types))) => {
            options.sources = fixture.sources.clone();
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
            let std = fixture.generate_std_for(&options, &project);
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

#[test]
fn mutable_custom_std_is_reloaded_between_lowerings() {
    let root =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("mutable-std-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("custom.ignite");
    std::fs::write(&file, "package std.custom\nfn value() -> Int = 7\n").unwrap();
    let first = generate_source(
        "import std.custom.value\nfn answer() -> Int = value()\n",
        &root,
    )
    .unwrap();
    assert!(first.lib_rs.contains("pub fn answer() -> i64"));
    let program_file = root.with_extension("ignite");
    std::fs::write(
        &program_file,
        "import std.custom.value\nfn answer() -> Int = value()\n",
    )
    .unwrap();
    let (_, first_std) = lower_typed(&program_file, &root, Path::new("../ignite-io"));
    assert!(first_std.lib_rs.contains("pub fn value() -> i64"));

    std::fs::write(
        &file,
        "package std.custom\nfn value() -> Text = \"updated\"\n",
    )
    .unwrap();
    let second = generate_source(
        "import std.custom.value\nfn answer() -> Text = value()\n",
        &root,
    )
    .unwrap();
    assert!(second.lib_rs.contains("pub fn answer() -> String"));
    std::fs::write(
        &program_file,
        "import std.custom.value\nfn answer() -> Text = value()\n",
    )
    .unwrap();
    let (_, second_std) = lower_typed(&program_file, &root, Path::new("../ignite-io"));
    assert!(second_std.lib_rs.contains("pub fn value() -> String"));

    std::fs::write(&file, "package std.custom\nfn value() -> Int = missing\n").unwrap();
    let failure = std::panic::catch_unwind(|| {
        generate_source(
            "import std.custom.value\nfn answer() -> Int = value()\n",
            &root,
        )
    })
    .expect_err("an edited std diagnostic must not be hidden by a fixture cache");
    let message = failure
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| failure.downcast_ref::<&str>().copied())
        .expect("the checker panic carries its rendered diagnostic");
    assert!(message.contains("E003"), "{message}");
    assert!(message.contains("custom.ignite"), "{message}");
}
