//! The probes of the 2026-11-01 soundness audit (`docs/vision.md` section
//! 10, `docs/audit/2026-11-01/`) as frontend regression tests: every
//! `tests/audit/*.ignite` is parsed, checked and typed against the real
//! standard library, and the diagnostics of the first failing stage (one
//! per line; empty when it checks clean) must equal the sibling `.expected`
//! file. The closure variants (`*_closure.ignite`) pass the handle through a
//! captured binding instead of an argument.

use crate::common;

use std::path::{Path, PathBuf};

use ignite_syntax::SourceFile;

fn audit_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/audit")
}

fn render(path: &Path, std: &common::Std) -> String {
    let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
    let source = SourceFile::new(file_name, std::fs::read_to_string(path).unwrap());
    let program = match ignite_syntax::parse(&source) {
        Ok(program) => program,
        Err(d) => return format!("{}\n", source.render_all(&d)),
    };
    let symbols = match ignite_check::check_module(&program, &std.modules) {
        Ok(symbols) => symbols,
        Err(d) => return format!("{}\n", source.render_all(&d)),
    };
    match ignite_types::type_module(&program, &symbols, &std.typed) {
        Ok(_) => String::new(),
        Err(d) => format!("{}\n", source.render_all(&d)),
    }
}

/// Every probe of `docs/audit/2026-11-01/<dir>/<name>.ignite` has a copy
/// `tests/audit/<dir>_<name>.ignite` with the same text.
#[test]
fn copies_match_the_audit_directory() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/audit/2026-11-01");
    let mut checked = 0;
    for dir in ["effects", "io", "llvm"] {
        for entry in std::fs::read_dir(root.join(dir)).unwrap() {
            let original = entry.unwrap().path();
            if original.extension().is_none_or(|ext| ext != "ignite") {
                continue;
            }
            let stem = original.file_stem().unwrap().to_string_lossy();
            let copy = audit_dir().join(format!("{dir}_{stem}.ignite"));
            let copied = std::fs::read_to_string(&copy).unwrap_or_else(|_| {
                panic!("{} has no copy {}", original.display(), copy.display())
            });
            assert_eq!(
                copied,
                std::fs::read_to_string(&original).unwrap(),
                "{} differs from {}",
                copy.display(),
                original.display()
            );
            checked += 1;
        }
    }
    assert!(checked > 50, "only {checked} probes found");
}

#[test]
fn audit_probes_match_expected() {
    let std = common::load_std(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../std"))
        .unwrap_or_else(|e| panic!("standard library:\n{e}"));
    let mut paths: Vec<PathBuf> = std::fs::read_dir(audit_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "ignite"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no probes found");
    let mut failures = Vec::new();
    for path in &paths {
        let expected_path = path.with_extension("expected");
        let expected = std::fs::read_to_string(&expected_path).unwrap_or_default();
        let actual = render(path, &std);
        if actual != expected {
            failures.push(format!(
                "{}\n--- expected\n{expected}--- actual\n{actual}",
                path.display()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
