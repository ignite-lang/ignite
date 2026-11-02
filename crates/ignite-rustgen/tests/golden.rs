//! Golden files: every `tests/golden/<name>.ignite` is parsed, checked
//! against a standard library and lowered. After collapsing whitespace runs
//! to one space, `src/lib.rs` must equal `<name>.rs`, the first executable
//! `<name>.main.rs`, every further executable `<name>.<executable>.rs`, and,
//! for programs lowered with the golden standard library
//! (`tests/golden/std`), the `ignite-std` crate `<name>.std.rs`.
//!
//! `IGNITE_BLESS=1 cargo test -p ignite-rustgen --test main golden::` rewrites the
//! expected files from the current output.

use crate::common;

use std::path::{Path, PathBuf};

use ignite_rustgen::GeneratedProject;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Programs that must type: their goldens are the typed generator's
/// output. The others fall back to the untyped generator when they do not.
const TYPED: &[&str] = &[
    "terminal",
    "syntax",
    "exits",
    "variants",
    "generics",
    "components",
    "services",
    "modules",
    "targets",
];

fn lower(stem: &str, std_root: &Path) -> (GeneratedProject, GeneratedProject) {
    let file = golden_dir().join(format!("{stem}.ignite"));
    let substrate = Path::new("../ignite-io");
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
    if std::env::var_os("IGNITE_BLESS").is_some() {
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
            .contains("ignite-std = { path = \"std\" }")
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

/// A server kind binding its listener and a client kind naming another
/// listener as its `Endpoint` target (`docs/slice.md` section 14.5): the
/// client's main builds the endpoint with the `--port` override and binds
/// nothing. `Endpoint` comes from the golden `std.net`, so the program
/// crate depends on `ignite-std`.
#[test]
fn system_with_server_and_client() {
    let project = check_program("services", &common::golden_std(), false);
    assert_eq!(executable_names(&project), ["greeter", "readerc"]);
    assert!(project.cargo_toml.contains("ignite-std"));
    let reader = norm(&project.executables[1].source);
    assert!(
        reader.contains("let back_endpoint = Endpoint { address: String::from(\"0.0.0.0\"), port: port.unwrap_or(7001) };"),
        "{reader}"
    );
    assert!(!reader.contains("TcpListener::bind"), "{reader}");
}

#[test]
fn protocol_import_spawn_and_function_values() {
    let project = check_program("modules", &common::golden_std(), true);
    assert_eq!(executable_names(&project), ["echod", "echo"]);
    assert!(
        project
            .cargo_toml
            .contains("ignite-std = { path = \"std\" }")
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

/// `Process.exit(c)` has type Never and lowers to `ignite_io::exit`, which
/// returns `!`: no `?` is needed, and `?` on it lowers to the call alone.
#[test]
fn process_exit_diverges() {
    let project = check_program("exits", &common::golden_std(), false);
    assert_eq!(executable_names(&project), ["exiter"]);
    let lib = norm(&project.lib_rs);
    assert!(lib.contains("Eof => ignite_io::exit((5) as i32)"), "{lib}");
    assert!(!lib.contains("ignite_io::Process::exit"), "{lib}");
    assert!(!lib.contains("exit((7) as i32))?"), "{lib}");
}

/// Bare variants of an enum that is not imported, resolved by expected
/// type, name the enum by its path: `ignite_std::modes::Mode` in a program,
/// `crate::modes::Mode` inside `ignite-std`.
#[test]
fn variants_of_enums_not_imported() {
    let std_root = golden_dir().join("variants/std");
    let (project, std) = lower("variants", &std_root);
    check_program("variants", &std_root, true);
    assert!(
        norm(&project.lib_rs).contains("level(&ignite_std::modes::Mode::Loud)"),
        "{}",
        project.lib_rs
    );
    assert!(
        norm(&std.lib_rs).contains("crate::modes::Mode::Quiet(_) =>"),
        "{}",
        std.lib_rs
    );
}

/// Slice 10 (`docs/slice.md` 14.4, 14.5): a component kind is a record
/// type with `fn K.main`. The generated main creates the store, binds the
/// listener with the `--port` override and the `listening on` log, builds
/// the embedded `banner` and then `server` with the kind's defaults for
/// the omitted fields, and calls `Greeter.main`.
#[test]
fn executable_components() {
    let project = check_program("components", &common::repository_std(), false);
    assert_eq!(executable_names(&project), ["greeterd", "pinger"]);
    let pinger = norm(&project.executables[1].source);
    for expected in [
        "let port = ignite_io::args::port()?;",
        "let ingress_endpoint = Endpoint { address: String::from(\"127.0.0.1\"), port: port.unwrap_or(2500) };",
        "let client = Pinger { target: ingress_endpoint.clone(), executable: String::from(\"pinger\") };",
        "(&client) }))",
    ] {
        assert!(
            pinger.contains(expected),
            "missing `{expected}` in\n{pinger}"
        );
    }
    assert!(!pinger.contains("TcpListener::bind"), "{pinger}");
    let main = norm(&project.executables[0].source);
    for expected in [
        "let seen: ignite_io::Store<String> = ignite_io::Store::new();",
        "let port = ignite_io::args::port()?;",
        "let ingress = ignite_io::TcpListener::bind(String::from(\"127.0.0.1\"), port.unwrap_or(2500))?;",
        "ignite_io::log(&format!(\"listening on 127.0.0.1:{}\", ingress.local_port()));",
        "let banner = Banner {",
        "let server = Greeter {",
        "executable: String::from(\"greeterd\")",
        "(&server) }))",
    ] {
        assert!(main.contains(expected), "missing `{expected}` in\n{main}");
    }
    let store = main.find("Store::new()").expect(&main);
    let bind = main.find("TcpListener::bind").expect(&main);
    let banner = main.find("let banner").expect(&main);
    let server = main.find("let server").expect(&main);
    assert!(store < bind && bind < banner && banner < server, "{main}");
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
        "pub handle: std::sync::Arc<dyn Fn(S, String) -> String + Send + Sync>,",
        "pub fn swap<A: Clone + Send + Sync + 'static, B: Clone + Send + Sync + 'static>(p: &Pair<A, B>) -> Pair<B, A>",
        "pub fn serve<S: Clone + Send + Sync + 'static>(mut stream: ignite_io::TcpStream, mut app: App<S>)",
        "let id: std::sync::Arc<dyn Fn(i64) -> i64 + Send + Sync> = (std::sync::Arc::new(|__a0: i64| identity::<i64>(&__a0)) as std::sync::Arc<dyn Fn(i64) -> i64 + Send + Sync>);",
        "counts.values().iter().cloned().map(&*(std::sync::Arc::new(|__a0: i64| show(&__a0)) as std::sync::Arc<dyn Fn(i64) -> String + Send + Sync>)).collect::<Vec<_>>()",
        "counts.update(id.clone(), &*(std::sync::Arc::new(|__a0: i64| bump(&__a0)) as std::sync::Arc<dyn Fn(i64) -> i64 + Send + Sync>))",
        "Some(rest.clone()).filter(|t: &String| !t.starts_with('+')).and_then(|t| t.parse::<i64>().ok())",
        "u8::try_from(n).ok()",
        "(b as i64)",
        "stream.set_idle_timeout(5000)",
        "(app.handle)(app.state.clone(),",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
    let store = main
        .find("let counts: ignite_io::Store<i64> = ignite_io::Store::new();")
        .expect(&main);
    let bind = main.find("TcpListener::bind").expect(&main);
    let server = main
        .find("let server = Server::<ignite_io::Store<i64>> { listener: ingress.clone(), application: App { state: counts.clone(), handle: (std::sync::Arc::new(|__a0: ignite_io::Store<i64>, __a1: String| respond(&__a0, &__a1)) as std::sync::Arc<dyn Fn(ignite_io::Store<i64>, String) -> String + Send + Sync>) }, executable: String::from(\"countersd\") };")
        .expect(&main);
    let call = main.find("Server__main(&server)").expect(&main);
    assert!(store < bind && bind < server && server < call, "{main}");
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
    assert!(
        lib.contains("ignite_io::checked::at(lines, 0, \"test.ignite\", 1, 39).clone()"),
        "{lib}"
    );
    assert!(
        lib.contains(
            "let line = ignite_io::checked::at(lines, 1, \"test.ignite\", 2, 51).clone();"
        ),
        "{lib}"
    );
    assert!(
        lib.contains("(*ignite_io::checked::at(bytes, 0, \"test.ignite\", 4, 33)) }"),
        "{lib}"
    );
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
        "ignite_io::checked::add(count(text, &2), count(&String::from(\"x\"), &1), \"test.ignite\", 6, 30)",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
}

const ECHO: &str = "import std.net.TcpStream\nimport std.io.IoError\n\
    type Server { listener: TcpListener, port: Int }\n\
    fn Server.main(self: Server) -> Result<Unit, IoError> {\n\
      let stream = self.listener.accept()?\n _ = stream.read(1)?\n Ok(())\n }\n";

/// Every `TcpListener` the component uses is bound before its value is
/// built (`docs/slice.md` section 14.5), also when the listener is named
/// inside another property's expression. A resource named like one of
/// `main`'s own bindings gets a trailing `_`.
#[test]
fn listeners_are_bound_before_the_component_value() {
    let project = common::generate_source(
        &format!(
            "{ECHO}type Holder {{ listener: TcpListener }}\n\
             fn bound(holder: Holder) -> Int = holder.listener.local_port()\n\
             system S {{\n\
               resource port: TcpListener {{ address = \"127.0.0.1\", port = 1 }}\n\
               component c: Server {{ listener = port, port = bound(Holder {{ listener = port }}) }}\n}}\n"
        ),
        &common::golden_std(),
    );
    let project = project.unwrap_or_else(|d| panic!("{d:?}"));
    let main = norm(&project.executables[0].source);
    let bind = main
        .find("let port_ = ignite_io::TcpListener::bind(")
        .expect(&main);
    let value = main
        .find("let c = Server { listener: port_.clone(), port: bound(&(Holder { listener: port_.clone() })) };")
        .expect(&main);
    assert!(bind < value, "{main}");
}

/// Slice 11b (`docs/friction.md` rows 32 to 35): a mutating call on an
/// indexed receiver writes through `ignite_io::checked::at_mut`; an enum
/// without payloads derives the `BTreeMap` key traits; `Exposure` is
/// `ignite_io::Exposure`; a record named `Box` is `Box_`, so the boxed
/// field stays `Box<Option<Box_>>`.
#[test]
fn target_bugs_of_slice_11b() {
    let project = check_program("targets", &common::golden_std(), false);
    let lib = norm(&project.lib_rs);
    for text in [
        "(*ignite_io::checked::at_mut(&mut lists, __p1, ",
        ")).push(__p2) }",
        ")).extend_from_slice(&__p4) }",
        ")).items.push(__p6) }",
        "#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)] pub enum Color {",
        "pub fn describe(e: &ignite_io::Exposure) -> String {",
        "ignite_io::Exposure::Loopback =>",
        "pub fn pick(open: &bool) -> ignite_io::Exposure {",
        "pub struct Box_ {",
        "pub inner: Box<Option<Box_>>,",
        "pub fn boxed() -> Box_ {",
        "Box_ { label: String::from(\"outer\"), inner: Box::new(Some(Box_ {",
    ] {
        assert!(lib.contains(text), "no `{text}` in\n{lib}");
    }
}

#[test]
fn syntax_copy_is_current() {
    let copy = std::fs::read_to_string(golden_dir().join("syntax.ignite")).unwrap();
    let original = std::fs::read_to_string(
        common::workspace().join("crates/ignite-llvm/tests/golden/syntax.ignite"),
    )
    .unwrap();
    assert_eq!(
        copy, original,
        "tests/golden/syntax.ignite is a copy of the LLVM golden program; refresh it and its goldens"
    );
}

#[test]
fn acceptance_copy_is_current() {
    let copy = std::fs::read_to_string(golden_dir().join("terminal.ignite")).unwrap();
    let original =
        std::fs::read_to_string(common::workspace().join("examples/terminal/terminal.ignite"))
            .unwrap();
    assert_eq!(
        copy, original,
        "tests/golden/terminal.ignite is a copy of examples/terminal/terminal.ignite; refresh it and its goldens"
    );
}
