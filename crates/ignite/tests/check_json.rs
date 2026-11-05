//! `ignite check --json` and `--stdin-for` (`docs/slice.md` section 1.1):
//! one JSON document of diagnostics on stdout, the exit status of the text
//! form, and a program buffer read from stdin while its siblings and the
//! standard library come from disk.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use ignite_eval::Json;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn ignite(args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(args)
        .current_dir(workspace())
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    child.wait_with_output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn fixture(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("check-json-{name}"));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    for (path, contents) in files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
    dir
}

fn arg(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// The diagnostics of a `--json` document; panics unless it is exactly
/// `{"diagnostics": [...]}` followed by one newline.
fn diagnostics(output: &Output) -> Vec<Json> {
    let stdout = text(&output.stdout);
    assert!(stdout.ends_with('\n'), "stdout: {stdout:?}");
    let document = Json::parse(stdout.trim_end()).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    let Json::Object(entries) = &document else {
        panic!("not an object: {stdout}");
    };
    assert_eq!(entries.keys().collect::<Vec<_>>(), ["diagnostics"]);
    let Some(Json::Array(items)) = document.get("diagnostics") else {
        panic!("diagnostics is no array: {stdout}");
    };
    items.clone()
}

fn string<'a>(value: &'a Json, key: &str) -> &'a str {
    match value.get(key) {
        Some(Json::Str(text)) => text,
        other => panic!("{key}: {other:?}"),
    }
}

/// `(line, column, offset)` of `start` or `end`.
fn position(value: &Json, key: &str) -> (i64, i64, i64) {
    let at = value.get(key).unwrap_or_else(|| panic!("no {key}"));
    let int = |name: &str| match at.get(name) {
        Some(Json::Int(n)) => *n,
        other => panic!("{key}.{name}: {other:?}"),
    };
    (int("line"), int("column"), int("offset"))
}

/// Every diagnostic has exactly the keys of the story's shape.
fn assert_shape(diagnostic: &Json) {
    let Json::Object(entries) = diagnostic else {
        panic!("not an object: {diagnostic:?}");
    };
    assert_eq!(
        entries.keys().collect::<Vec<_>>(),
        ["code", "end", "message", "path", "start"]
    );
    for key in ["start", "end"] {
        let Some(Json::Object(at)) = diagnostic.get(key) else {
            panic!("{key}: {diagnostic:?}");
        };
        assert_eq!(at.keys().collect::<Vec<_>>(), ["column", "line", "offset"]);
    }
}

/// `<path>:<line>:<col>: error[<code>]: <message>` split into its parts.
fn text_diagnostic(line: &str) -> (String, i64, i64, String, String) {
    let (head, message) = line.split_once(": error[").expect(line);
    let (code, message) = message.split_once("]: ").expect(line);
    let mut parts = head.rsplitn(3, ':');
    let col = parts.next().unwrap().parse().unwrap();
    let row = parts.next().unwrap().parse().unwrap();
    let path = parts.next().unwrap().to_string();
    (path, row, col, code.to_string(), message.to_string())
}

const CLEAN: &str = "package demo\n\nfn f(x: Int) -> Int {\n  return x\n}\n";
const TYPE_ERROR: &str = "package demo\n\nfn f(x: Int) -> Int {\n  return true\n}\n";

/// Acceptance 1: a type error is one diagnostic whose code, message and
/// start position are the text form's, and whose offsets slice the
/// offending source text.
#[test]
fn type_error_is_one_diagnostic_with_offsets_that_slice_the_source() {
    let dir = fixture("type-error", &[("program.ignite", TYPE_ERROR)]);
    let file = dir.join("program.ignite");
    let plain = ignite(&["check", arg(&file)], None);
    assert_eq!(plain.status.code(), Some(1));
    let stderr = text(&plain.stderr);
    let (path, line, column, code, message) = text_diagnostic(stderr.lines().next().unwrap());

    let output = ignite(&["check", arg(&file), "--json"], None);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
    let found = diagnostics(&output);
    assert_eq!(found.len(), 1, "{found:?}");
    let diagnostic = &found[0];
    assert_shape(diagnostic);
    assert_eq!(string(diagnostic, "path"), path);
    assert_eq!(string(diagnostic, "code"), code);
    assert_eq!(string(diagnostic, "message"), message);
    let start = position(diagnostic, "start");
    let end = position(diagnostic, "end");
    assert_eq!((start.0, start.1), (line, column));
    assert_eq!(
        &TYPE_ERROR[start.2 as usize..end.2 as usize],
        "true",
        "{diagnostic:?}"
    );
    assert_eq!((end.0, end.1), (4, 14));
}

