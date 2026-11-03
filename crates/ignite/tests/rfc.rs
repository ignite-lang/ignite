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

// Adversary cases for U3 (`docs/slice.md` 19.8, 19.9). Each drives the importer from the
// contract's text over a small hand-written fixture `fixtures/rfc/adversary_*.xml` or a
// source written into a scratch directory.

/// The import of the fixture `fixtures/rfc/<name>` with `flags`, run from the repository
/// root: stdout, after asserting exit 0 and an empty stderr.
fn adversary_import(name: &str, flags: &[&str]) -> String {
    let source = format!("crates/ignite/tests/fixtures/rfc/{name}");
    let mut args = vec!["rfc", "import", source.as_str()];
    args.extend(flags);
    let output = ignite(&repository(), &args);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stderr), "");
    text(&output.stdout)
}

/// The declaration `ignite rfc import` writes for one requirement (19.8, "Output" item 4);
/// `text` is the Text literal as written, quotes included.
fn requirement(
    rfc: u32,
    id: &str,
    section: &str,
    ordinal: usize,
    level: &str,
    party: &str,
    text: &str,
) -> (String, String) {
    (
        id.to_string(),
        format!(
            "const {id}: Requirement = Requirement {{\n  rfc = {rfc}\n  section = \"{section}\"\n  ordinal = {ordinal}\n  level = {level}\n  party = {party}\n  text = {text}\n}}\n"
        ),
    )
}

/// `ignite rfc import` of `source` written to `dir/name`, run in `dir`.
fn import_source(dir: &Path, name: &str, source: &[u8], flags: &[&str]) -> Output {
    std::fs::write(dir.join(name), source).unwrap();
    let mut args = vec!["rfc", "import", name];
    args.extend(flags);
    ignite(dir, &args)
}

/// 19.8 "Reader": the five predefined entities and numeric references are decoded once
/// (`&amp;amp;` is the text `&amp;`); comments, processing instructions and CDATA sections
/// inside a paragraph are skipped; attributes may be single-quoted; a DOCTYPE literal may
/// hold `[`. "Extraction" 3: marks inside `<em>` and inside an `<xref>` with content count.
#[test]
fn adversary_reader_decodes_entities_once_and_skips_markup() {
    let module = adversary_import("adversary_markup.xml", &[]);
    assert!(
        module.starts_with("// RFC 7002: Markup &amp; Entities\n"),
        "{module}"
    );
    assert!(
        module.contains("\n  title = \"Markup &amp; Entities\"\n"),
        "{module}"
    );
    assert!(
        module.contains("\n  url = \"https://example.invalid/rfc7002\"\n"),
        "{module}"
    );
    assert_eq!(
        requirements(&module),
        [
            requirement(
                7002,
                "R1_1",
                "1",
                1,
                "Must",
                "Server",
                "\"A server MUST keep &amp; as written, decode ABC and <x> once.\""
            ),
            requirement(
                7002,
                "R1_2",
                "1",
                2,
                "MustNot",
                "Client",
                "\"A client MUST NOT see [say \\\"R\\\"] as 'data'.\""
            ),
            requirement(
                7002,
                "R1_3",
                "1",
                3,
                "MustNot",
                "Sender",
                "\"A sender MUST NOT split, and a proxy SHALL wait.\""
            ),
            requirement(
                7002,
                "R1_4",
                "1",
                4,
                "May",
                "Server",
                "\"See a server MAY retry.\""
            ),
        ],
        "{module}"
    );
}

/// 19.8 "Reader": "the five predefined entities and numeric character references"; "Any
/// other entity ... is an error", exit 2, one line. XML's CharRef is `&#` digits `;` or
/// `&#x` hex digits `;`: a sign is no part of it.
#[test]
#[ignore = "adversary: `&#+66;` and `&#x+42;` are decoded as `B` instead of exit 2 (u32 parsing accepts a leading +)"]
fn adversary_signed_character_reference_is_malformed() {
    let dir = scratch("adversary-signed");
    for (name, source) in [
        ("decimal.xml", "<rfc number=\"1\">A&#+66;C</rfc>\n"),
        ("hex.xml", "<rfc number=\"1\">A&#x+42;C</rfc>\n"),
    ] {
        let output = import_source(&dir, name, source.as_bytes(), &["--url", "u"]);
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{name}: {stderr}");
        assert_eq!(text(&output.stdout), "", "{name}");
        assert_eq!(stderr.lines().count(), 1, "{name}: {stderr}");
        assert!(
            stderr.starts_with(&format!("{name}:1:18: ")),
            "{name}: {stderr}"
        );
    }
}

