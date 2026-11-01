//! Golden IR (`docs/llvm.md` section 8 item 5): `tests/golden/small.telos`
//! is parsed, checked, typed and lowered without a standard library; the
//! program module must equal `small.program.ll` and the main of its client
//! `small.main_small.ll`. Every module must also compile with `clang -c`.
//!
//! `TELOS_BLESS=1 cargo test -p telos-llvm --test golden` rewrites the
//! expected files from the current output.

use std::path::{Path, PathBuf};
use std::process::Command;

use telos_syntax::SourceFile;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn lower(stem: &str) -> telos_llvm::Generated {
    let path = golden_dir().join(format!("{stem}.telos"));
    let text = std::fs::read_to_string(&path).unwrap();
    let source = SourceFile::new(path.display().to_string(), text);
    let program = telos_syntax::parse(&source).unwrap_or_else(|d| panic!("{d:?}"));
    let symbols = telos_check::check(&program).unwrap_or_else(|d| panic!("{d:?}"));
    let types = telos_types::type_module(&program, &symbols, &Default::default())
        .unwrap_or_else(|d| panic!("{}", source.render_all(&d)));
    let input = telos_llvm::ModuleInput {
        path: &types.module,
        program: &program,
        symbols: &symbols,
        types: &types,
    };
    telos_llvm::generate(&[], &input)
}

fn compare(expected_path: &Path, actual: &str) -> Result<(), String> {
    if std::env::var_os("TELOS_BLESS").is_some() {
        std::fs::write(expected_path, actual).unwrap();
        return Ok(());
    }
    let expected = std::fs::read_to_string(expected_path)
        .map_err(|e| format!("{}: {e}", expected_path.display()))?;
    if expected == actual {
        Ok(())
    } else {
        Err(format!(
            "{} differs from the generated IR (TELOS_BLESS=1 to accept):\n{actual}",
            expected_path.display()
        ))
    }
}

/// `clang -c` of one IR module into the cargo target directory.
fn compiles(name: &str, ir: &str) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("llvm-golden");
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
fn small_program() {
    let generated = lower("small");
    let dir = golden_dir();
    let mut failures = Vec::new();
    if let Err(e) = compare(&dir.join("small.program.ll"), &generated.program) {
        failures.push(e);
    }
    assert_eq!(generated.mains.len(), 1);
    assert_eq!(generated.mains[0].executable, "small");
    if let Err(e) = compare(&dir.join("small.main_small.ll"), &generated.mains[0].ir) {
        failures.push(e);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    compiles("std", &generated.std);
    compiles("program", &generated.program);
    compiles("main_small", &generated.mains[0].ir);
}

/// Slice 6 surface syntax (`docs/slice.md` section 10): tuples, pattern
/// lets, Bool, or, constant, tuple and list patterns, field defaults,
/// `to_text`, `+` on `Text`, `return` expressions, UFCS, fn-field calls,
/// `for` over `Bytes` and aliases.
#[test]
fn syntax_program() {
    let generated = lower("syntax");
    let dir = golden_dir();
    let mut failures = Vec::new();
    if let Err(e) = compare(&dir.join("syntax.program.ll"), &generated.program) {
        failures.push(e);
    }
    assert_eq!(generated.mains.len(), 1);
    if let Err(e) = compare(&dir.join("syntax.main_syntax.ll"), &generated.mains[0].ir) {
        failures.push(e);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    let program = &generated.program;
    for symbol in [
        "define void @\"tests.syntax.Reply.action\"(ptr %sret)",
        "define i64 @\"tests.syntax.Reply.code\"()",
        "define internal void @\"copy.(Int,Text)\"(ptr %dst, ptr %src)",
    ] {
        assert!(program.contains(symbol), "missing {symbol}");
    }
    compiles("syntax_program", &generated.program);
    compiles("syntax_main", &generated.mains[0].ir);
}

/// `docs/llvm.md` section 4: a loop that modifies `out` copies it across a
/// due release; a loop that modifies only scalars releases at every
/// back-edge.
#[test]
fn loops_release_their_region() {
    let generated = lower("small");
    let program = &generated.program;
    let start = program
        .find("define void @\"tests.small.repeat\"")
        .expect("repeat is defined");
    let end = program[start..].find("\n}\n").expect("end of repeat") + start;
    let repeat = &program[start..end];
    for needle in [
        "call i64 @telos.arena_mark()",
        "call i1 @telos.arena_due(i64 ",
        "call void @telos_rt_arena_scratch(i8 1)",
        "call void @telos_rt_arena_scratch(i8 0)",
        "call void @telos_rt_arena_scratch_clear()",
        "call void @\"copy.Text\"(",
    ] {
        assert!(repeat.contains(needle), "repeat has no {needle}");
    }
    assert_eq!(repeat.matches("call i64 @telos.arena_mark()").count(), 3);
    assert_eq!(
        repeat
            .matches("call void @telos_rt_arena_release(i64 ")
            .count(),
        1
    );
    assert_eq!(
        repeat
            .matches("call void @telos.arena_release(i64 ")
            .count(),
        1
    );
    let main = &generated.mains[0].ir;
    let entry = &main[main.find("define i32 @main").expect("main is defined")..];
    let entry = &entry[..entry.find("\n}\n").expect("end of main")];
    assert!(!entry.contains("arena_release(i64 %"));
}

/// `docs/llvm.md` section 3: a place passed to a parameter the callee only
/// reads is not copied.
#[test]
fn read_only_parameters_are_borrowed() {
    let generated = lower("small");
    let program = &generated.program;
    let start = program
        .find("define void @\"tests.small.greet\"")
        .expect("greet is defined");
    let end = program[start..].find("\n}\n").expect("end of greet") + start;
    let greet = &program[start..end];
    assert!(greet.contains("@\"tests.small.shout\"(ptr %"), "{greet}");
    assert!(greet.contains(", ptr %p0)"), "{greet}");
    assert!(!greet.contains("copy.Text"), "{greet}");
}

#[test]
fn symbols_are_module_qualified() {
    let generated = lower("small");
    let program = &generated.program;
    for symbol in [
        "define i64 @\"tests.small.area\"(ptr %p0)",
        "define void @\"tests.small.label\"(ptr %sret, ptr %p0)",
        "define void @\"tests.small.checked\"(ptr %sret, i64 %p0)",
        "define i64 @\"tests.small.SCALE\"()",
        "define void @\"tests.small.Echo.connect\"(ptr %sret, i64 %p0)",
        "define internal void @\"copy.Text\"(ptr %dst, ptr %src)",
    ] {
        assert!(program.contains(symbol), "missing {symbol}");
    }
    let main = &generated.mains[0].ir;
    assert!(main.contains("define i32 @main(i32 %argc, ptr %argv)"));
    assert!(main.contains("declare void @\"tests.small.Echo.connect\"(ptr, i64)"));
    assert!(
        main.contains("store i32 3, ptr"),
        "connect failure is exit 3"
    );
    assert!(main.contains("store i32 4, ptr"), "role failure is exit 4");
}
