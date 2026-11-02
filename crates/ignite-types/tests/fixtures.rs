//! Every `tests/fixtures/*.ignite` is parsed, checked with `check_module` and
//! typed with `type_module` against the small standard library under
//! `tests/fixtures/std/` (module `std.a.b` is `std/a/b.ignite`); the rendered
//! type diagnostics (one per line, trailing newline; empty for a clean file)
//! must equal the sibling `.expected` file. A parse or check failure fails
//! the test, and so does any diagnostic in the fixture standard library.

use crate::common;

use std::path::{Path, PathBuf};

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

fn render(path: &Path, std: &common::Std) -> Result<String, String> {
    let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
    let text = std::fs::read_to_string(path).unwrap();
    let source = SourceFile::new(file_name, text);
    let program = ignite_syntax::parse(&source)
        .map_err(|d| format!("parse failed:\n{}", source.render_all(&d)))?;
    let symbols = ignite_check::check_module(&program, &std.modules)
        .map_err(|d| format!("check failed:\n{}", source.render_all(&d)))?;
    Ok(
        match ignite_types::type_module(&program, &symbols, &std.typed) {
            Ok(typed) => {
                for e in common::expressions(&program) {
                    if typed.try_type_of(e).is_none() {
                        return Err(format!(
                            "typed clean but `{}` has no type",
                            &source.text[e.span.start..e.span.end]
                        ));
                    }
                }
                String::new()
            }
            Err(diagnostics) => format!("{}\n", source.render_all(&diagnostics)),
        },
    )
}

#[test]
fn fixtures_match_expected() {
    let std = common::load_std(&fixture_dir().join("std"))
        .unwrap_or_else(|e| panic!("fixture standard library:\n{e}"));
    assert_eq!(
        std.typed.keys().map(String::as_str).collect::<Vec<_>>(),
        ["std.net", "std.shapes", "std.shapes.protocol"]
    );
    let paths = fixtures();
    assert!(!paths.is_empty(), "no fixtures found");
    let mut failures = Vec::new();
    for path in &paths {
        let expected_path = path.with_extension("expected");
        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("{}: {e}", expected_path.display()));
        match render(path, &std) {
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
fn every_type_code_has_a_fixture() {
    let names: Vec<String> = fixtures()
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    for code in [
        "e005", "e040", "e041", "e042", "e043", "e044", "e045", "e046", "e048", "e017", "e051",
        "e052", "e100", "e101", "e102", "e103", "e104", "e105", "e106", "e107", "e108", "e109",
        "valid",
    ] {
        assert!(
            names.iter().any(|n| n.starts_with(code)),
            "no fixture for {code}"
        );
    }
}
