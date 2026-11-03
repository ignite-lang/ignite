//! Every `tests/fixtures/*.ignite` is parsed and checked with `check_module`
//! against the small standard library under `tests/fixtures/std/` (module
//! `std.a.b` is `std/a/b.ignite`); the rendered diagnostics (one per line,
//! trailing newline; empty for a clean file) must equal the sibling
//! `.expected` file. A parse failure fails the test, and so does any
//! diagnostic in the fixture standard library.

use crate::common;

use std::path::{Path, PathBuf};

use ignite_check::Modules;
use ignite_syntax::SourceFile;

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixtures() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(fixture_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "ignite"))
        .collect();
    paths.sort();
    paths
}

fn render(path: &Path, modules: &Modules) -> Result<String, String> {
    let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
    let text = std::fs::read_to_string(path).unwrap();
    let source = SourceFile::new(file_name, text);
    let program = ignite_syntax::parse(&source)
        .map_err(|d| format!("parse failed:\n{}", source.render_all(&d)))?;
    Ok(match ignite_check::check_module(&program, modules) {
        Ok(_) => String::new(),
        Err(diagnostics) => format!("{}\n", source.render_all(&diagnostics)),
    })
}

#[test]
fn fixtures_match_expected() {
    let (_, modules) = common::load_std(&fixture_dir().join("std"))
        .unwrap_or_else(|e| panic!("fixture standard library:\n{e}"));
    assert_eq!(
        modules.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "std.greeting",
            "std.protocol.hello",
            "std.rfc",
            "std.rfc.rfc1",
            "std.salute"
        ]
    );
    let paths = fixtures();
    assert!(!paths.is_empty(), "no fixtures found");
    let mut failures = Vec::new();
    for path in &paths {
        let expected_path = path.with_extension("expected");
        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("{}: {e}", expected_path.display()));
        match render(path, &modules) {
            Ok(actual) if actual == expected => {}
            Ok(actual) => failures.push(format!(
                "{}\n--- expected\n{expected}--- actual\n{actual}",
                path.display()
            )),
            Err(message) => failures.push(format!("{}: {message}", path.display())),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn every_checker_code_has_a_fixture() {
    let names: Vec<String> = fixtures()
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    for code in [
        "e003", "e004", "e006", "e010", "e013", "e015", "e016", "e030", "e031", "e047", "e048",
        "e049", "e050", "e051", "e055", "e056", "e057", "e091", "e099",
    ] {
        assert!(
            names.iter().any(|n| n.starts_with(code)),
            "no fixture for {code}"
        );
    }
}

/// Adversary (slice 15 U1): `text` checked against the fixture standard
/// library as `probe.ignite`, rendered as `fixtures_match_expected` does.
fn check_probe(text: &str) -> String {
    let (_, modules) = common::load_std(&fixture_dir().join("std"))
        .unwrap_or_else(|e| panic!("fixture standard library:\n{e}"));
    let source = SourceFile::new("probe.ignite", text.to_string());
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    match ignite_check::check_module(&program, &modules) {
        Ok(_) => String::new(),
        Err(diagnostics) => format!("{}\n", source.render_all(&diagnostics)),
    }
}

/// Adversary (slice 15 U1, `docs/slice.md` 19.4): "The value of every
/// constant whose declared type is `std.rfc.Requirement` or
/// `std.rfc.Spec` ... is literal data". A constant declared through an
/// alias of `Requirement` has that declared type, so a call in its value
/// is E091 as it is without the alias.
#[test]
fn adversary_e091_through_an_alias_of_requirement() {
    let actual = check_probe(
        "import std.rfc.{Level, Requirement}\n\
         type Req = Requirement\n\
         fn section() -> Text = \"2\"\n\
         const R2_1: Req = Requirement { rfc = 9, section = section(), ordinal = 1, level = Must, text = \"a call\" }\n",
    );
    assert_eq!(
        actual,
        "probe.ignite:4:52: error[E091]: catalogue constant `R2_1` must be literal data, found a call\n"
    );
}

/// Adversary (slice 15 U1, 19.4): the `requirements` of a `Spec` is "a
/// list literal of names of `Requirement` constants of the same module";
/// a `Requirement` constant declared through an alias is one, so a literal
/// catalogue written that way is silent.
#[test]
fn adversary_e091_spec_lists_a_requirement_declared_through_an_alias() {
    let actual = check_probe(
        "import std.rfc.{Level, Requirement, Spec}\n\
         type Req = Requirement\n\
         const R2_1: Req = Requirement { rfc = 9, section = \"2\", ordinal = 1, level = Must, text = \"t\" }\n\
         const RFC9: Spec = Spec { rfc = 9, title = \"t\", url = \"u\", digest = \"d\", requirements = [R2_1] }\n",
    );
    assert_eq!(actual, "");
}

/// Adversary (slice 15 U1, 19.4): a glob import of the vocabulary makes
/// `Requirement` the `std.rfc` record as an explicit import does, so the
/// literal-data rule applies.
#[test]
fn adversary_e091_under_a_glob_import_of_the_vocabulary() {
    let actual = check_probe(
        "import std.rfc.*\n\
         fn section() -> Text = \"2\"\n\
         const R2_1: Requirement = Requirement { rfc = 9, section = section(), ordinal = 1, level = Must, text = \"a call\" }\n",
    );
    assert_eq!(
        actual,
        "probe.ignite:3:60: error[E091]: catalogue constant `R2_1` must be literal data, found a call\n"
    );
}

/// Adversary (slice 15 U1, 19.5, 19.6): the second segment of `Spec.Id`
/// names a constant of the spec's module: a local constant of the same
/// name does not hide the imported spec's requirement, and a local spec
/// does not reach a requirement the program only imports.
#[test]
fn adversary_reqref_resolves_in_the_spec_module_not_by_local_names() {
    let actual = check_probe(
        "import std.rfc.{Level, Requirement, Spec}\n\
         import std.rfc.rfc1.{RFC1, R3_1}\n\
         const R2_1: Int = 1\n\
         const RFC7: Spec = Spec { rfc = 7, title = \"t\", url = \"u\", digest = \"d\", requirements = [R1_1] }\n\
         const R1_1: Requirement = Requirement { rfc = 7, section = \"1\", ordinal = 1, level = Must, text = \"t\" }\n\
         fn shadowed() -> Int satisfies [RFC1.R2_1] = R2_1\n\
         fn imported() -> Int satisfies [RFC7.R3_1] = 1\n",
    );
    assert_eq!(
        actual,
        "probe.ignite:7:38: error[E003]: `RFC7` has no requirement `R3_1`\n"
    );
}