/// Acceptance 2: a clean file is an empty list and exit 0.
#[test]
fn clean_file_is_an_empty_list() {
    let output = ignite(
        &[
            "check",
            "examples/terminal/terminal.design.ignite",
            "--json",
        ],
        None,
    );
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "{\"diagnostics\":[]}\n");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
}

/// Acceptance 3: a syntax error has the same shape.
#[test]
fn syntax_error_has_the_same_shape() {
    let source = "package demo\n\nfn f(x: Int) -> Int {\n  return x +\n";
    let dir = fixture("syntax-error", &[("program.ignite", source)]);
    let output = ignite(&["check", arg(&dir.join("program.ignite")), "--json"], None);
    assert_eq!(output.status.code(), Some(1));
    let found = diagnostics(&output);
    assert!(!found.is_empty());
    for diagnostic in &found {
        assert_shape(diagnostic);
        assert!(
            ["E001", "E002"].contains(&string(diagnostic, "code")),
            "{diagnostic:?}"
        );
        assert!(string(diagnostic, "path").ends_with("program.ignite"));
        let (_, _, start) = position(diagnostic, "start");
        let (_, _, end) = position(diagnostic, "end");
        assert!(
            start <= end && end as usize <= source.len(),
            "{diagnostic:?}"
        );
    }
}

/// Acceptance 4: `--stdin-for PATH` checks the buffer on stdin, not the
/// file on disk, and leaves the file untouched; both ways round.
#[test]
fn stdin_for_checks_the_buffer_and_leaves_the_file() {
    let dir = fixture("stdin-for", &[("program.ignite", CLEAN)]);
    let file = dir.join("program.ignite");
    let output = ignite(
        &["check", "--json", "--stdin-for", arg(&file)],
        Some(TYPE_ERROR),
    );
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    let found = diagnostics(&output);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(string(&found[0], "path"), arg(&file));
    let (_, _, start) = position(&found[0], "start");
    let (_, _, end) = position(&found[0], "end");
    assert_eq!(&TYPE_ERROR[start as usize..end as usize], "true");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), CLEAN);

    std::fs::write(&file, TYPE_ERROR).unwrap();
    let output = ignite(&["check", "--json", "--stdin-for", arg(&file)], Some(CLEAN));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stdout));
    assert_eq!(text(&output.stdout), "{\"diagnostics\":[]}\n");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), TYPE_ERROR);

    let output = ignite(&["check", "--stdin-for", arg(&file)], Some(CLEAN));
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
}

/// Acceptance 5: a diagnostic in a sibling module carries the sibling's
/// path, also when the program comes from stdin.
#[test]
fn sibling_diagnostic_carries_the_sibling_path() {
    let dir = fixture(
        "sibling",
        &[
            ("program.ignite", CLEAN),
            (
                "ops.ignite",
                "package demo.ops\n\nfn g(x: Int) -> Int {\n  return true\n}\n",
            ),
        ],
    );
    let file = dir.join("program.ignite");
    for stdin in [None, Some(CLEAN)] {
        let args: &[&str] = match stdin {
            None => &["check", arg(&file), "--json"],
            Some(_) => &["check", "--json", "--stdin-for", arg(&file)],
        };
        let output = ignite(args, stdin);
        assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
        let found = diagnostics(&output);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_shape(&found[0]);
        assert_eq!(string(&found[0], "path"), arg(&dir.join("ops.ignite")));
    }
}
