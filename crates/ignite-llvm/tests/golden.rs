//! Golden IR (`docs/llvm.md` section 8 item 5): `tests/golden/small.ignite`
//! is parsed, checked, typed and lowered without a standard library; the
//! program module must equal `small.program.ll` and the main of its client
//! `small.main_small.ll`. Every module must also compile with `clang -c`.
//!
//! `IGNITE_BLESS=1 cargo test -p ignite-llvm --test main golden::` rewrites the
//! expected files from the current output.

use std::path::{Path, PathBuf};
use std::process::Command;

use ignite_syntax::SourceFile;
use ignite_types::Ty;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn lower(stem: &str) -> ignite_llvm::Generated {
    lower_with(stem, false).0
}

/// [`lower`], in a test build when `tests` (`docs/slice.md` 12.6), with the
/// symbols of the module's tests.
fn lower_with(stem: &str, tests: bool) -> (ignite_llvm::Generated, Vec<String>) {
    let path = golden_dir().join(format!("{stem}.ignite"));
    let text = std::fs::read_to_string(&path).unwrap();
    let source = SourceFile::new(path.display().to_string(), text);
    let program = ignite_syntax::parse(&source).unwrap_or_else(|d| panic!("{d:?}"));
    let symbols = ignite_check::check(&program).unwrap_or_else(|d| panic!("{d:?}"));
    let types = ignite_types::type_module(&program, &symbols, &Default::default())
        .unwrap_or_else(|d| panic!("{}", source.render_all(&d)));
    let input = ignite_llvm::ModuleInput {
        path: &types.module,
        program: &program,
        symbols: &symbols,
        types: &types,
    };
    let relative = SourceFile::new(format!("tests/golden/{stem}.ignite"), source.text.clone());
    let options = ignite_llvm::Options {
        sources: [(types.module.clone(), &relative)].into(),
        target: Some(target()),
        test_modules: if tests {
            vec![types.module.clone()]
        } else {
            Vec::new()
        },
        ..Default::default()
    };
    let symbols = types
        .tests()
        .iter()
        .map(|test| ignite_llvm::test_symbol(&test.module, test.decl))
        .collect();
    (ignite_llvm::generate_with(&[], &input, &options), symbols)
}

/// The target the goldens are pinned to, whatever the host.
fn target() -> ignite_llvm::TargetSpec {
    ignite_llvm::TargetSpec {
        triple: "x86_64-pc-linux-gnu".to_string(),
        datalayout:
            "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
                .to_string(),
    }
}

