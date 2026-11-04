use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ignite_perf::perf::safe_export;

/// A fresh git repository under the test target with `files` committed; its root.
fn repository(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("export-{name}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    for (path, content) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    for args in [
        &["init", "-q"][..],
        &["add", "-A"],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "-q",
            "-m",
            "t",
        ],
    ] {
        let status = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }
    root
}

#[test]
fn a_gitattributes_without_export_rules_is_exported() {
    let repo = repository(
        "plain",
        &[("a.txt", "a\n"), ("evidence/.gitattributes", "* -text\n")],
    );
    let out = repo.with_extension("out");
    let _ = fs::remove_dir_all(&out);
    safe_export(&repo, "HEAD", &out).unwrap();
    assert_eq!(fs::read_to_string(out.join("a.txt")).unwrap(), "a\n");
    assert_eq!(
        fs::read_to_string(out.join("evidence/.gitattributes")).unwrap(),
        "* -text\n"
    );
}

#[test]
fn a_gitattributes_with_an_export_rule_is_refused() {
    for (name, rule) in [
        ("ignore", "b.txt export-ignore\n"),
        ("subst", "b.txt export-subst\n"),
    ] {
        let repo = repository(name, &[("b.txt", "b\n"), (".gitattributes", rule)]);
        let out = repo.with_extension("out");
        let _ = fs::remove_dir_all(&out);
        let error = safe_export(&repo, "HEAD", &out).unwrap_err();
        assert!(error.contains("(export attributes)"), "{error}");
    }
}
