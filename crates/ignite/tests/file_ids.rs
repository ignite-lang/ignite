//! The `FileId` of a program (story `stable-node-ids`): the hash of its
//! canonical path, so every way of naming one file gives one id, while
//! diagnostics print the path as given.

use std::path::Path;

const RELATIVE: &str = "tests/fixtures/eval_equivalence.ignite";

/// The same file as `x/a.ignite`, `./x/a.ignite`, `x/../x/a.ignite` and its
/// absolute path; the test runs in the package directory.
fn spellings() -> Vec<String> {
    let absolute = Path::new(env!("CARGO_MANIFEST_DIR")).join(RELATIVE);
    vec![
        RELATIVE.to_string(),
        format!("./{RELATIVE}"),
        format!("tests/../{RELATIVE}"),
        absolute.display().to_string(),
    ]
}

#[test]
fn one_file_named_four_ways_has_one_id() {
    let mut ids = Vec::new();
    for spelling in spellings() {
        let loaded = ignite::load(Path::new(&spelling), None).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            loaded.source().path,
            spelling,
            "the path is printed as given"
        );
        let package = loaded.program().package.as_ref().expect("a package");
        assert_eq!(package.span.file, loaded.source().id());
        assert_eq!(package.id.file, loaded.source().id());
        ids.push(loaded.source().id());
    }
    assert!(ids.windows(2).all(|w| w[0] == w[1]), "{ids:?}");
}

#[test]
fn stdin_for_an_existing_file_has_its_id_and_prints_its_path() {
    let reference = ignite::load(Path::new(RELATIVE), None).unwrap_or_else(|e| panic!("{e}"));
    for spelling in spellings() {
        let error = ignite::check_text(Path::new(&spelling), "fn (\n".to_string(), None)
            .expect_err("a syntax error");
        let ignite::Error::Diagnostics {
            source,
            diagnostics,
        } = &error
        else {
            panic!("{error}")
        };
        assert_eq!(source.path, spelling);
        assert_eq!(source.id(), reference.source().id());
        assert!(
            source
                .render_all(diagnostics)
                .starts_with(&format!("{spelling}:")),
            "{error}"
        );
    }
}