/// 19.8 "Reader": "The error is `<file>:<line>:<col>: <message>` on stderr, exit 2"; row 23:
/// "one line `<file>:<line>:<col>: <message>`". An unescaped `&` in prose whose next `;` is
/// on a later line must still give one line.
#[test]
#[ignore = "adversary: the unknown-entity message quotes the text up to the next `;`, line breaks included, so stderr has two lines"]
fn adversary_entity_error_is_one_line() {
    let dir = scratch("adversary-entity-line");
    let source = "<rfc number=\"1\">Tom & Jerry\nsay; hi</rfc>\n";
    let output = import_source(&dir, "amp.xml", source.as_bytes(), &["--url", "u"]);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert_eq!(text(&output.stdout), "");
    assert!(stderr.starts_with("amp.xml:1:21: "), "{stderr}");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
}

/// 19.8 "Extraction" 3: a block's text is its character data with the markup removed, an
/// `<xref>` or `<eref>` with content rendered as the content, and then "Runs of whitespace
/// become one space". Whitespace at the edge of an element's content is still whitespace of
/// the block, so words on either side stay apart.
#[test]
#[ignore = "adversary: xref/eref content is trimmed on its own, so `read<xref> Section 3 </xref>and` renders `readSection 3and`"]
fn adversary_inline_element_content_keeps_its_spaces() {
    let dir = scratch("adversary-inline-space");
    let source = "<rfc number=\"7006\">\n  <link rel=\"alternate\" href=\"u\"/>\n  <middle>\n    <section pn=\"section-1\">\n      <t pn=\"section-1-1\">A server <bcp14>MUST</bcp14> read<xref target=\"s\"> Section 3 </xref>and<eref target=\"u\"> the notes </eref>now.</t>\n    </section>\n  </middle>\n</rfc>\n";
    let output = import_source(&dir, "space.xml", source.as_bytes(), &[]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let module = text(&output.stdout);
    assert_eq!(
        requirements(&module),
        [requirement(
            7006,
            "R1_1",
            "1",
            1,
            "Must",
            "Server",
            "\"A server MUST read Section 3 and the notes now.\""
        )],
        "{module}"
    );
}

/// 19.8 "Extraction" 2, 5 and 7: every keyword's level, the first mark's keyword for a
/// sentence with two, no requirement from lowercase keywords, the last party word before
/// the first mark (not after it), a `<th>` block, and no requirement from a `<dt>`.
#[test]
fn adversary_every_keyword_gives_its_level_and_party() {
    let module = adversary_import("adversary_levels.xml", &[]);
    let found = requirements(&module);
    let expected = [
        (
            "R1_1",
            "1",
            1,
            "ShallNot",
            "Sender",
            "The sender SHALL NOT wait.",
        ),
        (
            "R1_2",
            "1",
            2,
            "ShouldNot",
            "Server",
            "Clients and servers SHOULD NOT retry.",
        ),
        (
            "R1_3",
            "1",
            3,
            "NotRecommended",
            "Any",
            "Echoing is NOT RECOMMENDED for caches.",
        ),
        (
            "R1_4",
            "1",
            4,
            "Recommended",
            "Any",
            "Caching is RECOMMENDED.",
        ),
        (
            "R1_5",
            "1",
            5,
            "Required",
            "Client",
            "A user agent is REQUIRED to stop.",
        ),
        ("R1_6", "1", 6, "Optional", "Any", "A reply is OPTIONAL."),
        ("R1_7", "1", 7, "May", "Any", "It MAY be sent by a server."),
        (
            "R1_8",
            "1",
            8,
            "Should",
            "Proxy",
            "A gateway SHOULD log it and MUST drop it.",
        ),
        (
            "R2_1",
            "2",
            1,
            "Must",
            "Recipient",
            "A recipient MUST read the head.",
        ),
    ]
    .map(|(id, section, ordinal, level, party, sentence)| {
        requirement(
            7003,
            id,
            section,
            ordinal,
            level,
            party,
            &format!("\"{sentence}\""),
        )
    });
    assert_eq!(found, expected, "{module}");
    assert!(!module.contains("lowercase"), "{module}");
    assert!(!module.contains("never see this term"), "{module}");
}

/// 19.8 "Extraction" 1 and 6 and "--sections": the innermost section's `pn`, ordinals per
/// section over the whole file, blocks after a subsection back in their own section; a
/// requirement is kept when its section equals an entry or starts with it followed by `.`,
/// so `2` keeps neither `20` nor anything outside 2, and overlapping entries keep each
/// requirement once.
#[test]
fn adversary_nested_and_overlapping_sections() {
    let all = adversary_import("adversary_sections.xml", &[]);
    let words = ["one", "two", "three", "four", "five", "six", "seven"];
    let places = [
        ("R2_1", "2", 1),
        ("R2_1_1", "2.1", 1),
        ("R2_1_1_1", "2.1.1", 1),
        ("R2_1_2", "2.1", 2),
        ("R2_2", "2", 2),
        ("R20_1", "20", 1),
        ("R3_1", "3", 1),
    ];
    let expected: Vec<_> = places
        .iter()
        .zip(words)
        .map(|(&(id, section, ordinal), word)| {
            requirement(
                7004,
                id,
                section,
                ordinal,
                "Must",
                "Server",
                &format!("\"A server MUST do {word}.\""),
            )
        })
        .collect();
    assert_eq!(requirements(&all), expected, "{all}");
    let two = adversary_import("adversary_sections.xml", &["--sections", "2"]);
    assert_eq!(requirements(&two), expected[..5], "{two}");
    let overlapping = adversary_import("adversary_sections.xml", &["--sections", "2.1,2,2.1.1"]);
    assert_eq!(requirements(&overlapping), expected[..5], "{overlapping}");
    let picked = adversary_import("adversary_sections.xml", &["--sections", "2.1.1,20"]);
    assert_eq!(
        requirements(&picked),
        [expected[2].clone(), expected[5].clone()],
        "{picked}"
    );
    assert!(
        picked.contains("  requirements = [\n    R2_1_1_1,\n    R20_1\n  ]\n"),
        "{picked}"
    );
}

/// 19.8 "Output is deterministic" and "Digest": a CRLF copy of the sample imports twice to
/// identical bytes, to the same module as the LF original except the digest line, and the
/// digest is the sha256 of the CRLF bytes as they are on disk.
#[test]
fn adversary_crlf_source_imports_the_same_module() {
    let lf = std::fs::read_to_string(fixtures().join("sample.xml")).unwrap();
    assert!(!lf.contains('\r'));
    let crlf = lf.replace('\n', "\r\n");
    let lf_dir = scratch("adversary-lf");
    let crlf_dir = scratch("adversary-crlf");
    let flags = ["--package", "tests.rfc.sample"];
    let lf_out = import_source(&lf_dir, "sample.xml", lf.as_bytes(), &flags);
    assert_eq!(lf_out.status.code(), Some(0), "{}", text(&lf_out.stderr));
    let mut modules = Vec::new();
    for run in 1..=2 {
        let output = import_source(&crlf_dir, "sample.xml", crlf.as_bytes(), &flags);
        assert_eq!(
            output.status.code(),
            Some(0),
            "run {run}: {}",
            text(&output.stderr)
        );
        assert_eq!(text(&output.stderr), "", "run {run}");
        modules.push(output.stdout);
    }
    assert_eq!(modules[0], modules[1]);
    let crlf_module = text(&modules[0]);
    let lf_module = text(&lf_out.stdout);
    let crlf_digest = format!("  digest = \"{}\"\n", digest(crlf.as_bytes()));
    let lf_digest = format!("  digest = \"{}\"\n", digest(lf.as_bytes()));
    assert!(crlf_module.contains(&crlf_digest), "{crlf_module}");
    assert!(lf_module.contains(&lf_digest), "{lf_module}");
    assert_eq!(crlf_module.replace(&crlf_digest, &lf_digest), lf_module);
}

/// 19.8 "Output": "Texts escape `\` as `\\`, `"` as `\"`, `{` as `{{` and `}` as `}}`";
/// a title, a URL and a text full of them, non-ASCII and C1/DEL control characters, write a
/// module that `ignite check` passes silently.
#[test]
fn adversary_escaped_output_checks_silently() {
    let module = adversary_import("adversary_escapes.xml", &["--package", "tests.rfc.escapes"]);
    assert!(
        module.contains("\n  title = \"Esc {{a}} \\\"b\\\" \\\\c\"\n"),
        "{module}"
    );
    assert!(
        module.contains("\n  url = \"https://example.invalid/rfc7005?a=\\\"b\\\"&c={{d}}\"\n"),
        "{module}"
    );
    assert_eq!(
        requirements(&module),
        [requirement(
            7005,
            "R1_1",
            "1",
            1,
            "Must",
            "Server",
            "\"A server MUST keep \\\\ and \\\\\\\\ and \\\"q\\\" and {{x}} and {{{{y}}}} and }}}} and caf\u{e9} \u{65e5}\u{672c} and \\u{80}\\u{9f}\\u{7f} as data.\""
        )],
        "{module}"
    );
    let dir = scratch("adversary-escapes");
    let path = dir.join("escapes.ignite");
    std::fs::write(&path, &module).unwrap();
    let output = ignite(&repository(), &["check", path.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "");
    assert_eq!(text(&output.stderr), "");
}

/// 19.8 "Output" item 3 with no requirement left (`--sections` matching nothing): the
/// module still checks silently.
#[test]
fn adversary_empty_selection_checks_silently() {
    let module = adversary_import(
        "adversary_levels.xml",
        &["--sections", "9", "--package", "tests.rfc.empty"],
    );
    assert!(requirements(&module).is_empty(), "{module}");
    assert!(module.contains("\n  requirements = []\n"), "{module}");
    let dir = scratch("adversary-empty");
    let path = dir.join("empty.ignite");
    std::fs::write(&path, &module).unwrap();
    let output = ignite(&repository(), &["check", path.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stderr), "");
}

/// 19.8 "`rfc check`": it compares only the single `Spec`'s `digest` with the digest of
/// SOURCE's bytes, for any source file. A hand-edited text or list keeps exit 0; a wrong
/// digest is exit 1 and one line naming the catalogue and both digests; no SOURCE is
/// `unchecked`; two `Spec` constants or a `Spec` without `digest` are exit 2.
#[test]
fn adversary_check_reads_only_the_digest() {
    let dir = scratch("adversary-check");
    std::fs::copy(fixtures().join("sample.xml"), dir.join("sample.xml")).unwrap();
    let golden = golden();
    let sentence = "A server MUST echo every line, and it SHOULD do so at once.";
    assert!(golden.contains(sentence));
    assert!(golden.contains("    R2_1_2,\n"));
    let edited = golden
        .replace(sentence, "Edited by hand.")
        .replacen("    R2_1_2,\n", "", 1);
    std::fs::write(dir.join("edited.ignite"), &edited).unwrap();
    let output = ignite(&dir, &["rfc", "check", "edited.ignite", "sample.xml"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "");
    assert_eq!(text(&output.stderr), "");
    let output = ignite(&dir, &["rfc", "check", "edited.ignite"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "edited.ignite: unchecked\n");
    assert_eq!(text(&output.stderr), "");

    let recorded = digest(&std::fs::read(dir.join("sample.xml")).unwrap());
    let wrong = format!("sha256:{}", "0".repeat(64));
    std::fs::write(dir.join("wrong.ignite"), golden.replace(&recorded, &wrong)).unwrap();
    let output = ignite(&dir, &["rfc", "check", "wrong.ignite", "sample.xml"]);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(text(&output.stdout), "");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.contains("wrong.ignite"), "{stderr}");
    assert!(stderr.contains(&wrong), "{stderr}");
    assert!(stderr.contains(&recorded), "{stderr}");

    let notes = b"plain text\r\nof an old RFC\r\n";
    std::fs::write(dir.join("rfc1.txt"), notes).unwrap();
    std::fs::write(
        dir.join("prose.ignite"),
        golden.replace(&recorded, &digest(notes)),
    )
    .unwrap();
    let output = ignite(&dir, &["rfc", "check", "prose.ignite", "rfc1.txt"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(text(&output.stderr), "");

    let second = format!(
        "{golden}\nconst RFC7002: Spec = Spec {{\n  rfc = 7002\n  title = \"t\"\n  url = \"u\"\n  digest = \"{recorded}\"\n  requirements = []\n}}\n"
    );
    std::fs::write(dir.join("two.ignite"), second).unwrap();
    let output = ignite(&dir, &["rfc", "check", "two.ignite", "sample.xml"]);
    assert_eq!(output.status.code(), Some(2), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "");

    let line = format!("  digest = \"{recorded}\"\n");
    assert!(golden.contains(&line));
    std::fs::write(dir.join("nodigest.ignite"), golden.replace(&line, "")).unwrap();
    let output = ignite(&dir, &["rfc", "check", "nodigest.ignite"]);
    assert_eq!(output.status.code(), Some(2), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "");
}

/// 19.8 "Reader": "a file over 16 MiB is an error", `<file>:<line>:<col>: <message>`, exit 2;
/// a file of exactly 16 MiB is read.
#[test]
fn adversary_size_limit_is_sixteen_mib() {
    const LIMIT: usize = 16 << 20;
    let dir = scratch("adversary-size");
    let head = "<rfc number=\"1\">\n<!--";
    let tail = "-->\n</rfc>\n";
    let source = |size: usize| {
        let mut bytes = head.as_bytes().to_vec();
        bytes.resize(size - tail.len(), b'x');
        bytes.extend_from_slice(tail.as_bytes());
        assert_eq!(bytes.len(), size);
        bytes
    };
    let output = import_source(&dir, "limit.xml", &source(LIMIT), &["--url", "u"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(
        text(&output.stdout).contains("\n  requirements = []\n"),
        "{}",
        text(&output.stdout)
    );
    let output = import_source(&dir, "over.xml", &source(LIMIT + 1), &["--url", "u"]);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert_eq!(text(&output.stdout), "");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("over.xml:1:1: "), "{stderr}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// 19.8: "Exit 0 when the module is written; 2 on a usage error". A `--package` that is no
/// package name writes a module whose first declaration does not parse; it should be a
/// usage error instead. Outside the letter of 19.8, which does not list `--package` values.
#[test]
#[ignore = "adversary: `--package 'not a package'` exits 0 and writes `package not a package`, which does not parse"]
fn adversary_invalid_package_is_a_usage_error() {
    let output = ignite(
        &repository(),
        &["rfc", "import", SAMPLE, "--package", "not a package"],
    );
    let module = text(&output.stdout);
    assert_eq!(output.status.code(), Some(2), "{module}");
    assert_eq!(module, "");
}

/// The 16 MiB bound of 19.8 bounds the work: a 2.4 MB block of 100,000 marks and 100,000
/// sentences without one imports in well under 20 s. Extraction matches every mark of a
/// block against every sentence, which is quadratic in the block.
#[test]
#[ignore = "adversary: extraction scans every mark of a block per sentence, quadratic; a 2.4 MB block takes far over 20 s"]
fn adversary_import_time_is_linear_in_the_block() {
    let dir = scratch("adversary-quadratic");
    let mut source = String::from(
        "<rfc number=\"7007\">\n<middle>\n<section pn=\"section-1\">\n<t pn=\"section-1-1\">A",
    );
    for _ in 0..100_000 {
        source.push_str(" <bcp14>MAY</bcp14>");
    }
    source.push('.');
    for _ in 0..100_000 {
        source.push_str(" B c.");
    }
    source.push_str("</t>\n</section>\n</middle>\n</rfc>\n");
    let start = std::time::Instant::now();
    let output = import_source(&dir, "many.xml", source.as_bytes(), &["--url", "u"]);
    let elapsed = start.elapsed();
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(requirements(&text(&output.stdout)).len(), 1);
    assert!(
        elapsed < std::time::Duration::from_secs(20),
        "import took {elapsed:?}"
    );
}
