//! `ignite rfc import` and `ignite rfc check` (`docs/slice.md` 19.8, rows 19 to 23 of
//! 19.0) over the hand-written RFCXML fixture `fixtures/rfc/sample.xml`, whose imported
//! module is the golden `fixtures/rfc/sample.ignite`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

/// The source path as the golden's header records it, relative to the repository root.
const SAMPLE: &str = "crates/ignite/tests/fixtures/rfc/sample.xml";
const GOLDEN: &str = "crates/ignite/tests/fixtures/rfc/sample.ignite";

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rfc")
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("rfc")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
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

fn golden() -> String {
    std::fs::read_to_string(repository().join(GOLDEN)).unwrap()
}

/// The import of the fixture with `flags`, run from the repository root: stdout, after
/// asserting exit 0 and an empty stderr.
fn import(flags: &[&str]) -> String {
    let mut args = vec!["rfc", "import", SAMPLE];
    args.extend(flags);
    let output = ignite(&repository(), &args);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stderr), "");
    text(&output.stdout)
}

/// Each `Requirement` constant of a module text: its id and its whole declaration.
fn requirements(module: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut rest = module;
    while let Some(start) = rest.find("\nconst R") {
        let decl = &rest[start + 1..];
        let end = decl.find("\n}\n").map_or(decl.len(), |end| end + 3);
        let decl = &decl[..end];
        if let Some(head) = decl.strip_prefix("const ")
            && let Some((id, _)) = head.split_once(": Requirement = Requirement {")
        {
            found.push((id.to_string(), decl.to_string()));
        }
        rest = &rest[start + 1 + end..];
    }
    found
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[test]
fn import_writes_the_golden_twice() {
    let dir = scratch("twice");
    let out = dir.join("sample.ignite");
    let out_arg = out.to_str().unwrap();
    for run in 1..=2 {
        let output = ignite(
            &repository(),
            &[
                "rfc",
                "import",
                SAMPLE,
                "--package",
                "tests.rfc.sample",
                "--out",
                out_arg,
            ],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "run {run}: {}",
            text(&output.stderr)
        );
        assert_eq!(text(&output.stdout), "", "run {run}");
        assert_eq!(text(&output.stderr), "", "run {run}");
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            golden(),
            "run {run}"
        );
    }
    assert_eq!(import(&["--package", "tests.rfc.sample"]), golden());
}

#[test]
fn imported_golden_checks_silently() {
    let output = ignite(&repository(), &["check", GOLDEN]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "");
    assert_eq!(text(&output.stderr), "");
}

#[test]
fn lowercase_must_is_no_requirement() {
    let source = std::fs::read_to_string(repository().join(SAMPLE)).unwrap();
    assert!(source.contains("An implementation must keep this lowercase word as prose."));
    let module = import(&[]);
    assert!(!module.contains("lowercase"), "{module}");
    assert!(!module.contains("A client must not care how."), "{module}");
    assert!(
        requirements(&module)
            .iter()
            .all(|(_, decl)| !decl.contains(" must ")),
        "{module}"
    );
}

#[test]
fn marked_must_is_one_requirement() {
    let module = import(&[]);
    let sentence = "A server MUST echo every line, and it SHOULD do so at once.";
    let holding: Vec<_> = requirements(&module)
        .into_iter()
        .filter(|(_, decl)| decl.contains(sentence))
        .collect();
    assert_eq!(holding.len(), 1, "{module}");
    let (id, decl) = &holding[0];
    assert_eq!(id, "R2_1_1");
    assert!(decl.contains("\n  level = Must\n"), "{decl}");
    assert!(decl.contains("\n  party = Server\n"), "{decl}");
    assert!(
        decl.contains(&format!("\n  text = \"{sentence}\"\n")),
        "{decl}"
    );
}

#[test]
fn sections_filter_keeps_ordinals() {
    let all = import(&[]);
    let filtered = import(&["--sections", "2"]);
    assert!(
        filtered.starts_with(&format!(
            "// RFC 7001: A Line Echo & Its Keywords\n// ignite rfc import {SAMPLE} --sections 2\n"
        )),
        "{filtered}"
    );
    assert!(
        filtered.contains("\npackage std.rfc.rfc7001\n"),
        "{filtered}"
    );
    let kept: Vec<(String, String)> = requirements(&all)
        .into_iter()
        .filter(|(_, decl)| decl.contains("\n  section = \"2."))
        .collect();
    let ids: Vec<&str> = kept.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "R2_1_1", "R2_1_2", "R2_1_3", "R2_1_4", "R2_2_1", "R2_2_2", "R2_2_3"
        ]
    );
    assert_eq!(requirements(&filtered), kept, "{filtered}");
    let listed: String = ids.iter().map(|id| format!("    {id},\n")).collect();
    let listed = format!(
        "  requirements = [\n{}\n  ]\n",
        listed.trim_end_matches(",\n")
    );
    assert!(filtered.contains(&listed), "{filtered}");
    assert!(!filtered.contains("R3_"), "{filtered}");
    assert!(!filtered.contains("R1_1"), "{filtered}");
}

