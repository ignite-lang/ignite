//! Test builds (`docs/slice.md` section 12.6): `tests/golden/tests.telos`
//! is parsed, checked, typed and lowered with `generate_with` in test mode.
//! The tests become `void ()` functions, the only executable is
//! `telos-test` with a table of them, and every module compiles with
//! `clang -c`. A normal build of the same file drops every test-only
//! declaration.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use telos_syntax::SourceFile;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/tests.telos")
}

struct Front {
    source: SourceFile,
    program: telos_syntax::ast::Program,
    symbols: telos_check::Symbols,
    types: telos_types::TypedProgram,
}

fn front() -> Front {
    let path = fixture();
    let text = std::fs::read_to_string(&path).unwrap();
    let source = SourceFile::new("tests.telos", text);
    let program = telos_syntax::parse(&source).unwrap_or_else(|d| panic!("{d:?}"));
    let symbols = telos_check::check(&program).unwrap_or_else(|d| panic!("{d:?}"));
    let types = telos_types::type_module(&program, &symbols, &Default::default())
        .unwrap_or_else(|d| panic!("{}", source.render_all(&d)));
    Front {
        source,
        program,
        symbols,
        types,
    }
}

fn generate(front: &Front, tests: bool) -> telos_llvm::Generated {
    let input = telos_llvm::ModuleInput {
        path: &front.types.module,
        program: &front.program,
        symbols: &front.symbols,
        types: &front.types,
    };
    let mut sources = BTreeMap::new();
    sources.insert(front.types.module.clone(), &front.source);
    let options = telos_llvm::Options {
        test_modules: if tests {
            vec![front.types.module.clone()]
        } else {
            Vec::new()
        },
        sources,
    };
    telos_llvm::generate_with(&[], &input, &options)
}

/// `clang -c` of one IR module into the cargo target directory.
fn compiles(name: &str, ir: &str) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("llvm-tests-build");
    std::fs::create_dir_all(&dir).unwrap();
    let ll = dir.join(format!("{name}.ll"));
    std::fs::write(&ll, ir).unwrap();
    let output = Command::new("clang")
        .args(["-c", "-O1", "-Wno-override-module"])
        .arg(&ll)
        .arg("-o")
        .arg(dir.join(format!("{name}.o")))
        .output()
        .expect("run clang");
    assert!(
        output.status.success(),
        "{name}.ll does not compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_build_lowers_tests_and_a_runner_main() {
    let front = front();
    let tests = front.types.tests();
    assert_eq!(tests.len(), 4, "the typer lists the fixture's tests");
    let generated = generate(&front, true);
    assert_eq!(generated.mains.len(), 1);
    assert_eq!(generated.mains[0].executable, telos_llvm::TEST_EXECUTABLE);
    let program = &generated.program;
    let main = &generated.mains[0].ir;
    for test in tests {
        let symbol = telos_llvm::test_symbol(&test.module, test.decl);
        assert!(
            program.contains(&format!("define void @\"{symbol}\"()")),
            "missing {symbol}"
        );
        assert!(main.contains(&format!("ptr @\"{symbol}\"")), "{main}");
        assert!(main.contains(&test.full_name), "{main}");
    }
    assert!(main.contains("[4 x { ptr, i64, i64, ptr }]"), "{main}");
    assert!(main.contains("call void @\"telos_rt_test_main\"(ptr @telos.tests, i64 4)"));
    for needle in [
        "define internal i1 @\"eq.tests.inline.Pair\"(ptr %a, ptr %b)",
        "define internal i1 @\"eq.tests.inline.Shape\"(ptr %a, ptr %b)",
        "define internal i1 @\"eq.(Int,Text)\"(ptr %a, ptr %b)",
        "define internal i1 @\"eq.List<Option<Int>>\"(ptr %a, ptr %b)",
        "declare void @\"telos_rt_assert_failed\"(ptr, ptr, ptr, ptr, ptr)",
        "declare void @\"telos_rt_try_failed\"(ptr, ptr)",
        "c\"tests.telos:24:3\"",
        "c\"half(5) != 3\"",
    ] {
        assert!(program.contains(needle), "program has no {needle}");
    }
    compiles("tests_program", program);
    compiles("tests_std", &generated.std);
    compiles("tests_main", main);
}

#[test]
fn normal_build_drops_test_only_declarations() {
    let front = front();
    let generated = generate(&front, false);
    let program = &generated.program;
    assert!(!program.contains(".test."), "{program}");
    assert!(!program.contains("tests.inline.pair"), "{program}");
    assert!(generated.mains.is_empty());
    assert!(
        program.contains("telos_rt_assert_failed"),
        "an assert in a normal fn is lowered"
    );
    assert!(!program.contains("telos_rt_try_failed"));
    compiles("normal_program", program);
}