fn compare(expected_path: &Path, actual: &str) -> Result<(), String> {
    if std::env::var_os("IGNITE_BLESS").is_some() {
        std::fs::write(expected_path, actual).unwrap();
        return Ok(());
    }
    let expected = std::fs::read_to_string(expected_path)
        .map_err(|e| format!("{}: {e}", expected_path.display()))?;
    if expected == actual {
        Ok(())
    } else {
        Err(format!(
            "{} differs from the generated IR (IGNITE_BLESS=1 to accept):\n{actual}",
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
        .args(["-c", "-O1", "-Werror=override-module"])
        .arg(format!("--target={}", target().triple))
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
        "call i64 @ignite.arena_mark()",
        "call i1 @ignite.arena_due(i64 ",
        "call void @ignite_rt_arena_scratch(i8 1)",
        "call void @ignite_rt_arena_scratch(i8 0)",
        "call void @ignite_rt_arena_scratch_clear()",
        "call void @\"copy.Text\"(",
    ] {
        assert!(repeat.contains(needle), "repeat has no {needle}");
    }
    assert_eq!(repeat.matches("call i64 @ignite.arena_mark()").count(), 3);
    assert_eq!(
        repeat
            .matches("call void @ignite_rt_arena_release(i64 ")
            .count(),
        1
    );
    assert_eq!(
        repeat
            .matches("call void @ignite.arena_release(i64 ")
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
    assert!(main.contains("declare void @\"tests.small.Dialer.main\"(ptr, ptr)"));
    assert!(main.contains("call void @\"tests.small.Dialer.main\"(ptr %"));
    assert!(
        main.contains("call void @ignite_rt_fail(ptr %"),
        "an Err from the component's main is reported with the error's exit code"
    );
    assert!(
        main.contains("call void @ignite_rt_exit(i32 0)"),
        "Ok is exit 0"
    );
}

/// `docs/llvm.md` sections 1 and 7: every module starts with the target
/// it is pinned to and carries debug metadata naming the Ignite source.
#[test]
fn modules_name_target_and_source() {
    let generated = lower("small");
    let expected = target();
    for ir in [&generated.std, &generated.program, &generated.mains[0].ir] {
        assert!(
            ir.starts_with(&format!(
                "target triple = \"{}\"\ntarget datalayout = \"{}\"\n",
                expected.triple, expected.datalayout
            )),
            "{ir}"
        );
    }
    let program = &generated.program;
    assert!(program.contains("!DIFile(filename: \"small.ignite\", directory: \"./tests/golden\")"));
    assert!(program.contains("distinct !DISubprogram(name: \"tests.small.area\""));
    assert!(program.contains(" = !DILocation(line: "));
    assert!(
        program
            .lines()
            .any(|l| l.starts_with("  %") && l.contains(", !dbg !"))
    );
    assert!(program.contains("!llvm.dbg.cu = !{!"));
}

/// The text of the function defined as `symbol` in `ir`, from its `define`
/// line to its closing brace.
fn function<'a>(ir: &'a str, symbol: &str) -> &'a str {
    let quoted = format!("@\"{symbol}\"(");
    let start = ir
        .match_indices("define ")
        .map(|(i, _)| i)
        .find(|&i| ir[i..].lines().next().is_some_and(|l| l.contains(&quoted)))
        .unwrap_or_else(|| panic!("{symbol} is not defined"));
    let end = ir[start..].find("\n}\n").expect("end of the function") + start;
    &ir[start..end]
}

/// The newest `libignite_rt-*.a` cargo built beside this test executable
/// (`ignite-rt` is a dev-dependency, and its crate types include
/// `staticlib`).
fn runtime_archive() -> PathBuf {
    let exe = std::env::current_exe().expect("test executable path");
    let deps = exe.parent().expect("test executable directory");
    std::fs::read_dir(deps)
        .expect("read the deps directory")
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("libignite_rt-") && name.ends_with(".a")
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .max()
        .map(|(_, path)| path)
        .unwrap_or_else(|| panic!("no libignite_rt-*.a in {}", deps.display()))
}

/// Slice 14 (`docs/slice.md` 18.11): `tests/golden/bounds.ignite` holds a
/// `ByteStream`-bounded `relay`, a role generic over the stream, uses with
/// a `TcpStream` and with a `PipeStream`, and spawns of bounded instances.
/// Each instance calls the entries of its own stream kind and no other
/// (D11), a moved pipe end is owned and released by its task as a moved
/// `TcpStream` is, and the modules link with `libignite_rt.a`.
#[test]
fn bounded_streams() {
    let generated = lower("bounds");
    let dir = golden_dir();
    let mut failures = Vec::new();
    if let Err(e) = compare(&dir.join("bounds.program.ll"), &generated.program) {
        failures.push(e);
    }
    assert_eq!(generated.mains.len(), 1);
    assert_eq!(generated.mains[0].executable, "hub");
    if let Err(e) = compare(&dir.join("bounds.main_hub.ll"), &generated.mains[0].ir) {
        failures.push(e);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));

    let program = &generated.program;
    let kinds = [
        (Ty::TcpStream, "@ignite_rt_tcp_", "@ignite_rt_pipe_"),
        (Ty::PipeStream, "@ignite_rt_pipe_", "@ignite_rt_tcp_"),
    ];
    for (ty, own, other) in &kinds {
        let relay = function(
            program,
            &ignite_llvm::instance_symbol("tests.bounds.relay", std::slice::from_ref(ty)),
        );
        for entry in ["read", "write_all", "close", "set_idle_timeout"] {
            let call = format!("{own}{entry}(i64 ");
            assert!(
                relay.contains(&call),
                "relay<{ty:?}> has no {call}:\n{relay}"
            );
        }
        let serve = function(
            program,
            &ignite_llvm::instance_symbol("tests.bounds.Echo.serve", std::slice::from_ref(ty)),
        );
        assert!(serve.contains(&format!("{own}write_all(i64 ")), "{serve}");
        for instance in [relay, serve] {
            assert!(!instance.contains(other), "{instance}");
        }
    }
    let over_pipe = function(program, "tests.bounds.over_pipe");
    assert!(over_pipe.contains("call i64 @ignite_rt_pipe_received_batches(i64 "));
    assert!(!over_pipe.contains("@ignite_rt_tcp_read"), "{over_pipe}");
    assert!(
        over_pipe.contains("call void @ignite_rt_tcp_release(i64 "),
        "a pipe end whose task cannot start is released:\n{over_pipe}"
    );
    let thunk = program
        .lines()
        .find(|l| l.starts_with("define internal void @\"spawn.tests.bounds.relay<PipeStream>."))
        .and_then(|l| l.split('"').nth(1))
        .expect("the spawn thunk of relay<PipeStream>");
    let thunk = function(program, thunk);
    for needle in [
        "call void @ignite_rt_tcp_own(i64 ",
        "call void @ignite_rt_tcp_release(i64 ",
    ] {
        assert!(
            thunk.contains(needle),
            "the moved pipe end's task has no {needle}:\n{thunk}"
        );
    }

    compiles("bounds_std", &generated.std);
    compiles("bounds_program", program);
    compiles("bounds_main_hub", &generated.mains[0].ir);
    links("hub", &["bounds_std", "bounds_program", "bounds_main_hub"]);
}

