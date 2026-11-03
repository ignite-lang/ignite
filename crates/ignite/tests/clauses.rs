//! A clause-bearing program (`docs/slice.md` 19.0 row 12): it checks
//! silently, its tests pass on both targets, and its clauses (`conforms`,
//! `scope`, statuses, `satisfies`, `verifies`) change no byte that either
//! target emits.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::common;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clauses/claims.ignite")
}

fn tmp(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn ignite(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run ignite")
}

#[test]
fn clause_fixture_checks_silently() {
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("check")
        .arg(fixture())
        .output()
        .expect("run ignite check");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "");
    assert_eq!(text(&output.stderr), "");
}

#[test]
fn clause_fixture_tests_pass_on_both_targets() {
    for target in ["rust", "llvm"] {
        let dir = tmp(&format!("clauses-test-{target}"));
        let mut command = Command::new(env!("CARGO_BIN_EXE_ignite"));
        command
            .arg("test")
            .arg(fixture())
            .args(["--target", target, "--out"])
            .arg(dir.join("out"))
            .arg("--runtime")
            .arg(common::runner_archive());
        if target == "rust" {
            command.arg("--target-dir").arg(dir.join("cargo"));
        }
        let output = command.output().expect("run ignite test");
        let stdout = text(&output.stdout);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{target}: stdout: {stdout}\nstderr: {}",
            text(&output.stderr)
        );
        assert!(
            stdout.contains("test result: ok. 2 passed; 0 failed;"),
            "{target}: {stdout}"
        );
    }
}

/// The ranges of every clause of `source`: from the clause word through
/// its closing bracket or brace, and each status line from its first
/// non-blank character to its end. The fixture writes no clause word
/// anywhere else.
fn clause_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for word in [" conforms [", " scope {", " satisfies [", " verifies ["] {
        let close = if word.ends_with('{') { '}' } else { ']' };
        let open = word.chars().last().unwrap();
        let mut from = 0;
        while let Some(at) = source[from..].find(word) {
            let start = from + at + 1;
            let mut depth = 0;
            let mut end = start;
            for (offset, c) in source[start..].char_indices() {
                if c == open {
                    depth += 1;
                } else if c == close {
                    depth -= 1;
                    if depth == 0 {
                        end = start + offset + 1;
                        break;
                    }
                }
            }
            assert!(end > start, "unclosed clause at byte {start}");
            ranges.push((start, end));
            from = end;
        }
    }
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("unsupported ") || trimmed.starts_with("not_applicable ") {
            let start = offset + line.len() - trimmed.len();
            ranges.push((start, offset + line.trim_end_matches('\n').len()));
        }
        offset += line.len();
    }
    ranges
}

/// `source` with every character of every clause, except a newline, a
/// space, so that every other token keeps its line and column.
fn blanked(source: &str) -> String {
    let ranges = clause_ranges(source);
    source
        .char_indices()
        .map(|(i, c)| {
            if c != '\n'
                && ranges
                    .iter()
                    .any(|(start, end)| (*start..*end).contains(&i))
            {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// `ignite emit`'s stdout and every `.ll` file `ignite build --target
/// llvm` writes under `out`, by relative path.
fn emitted(dir: &Path) -> (String, Vec<(String, String)>) {
    let emit = ignite(dir, &["emit", "claims.ignite"]);
    assert_eq!(emit.status.code(), Some(0), "{}", text(&emit.stderr));
    let out = dir.join("out");
    let _ = std::fs::remove_dir_all(&out);
    let runtime = common::runtime_archive();
    let build = ignite(
        dir,
        &[
            "build",
            "--target",
            "llvm",
            "claims.ignite",
            "--out",
            "out",
            "--runtime",
            runtime.to_str().expect("runtime path"),
        ],
    );
    assert_eq!(build.status.code(), Some(0), "{}", text(&build.stderr));
    let mut files = Vec::new();
    let mut pending = vec![out.clone()];
    while let Some(path) = pending.pop() {
        for entry in std::fs::read_dir(&path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "ll") {
                let name = path.strip_prefix(&out).unwrap().display().to_string();
                files.push((name, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    files.sort();
    (text(&emit.stdout), files)
}

#[test]
fn clauses_emit_nothing_on_either_target() {
    let dir = tmp("clauses-emit");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = std::fs::read_to_string(fixture()).unwrap();
    let blank = blanked(&source);
    assert_eq!(blank.len(), source.len());
    for word in [
        "conforms",
        "scope",
        "satisfies",
        "verifies",
        "unsupported",
        "not_applicable",
    ] {
        assert!(source.contains(word), "the fixture has no `{word}`");
        assert!(!blank.contains(word), "the blanked copy still has `{word}`");
    }
    std::fs::write(dir.join("claims.ignite"), &source).unwrap();
    let (rust, ll) = emitted(&dir);
    std::fs::write(dir.join("claims.ignite"), &blank).unwrap();
    let (rust_blank, ll_blank) = emitted(&dir);
    assert!(!rust.is_empty());
    assert_eq!(rust, rust_blank, "ignite emit differs without the clauses");
    let names: Vec<&str> = ll.iter().map(|(name, _)| name.as_str()).collect();
    assert!(names.contains(&"llvm/std.ll"), "{names:?}");
    assert!(names.contains(&"llvm/program.ll"), "{names:?}");
    assert_eq!(
        names,
        ll_blank.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
    );
    for ((name, with), (_, without)) in ll.iter().zip(&ll_blank) {
        assert!(with == without, "{name} differs without the clauses");
    }
}
