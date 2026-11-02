//! Every `tests/fixtures/*.ignite` parses to exactly the diagnostics in its
//! sibling `.expected` file: rendered with the file name only, one per line,
//! with a trailing newline. An empty `.expected` means the file is valid.

use std::fs;
use std::path::Path;

use ignite_syntax::{SourceFile, parse};

fn render(name: &str, text: String) -> String {
    let source = SourceFile::new(name, text);
    match parse(&source) {
        Ok(_) => String::new(),
        Err(diagnostics) => format!("{}\n", source.render_all(&diagnostics)),
    }
}

/// On a thread with a large stack: `e054_nesting.ignite` nests 257 levels,
/// more than the debug parser fits in a test thread's 2 MiB.
#[test]
fn fixtures_match_expected_diagnostics() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(check_fixtures)
        .expect("thread starts")
        .join()
        .expect("fixtures match");
}

fn check_fixtures() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .expect("fixtures directory is readable")
        .map(|e| e.expect("directory entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "ignite"))
        .collect();
    entries.sort();
    assert!(entries.len() >= 7, "expected at least 7 fixtures");
    let mut failures = Vec::new();
    for path in &entries {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("UTF-8 file name");
        let text = fs::read_to_string(path).expect("fixture is readable");
        let expected_path = path.with_extension("expected");
        let expected = fs::read_to_string(&expected_path)
            .unwrap_or_else(|_| panic!("missing {}", expected_path.display()));
        let actual = render(name, text);
        if actual != expected {
            failures.push(format!(
                "{name}\n--- expected ---\n{expected}--- actual ---\n{actual}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
