//! Golden files: every `tests/golden/<name>.telos` is parsed, checked
//! against a standard library and lowered. After collapsing whitespace runs
//! to one space, `src/lib.rs` must equal `<name>.rs`, the first executable
//! `<name>.main.rs`, every further executable `<name>.<executable>.rs`, and,
//! for programs lowered with the golden standard library
//! (`tests/golden/std`), the `telos-std` crate `<name>.std.rs`.
//!
//! `TELOS_BLESS=1 cargo test -p telos-rustgen --test golden` rewrites the
//! expected files from the current output.

mod common;

use std::path::{Path, PathBuf};

use telos_rustgen::GeneratedProject;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Programs that must type: their goldens are the typed generator's
/// output. The others fall back to the untyped generator when they do not.
const TYPED: &[&str] = &["terminal", "syntax", "exits", "variants", "generics"];

fn lower(stem: &str, std_root: &Path) -> (GeneratedProject, GeneratedProject) {
    let file = golden_dir().join(format!("{stem}.telos"));
    let substrate = Path::new("../telos-io");
    if TYPED.contains(&stem) {
        common::lower_typed(&file, std_root, substrate)
    } else {
        common::lower(&file, std_root, substrate)
    }
}

fn norm(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn compare(expected_path: &Path, actual: &str) -> Result<(), String> {
    if std::env::var_os("TELOS_BLESS").is_some() {
        std::fs::write(expected_path, actual).unwrap();
        return Ok(());
    }
    let expected = std::fs::read_to_string(expected_path)
        .map_err(|e| format!("{}: {e}", expected_path.display()))?;
    if norm(&expected) == norm(actual) {
        Ok(())
    } else {
        Err(format!(
            "{} differs from the generated text:\n{actual}",
            expected_path.display()
        ))
    }
}

/// Compare the program project; with `with_std`, also the std crate.
fn check_program(stem: &str, std_root: &Path, with_std: bool) -> GeneratedProject {
    let dir = golden_dir();
    let (project, std) = lower(stem, std_root);
    let mut failures = Vec::new();
    let mut expect = |path: PathBuf, actual: &str| {
        if let Err(e) = compare(&path, actual) {
            failures.push(e);
        }
    };
    expect(dir.join(format!("{stem}.rs")), &project.lib_rs);
    let main_path = dir.join(format!("{stem}.main.rs"));
    match project.executables.split_first() {
        Some((first, rest)) => {
            expect(main_path, &first.source);
            for executable in rest {
                expect(
                    dir.join(format!("{stem}.{}.rs", executable.name)),
                    &executable.source,
                );
            }
        }
        None => assert!(
            !main_path.exists(),
            "{} exists but the program has no system",
            main_path.display()
        ),
    }
    if with_std {
        expect(dir.join(format!("{stem}.std.rs")), &std.lib_rs);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    project
}

fn executable_names(project: &GeneratedProject) -> Vec<&str> {
    project
        .executables
        .iter()
        .map(|e| e.name.as_str())
        .collect()
}

#[test]
fn terminal() {
    let project = check_program("terminal", &common::repository_std(), false);
    assert_eq!(executable_names(&project), ["terminald", "tn"]);
    assert_eq!(project.package_name, "terminal");
    assert!(
        project
            .cargo_toml
            .contains("telos-std = { path = \"std\" }")
    );
}

#[test]
fn pure_functions() {
    check_program("pure", &common::golden_std(), false);
}

#[test]
fn record_and_enum_with_match() {
    check_program("shapes", &common::golden_std(), false);
}

#[test]
fn system_with_service_and_client() {
    let project = check_program("services", &common::golden_std(), false);
    assert_eq!(executable_names(&project), ["greeter", "readerc"]);
    assert!(!project.cargo_toml.contains("telos-std"));
    assert!(norm(&project.executables[1].source).contains(
        "let stream = telos_io::TcpStream::connect(String::from(\"0.0.0.0\"), port).map_err(|error| error.with_exit_code(3))?;"
    ));
}

#[test]
fn protocol_import_spawn_and_function_values() {
    let project = check_program("modules", &common::golden_std(), true);
    assert_eq!(executable_names(&project), ["echod", "echo"]);
    assert!(
        project
            .cargo_toml
            .contains("telos-std = { path = \"std\" }")
    );
}

#[test]
fn constants_qualified_patterns_and_io_error() {
    let project = check_program("consts", &common::golden_std(), true);
    assert!(project.executables.is_empty());
}

/// Slice 6 surface syntax (`docs/slice.md` section 10), the program the
/// LLVM golden uses too.
#[test]
fn surface_syntax() {
    let project = check_program("syntax", &common::golden_std(), false);
    assert_eq!(executable_names(&project), ["syntax"]);
}

/// `Process.exit(c)` has type Never and lowers to `telos_io::exit`, which
/// returns `!`: no `?` is needed, and `?` on it lowers to the call alone.
#[test]
fn process_exit_diverges() {
    let project = check_program("exits", &common::golden_std(), false);
    assert_eq!(executable_names(&project), ["exiter"]);
    let lib = norm(&project.lib_rs);
    assert!(lib.contains("Eof => telos_io::exit((5) as i32)"), "{lib}");
    assert!(!lib.contains("telos_io::Process::exit"), "{lib}");
    assert!(!lib.contains("exit((7) as i32))?"), "{lib}");
}

/// Bare variants of an enum that is not imported, resolved by expected
/// type, name the enum by its path: `telos_std::modes::Mode` in a program,
/// `crate::modes::Mode` inside `telos-std`.
#[test]
fn variants_of_enums_not_imported() {
    let std_root = golden_dir().join("variants/std");
    let (project, std) = lower("variants", &std_root);
    check_program("variants", &std_root, true);
    assert!(
        norm(&project.lib_rs).contains("level(&telos_std::modes::Mode::Loud)"),
        "{}",
        project.lib_rs
    );
    assert!(
        norm(&std.lib_rs).contains("crate::modes::Mode::Quiet(_) =>"),
        "{}",
        std.lib_rs
    );
}

/// Slice 7 (`docs/slice.md` section 11): generic records, enums, fns and
/// roles with the Rust bounds, an effectful fn type, a `Store` resource
/// created first in `main` and named in the `application`, the new
/// builtins, and parameters passed by reference.
#[test]
fn generics_stores_and_builtins() {
    let project = check_program("generics", &common::golden_std(), false);
    assert_eq!(executable_names(&project), ["countersd"]);
    let lib = norm(&project.lib_rs);
    let main = norm(&project.executables[0].source);
    for expected in [
        "pub struct Pair<A: Clone + Send + Sync + 'static, B: Clone + Send + Sync + 'static>",
        "pub enum Choice<T: Clone + Send + Sync + 'static>",
        "impl<T: Clone + Send + Sync + 'static> Labelled<T>",
        "label: Labelled::<i64>::default_label()",
        "pub handle: fn(S, String) -> String,",
        "pub fn swap<A: Clone + Send + Sync + 'static, B: Clone + Send + Sync + 'static>(p: &Pair<A, B>) -> Pair<B, A>",
        "pub fn serve<S: Clone + Send + Sync + 'static>(mut stream: telos_io::TcpStream, mut app: App<S>)",
        "let id: fn(i64) -> i64 = ((|__a0: i64| identity::<i64>(&__a0)) as fn(i64) -> i64);",
        "counts.values().iter().cloned().map(((|__a0: i64| show(&__a0)) as fn(i64) -> String)).collect::<Vec<_>>()",
        "counts.update(id.clone(), ((|__a0: i64| bump(&__a0)) as fn(i64) -> i64))",
        "Some(rest.clone()).filter(|t: &String| !t.starts_with('+')).and_then(|t| t.parse::<i64>().ok())",
        "u8::try_from(n).ok()",
        "(b as i64)",
        "stream.set_idle_timeout(5000)",
        "(app.handle)(app.state.clone(),",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
    let store = main
        .find("let counts: telos_io::Store<i64> = telos_io::Store::new();")
        .expect(&main);
    let application = main
        .find("let application = App { state: counts.clone(), handle: ((|__a0: telos_io::Store<i64>, __a1: String| respond(&__a0, &__a1)) as fn(telos_io::Store<i64>, String) -> String) };")
        .expect(&main);
    let accept = main.find("loop {").expect(&main);
    assert!(store < application && application < accept, "{main}");
}

/// `l.at(i)` and `l[i]` on a list whose elements are not `Copy` in Rust
/// copy the element; on `Bytes` and `List<Int>` they read it.
#[test]
fn element_reads_copy_only_non_copy_elements() {
    let project = common::generate_source(
        "fn first(lines: List<Text>) -> Text = lines.at(0)\n\
         fn second(lines: List<Text>) -> Text { let line = lines[1]\n line }\n\
         fn byte(bytes: Bytes) -> Byte = bytes.at(0)\n",
        &common::golden_std(),
    )
    .unwrap();
    let lib = norm(&project.lib_rs);
    assert!(lib.contains("lines[(0) as usize].clone()"), "{lib}");
    assert!(
        lib.contains("let line = lines[(1) as usize].clone();"),
        "{lib}"
    );
    assert!(lib.contains("bytes[(0) as usize] }"), "{lib}");
}

/// Read-only parameters that are not `Copy` are references; a parameter
/// the body changes, and one of a `Copy` type, is copied once on entry.
#[test]
fn parameters_are_borrowed_unless_changed() {
    let project = common::generate_source(
        "fn count(text: Text, at: Int) -> Int = if text.len() > at { 1 } else { 0 }\n\
         fn same(a: Text, b: Text) -> Bool = a == b\n\
         fn grow(items: List<Int>) -> List<Int> { items.push(1)\n items }\n\
         fn keep(text: Text) -> Text = text\n\
         fn user(text: Text) -> Int = count(text, 2) + count(\"x\", 1)\n",
        &common::golden_std(),
    )
    .unwrap();
    let lib = norm(&project.lib_rs);
    for expected in [
        "pub fn count(text: &String, at: &i64) -> i64 { let mut at = at.clone();",
        "pub fn same(a: &String, b: &String) -> bool { (*a) == (*b) }",
        "pub fn grow(items: &Vec<i64>) -> Vec<i64> { let mut items = items.clone(); items.push(1); items }",
        "pub fn keep(text: &String) -> String { text.clone() }",
        "count(text, &2) + count(&String::from(\"x\"), &1)",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
}

fn generator_errors(text: &str) -> Vec<(&'static str, String)> {
    match common::generate_source(text, &common::golden_std()) {
        Ok(project) => panic!("expected diagnostics, got\n{}", project.lib_rs),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|d| (d.code, d.message))
            .collect(),
    }
}

const ECHO: &str = "import std.net.TcpStream\nimport std.io.IoError\n\
    protocol Echo {\n\
      fn serve(stream: TcpStream, app: Int) -> Result<Unit, IoError> requires [stream.read] {\n\
        _ = stream.read(1)?\n Ok(())\n }\n}\n";

#[test]
fn store_resources_take_one_type_argument_and_no_properties() {
    let errors = generator_errors(&format!(
        "{ECHO}system S {{\n\
           resource ingress: TcpListener {{ address = \"127.0.0.1\", port = 1 }}\n\
           resource s: Store<Int> {{ size = 3 }}\n\
           component c: Service {{ listener = s, protocol = Echo, application = 1 }}\n}}\n"
    ));
    assert_eq!(
        errors,
        [
            (
                "E020",
                "unsupported property `size` on resource kind `Store`".to_string()
            ),
            (
                "E020",
                "property `listener` must name a `TcpListener` resource of this system".to_string()
            ),
        ]
    );
}

#[test]
fn a_listener_the_component_does_not_bind_is_not_a_value() {
    let errors = generator_errors(&format!(
        "{ECHO}type Holder {{ listener: TcpListener }}\n\
         fn port(holder: Holder) -> Int = 1\n\
         system S {{\n\
           resource ingress: TcpListener {{ address = \"127.0.0.1\", port = 1 }}\n\
           resource other: TcpListener {{ address = \"127.0.0.1\", port = 2 }}\n\
           component c: Service {{ listener = ingress, protocol = Echo, application = port(Holder {{ listener = other }}) }}\n}}\n"
    ));
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].0, "E020");
    assert!(errors[0].1.contains("resource `other`"), "{errors:?}");
}

/// The component's own listener named in the `application` is bound
/// first, so the application is built after `bind`.
#[test]
fn the_own_listener_as_a_value_is_bound_first() {
    let project = common::generate_source(
        &format!(
            "{ECHO}type Holder {{ listener: TcpListener }}\n\
             fn bound(holder: Holder) -> Int = holder.listener.local_port()\n\
             system S {{\n\
               resource port: TcpListener {{ address = \"127.0.0.1\", port = 1 }}\n\
               component c: Service {{ listener = port, protocol = Echo, application = bound(Holder {{ listener = port }}) }}\n}}\n"
        ),
        &common::golden_std(),
    );
    let project = project.unwrap_or_else(|d| panic!("{d:?}"));
    let main = norm(&project.executables[0].source);
    let bind = main
        .find("let port_ = telos_io::TcpListener::bind(")
        .expect(&main);
    let application = main
        .find("let application = bound(&(Holder { listener: port_.clone() }));")
        .expect(&main);
    assert!(bind < application, "{main}");
}

#[test]
fn syntax_copy_is_current() {
    let copy = std::fs::read_to_string(golden_dir().join("syntax.telos")).unwrap();
    let original = std::fs::read_to_string(
        common::workspace().join("crates/telos-llvm/tests/golden/syntax.telos"),
    )
    .unwrap();
    assert_eq!(
        copy, original,
        "tests/golden/syntax.telos is a copy of the LLVM golden program; refresh it and its goldens"
    );
}

#[test]
fn acceptance_copy_is_current() {
    let copy = std::fs::read_to_string(golden_dir().join("terminal.telos")).unwrap();
    let original =
        std::fs::read_to_string(common::workspace().join("examples/terminal/terminal.telos"))
            .unwrap();
    assert_eq!(
        copy, original,
        "tests/golden/terminal.telos is a copy of examples/terminal/terminal.telos; refresh it and its goldens"
    );
}
