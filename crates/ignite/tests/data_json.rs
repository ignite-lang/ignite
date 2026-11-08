//! `std.json`'s `read`, `write` and `write_pretty` over `std.data` values
//! across the evaluator and both targets (`docs/slice.md` 27.5): every fn
//! of `fixtures/data_json.ignite` without parameters gives the same debug
//! text in the evaluator, in a Rust-target build and in an LLVM-target
//! build.

use std::path::{Path, PathBuf};
use std::process::Command;

use ignite::Target;
use ignite_eval::{Evaluator, Fuel};
use ignite_syntax::ast::{Decl, Program};
use ignite_types::QualName;

use crate::common;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data_json.ignite")
}

/// The fns of the fixture without parameters, in declaration order: the
/// ones the printer's `main` shows.
fn printed_fns(program: &Program) -> Vec<String> {
    program
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Fn(f) if f.params.is_empty() => Some(f.qualified_name()),
            _ => None,
        })
        .collect()
}

/// The debug text of every printed fn, by the evaluator.
fn evaluated(loaded: &ignite::Loaded) -> Vec<String> {
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let module = loaded.types().module.clone();
    printed_fns(loaded.program())
        .iter()
        .map(|name| {
            evaluator
                .call(&QualName::new(module.clone(), name.clone()), Vec::new())
                .unwrap_or_else(|e| panic!("{name}: {e:?}"))
                .debug_text()
        })
        .collect()
}

/// The lines the fixture prints when built for `target`.
fn printed(target: Target) -> Vec<String> {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let options = match target {
        Target::Rust => ignite::BuildOptions {
            target: Target::Rust,
            out: Some(tmp.join("data-json")),
            ..Default::default()
        },
        Target::Llvm => ignite::BuildOptions {
            target: Target::Llvm,
            out: Some(tmp.join("data-json-llvm")),
            runtime: Some(common::runtime_archive()),
            ..Default::default()
        },
    };
    let built = ignite::build(&fixture(), &options).unwrap_or_else(|e| panic!("{e}"));
    let executable = built
        .executables
        .iter()
        .find(|path| path.file_stem().and_then(|s| s.to_str()) == Some("dataprint"))
        .unwrap_or_else(|| panic!("no dataprint in {:?}", built.executables));
    let output = Command::new(executable)
        .output()
        .unwrap_or_else(|e| panic!("run dataprint: {e}"));
    assert!(
        output.status.success(),
        "{target:?}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("debug text is UTF-8");
    stdout.lines().map(str::to_string).collect()
}

/// 27.5: ten documents read to the same values and write to the same texts
/// in the evaluator and on both targets; eight read, two give the same
/// format errors, and every written text reads back to its value.
#[test]
fn read_and_write_agree_across_the_evaluator_and_both_targets() {
    let loaded = ignite::load(&fixture(), None).unwrap_or_else(|e| panic!("{e}"));
    let names = printed_fns(loaded.program());
    assert_eq!(
        names,
        ["documents", "values", "compact", "pretty", "round_trips"]
    );
    let ours = evaluated(&loaded);
    assert_eq!(ours[1].matches("Ok(").count(), 8, "{}", ours[1]);
    assert!(
        ours[1].contains(r#"Err(FormatError { message: "duplicate member `a`", line: 1, column: 10, offset: 9 })"#),
        "{}",
        ours[1]
    );
    assert!(
        ours[1].contains(r#"Err(FormatError { message: "unexpected character", line: 2, column: 4, offset: 10 })"#),
        "{}",
        ours[1]
    );
    assert!(
        ours[2].contains(r#""{\"id\":1,\"title\":\"milk\",\"done\":false,\"tags\":[]}""#),
        "{}",
        ours[2]
    );
    // One entry per document: ten, and every written text reads back.
    assert_eq!(
        ours[4],
        "[true, true, true, true, true, true, true, true, true, true]"
    );
    std::thread::scope(|scope| {
        let handles: Vec<_> = [Target::Rust, Target::Llvm]
            .into_iter()
            .map(|target| scope.spawn(move || (target, printed(target))))
            .collect();
        for handle in handles {
            let (target, theirs) = handle.join().expect("printer thread");
            assert_eq!(theirs.len(), names.len(), "{target:?} printed {theirs:?}");
            for ((name, ours), theirs) in names.iter().zip(&ours).zip(&theirs) {
                assert_eq!(ours, theirs, "{name} on {target:?}");
            }
        }
    });
}