#[test]
fn check_matches_the_source() {
    let output = ignite(
        &fixtures(),
        &["rfc", "check", "sample.ignite", "sample.xml"],
    );
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "");
    assert_eq!(text(&output.stderr), "");
}

#[test]
fn check_names_the_catalogue_when_a_byte_changes() {
    let dir = scratch("changed");
    let mut bytes = std::fs::read(fixtures().join("sample.xml")).unwrap();
    let recorded = digest(&bytes);
    assert!(golden().contains(&format!("  digest = \"{recorded}\"\n")));
    let at = bytes.len() / 2;
    bytes[at] ^= 0x01;
    let changed = dir.join("sample.xml");
    std::fs::write(&changed, &bytes).unwrap();
    let actual = digest(&bytes);
    let output = ignite(
        &fixtures(),
        &["rfc", "check", "sample.ignite", changed.to_str().unwrap()],
    );
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(text(&output.stdout), "");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("sample.ignite: "), "{stderr}");
    assert!(stderr.contains(&recorded), "{stderr}");
    assert!(stderr.contains(&actual), "{stderr}");
}

#[test]
fn check_without_source_is_unchecked() {
    let output = ignite(&fixtures(), &["rfc", "check", "sample.ignite"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "sample.ignite: unchecked\n");
    assert_eq!(text(&output.stderr), "");
}

#[test]
fn malformed_xml_is_exit_2() {
    let dir = scratch("malformed");
    let cases = [
        (
            "unclosed.xml",
            "<rfc number=\"1\">\n  <middle>\n    <t>text</t>\n</rfc>\n",
            "2:3",
        ),
        (
            "entity.xml",
            "<rfc number=\"1\">\n  <middle><t>a&nbsp;b</t></middle>\n</rfc>\n",
            "2:15",
        ),
        (
            "subset.xml",
            "<?xml version=\"1.0\"?>\n<!DOCTYPE rfc [\n  <!ENTITY x \"y\">\n]>\n<rfc number=\"1\"/>\n",
            "2:15",
        ),
    ];
    for (name, source, position) in cases {
        let path = dir.join(name);
        std::fs::write(&path, source).unwrap();
        let path = path.to_str().unwrap();
        let output = ignite(&dir, &["rfc", "import", path]);
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{name}: {stderr}");
        assert_eq!(text(&output.stdout), "", "{name}");
        assert_eq!(stderr.lines().count(), 1, "{name}: {stderr}");
        let prefix = format!("{path}:{position}: ");
        assert!(stderr.starts_with(&prefix), "{name}: {stderr}");
        assert!(stderr.trim_end().len() > prefix.len(), "{name}: {stderr}");
    }
}

#[test]
fn missing_file_is_exit_3() {
    let dir = scratch("missing");
    let output = ignite(&dir, &["rfc", "import", "absent.xml"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "");
    let output = ignite(&dir, &["rfc", "check", "absent.ignite"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output.stderr));
    let catalogue = fixtures().join("sample.ignite");
    let output = ignite(
        &dir,
        &["rfc", "check", catalogue.to_str().unwrap(), "absent.xml"],
    );
    assert_eq!(output.status.code(), Some(3), "{}", text(&output.stderr));
    let output = ignite(
        &repository(),
        &[
            "rfc",
            "import",
            SAMPLE,
            "--out",
            dir.join("no/such/dir/out.ignite").to_str().unwrap(),
        ],
    );
    assert_eq!(output.status.code(), Some(3), "{}", text(&output.stderr));
}

#[test]
fn root_without_number_and_catalogue_without_spec_are_exit_2() {
    let dir = scratch("usage");
    let root = dir.join("root.xml");
    std::fs::write(&root, "<rfc version=\"3\">\n</rfc>\n").unwrap();
    let output = ignite(&dir, &["rfc", "import", root.to_str().unwrap()]);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.starts_with(&format!("{}:1:1: ", root.display())),
        "{stderr}"
    );
    let catalogue = dir.join("none.ignite");
    std::fs::write(&catalogue, "package none\n\nconst X: Int = 1\n").unwrap();
    let output = ignite(&dir, &["rfc", "check", catalogue.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "");
    let output = ignite(&repository(), &["rfc", "import", "--sections", ",", SAMPLE]);
    assert_eq!(output.status.code(), Some(2), "{}", text(&output.stderr));
}