/// Link the objects `compiles` wrote for `modules` with `libignite_rt.a`
/// and the native libraries it needs (`RUNTIME_LIBS` of the `ignite`
/// crate) into the executable `name`.
fn links(name: &str, modules: &[&str]) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("llvm-golden");
    let executable = dir.join(name);
    let output = Command::new("clang")
        .arg(format!("--target={}", target().triple))
        .args(modules.iter().map(|m| dir.join(format!("{m}.o"))))
        .arg(runtime_archive())
        .args([
            "-lgcc_s",
            "-lutil",
            "-lrt",
            "-lpthread",
            "-lm",
            "-ldl",
            "-lc",
        ])
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("run clang");
    assert!(
        output.status.success(),
        "{modules:?} do not link with libignite_rt.a:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    executable
}

/// The test of `bounds.ignite`, lowered in a test build and linked with
/// `libignite_rt.a`, runs: `Pipe.pair()` builds the tuple of two ends, the
/// `PipeStream` instance of `relay` echoes over one in this task, and a
/// spawned instance owns a moved end and releases it when it ends. The
/// archive of this crate's dev-dependency has no test runner (feature
/// `runner`), so a `main` written here calls the test directly; a failed
/// `assert` or `?` exits 101.
#[test]
fn bounded_streams_run_over_a_pipe() {
    let (generated, tests) = lower_with("bounds", true);
    assert_eq!(tests.len(), 1);
    let test = function(&generated.program, &tests[0]);
    assert!(
        test.contains("call void @ignite_rt_pipe_pair(ptr "),
        "{test}"
    );
    assert!(
        generated
            .program
            .contains("declare void @ignite_rt_pipe_pair(ptr, ptr)")
    );
    let spec = target();
    let main = format!(
        "target triple = \"{}\"\ntarget datalayout = \"{}\"\n\n\
         declare void @ignite_rt_init(i32, ptr)\n\
         declare void @ignite_rt_exit(i32) noreturn\n\
         declare void @\"{test}\"()\n\n\
         define i32 @main(i32 %argc, ptr %argv) {{\n\
         entry:\n  call void @ignite_rt_init(i32 %argc, ptr %argv)\n  \
         call void @\"{test}\"()\n  call void @ignite_rt_exit(i32 0)\n  unreachable\n}}\n",
        spec.triple,
        spec.datalayout,
        test = tests[0],
    );
    compiles("bounds_test_std", &generated.std);
    compiles("bounds_test_program", &generated.program);
    compiles("bounds_test_main", &main);
    let executable = links(
        "bounds-test",
        &["bounds_test_std", "bounds_test_program", "bounds_test_main"],
    );
    let output = Command::new(&executable)
        .output()
        .expect("run the bounds test");
    assert!(
        output.status.success(),
        "the bounds test failed with {}:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Link `generated` (a test build) with a `main` that calls `test` and run
/// it, as `bounded_streams_run_over_a_pipe` does; `name` keeps the objects
/// apart.
fn run_test_build(name: &str, generated: &ignite_llvm::Generated, test: &str) {
    let spec = target();
    let main = format!(
        "target triple = \"{}\"\ntarget datalayout = \"{}\"\n\n\
         declare void @ignite_rt_init(i32, ptr)\n\
         declare void @ignite_rt_exit(i32) noreturn\n\
         declare void @\"{test}\"()\n\n\
         define i32 @main(i32 %argc, ptr %argv) {{\n\
         entry:\n  call void @ignite_rt_init(i32 %argc, ptr %argv)\n  \
         call void @\"{test}\"()\n  call void @ignite_rt_exit(i32 0)\n  unreachable\n}}\n",
        spec.triple, spec.datalayout,
    );
    let std = format!("{name}_std");
    let program = format!("{name}_program");
    let main_name = format!("{name}_main");
    compiles(&std, &generated.std);
    compiles(&program, &generated.program);
    compiles(&main_name, &main);
    let executable = links(name, &[&std, &program, &main_name]);
    let output = Command::new(&executable)
        .output()
        .expect("run the test executable");
    assert!(
        output.status.success(),
        "{name} failed with {}:\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Adversary (slice 14 U4): U3's richer `bounds.ignite`, copied verbatim
/// as `bounds_rich.ignite`. Nested bounded instances (`forward<T>` calling
/// `once` and `relay`, and spawning `relay`), UFCS, `stream.copy()`, a
/// bounded role with two parameters and an unbounded `identity<T>` lower;
/// every instance calls only its own kind's entries (D11); all modules
/// compile, link with `libignite_rt.a`, and the test build's test passes.
#[test]
fn adversary_rich_bounds_lower_link_and_run() {
    let generated = lower("bounds_rich");
    let program = &generated.program;
    let kinds = [
        (Ty::TcpStream, "@ignite_rt_tcp_", "@ignite_rt_pipe_"),
        (Ty::PipeStream, "@ignite_rt_pipe_", "@ignite_rt_tcp_"),
    ];
    for (ty, own, other) in &kinds {
        let one = std::slice::from_ref(ty);
        for name in [
            "examples.bounds.relay",
            "examples.bounds.once",
            "examples.bounds.forward",
            "examples.bounds.Relay.serve",
        ] {
            let symbol = ignite_llvm::instance_symbol(name, one);
            let body = function(program, &symbol);
            assert!(!body.contains(other), "{symbol} calls {other}:\n{body}");
        }
        let relay = function(
            program,
            &ignite_llvm::instance_symbol("examples.bounds.relay", one),
        );
        assert_eq!(
            relay.matches(&format!("{own}write_all(i64 ")).count(),
            2,
            "relay writes the echo and the copy's x:\n{relay}"
        );
        let forward = function(
            program,
            &ignite_llvm::instance_symbol("examples.bounds.forward", one),
        );
        for callee in ["examples.bounds.relay", "examples.bounds.once"] {
            let call = format!("@\"{}\"(", ignite_llvm::instance_symbol(callee, one));
            assert!(forward.contains(&call), "forward has no {call}:\n{forward}");
        }
    }
    assert_eq!(generated.mains.len(), 1);
    compiles("rich_std", &generated.std);
    compiles("rich_program", program);
    compiles("rich_main", &generated.mains[0].ir);
    links("relayd", &["rich_std", "rich_program", "rich_main"]);

    let (generated, tests) = lower_with("bounds_rich", true);
    assert_eq!(tests.len(), 1);
    run_test_build("rich-test", &generated, &tests[0]);
}

/// Adversary (slice 14 U4): a `spawn` inside a bounded instance of a
/// `stream.copy()` moves nothing, for either stream kind: the copy shares
/// the handle its receiver holds (18.4), so neither the task nor the
/// failed-spawn path of `hand<S>` releases it
/// (`adversary_a_moved_copy_does_not_release_the_held_end` runs it).
#[test]
fn adversary_spawn_inside_a_bounded_instance_keeps_a_copied_end() {
    let generated = lower("bounds_adv");
    let program = &generated.program;
    for (ty, kind) in [(Ty::TcpStream, "TcpStream"), (Ty::PipeStream, "PipeStream")] {
        let symbol = ignite_llvm::instance_symbol("tests.bounds_adv.hand", &[ty]);
        let hand = function(program, &symbol);
        assert!(
            !hand.contains("call void @ignite_rt_tcp_release(i64 "),
            "the failed-spawn path of {symbol} releases a copied end:\n{hand}"
        );
        let prefix = format!("define internal void @\"spawn.tests.bounds_adv.relay<{kind}>.");
        let thunk = program
            .lines()
            .find(|l| l.starts_with(&prefix))
            .and_then(|l| l.split('"').nth(1))
            .unwrap_or_else(|| panic!("no spawn thunk of relay<{kind}>:\n{program}"));
        let thunk = function(program, thunk);
        assert!(
            !thunk.contains("call void @ignite_rt_tcp_own(i64 "),
            "the copied {kind} of {symbol} is owned by its task:\n{thunk}"
        );
    }
}

/// Adversary (slice 14 U4): a fn value of a bounded generic, `debug` of
/// a pipe end, and a spawn from inside an
/// instance, run in a test build linked with `libignite_rt.a`.
#[test]
fn adversary_fn_value_debug_and_nested_spawn_run_over_a_pipe() {
    let (generated, tests) = lower_with("bounds_adv", true);
    assert_eq!(tests.len(), 1);
    run_test_build("adv-test", &generated, &tests[0]);
}

fn typed(
    name: &str,
    text: &str,
    modules: &ignite_check::Modules,
    typed: &ignite_types::TypedModules,
) -> Result<
    (
        SourceFile,
        ignite_syntax::ast::Program,
        ignite_check::Symbols,
        ignite_types::TypedProgram,
    ),
    Vec<String>,
> {
    let source = SourceFile::new(name.to_string(), text.to_string());
    let render = |d: Vec<ignite_syntax::Diagnostic>| {
        d.iter()
            .map(|d| format!("{} {}", d.code, d.message))
            .collect::<Vec<_>>()
    };
    let program = ignite_syntax::parse(&source).map_err(render)?;
    let symbols = ignite_check::check_module(&program, modules).map_err(render)?;
    let types = ignite_types::type_module(&program, &symbols, typed).map_err(render)?;
    Ok((source, program, symbols, types))
}

/// Adversary (slice 14 U4): `received_batches` is a `PipeStream` method
/// only (18.4); on a bounded receiver it is E041 (18.3 "Any other method
/// or field is E041"), and on a `TcpStream` too, so no TCP instance can
/// reach `ignite_rt_pipe_received_batches`.
#[test]
fn adversary_received_batches_is_a_pipe_method_only() {
    let empty = (Default::default(), Default::default());
    for (case, text) in [
        (
            "bounded",
            "package tests.rb\n\
             fn count<S: ByteStream>(stream: S) -> Int requires [stream.read] = stream.received_batches()\n\
             fn over_tcp(stream: TcpStream) -> Int requires [stream.read] = count(stream)\n",
        ),
        (
            "tcp",
            "package tests.rb\n\
             fn direct(stream: TcpStream) -> Int requires [stream.read] = stream.received_batches()\n",
        ),
    ] {
        match typed("rb.ignite", text, &empty.0, &empty.1) {
            Ok(_) => panic!("{case}: received_batches on a non-pipe receiver type-checks"),
            Err(codes) => assert!(
                codes.iter().any(|c| c.starts_with("E041")),
                "{case}: {codes:?}"
            ),
        }
    }
}

/// Adversary (slice 14 U4): a bounded fn of a standard-library module,
/// instantiated by the program with both stream kinds (the shape of U5's
/// `Telnet.serve<S>`): both instances are defined once, each calls its
/// own kind's entries, every module compiles, links and the test runs.
#[test]
fn adversary_std_bounded_fn_instantiated_by_the_program() {
    let std_text = "package std.adv\n\
        fn relay<S: ByteStream>(stream: S) -> Result<Unit, IoError> requires [stream.read, stream.write] {\n\
          match stream.read(4096)? {\n\
            Eof => return stream.close()\n\
            Data(bytes) => stream.write_all(bytes)?\n\
          }\n\
          return stream.close()\n\
        }\n";
    let program_text = "package tests.cross\n\
        import std.adv.{relay}\n\
        fn over_tcp(stream: TcpStream) -> Result<Unit, IoError> requires [stream.read, stream.write] = relay(stream)\n\
        fn over_pipe(end: PipeStream) -> Result<Unit, IoError> requires [end.read, end.write] = relay(end)\n\
        test \"a std instance relays over a pipe\" {\n\
          let (client, server) = Pipe.pair()\n\
          client.write_all(\"hey\".to_bytes())?\n\
          client.close()?\n\
          over_pipe(server)?\n\
          let reply = match client.read(16)? {\n\
            Eof => \"eof\"\n\
            Data(bytes) => bytes.to_text()\n\
          }\n\
          assert reply == \"hey\"\n\
        }\n";
    let (std_source, std_program, std_symbols, std_types) = typed(
        "std/adv.ignite",
        std_text,
        &Default::default(),
        &Default::default(),
    )
    .unwrap_or_else(|d| panic!("{d:?}"));
    let modules: ignite_check::Modules = [("std.adv".to_string(), std_symbols.clone())].into();
    let typed_std: ignite_types::TypedModules =
        [("std.adv".to_string(), std_types.exports())].into();
    let (source, program, symbols, types) =
        typed("cross.ignite", program_text, &modules, &typed_std)
            .unwrap_or_else(|d| panic!("{d:?}"));
    let std_input = ignite_llvm::ModuleInput {
        path: &std_types.module,
        program: &std_program,
        symbols: &std_symbols,
        types: &std_types,
    };
    let input = ignite_llvm::ModuleInput {
        path: &types.module,
        program: &program,
        symbols: &symbols,
        types: &types,
    };
    for tests in [false, true] {
        let options = ignite_llvm::Options {
            sources: [
                (std_types.module.clone(), &std_source),
                (types.module.clone(), &source),
            ]
            .into(),
            target: Some(target()),
            test_modules: if tests {
                vec![types.module.clone()]
            } else {
                Vec::new()
            },
            ..Default::default()
        };
        let generated = ignite_llvm::generate_with(&[std_input], &input, &options);
        let all = format!("{}\n{}", generated.std, generated.program);
        for (ty, own, other) in [
            (Ty::TcpStream, "@ignite_rt_tcp_", "@ignite_rt_pipe_"),
            (Ty::PipeStream, "@ignite_rt_pipe_", "@ignite_rt_tcp_"),
        ] {
            let symbol = ignite_llvm::instance_symbol("std.adv.relay", &[ty]);
            let quoted = format!("@\"{symbol}\"(");
            let defines = all
                .lines()
                .filter(|l| l.starts_with("define ") && l.contains(&quoted))
                .count();
            assert!(defines >= 1, "{symbol} is not defined:\n{all}");
            let in_std = generated
                .std
                .lines()
                .any(|l| l.starts_with("define ") && l.contains(&quoted));
            let body = function(
                if in_std {
                    &generated.std
                } else {
                    &generated.program
                },
                &symbol,
            );
            assert!(body.contains(&format!("{own}read(i64 ")), "{body}");
            assert!(!body.contains(other), "{body}");
        }
        if tests {
            let test = ignite_llvm::test_symbol(&types.tests()[0].module, types.tests()[0].decl);
            run_test_build("cross-test", &generated, &test);
        } else {
            compiles("cross_std", &generated.std);
            compiles("cross_program", &generated.program);
        }
    }
}

/// Adversary (slice 14 U4): an end and its copies share one handle (18.4
/// "its clones share it"). A copy moved into a task is owned by it
/// (`fresh_let` of a method call), so the task's release must not take the
/// end the test still holds: after the task ended, `far` still counts the
/// one delivery, as on the Rust target, where a spawn clones its arguments.
#[test]
fn adversary_a_moved_copy_does_not_release_the_held_end() {
    let (generated, tests) = lower_with("bounds_alias", true);
    assert_eq!(tests.len(), 1);
    run_test_build("alias-test", &generated, &tests[0]);
}
